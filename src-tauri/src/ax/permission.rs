//! macOS Accessibility permission (TCC) handling.
//!
//! The AX API only works once the user grants Accessibility access in
//! System Settings → Privacy & Security → Accessibility. `AXIsProcessTrusted`
//! reports the current grant; `AXIsProcessTrustedWithOptions` with the prompt
//! option shows the system dialog the first time (macOS suppresses it after a
//! denial, hence the deep-link fallback into the Settings pane).

use std::ffi::c_void;

use objc2_application_services::{
    kAXTrustedCheckOptionPrompt, AXIsProcessTrusted, AXIsProcessTrustedWithOptions,
};
use objc2_core_foundation::{
    kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks, CFBoolean, CFDictionary,
    CFRetained,
};

/// Whether this process currently has Accessibility permission.
pub fn is_trusted() -> bool {
    // Safe: no arguments, reads a global TCC flag.
    unsafe { AXIsProcessTrusted() }
}

/// Prompt for Accessibility permission. Shows the system dialog the first time;
/// after a prior denial macOS won't re-prompt, so callers should also guide the
/// user to the Settings pane (see `open_settings`). Returns the trust state at
/// call time (which is `false` until the user actually flips the toggle).
pub fn request_trust() -> bool {
    // Build { kAXTrustedCheckOptionPrompt: true } as a CFDictionary and pass it
    // to AXIsProcessTrustedWithOptions.
    let value = CFBoolean::new(true);

    unsafe {
        let key = kAXTrustedCheckOptionPrompt;
        let mut keys: [*const c_void; 1] = [(key as *const CFString_).cast()];
        let mut values: [*const c_void; 1] = [(value as *const CFBoolean).cast()];

        // Pointers are valid for the duration of the call; the standard CFType
        // callbacks make CF retain/release the key and value correctly.
        let dict: Option<CFRetained<CFDictionary>> = CFDictionary::new(
            None,
            keys.as_mut_ptr(),
            values.as_mut_ptr(),
            1,
            &raw const kCFTypeDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        );

        AXIsProcessTrustedWithOptions(dict.as_deref())
    }
}

/// Open the Accessibility pane of System Settings so the user can grant access
/// manually (needed when the one-shot prompt was already dismissed/denied).
pub fn open_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

// The prompt-option key is typed as `&CFString` in the bindings; we only need
// its pointer, so alias the opaque CF string type for the cast above.
use objc2_core_foundation::CFString as CFString_;
