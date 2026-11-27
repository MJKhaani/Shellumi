//! Native OS clipboard broker, echo suppression, and cross-platform file references.
//!
//! GPUI 0.2.2 `ClipboardItem` supports string and image entries, but not native file lists
//! (`NSFilenamesPboardType` / `NSURL` / `CF_HDROP` / `text/uri-list`). This module provides
//! a cross-platform bridge and a privacy-preserving `NativeClipboardBroker`.

use std::path::PathBuf;

/// Hash clipboard payload without retaining or logging plaintext contents.
pub fn hash_clipboard_bytes(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Per-session / per-app clipboard broker tracking change tokens and hashes for echo suppression.
#[derive(Debug, Clone, Default)]
pub struct NativeClipboardBroker {
    last_observed_change: Option<isize>,
    last_local_offer_hash: Option<u64>,
    last_remote_written_hash: Option<u64>,
}

impl NativeClipboardBroker {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that we wrote remote clipboard content to the local OS clipboard.
    pub fn record_remote_write(&mut self, change_count: isize, text: &str) {
        self.last_observed_change = Some(change_count);
        self.last_remote_written_hash = Some(hash_clipboard_bytes(text.as_bytes()));
    }

    /// Check whether a local clipboard read should be sent to the remote session.
    /// Suppresses echo if the OS change token is unchanged and the hash matches our own
    /// remote->local write; allows re-sending identical text if the user explicitly copied again
    /// (new change count).
    pub fn should_send_local_text(&mut self, change_count: isize, text: &str) -> bool {
        let h = hash_clipboard_bytes(text.as_bytes());
        if self.last_observed_change == Some(change_count)
            && self.last_remote_written_hash == Some(h)
        {
            return false;
        }
        self.last_observed_change = Some(change_count);
        self.last_local_offer_hash = Some(h);
        self.last_remote_written_hash = None;
        true
    }

    /// Clear broker state on disconnect, view-only, or policy disable.
    pub fn reset(&mut self) {
        self.last_observed_change = None;
        self.last_local_offer_hash = None;
        self.last_remote_written_hash = None;
    }
}

/// Parse a `text/uri-list` payload into validated local regular file paths.
pub fn parse_file_uri_list(uri_list: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for line in uri_list.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("file://") else {
            continue;
        };
        let path_part = if let Some(stripped) = rest.strip_prefix("localhost") {
            stripped
        } else if rest.starts_with('/') {
            rest
        } else {
            continue;
        };
        let decoded = percent_decode(path_part);
        let pb = PathBuf::from(decoded);
        if pb.is_absolute() && pb.is_file() {
            out.push(pb);
        }
    }
    out
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Return the current OS pasteboard change count (or 0 if unavailable).
#[cfg(target_os = "macos")]
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

#[cfg(target_os = "windows")]
pub fn pasteboard_change_count() -> isize {
    unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() as isize }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
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
        let mut out = Vec::new();

        // 1. Legacy Finder file list (NSFilenamesPboardType)
        let pboard_type: *mut Object = msg_send![
            nsstring_cls,
            stringWithUTF8String: c"NSFilenamesPboardType".as_ptr()
        ];
        if !pboard_type.is_null() {
            let list: *mut Object = msg_send![pb, propertyListForType: pboard_type];
            if !list.is_null() {
                let count: usize = msg_send![list, count];
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
            }
        }
        if !out.is_empty() {
            return out;
        }

        // 2. Modern NSURL file URL representation (public.file-url)
        let url_type: *mut Object = msg_send![
            nsstring_cls,
            stringWithUTF8String: c"public.file-url".as_ptr()
        ];
        if !url_type.is_null() {
            let url_str_obj: *mut Object = msg_send![pb, stringForType: url_type];
            if !url_str_obj.is_null() {
                let c_str: *const std::os::raw::c_char = msg_send![url_str_obj, UTF8String];
                if !c_str.is_null() {
                    if let Ok(uri) = CStr::from_ptr(c_str).to_str() {
                        out.extend(parse_file_uri_list(uri));
                    }
                }
            }
        }
        out
    }
}

#[cfg(target_os = "windows")]
pub fn read_clipboard_files() -> Vec<PathBuf> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, OpenClipboard,
    };
    use windows_sys::Win32::UI::Shell::DragQueryFileW;

    const CF_HDROP: u32 = 15;
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let hdrop = GetClipboardData(CF_HDROP);
        if !hdrop.is_null() {
            let count = DragQueryFileW(hdrop as _, u32::MAX, std::ptr::null_mut(), 0);
            for idx in 0..count.min(256) {
                let len = DragQueryFileW(hdrop as _, idx, std::ptr::null_mut(), 0);
                if len == 0 {
                    continue;
                }
                let mut buf = vec![0u16; (len + 1) as usize];
                let copied = DragQueryFileW(hdrop as _, idx, buf.as_mut_ptr(), len + 1);
                if copied > 0 {
                    let s = String::from_utf16_lossy(&buf[..copied as usize]);
                    let pb = PathBuf::from(s);
                    if pb.is_absolute() && pb.is_file() {
                        out.push(pb);
                    }
                }
            }
        }
        CloseClipboard();
        out
    }
}

