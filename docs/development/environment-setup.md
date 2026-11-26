# Environment Setup

[Docs Home](../README.md) / [Development](README.md) / Environment Setup

---

## Rust Toolchain

Install `rustup` with the stable toolchain and `rustfmt` + `clippy`:

```sh
rustup toolchain install stable --component rustfmt --component clippy
```

## Platform System Dependencies

- **macOS:** Xcode Command Line Tools and Metal compiler (`xcrun --find metal`).
- **Linux (Ubuntu/Debian):**
  ```sh
  sudo apt-get update && sudo apt-get install -y --no-install-recommends \
    pkg-config clang cmake libssl-dev libfontconfig1-dev libfreetype6-dev \
    libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
    libx11-xcb-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libasound2-dev dpkg-dev desktop-file-utils
  ```
- **Windows:** Visual Studio C++ Build Tools (MSVC x86_64) and PowerShell.
