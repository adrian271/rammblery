//! Reading string attributes off an AXUIElement.
//!
//! The AX `kAX*` attribute names are plain C string constants that the objc2
//! bindings don't re-export, so we materialize the few we need as CFStrings
//! once and reuse them.

use std::ptr::NonNull;

use objc2_application_services::AXUIElement;
use objc2_core_foundation::{CFRetained, CFString, CFType};

thread_local! {
    // AX attribute-name CFStrings. Built lazily per AX thread (they never leave it).
    static ATTR_ROLE: CFRetained<CFString> = CFString::from_static_str("AXRole");
    static ATTR_VALUE: CFRetained<CFString> = CFString::from_static_str("AXValue");
    static ATTR_FOCUSED: CFRetained<CFString> = CFString::from_static_str("AXFocusedUIElement");
    static ATTR_SUBROLE: CFRetained<CFString> = CFString::from_static_str("AXSubrole");
    static ATTR_NUMCHARS: CFRetained<CFString> =
        CFString::from_static_str("AXNumberOfCharacters");
}

/// Copy a string-valued attribute off an element, returning `None` if the
/// attribute is missing or not a string.
fn copy_string_attribute(element: &AXUIElement, attr: &CFString) -> Option<String> {
    let mut raw: *const CFType = std::ptr::null();
    let err = unsafe {
        element.copy_attribute_value(attr, NonNull::from(&mut raw))
    };
    // 0 == kAXErrorSuccess.
    if err.0 != 0 || raw.is_null() {
        return None;
    }
    // Take ownership of the +1 retained value CF handed us.
    let value = unsafe { CFRetained::from_raw(NonNull::new_unchecked(raw as *mut CFType)) };
    value.downcast_ref::<CFString>().map(|s| s.to_string())
}

/// The `AXRole` of an element (e.g. "AXTextArea", "AXTextField"), if any.
pub fn role(element: &AXUIElement) -> Option<String> {
    ATTR_ROLE.with(|a| copy_string_attribute(element, a))
}

/// The `AXSubrole` of an element (e.g. "AXSecureTextField"), if any.
pub fn subrole(element: &AXUIElement) -> Option<String> {
    ATTR_SUBROLE.with(|a| copy_string_attribute(element, a))
}

/// The `AXValue` (full text) of an element, if it's a string value.
pub fn value(element: &AXUIElement) -> Option<String> {
    ATTR_VALUE.with(|a| copy_string_attribute(element, a))
}

/// The system-wide focused UI element, if any.
pub fn focused_element(system_wide: &AXUIElement) -> Option<CFRetained<AXUIElement>> {
    let mut raw: *const CFType = std::ptr::null();
    let err = ATTR_FOCUSED.with(|attr| unsafe {
        system_wide.copy_attribute_value(attr, NonNull::from(&mut raw))
    });
    if err.0 != 0 || raw.is_null() {
        return None;
    }
    // The focused element comes back as an AXUIElement (a CFType); reinterpret
    // and take ownership of the +1 reference.
    let elem = unsafe { CFRetained::from_raw(NonNull::new_unchecked(raw as *mut AXUIElement)) };
    Some(elem)
}
