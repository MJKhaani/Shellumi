# xattr 0.2.3 compatibility patch

Source: crates.io `xattr` 0.2.3, checksum
`6d1526bbe5aaeb5eb06885f4d987bcdfa5e23187055de9b83fe00156a821fabc`.
Upstream: https://github.com/Stebalien/xattr. MIT OR Apache-2.0 licenses retained.

GPUI 0.2.2 depends on `zed-async-tar`, which requires xattr 0.2.x. The latest
0.2.x release is old and references `libc::ENOATTR` on Linux/Android. Current libc
removed that deprecated alias, breaking Shellumi's previous Linux release build.
The sole functional source change is to use `libc::ENODATA` on Linux/Android,
while retaining `ENOATTR` for macOS/BSD. Deprecated macOS `libc::uint32_t` aliases are also spelled `u32` (the identical
ABI type) to avoid warnings with current libc. Public API, syscall implementations and
other dependencies are unchanged. This reuses the already-resolved dependency;
it does not downgrade libc or add a new runtime dependency. Formatting may follow
the workspace formatter. Remove the patch when the GPUI dependency chain moves
to a maintained xattr version. Local macOS upstream tests validate the unchanged native behavior. A Linux-target
compile check was attempted, but the Rust standard-library download failed DNS
resolution. Native Linux compilation and full app validation remain with CI.
