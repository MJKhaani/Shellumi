//! C-ABI and UniFFI foreign function interfaces for mobile platforms (iOS and Android).
//!
//! Provides a memory-safe, stable C-compatible ABI allowing native mobile frontends
//! (Swift/SwiftUI on iOS and Kotlin/Jetpack Compose on Android) to call Shellumi's
//! portable Rust core without webviews, Electron, or hybrid wrappers.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::mobile::{
    BiometricRequirement, MobileCredentialSpec, MobileFfiBridge, MobileLifecycleEvent,
    MobileLifecycleManager, MobilePlatform, MobileSessionState, VirtualKey,
};

/// Modifier bitmasks for [`shellumi_translate_virtual_key_c`].
pub const SHELLUMI_MODIFIER_CTRL: u32 = 0x01;
pub const SHELLUMI_MODIFIER_ALT: u32 = 0x02;
pub const SHELLUMI_MODIFIER_SHIFT: u32 = 0x04;

/// Internal helper safely converting a C string pointer to a Rust `&str`.
/// Returns `None` if the pointer is null or contains invalid UTF-8.
unsafe fn safe_c_str_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

/// Internal helper converting a Rust string into a newly heap-allocated C string.
/// Caller is responsible for freeing with [`shellumi_string_free`].
fn str_to_c_char(s: &str) -> *mut c_char {
    match CString::new(s) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => {
            // Null byte found inside the string; return fallback error JSON
            let fallback = CString::new(r#"{"ok":false,"error":"Invalid string with null byte"}"#)
                .expect("Fallback string is valid");
            fallback.into_raw()
        }
    }
}

/// Returns the Shellumi core semantic version string as a static C string.
/// This pointer is statically allocated and must NOT be freed.
#[no_mangle]
pub extern "C" fn shellumi_core_version() -> *const c_char {
    static VERSION: &[u8] = b"0.1.0\0";
    VERSION.as_ptr() as *const c_char
}

/// Checks whether a given mobile platform name ("ios", "android", "simulator") is supported.
///
/// # Safety
/// `platform_name` must be a valid null-terminated C string or null pointer.
#[no_mangle]
pub unsafe extern "C" fn shellumi_mobile_platform_supported(platform_name: *const c_char) -> bool {
    match safe_c_str_to_str(platform_name) {
        Some(s) => {
            let lower = s.to_ascii_lowercase();
            matches!(lower.as_str(), "ios" | "android" | "simulator")
        }
        None => false,
    }
}

/// Parses an SSH destination string (e.g. `user@hostname:2222`) into structured JSON.
///
/// # Returns
/// A heap-allocated C string containing a JSON payload:
/// `{"ok": true, "host": "...", "username": "...", "port": 2222}`
/// or `{"ok": false, "error": "..."}`.
///
/// # Safety
/// The returned pointer must be deallocated by calling [`shellumi_string_free`].
#[no_mangle]
pub unsafe extern "C" fn shellumi_parse_destination_c(target: *const c_char) -> *mut c_char {
    let target_str = match safe_c_str_to_str(target) {
        Some(s) => s,
        None => {
            return str_to_c_char(
                r#"{"ok":false,"error":"Target string is null or invalid UTF-8"}"#,
            );
        }
    };

    let json_result = MobileFfiBridge::parse_destination_json(target_str);
    str_to_c_char(&json_result)
}

/// Generates an Ed25519 SSH keypair with zeroized private key memory.
///
/// # Parameters
/// - `comment`: Optional comment string (can be NULL).
///
/// # Returns
/// A heap-allocated C string containing a JSON payload:
/// `{"ok": true, "key": {"public_key": "...", "fingerprint": "...", "comment": "..."}}`
/// or `{"ok": false, "error": "..."}`.
///
/// # Safety
/// The returned pointer must be deallocated by calling [`shellumi_string_free`].
#[no_mangle]
pub unsafe extern "C" fn shellumi_generate_keypair_c(comment: *const c_char) -> *mut c_char {
    let comment_str = safe_c_str_to_str(comment).unwrap_or("shellumi-mobile");
    let json_result = MobileFfiBridge::generate_keypair_json(comment_str);
    str_to_c_char(&json_result)
}

