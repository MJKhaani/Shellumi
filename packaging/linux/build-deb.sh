#!/usr/bin/env bash
set -euo pipefail

# Shellumi Debian Package (.deb) Build Script
# Usage: ./packaging/linux/build-deb.sh [arch] [app-bin] [cli-bin]

ARCH="${1:-amd64}"
APP_BIN="${2:-target/release/shellumi-app}"
CLI_BIN="${3:-target/release/shellumi-cli}"
VERSION="0.1.0"
PKG_DIR="dist/shellumi_${VERSION}_${ARCH}"

echo "==> Preparing Debian package directory structure..."
rm -rf "${PKG_DIR}"
mkdir -p "${PKG_DIR}/DEBIAN"
mkdir -p "${PKG_DIR}/usr/bin"
mkdir -p "${PKG_DIR}/usr/share/applications"
mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/scalable/apps"
mkdir -p "${PKG_DIR}/usr/share/doc/shellumi"

# Control file
sed "s/Architecture: amd64/Architecture: ${ARCH}/" packaging/linux/debian/control > "${PKG_DIR}/DEBIAN/control"

# Binaries
if [ -f "${APP_BIN}" ]; then
    cp "${APP_BIN}" "${PKG_DIR}/usr/bin/shellumi-app"
    chmod 755 "${PKG_DIR}/usr/bin/shellumi-app"
fi

if [ -f "${CLI_BIN}" ]; then
    cp "${CLI_BIN}" "${PKG_DIR}/usr/bin/shellumi-cli"
    chmod 755 "${PKG_DIR}/usr/bin/shellumi-cli"
fi

# Desktop entry & Icon
cp packaging/linux/shellumi.desktop "${PKG_DIR}/usr/share/applications/"
if [ -f "resources/logo.svg" ]; then
    cp resources/logo.svg "${PKG_DIR}/usr/share/icons/hicolor/scalable/apps/shellumi.svg"
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
    dpkg-deb --build "${PKG_DIR}" "dist/shellumi_${VERSION}_${ARCH}.deb"
    echo "==> Successfully created dist/shellumi_${VERSION}_${ARCH}.deb"
else
    echo "==> dpkg-deb not available, creating tar.gz release bundle..."
    tar -czf "dist/shellumi_${VERSION}_linux_${ARCH}.tar.gz" -C "${PKG_DIR}/usr" .
    echo "==> Successfully created dist/shellumi_${VERSION}_linux_${ARCH}.tar.gz"
fi
