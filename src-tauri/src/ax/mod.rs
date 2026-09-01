//! macOS Accessibility integration.
//!
//! This module is only compiled on macOS. It provides Accessibility permission
//! checks and a dedicated thread that observes the system-wide focused text
//! field. M1 is a read-only proof of concept that logs what it sees.

pub mod focus;
pub mod permission;
pub mod text;

use std::sync::Arc;

pub use focus::AxControl;

/// Spawn the dedicated AX thread running the focus/text poll loop.
/// Returns the shared control handle for enable/disable and shutdown.
pub fn spawn_ax_thread(enabled: bool) -> Arc<AxControl> {
    let control = AxControl::new(enabled);
    let thread_control = control.clone();
    std::thread::Builder::new()
        .name("rammblery-ax".into())
        .spawn(move || {
            focus::run_poll_loop(thread_control);
        })
        .expect("failed to spawn AX thread");
    control
}
