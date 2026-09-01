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

use super::text;

const POLL_INTERVAL: Duration = Duration::from_millis(300);

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
/// on this thread (they are not `Send`).
pub fn run_poll_loop(control: Arc<AxControl>) {
    // System-wide element used to query the current focused UI element.
    let system_wide = unsafe { AXUIElement::new_system_wide() };

    // Last focused element + its last-seen text, to detect changes.
    let mut last_focused: Option<objc2_core_foundation::CFRetained<AXUIElement>> = None;
    let mut last_value: Option<String> = None;

    while control.running.load(Ordering::SeqCst) {
        std::thread::sleep(POLL_INTERVAL);

        if !control.enabled.load(Ordering::SeqCst) {
            last_focused = None;
            last_value = None;
            continue;
        }

        let focused = match text::focused_element(&system_wide) {
            Some(el) => el,
            None => continue,
        };

        // Did the focused element itself change?
        let focus_changed = match &last_focused {
            Some(prev) => !CFEqual(Some(prev), Some(&focused)),
            None => true,
        };

        let role = text::role(&focused);
        let subrole = text::subrole(&focused);

        // Skip non-text and secure (password) fields.
        let is_text = matches!(
            role.as_deref(),
            Some("AXTextArea") | Some("AXTextField") | Some("AXComboBox")
        );
        let is_secure = subrole.as_deref() == Some("AXSecureTextField");

        if focus_changed {
            let pid = element_pid(&focused);
            let app = pid.and_then(app_name_for_pid);
            log::info!(
                "[focus] pid={} app={:?} role={:?} subrole={:?} text_field={}",
                pid.unwrap_or(-1),
                app,
                role,
                subrole,
                is_text && !is_secure
            );
            last_focused = Some(focused.clone());
            last_value = None;
        }

        if !is_text || is_secure {
            continue;
        }

        let value = text::value(&focused);
        if value != last_value {
            if let Some(v) = &value {
                let preview: String = v.chars().take(120).collect();
                log::info!("[value] ({} chars) {:?}", v.chars().count(), preview);
            }
            last_value = value;
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
