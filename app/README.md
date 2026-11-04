# Harbor

A native Rust desktop app using egui/eframe and portable-pty, targeting Windows,
macOS, and Linux. No Electron or JavaScript runtime is required.

Install stable Rust, then run `cargo run --release` from this directory. Build
with `cargo build --release`. Build separately on each target operating system.
Linux requires the native development packages for the window system and OpenGL
(on Debian/Ubuntu: `build-essential pkg-config libssl-dev libx11-dev libxi-dev
libxcursor-dev libxrandr-dev libxinerama-dev libgl1-mesa-dev libwayland-dev
libxkbcommon-dev`).

The local terminal uses COMSPEC (cmd.exe by default) on Windows, and SHELL
(/bin/sh by default) on Unix. Remote connections require OpenSSH's `ssh` on PATH;
on Windows enable the OpenSSH Client optional feature. Host-key verification is
handled by OpenSSH. Connection metadata is saved using eframe's platform-specific
application storage; passwords are never saved.

This initial version provides saved hosts and a single terminal session with
line-based input. Full terminal keyboard interaction, file transfer, and port
forwarding are not yet implemented. Windows and Linux builds require verification
on those systems.
