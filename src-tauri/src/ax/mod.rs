//! macOS Accessibility integration.
//!
//! This module is only compiled on macOS. It provides Accessibility permission
//! checks and a dedicated thread that observes the system-wide focused text
//! field, emitting events to the checker task.

pub mod focus;
pub mod geometry;
pub mod permission;
pub mod text;

use std::sync::Arc;

pub use focus::AxControl;
pub use geometry::Frame;

/// Events emitted by the AX thread to the checker task.
#[derive(Debug, Clone)]
pub enum AxEvent {
    /// The focused text field's content (or focus target) changed. Carries the
    /// full text and the field's on-screen frame for panel positioning. The
    /// `session_id` increments on every focus change so stale work can be dropped.
    TextChanged {
        session_id: u64,
        text: String,
        frame: Option<Frame>,
    },
    /// Focus left a usable text field (e.g. moved to a button or another app's
    /// non-text UI) — the panel should hide.
    Hide { session_id: u64 },
}

/// Spawn the dedicated AX thread running the focus/text poll loop.
/// Returns the shared control handle for enable/disable and shutdown.
pub fn spawn_ax_thread(
    enabled: bool,
    tx: tokio::sync::mpsc::UnboundedSender<AxEvent>,
) -> Arc<AxControl> {
    let control = AxControl::new(enabled);
    let thread_control = control.clone();
    std::thread::Builder::new()
        .name("rammblery-ax".into())
        .spawn(move || {
            focus::run_poll_loop(thread_control, tx);
        })
        .expect("failed to spawn AX thread");
    control
}
