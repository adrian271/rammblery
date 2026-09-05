//! The floating suggestion panel: a second webview window that hovers next to
//! the focused text field in other apps.
//!
//! On macOS it's re-classed to a non-activating `NSPanel` so that showing it
//! and clicking its buttons never steals key focus from the app the user is
//! typing in.

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::ax::Frame;
use crate::claude::Suggestion;

pub const LABEL: &str = "suggestions";
const WIDTH: f64 = 340.0;
const HEIGHT: f64 = 300.0;
/// Gap between the focused field and the panel.
const GAP: f64 = 8.0;

#[derive(Serialize, Clone)]
struct ShowPayload {
    session_id: u64,
    suggestions: Vec<Suggestion>,
}

/// Build the (initially hidden) suggestion window. Call once at setup.
pub fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Rammblery Suggestions")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(true)
        .visible(false)
        .focused(false)
        .accept_first_mouse(true)
        .build()?;

    #[cfg(target_os = "macos")]
    make_nonactivating_panel(&window);

    Ok(window)
}

/// Position the panel below `frame`, push the suggestions to the webview, and
/// show it. Called from the checker (a tokio task) when a check completes.
///
/// All window/AppKit operations are marshalled onto the main thread, since
/// showing via `orderFront:` (rather than `makeKeyAndOrderFront:`) touches
/// AppKit directly and must not run off the main thread.
pub fn show(app: &AppHandle, frame: Option<Frame>, session_id: u64, suggestions: Vec<Suggestion>) {
    log::info!(
        "[panel] show: session={} n={} frame={:?}",
        session_id,
        suggestions.len(),
        frame
    );
    let Some(window) = app.get_webview_window(LABEL) else {
        log::warn!("[panel] suggestions window not found");
        return;
    };

    if suggestions.is_empty() {
        let _ = window.clone().run_on_main_thread(move || {
            let _ = window.hide();
        });
        return;
    }

    let _ = window.clone().run_on_main_thread(move || {
        let _ = window.set_size(LogicalSize::new(WIDTH, HEIGHT));
        if let Some(f) = frame {
            // Position below the field, clamped to the field's screen so it stays
            // visible on multi-monitor / negative-coordinate layouts.
            #[cfg(target_os = "macos")]
            if let Some(mtm) = objc2::MainThreadMarker::new() {
                let (x, y) = crate::ax::geometry::clamp_below(f, WIDTH, HEIGHT, GAP, mtm);
                let _ = window.set_position(LogicalPosition::new(x, y));
            }
            #[cfg(not(target_os = "macos"))]
            let _ = window.set_position(LogicalPosition::new(f.x, f.y + f.height + GAP));
        }
        let _ = window.emit(
            "suggestions://show",
            ShowPayload {
                session_id,
                suggestions,
            },
        );
        order_front(&window);
    });
}

/// Hide the panel.
pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.clone().run_on_main_thread(move || {
            let _ = window.emit("suggestions://hide", ());
            let _ = window.hide();
        });
    }
}

/// Order the panel to the front without making it key / activating our app.
/// Must be called on the main thread.
fn order_front(window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        use objc2::rc::Retained;
        use objc2_app_kit::NSWindow;
        if let Ok(ptr) = window.ns_window() {
            if let Some(ns) = unsafe { Retained::retain(ptr as *mut NSWindow) } {
                ns.orderFront(None);
                return;
            }
        }
    }
    let _ = window.show();
}

/// Re-class the window's `NSWindow` into a non-activating `NSPanel` and set the
/// window level / collection behavior so it floats over other apps across all
/// Spaces without stealing focus.
#[cfg(target_os = "macos")]
fn make_nonactivating_panel(window: &WebviewWindow) {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::ClassType;
    use objc2_app_kit::{
        NSFloatingWindowLevel, NSPanel, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
    };

    let Ok(ptr) = window.ns_window() else {
        log::warn!("panel: ns_window() unavailable; non-activating panel not applied");
        return;
    };

    unsafe {
        let ns: Retained<NSWindow> = match Retained::retain(ptr as *mut NSWindow) {
            Some(w) => w,
            None => return,
        };

        // Re-class the NSWindow instance to NSPanel. NSPanel adds no ivars over
        // NSWindow, so the memory layout is compatible (this is the standard trick).
        let obj: &AnyObject = &*(Retained::as_ptr(&ns) as *const AnyObject);
        AnyObject::set_class(obj, NSPanel::class());

        // Now that it's a panel, the non-activating style mask is honored.
        let mask = ns.styleMask() | NSWindowStyleMask::NonactivatingPanel;
        ns.setStyleMask(mask);
        ns.setLevel(NSFloatingWindowLevel);
        ns.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
    }
}
