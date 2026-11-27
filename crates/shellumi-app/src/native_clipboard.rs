//! Native OS clipboard bridge for file references and pasteboard change tracking.
//!
//! GPUI 0.2.2 `ClipboardItem` supports string and image entries, but not native file lists
//! (`NSFilenamesPboardType` / `CF_HDROP` / `text/uri-list`). This module provides a minimal
//! platform bridge using existing workspace dependencies.

use std::path::PathBuf;

/// Return the current OS pasteboard change count (or 0 if unavailable).
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub fn pasteboard_change_count() -> isize {
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};
    unsafe {
        let Some(cls) = Class::get("NSPasteboard") else {
            return 0;
        };
        let pb: *mut Object = msg_send![cls, generalPasteboard];
        if pb.is_null() {
            return 0;
        }
        let count: isize = msg_send![pb, changeCount];
        count
    }
}

#[cfg(not(target_os = "macos"))]
pub fn pasteboard_change_count() -> isize {
    0
}

/// Read local regular file paths currently on the OS clipboard (e.g., copied in Finder).
#[cfg(target_os = "macos")]
pub fn read_clipboard_files() -> Vec<PathBuf> {
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};
    use std::ffi::CStr;

    unsafe {
        let Some(pb_cls) = Class::get("NSPasteboard") else {
            return Vec::new();
        };
        let Some(nsstring_cls) = Class::get("NSString") else {
            return Vec::new();
        };
        let pb: *mut Object = msg_send![pb_cls, generalPasteboard];
        if pb.is_null() {
            return Vec::new();
        }
        let pboard_type: *mut Object = msg_send![
            nsstring_cls,
            stringWithUTF8String: c"NSFilenamesPboardType".as_ptr()
        ];
        if pboard_type.is_null() {
            return Vec::new();
        }
        let list: *mut Object = msg_send![pb, propertyListForType: pboard_type];
        if list.is_null() {
            return Vec::new();
        }
        let count: usize = msg_send![list, count];
        let mut out = Vec::with_capacity(count.min(256));
        for idx in 0..count.min(256) {
            let item: *mut Object = msg_send![list, objectAtIndex: idx];
            if item.is_null() {
                continue;
            }
            let c_str: *const std::os::raw::c_char = msg_send![item, UTF8String];
            if c_str.is_null() {
                continue;
            }
            if let Ok(path_str) = CStr::from_ptr(c_str).to_str() {
                let pb_path = PathBuf::from(path_str);
                if pb_path.is_absolute() && pb_path.is_file() {
                    out.push(pb_path);
                }
            }
        }
        out
    }
}

#[cfg(not(target_os = "macos"))]
pub fn read_clipboard_files() -> Vec<PathBuf> {
    Vec::new()
}

/// Publish validated local file paths onto the OS clipboard so the user can paste them
/// into Finder / desktop file managers.
#[cfg(target_os = "macos")]
pub fn write_clipboard_files(paths: &[PathBuf]) -> Result<(), String> {
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};
    use std::ffi::CString;

    let valid: Vec<PathBuf> = paths
        .iter()
        .filter(|p| p.is_absolute() && p.is_file())
        .cloned()
        .collect();
    if valid.is_empty() {
        return Err("No valid local files to publish to clipboard".into());
    }

    unsafe {
        let pb_cls = Class::get("NSPasteboard").ok_or("NSPasteboard unavailable")?;
        let nsstring_cls = Class::get("NSString").ok_or("NSString unavailable")?;
        let array_cls = Class::get("NSMutableArray").ok_or("NSMutableArray unavailable")?;

        let pb: *mut Object = msg_send![pb_cls, generalPasteboard];
        if pb.is_null() {
            return Err("Failed to acquire generalPasteboard".into());
        }
        let pboard_type: *mut Object = msg_send![
            nsstring_cls,
            stringWithUTF8String: c"NSFilenamesPboardType".as_ptr()
        ];
        let types_arr: *mut Object = msg_send![array_cls, arrayWithCapacity: 1usize];
        let _: () = msg_send![types_arr, addObject: pboard_type];
        let _: isize = msg_send![pb, declareTypes: types_arr owner: std::ptr::null::<Object>()];

        let files_arr: *mut Object = msg_send![array_cls, arrayWithCapacity: valid.len()];
        for path in &valid {
            let Ok(c_path) = CString::new(path.to_string_lossy().as_bytes()) else {
                continue;
            };
            let ns_path: *mut Object =
                msg_send![nsstring_cls, stringWithUTF8String: c_path.as_ptr()];
            if !ns_path.is_null() {
                let _: () = msg_send![files_arr, addObject: ns_path];
            }
        }
        let ok: bool = msg_send![pb, setPropertyList: files_arr forType: pboard_type];
        if ok {
            Ok(())
        } else {
            Err("NSPasteboard setPropertyList failed".into())
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn write_clipboard_files(_paths: &[PathBuf]) -> Result<(), String> {
    Err("Native OS file clipboard bridge is not yet implemented on this platform".into())
}
