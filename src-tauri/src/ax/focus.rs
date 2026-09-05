//! Focus + text observation (M1: read-only polling proof of concept).
//!
//! A dedicated thread polls the system-wide focused element every ~300ms,
//! reads its role/value, and logs changes. Later milestones add AXObserver
//! notifications for lower latency and emit events to the checker task.

use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use objc2_application_services::AXUIElement;
use objc2_core_foundation::CFEqual;
use tokio::sync::mpsc::UnboundedSender;

use super::{geometry, text, AxEvent};

const POLL_INTERVAL: Duration = Duration::from_millis(300);
/// Skip documents larger than this (char count) — checking them is slow and costly.
const MAX_CHARS: usize = 20_000;

/// Shared control state for the AX thread.
pub struct AxControl {
    /// When false, the poll loop idles without touching the AX API.
    pub enabled: AtomicBool,
    /// When false, the thread exits its loop.
    running: AtomicBool,
}

impl AxControl {
    pub fn new(enabled: bool) -> Arc<Self> {
        Arc::new(Self {
            enabled: AtomicBool::new(enabled),
            running: AtomicBool::new(true),
        })
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    /// Signal the poll loop to exit. Wired to app shutdown in a later milestone.
    #[allow(dead_code)]
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// The poll loop. Runs on the dedicated AX thread. All AXUIElement values stay
/// on this thread (they are not `Send`). Emits `AxEvent`s to the checker.
pub fn run_poll_loop(control: Arc<AxControl>, tx: UnboundedSender<AxEvent>) {
    // System-wide element used to query the current focused UI element.
    let system_wide = unsafe { AXUIElement::new_system_wide() };
    let own_pid = std::process::id() as libc::pid_t;

    // Last focused element + its last-seen text, to detect changes.
    let mut last_focused: Option<objc2_core_foundation::CFRetained<AXUIElement>> = None;
    let mut last_value: Option<String> = None;
    let mut session_id: u64 = 0;

    while control.running.load(Ordering::SeqCst) {
        std::thread::sleep(POLL_INTERVAL);

        if !control.enabled.load(Ordering::SeqCst) {
            if last_focused.is_some() {
                // Went idle; tell the checker to hide.
                session_id += 1;
                let _ = tx.send(AxEvent::Hide { session_id });
            }
            last_focused = None;
            last_value = None;
            continue;
        }

        let focused = match text::focused_element(&system_wide) {
            Some(el) => el,
            None => continue,
        };

        // Ignore our own windows so the panel/editor don't feed back into checks.
        if element_pid(&focused) == Some(own_pid) {
            continue;
        }

        // Did the focused element itself change?
        let focus_changed = match &last_focused {
            Some(prev) => !CFEqual(Some(prev), Some(&focused)),
            None => true,
        };

        let role = text::role(&focused);
        let subrole = text::subrole(&focused);

        // Only usable text fields, and never secure (password) fields.
        let is_text = matches!(
            role.as_deref(),
            Some("AXTextArea") | Some("AXTextField") | Some("AXComboBox")
        );
        let is_secure = subrole.as_deref() == Some("AXSecureTextField");
        let usable = is_text && !is_secure;

        if focus_changed {
            session_id += 1;
            let pid = element_pid(&focused);
            let app = pid.and_then(app_name_for_pid);
            log::info!(
                "[focus] session={} pid={} app={:?} role={:?} subrole={:?} usable={}",
                session_id,
                pid.unwrap_or(-1),
                app,
                role,
                subrole,
                usable
            );
            last_focused = Some(focused.clone());
            last_value = None;

            if !usable {
                let _ = tx.send(AxEvent::Hide { session_id });
                continue;
            }
        } else if !usable {
            continue;
        }

        // Read current value; emit on change (covers both focus-in and edits).
        let value = text::value(&focused);
        if value != last_value {
            last_value = value.clone();
            if let Some(v) = value {
                let n = v.chars().count();
                if n == 0 || n > MAX_CHARS {
                    let _ = tx.send(AxEvent::Hide { session_id });
                } else {
                    let frame = geometry::frame(&focused);
                    log::info!("[value] session={} ({} chars) frame={:?}", session_id, n, frame);
                    let _ = tx.send(AxEvent::TextChanged {
                        session_id,
                        text: v,
                        frame,
                    });
                }
            }
        }
    }
}

/// The pid owning an AX element, if obtainable.
fn element_pid(element: &AXUIElement) -> Option<libc::pid_t> {
    let mut pid: libc::pid_t = 0;
    let err = unsafe { element.pid(NonNull::from(&mut pid)) };
    if err.0 == 0 {
        Some(pid)
    } else {
        None
    }
}

/// Localized app name for a pid, via NSRunningApplication.
fn app_name_for_pid(pid: libc::pid_t) -> Option<String> {
    use objc2_app_kit::NSRunningApplication;
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    app.localizedName().map(|n| n.to_string())
}