#[cfg(target_os = "linux")]
pub fn read_clipboard_files() -> Vec<PathBuf> {
    for (cmd, args) in [
        ("wl-paste", &["--no-newline", "--type", "text/uri-list"][..]),
        (
            "xclip",
            &["-o", "-selection", "clipboard", "-t", "text/uri-list"][..],
        ),
    ] {
        if let Ok(output) = std::process::Command::new(cmd).args(args).output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let paths = parse_file_uri_list(&text);
                if !paths.is_empty() {
                    return paths;
                }
            }
        }
    }
    Vec::new()
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn read_clipboard_files() -> Vec<PathBuf> {
    Vec::new()
}

/// Publish validated local file paths onto the OS clipboard so the user can paste them
/// into Finder / Explorer / desktop file managers.
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

#[cfg(target_os = "windows")]
pub fn write_clipboard_files(paths: &[PathBuf]) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };

    #[repr(C)]
    struct DropFiles {
        p_files: u32,
        pt_x: i32,
        pt_y: i32,
        f_nc: i32,
        f_wide: i32,
    }

    const CF_HDROP: u32 = 15;
    let valid: Vec<PathBuf> = paths
        .iter()
        .filter(|p| p.is_absolute() && p.is_file())
        .cloned()
        .collect();
    if valid.is_empty() {
        return Err("No valid local files to publish to clipboard".into());
    }

    let mut wide_chars: Vec<u16> = Vec::new();
    for p in &valid {
        wide_chars.extend(p.as_os_str().encode_wide());
        wide_chars.push(0);
    }
    wide_chars.push(0);

    let header_size = std::mem::size_of::<DropFiles>();
    let total_bytes = header_size + wide_chars.len() * 2;
    unsafe {
        let hmem = GlobalAlloc(GMEM_MOVEABLE, total_bytes);
        if hmem.is_null() {
            return Err("GlobalAlloc failed for CF_HDROP".into());
        }
        let ptr = GlobalLock(hmem) as *mut u8;
        if ptr.is_null() {
            return Err("GlobalLock failed for CF_HDROP".into());
        }
        let header = DropFiles {
            p_files: header_size as u32,
            pt_x: 0,
            pt_y: 0,
            f_nc: 0,
            f_wide: 1,
        };
        std::ptr::copy_nonoverlapping(&header as *const DropFiles as *const u8, ptr, header_size);
        std::ptr::copy_nonoverlapping(
            wide_chars.as_ptr() as *const u8,
            ptr.add(header_size),
            wide_chars.len() * 2,
        );
        GlobalUnlock(hmem);

        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err("OpenClipboard failed".into());
        }
        EmptyClipboard();
        let res = SetClipboardData(CF_HDROP, hmem as _);
        CloseClipboard();
        if res.is_null() {
            Err("SetClipboardData(CF_HDROP) failed".into())
        } else {
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
pub fn write_clipboard_files(paths: &[PathBuf]) -> Result<(), String> {
    use std::io::Write;
    let valid: Vec<PathBuf> = paths
        .iter()
        .filter(|p| p.is_absolute() && p.is_file())
        .cloned()
        .collect();
    if valid.is_empty() {
        return Err("No valid local files to publish to clipboard".into());
    }
    let payload: String = valid
        .iter()
        .map(|p| format!("file://{}\r\n", p.to_string_lossy()))
        .collect();

    for (cmd, args) in [
        ("wl-copy", &["--type", "text/uri-list"][..]),
        (
            "xclip",
            &["-i", "-selection", "clipboard", "-t", "text/uri-list"][..],
        ),
    ] {
        if let Ok(mut child) = std::process::Command::new(cmd)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(payload.as_bytes());
            }
            if child.wait().is_ok_and(|s| s.success()) {
                return Ok(());
            }
        }
    }
    Err("No Linux clipboard utility (wl-copy or xclip) succeeded for text/uri-list".into())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn write_clipboard_files(_paths: &[PathBuf]) -> Result<(), String> {
    Err("Native OS file clipboard bridge is not supported on this platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broker_suppresses_echo_but_allows_explicit_recopy_on_new_change_count() {
        let mut broker = NativeClipboardBroker::new();
        broker.record_remote_write(42, "hello");
        // Same changeCount and hash -> echo suppressed
        assert!(!broker.should_send_local_text(42, "hello"));
        // User explicitly copies "hello" again -> new changeCount -> allowed
        assert!(broker.should_send_local_text(43, "hello"));
        // Reset clears tracking
        broker.reset();
        assert!(broker.should_send_local_text(42, "hello"));
    }

    #[test]
    fn parse_file_uri_list_validates_local_regular_files() {
        let dir = tempfile::tempdir().unwrap();
        let fpath = dir.path().join("sample file.txt");
        std::fs::write(&fpath, b"abc").unwrap();
        let encoded = fpath.to_string_lossy().replace(' ', "%20");
        let uri_list = format!("# comment\r\nfile://{encoded}\r\nhttps://example.com/nope\r\n");
        let parsed = parse_file_uri_list(&uri_list);
        assert_eq!(parsed, vec![fpath]);
    }
}
