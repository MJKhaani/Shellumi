#!/usr/bin/env bash
set -euo pipefail

# Shellumi Debian Package (.deb) Build Script
# Usage: ./packaging/linux/build-deb.sh [arch] [app-bin] [cli-bin]

ARCH="${1:-amd64}"
APP_BIN="${2:-target/release/shellumi-app}"
CLI_BIN="${3:-target/release/shellumi-cli}"
VERSION="${SHELLUMI_VERSION:-0.0.3-alpha}"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] || { echo "Expected a semantic release version" >&2; exit 1; }
case "$ARCH" in x86_64|amd64) ARCH=amd64 ;; aarch64|arm64) ARCH=arm64 ;; *) echo "Unsupported package architecture" >&2; exit 1 ;; esac
# Keep the replacement literal; Bash parameter substitution can expand ~ to HOME.
DEB_VERSION="$(printf '%s\n' "$VERSION" | sed 's/-/~/')"
[[ -f "$APP_BIN" && -f "$CLI_BIN" ]] || { echo "Both release binaries are required" >&2; exit 1; }
PKG_DIR="dist/shellumi_${VERSION}_${ARCH}"

echo "==> Preparing Debian package directory structure..."
rm -rf "${PKG_DIR}"
mkdir -p "${PKG_DIR}/DEBIAN"
mkdir -p "${PKG_DIR}/usr/bin"
mkdir -p "${PKG_DIR}/usr/share/applications"
mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/scalable/apps"
mkdir -p "${PKG_DIR}/usr/share/doc/shellumi"

# Control file
sed -e "s/Architecture: amd64/Architecture: ${ARCH}/" -e "s/^Version: .*/Version: ${DEB_VERSION}/" packaging/linux/debian/control > "${PKG_DIR}/DEBIAN/control"

# Binaries
if [ -f "${APP_BIN}" ]; then
    cp "${APP_BIN}" "${PKG_DIR}/usr/bin/shellumi-app"
    chmod 755 "${PKG_DIR}/usr/bin/shellumi-app"
fi

if [ -f "${CLI_BIN}" ]; then
    cp "${CLI_BIN}" "${PKG_DIR}/usr/bin/shellumi-cli"
    chmod 755 "${PKG_DIR}/usr/bin/shellumi-cli"
fi

# Desktop entry & Icons (FreeDesktop Hicolor Theme)
cp packaging/linux/shellumi.desktop "${PKG_DIR}/usr/share/applications/"
for size in 16x16 24x24 32x32 48x48 64x64 128x128 256x256 512x512; do
    if [ -f "resources/icons/hicolor/${size}/apps/shellumi.png" ]; then
        mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/${size}/apps"
        cp "resources/icons/hicolor/${size}/apps/shellumi.png" "${PKG_DIR}/usr/share/icons/hicolor/${size}/apps/shellumi.png"
    fi
done
if [ -f "resources/icons/hicolor/scalable/apps/shellumi.svg" ]; then
    mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/scalable/apps"
    cp "resources/icons/hicolor/scalable/apps/shellumi.svg" "${PKG_DIR}/usr/share/icons/hicolor/scalable/apps/shellumi.svg"
elif [ -f "resources/logo.svg" ]; then
    mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/scalable/apps"
    cp "resources/logo.svg" "${PKG_DIR}/usr/share/icons/hicolor/scalable/apps/shellumi.svg"
fi

# Documentation & License
if [ -f "LICENSE" ]; then
    cp LICENSE "${PKG_DIR}/usr/share/doc/shellumi/copyright"
fi
if [ -f "README.md" ]; then
    cp README.md "${PKG_DIR}/usr/share/doc/shellumi/"
fi

# Build .deb
if command -v dpkg-deb >/dev/null 2>&1; then
    echo "==> Building .deb package..."
    dpkg-deb --root-owner-group --build "${PKG_DIR}" "dist/shellumi_${VERSION}_${ARCH}.deb"
    echo "==> Successfully created dist/shellumi_${VERSION}_${ARCH}.deb"
else
    echo "==> dpkg-deb not available, creating tar.gz release bundle..."
    tar -czf "dist/shellumi_${VERSION}_linux_${ARCH}.tar.gz" -C "${PKG_DIR}/usr" .
    echo "==> Successfully created dist/shellumi_${VERSION}_linux_${ARCH}.tar.gz"
fi