/// Translates a virtual accessory key name and modifier bitmask into raw ANSI escape bytes.
///
/// # Parameters
/// - `key_name`: Name of key (e.g. "esc", "tab", "arrow_up", "arrow_down", "arrow_left",
///   "arrow_right", "pipe", "tilde", "slash", "backslash", "home", "end", "f1".."f12",
///   or single character string "c", "x", "a").
/// - `modifier_flags`: Bitmask of active modifiers:
///   `0x01` = Ctrl, `0x02` = Alt, `0x04` = Shift.
///
/// # Returns
/// A heap-allocated C string containing a JSON payload:
/// `{"ok": true, "key": "...", "bytes": [27, 91, 65], "hex": "1B 5B 41"}`
/// or `{"ok": false, "error": "..."}`.
///
/// # Safety
/// The returned pointer must be deallocated by calling [`shellumi_string_free`].
#[no_mangle]
pub unsafe extern "C" fn shellumi_translate_virtual_key_c(
    key_name: *const c_char,
    modifier_flags: u32,
) -> *mut c_char {
    let key_str = match safe_c_str_to_str(key_name) {
        Some(s) => s,
        None => {
            return str_to_c_char(r#"{"ok":false,"error":"Key name is null or invalid UTF-8"}"#);
        }
    };

    let is_ctrl = (modifier_flags & SHELLUMI_MODIFIER_CTRL) != 0;
    let is_alt = (modifier_flags & SHELLUMI_MODIFIER_ALT) != 0;

    let virtual_key = match key_str.to_ascii_lowercase().as_str() {
        "esc" | "escape" => VirtualKey::Escape,
        "tab" => VirtualKey::Tab,
        "arrow_up" | "up" => VirtualKey::ArrowUp,
        "arrow_down" | "down" => VirtualKey::ArrowDown,
        "arrow_left" | "left" => VirtualKey::ArrowLeft,
        "arrow_right" | "right" => VirtualKey::ArrowRight,
        "pipe" | "|" => VirtualKey::Pipe,
        "tilde" | "~" => VirtualKey::Tilde,
        "slash" | "/" => VirtualKey::Slash,
        "backslash" | "\\" => VirtualKey::Backslash,
        "hyphen" | "-" => VirtualKey::Hyphen,
        "colon" | ":" => VirtualKey::Colon,
        "backtick" | "`" => VirtualKey::Backtick,
        "home" => VirtualKey::Home,
        "end" => VirtualKey::End,
        "pageup" | "pgup" => VirtualKey::PageUp,
        "pagedown" | "pgdn" => VirtualKey::PageDown,
        "insert" => VirtualKey::Insert,
        "delete" => VirtualKey::Delete,
        "f1" => VirtualKey::Function(1),
        "f2" => VirtualKey::Function(2),
        "f3" => VirtualKey::Function(3),
        "f4" => VirtualKey::Function(4),
        "f5" => VirtualKey::Function(5),
        "f6" => VirtualKey::Function(6),
        "f7" => VirtualKey::Function(7),
        "f8" => VirtualKey::Function(8),
        "f9" => VirtualKey::Function(9),
        "f10" => VirtualKey::Function(10),
        "f11" => VirtualKey::Function(11),
        "f12" => VirtualKey::Function(12),
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap();
            if is_ctrl && is_alt {
                let ctrl_byte = VirtualKey::ControlKey(c).to_bytes();
                let mut bytes = vec![0x1B];
                bytes.extend_from_slice(&ctrl_byte);
                let hex = bytes
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                let out = serde_json::json!({
                    "ok": true,
                    "key": format!("ctrl+alt+{}", c),
                    "bytes": bytes,
                    "hex": hex
                });
                return str_to_c_char(&out.to_string());
            } else if is_ctrl {
                VirtualKey::ControlKey(c)
            } else if is_alt {
                VirtualKey::AltKey(c)
            } else {
                let bytes = c.to_string().into_bytes();
                let hex = bytes
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                let out = serde_json::json!({
                    "ok": true,
                    "key": c.to_string(),
                    "bytes": bytes,
                    "hex": hex
                });
                return str_to_c_char(&out.to_string());
            }
        }
        _ => {
            return str_to_c_char(&format!(
                r#"{{"ok":false,"error":"Unknown virtual key: {}"}}"#,
                key_str
            ));
        }
    };

    let bytes = virtual_key.to_bytes();
    let hex = bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ");
    let out = serde_json::json!({
        "ok": true,
        "key": virtual_key.label(),
        "bytes": bytes,
        "hex": hex
    });
    str_to_c_char(&out.to_string())
}

