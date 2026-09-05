//! Reading the on-screen frame of an AX element for panel positioning.
//!
//! `kAXPosition`/`kAXSize` come back as `AXValue`s wrapping CGPoint/CGSize.
//! AX coordinates are global, top-left origin points — the same convention
//! Tauri's `set_position(LogicalPosition)` uses on macOS — so the rect passes
//! straight through to window placement.

use std::ptr::NonNull;

use objc2_application_services::{AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{CFRetained, CFString, CFType, CGPoint, CGSize};

/// A screen-space rectangle in AX/Tauri logical points (top-left origin).
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

thread_local! {
    static ATTR_POSITION: CFRetained<CFString> = CFString::from_static_str("AXPosition");
    static ATTR_SIZE: CFRetained<CFString> = CFString::from_static_str("AXSize");
}

/// Copy an AXValue-typed attribute and decode it into `out` (a CGPoint/CGSize).
fn copy_axvalue(element: &AXUIElement, attr: &CFString, ty: AXValueType, out: NonNull<c_void>) -> bool {
    let mut raw: *const CFType = std::ptr::null();
    let err = unsafe { element.copy_attribute_value(attr, NonNull::from(&mut raw)) };
    if err.0 != 0 || raw.is_null() {
        return false;
    }
    let value = unsafe { CFRetained::from_raw(NonNull::new_unchecked(raw as *mut CFType)) };
    match value.downcast_ref::<AXValue>() {
        Some(axv) => unsafe { axv.value(ty, out) },
        None => false,
    }
}

use std::ffi::c_void;

/// The focused element's screen frame, if both position and size are available.
pub fn frame(element: &AXUIElement) -> Option<Frame> {
    let mut point = CGPoint::ZERO;
    let mut size = CGSize::ZERO;

    let got_pos = ATTR_POSITION.with(|a| {
        copy_axvalue(
            element,
            a,
            AXValueType::CGPoint,
            NonNull::from(&mut point).cast(),
        )
    });
    let got_size = ATTR_SIZE.with(|a| {
        copy_axvalue(
            element,
            a,
            AXValueType::CGSize,
            NonNull::from(&mut size).cast(),
        )
    });

    if got_pos && got_size {
        Some(Frame {
            x: point.x as f64,
            y: point.y as f64,
            width: size.width as f64,
            height: size.height as f64,
        })
    } else {
        None
    }
}

/// Clamp a panel of `panel_w`×`panel_h` to sit just below `field`, kept within
/// the visible area of whichever screen the field is on. Must be called on the
/// main thread (NSScreen is main-thread-only).
///
/// AX coordinates and Tauri's `set_position` are both global, top-left origin.
/// NSScreen frames are Cocoa (bottom-left origin), so we convert each screen's
/// visible frame to top-left using the primary screen's height.
pub fn clamp_below(
    field: Frame,
    panel_w: f64,
    panel_h: f64,
    gap: f64,
    mtm: objc2::MainThreadMarker,
) -> (f64, f64) {
    use objc2_app_kit::NSScreen;

    let desired_x = field.x;
    let desired_y = field.y + field.height + gap;

    let screens = NSScreen::screens(mtm);
    let Some(primary) = screens.firstObject() else {
        return (desired_x, desired_y);
    };
    let primary_h = primary.frame().size.height as f64;

    // Convert a Cocoa NSRect to a top-left-origin (x, y, w, h) tuple.
    let to_top_left = |r: objc2_foundation::NSRect| {
        let x = r.origin.x as f64;
        let w = r.size.width as f64;
        let h = r.size.height as f64;
        let y = primary_h - (r.origin.y as f64 + h);
        (x, y, w, h)
    };

    // Find the screen containing the field's top-left point; else use primary.
    let mut chosen = to_top_left(primary.visibleFrame());
    for screen in screens.iter() {
        let (x, y, w, h) = to_top_left(screen.visibleFrame());
        if field.x >= x && field.x < x + w && field.y >= y && field.y < y + h {
            chosen = (x, y, w, h);
            break;
        }
    }

    let (sx, sy, sw, sh) = chosen;
    let max_x = (sx + sw - panel_w).max(sx);
    let max_y = (sy + sh - panel_h).max(sy);
    let x = desired_x.clamp(sx, max_x);
    let y = desired_y.clamp(sy, max_y);
    log::info!(
        "[panel] field=({:.0},{:.0}) screen=({:.0},{:.0},{:.0},{:.0}) → panel=({:.0},{:.0})",
        field.x, field.y, sx, sy, sw, sh, x, y
    );
    (x, y)
}