/// Performs a deterministic state transition on the mobile session lifecycle.
///
/// # Parameters
/// - `current_state_tag`: Current state ("connecting", "connected", "grace", "suspended", "reconnecting", "disconnected").
/// - `event_name`: Trigger event ("foreground", "background", "tick", "expired", "net_lost", "net_restored", "disconnect").
/// - `param`: Integer parameter (e.g. grace seconds for background, elapsed seconds for tick).
/// - `epoch_seconds`: Current Unix epoch seconds for state timestamping.
///
/// # Returns
/// A heap-allocated C string with JSON:
/// `{"ok": true, "state": "...", "is_active": true/false, "reconnect_backoff": ...}`.
///
/// # Safety
/// The returned pointer must be deallocated by calling [`shellumi_string_free`].
#[no_mangle]
pub unsafe extern "C" fn shellumi_lifecycle_transition_c(
    current_state_tag: *const c_char,
    event_name: *const c_char,
    param: u64,
    epoch_seconds: u64,
) -> *mut c_char {
    let state_str = safe_c_str_to_str(current_state_tag).unwrap_or("connecting");
    let event_str = match safe_c_str_to_str(event_name) {
        Some(s) => s,
        None => {
            return str_to_c_char(r#"{"ok":false,"error":"Event name is null"}"#);
        }
    };

    let initial_state = match state_str.to_ascii_lowercase().as_str() {
        "connected" => MobileSessionState::Connected,
        "grace" => MobileSessionState::BackgroundGracePeriod {
            remaining_secs: param as u32,
        },
        "suspended" => MobileSessionState::Suspended {
            saved_at_epoch: epoch_seconds,
        },
        "reconnecting" => MobileSessionState::Reconnecting {
            attempt: 1,
            backoff_secs: 2,
        },
        "disconnected" => MobileSessionState::Disconnected {
            reason: "Initial disconnected state".to_string(),
        },
        _ => MobileSessionState::Connecting,
    };

    let mut manager = MobileLifecycleManager::new(MobilePlatform::Simulator);
    manager.state = initial_state;

    let event = match event_str.to_ascii_lowercase().as_str() {
        "foreground" => MobileLifecycleEvent::AppForegrounded,
        "background" => MobileLifecycleEvent::AppBackgrounded {
            grace_seconds: param as u32,
        },
        "tick" => MobileLifecycleEvent::BackgroundTimerTick {
            elapsed_seconds: param as u32,
        },
        "expired" => MobileLifecycleEvent::BackgroundExpired,
        "net_lost" => MobileLifecycleEvent::NetworkLost,
        "net_restored" => MobileLifecycleEvent::NetworkRestored,
        "disconnect" => MobileLifecycleEvent::UserDisconnect,
        _ => {
            return str_to_c_char(&format!(
                r#"{{"ok":false,"error":"Unknown lifecycle event: {}"}}"#,
                event_str
            ));
        }
    };

    manager.transition(event, epoch_seconds);

    let (state_name, backoff) = match manager.state {
        MobileSessionState::Connecting => ("connecting", 0),
        MobileSessionState::Connected => ("connected", 0),
        MobileSessionState::BackgroundGracePeriod { remaining_secs } => {
            ("grace", remaining_secs as u64)
        }
        MobileSessionState::Suspended { .. } => ("suspended", 0),
        MobileSessionState::Reconnecting { backoff_secs, .. } => {
            ("reconnecting", backoff_secs as u64)
        }
        MobileSessionState::Disconnected { .. } => ("disconnected", 0),
    };

    let out = serde_json::json!({
        "ok": true,
        "state": state_name,
        "is_active": manager.state.is_active(),
        "backoff_or_grace": backoff
    });
    str_to_c_char(&out.to_string())
}

/// Retrieves platform-specific hardware credential storage specifications.
///
/// # Parameters
/// - `platform`: "ios" or "android".
/// - `service_name`: Mobile keychain service identifier (e.g. "org.shellumi.app").
/// - `account_name`: Account/Host key identifier (e.g. "admin@10.0.0.1").
/// - `require_biometric`: Whether hardware biometric unlock is required.
///
/// # Returns
/// A heap-allocated C string containing JSON specifications for Keychain / Keystore APIs.
///
/// # Safety
/// The returned pointer must be deallocated by calling [`shellumi_string_free`].
#[no_mangle]
pub unsafe extern "C" fn shellumi_get_credential_spec_c(
    platform: *const c_char,
    service_name: *const c_char,
    account_name: *const c_char,
    require_biometric: bool,
) -> *mut c_char {
    let platform_str = safe_c_str_to_str(platform).unwrap_or("ios");
    let service_str = safe_c_str_to_str(service_name).unwrap_or("org.shellumi.app");
    let account_str = safe_c_str_to_str(account_name).unwrap_or("default");

    let bio = if require_biometric {
        BiometricRequirement::BiometricAny
    } else {
        BiometricRequirement::None
    };

    let spec = if platform_str.eq_ignore_ascii_case("android") {
        MobileCredentialSpec::new_android(service_str, account_str, bio)
    } else {
        MobileCredentialSpec::new_ios(service_str, account_str, bio)
    };

    let out = serde_json::json!({
        "ok": true,
        "target": format!("{:?}", spec.target),
        "service": spec.service_name,
        "account": spec.account_name,
        "biometric_required": spec.biometric != BiometricRequirement::None,
        "flags_summary": spec.platform_flags_summary()
    });
    str_to_c_char(&out.to_string())
}

/// Deallocates a C string previously allocated by Shellumi C-ABI functions.
///
/// # Safety
/// Calling this with a pointer not allocated by Shellumi, or calling it twice
/// on the same pointer (double free), will cause undefined behavior.
/// Passing a NULL pointer is explicitly safe and performs a no-op.
#[no_mangle]
pub unsafe extern "C" fn shellumi_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // Reconstruct CString and let it drop to free heap memory safely
    drop(CString::from_raw(ptr));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_core_version_c_abi() {
        let ptr = shellumi_core_version();
        assert!(!ptr.is_null());
        let str_val = unsafe { CStr::from_ptr(ptr).to_str().unwrap() };
        assert_eq!(str_val, "0.1.0");
    }

    #[test]
    fn verifies_platform_supported_c_abi() {
        unsafe {
            let ios_str = CString::new("ios").unwrap();
            assert!(shellumi_mobile_platform_supported(ios_str.as_ptr()));

            let android_str = CString::new("Android").unwrap();
            assert!(shellumi_mobile_platform_supported(android_str.as_ptr()));

            let sim_str = CString::new("Simulator").unwrap();
            assert!(shellumi_mobile_platform_supported(sim_str.as_ptr()));

            let invalid_str = CString::new("windows-phone").unwrap();
            assert!(!shellumi_mobile_platform_supported(invalid_str.as_ptr()));

            assert!(!shellumi_mobile_platform_supported(std::ptr::null()));
        }
    }

    #[test]
    fn verifies_parse_destination_c_abi() {
        unsafe {
            // Valid destination
            let input = CString::new("dev@192.168.1.100:2222").unwrap();
            let res_ptr = shellumi_parse_destination_c(input.as_ptr());
            assert!(!res_ptr.is_null());
            let json_str = CStr::from_ptr(res_ptr).to_str().unwrap();
            assert!(json_str.contains(r#""ok":true"#));
            assert!(json_str.contains(r#""host":"192.168.1.100""#));
            assert!(json_str.contains(r#""username":"dev""#));
            assert!(json_str.contains(r#""port":2222"#));
            shellumi_string_free(res_ptr);

            // Invalid destination
            let bad_input = CString::new("invalid-no-at-sign").unwrap();
            let bad_ptr = shellumi_parse_destination_c(bad_input.as_ptr());
            let bad_json = CStr::from_ptr(bad_ptr).to_str().unwrap();
            assert!(bad_json.contains(r#""ok":false"#));
            shellumi_string_free(bad_ptr);

            // Null pointer safety
            let null_ptr = shellumi_parse_destination_c(std::ptr::null());
            let null_json = CStr::from_ptr(null_ptr).to_str().unwrap();
            assert!(null_json.contains(r#""ok":false"#));
            shellumi_string_free(null_ptr);
        }
    }

    #[test]
    fn verifies_generate_keypair_c_abi() {
        unsafe {
            let comment = CString::new("mobile-key").unwrap();
            let res_ptr = shellumi_generate_keypair_c(comment.as_ptr());
            assert!(!res_ptr.is_null());
            let json_str = CStr::from_ptr(res_ptr).to_str().unwrap();
            assert!(json_str.contains(r#""ok":true"#));
            assert!(json_str.contains(r#""fingerprint":""#));
            assert!(json_str.contains(r#""comment":"mobile-key""#));
            shellumi_string_free(res_ptr);

            // Null comment fallback
            let null_comment_ptr = shellumi_generate_keypair_c(std::ptr::null());
            let json_str_2 = CStr::from_ptr(null_comment_ptr).to_str().unwrap();
            assert!(json_str_2.contains(r#""ok":true"#));
            assert!(json_str_2.contains(r#""comment":"shellumi-mobile""#));
            shellumi_string_free(null_comment_ptr);
        }
    }

    #[test]
    fn verifies_translate_virtual_key_c_abi() {
        unsafe {
            // Arrow Up
            let key = CString::new("arrow_up").unwrap();
            let res_ptr = shellumi_translate_virtual_key_c(key.as_ptr(), 0);
            let json_str = CStr::from_ptr(res_ptr).to_str().unwrap();
            assert!(json_str.contains(r#""ok":true"#));
            assert!(json_str.contains(r#""hex":"1B 5B 41""#));
            shellumi_string_free(res_ptr);

            // Ctrl+C with bitmask
            let c_key = CString::new("c").unwrap();
            let res_ptr = shellumi_translate_virtual_key_c(c_key.as_ptr(), SHELLUMI_MODIFIER_CTRL);
            let json_str = CStr::from_ptr(res_ptr).to_str().unwrap();
            assert!(json_str.contains(r#""ok":true"#));
            assert!(json_str.contains(r#""bytes":[3]"#));
            shellumi_string_free(res_ptr);

            // Ctrl+Alt+A
            let a_key = CString::new("a").unwrap();
            let res_ptr = shellumi_translate_virtual_key_c(
                a_key.as_ptr(),
                SHELLUMI_MODIFIER_CTRL | SHELLUMI_MODIFIER_ALT,
            );
            let json_str = CStr::from_ptr(res_ptr).to_str().unwrap();
            assert!(json_str.contains(r#""ok":true"#));
            assert!(json_str.contains(r#""bytes":[27,1]"#));
            shellumi_string_free(res_ptr);

            // Null key name
            let null_res = shellumi_translate_virtual_key_c(std::ptr::null(), 0);
            let json_str = CStr::from_ptr(null_res).to_str().unwrap();
            assert!(json_str.contains(r#""ok":false"#));
            shellumi_string_free(null_res);
        }
    }

    #[test]
    fn verifies_lifecycle_transition_c_abi() {
        unsafe {
            let state = CString::new("connected").unwrap();
            let event = CString::new("background").unwrap();
            let res_ptr = shellumi_lifecycle_transition_c(state.as_ptr(), event.as_ptr(), 30, 1000);
            let json_str = CStr::from_ptr(res_ptr).to_str().unwrap();
            assert!(json_str.contains(r#""ok":true"#));
            assert!(json_str.contains(r#""state":"grace""#));
            assert!(json_str.contains(r#""is_active":true"#));
            shellumi_string_free(res_ptr);

            // Grace period tick expiration
            let state = CString::new("grace").unwrap();
            let event = CString::new("tick").unwrap();
            let res_ptr = shellumi_lifecycle_transition_c(state.as_ptr(), event.as_ptr(), 30, 1030);
            let json_str = CStr::from_ptr(res_ptr).to_str().unwrap();
            assert!(json_str.contains(r#""ok":true"#));
            assert!(json_str.contains(r#""state":"suspended""#));
            assert!(json_str.contains(r#""is_active":false"#));
            shellumi_string_free(res_ptr);
        }
    }

    #[test]
    fn verifies_credential_spec_c_abi() {
        unsafe {
            let platform = CString::new("ios").unwrap();
            let service = CString::new("org.shellumi.app").unwrap();
            let account = CString::new("admin@prod").unwrap();
            let res_ptr = shellumi_get_credential_spec_c(
                platform.as_ptr(),
                service.as_ptr(),
                account.as_ptr(),
                true,
            );
            let json_str = CStr::from_ptr(res_ptr).to_str().unwrap();
            assert!(json_str.contains(r#""ok":true"#));
            assert!(json_str.contains("kSecAccessControlBiometryAny"));
            shellumi_string_free(res_ptr);
        }
    }

    #[test]
    fn verifies_string_free_null_safety() {
        unsafe {
            // Null pointer must not panic or cause SIGSEGV
            shellumi_string_free(std::ptr::null_mut());
        }
    }
}
