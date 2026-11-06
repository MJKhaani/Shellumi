#!/usr/bin/env bash
set -euo pipefail

# Shellumi macOS App Bundle & DMG Packaging Script
# Usage: ./packaging/macos/build-dmg.sh [target-arch] [release-binary-path]

ARCH="${1:-$(uname -m)}"
BIN_PATH="${2:-target/release/shellumi-app}"
VERSION="${SHELLUMI_VERSION:-0.0.1-alpha}"
OUTPUT_DIR="dist"
APP_NAME="Shellumi.app"
APP_BUNDLE="${OUTPUT_DIR}/${APP_NAME}"
DMG_NAME="Shellumi-${VERSION}-macos-${ARCH}.dmg"

echo "==> Building macOS Application Bundle for ${ARCH}..."
mkdir -p "${APP_BUNDLE}/Contents/MacOS"
mkdir -p "${APP_BUNDLE}/Contents/Resources"

cp "${BIN_PATH}" "${APP_BUNDLE}/Contents/MacOS/shellumi-app"
chmod +x "${APP_BUNDLE}/Contents/MacOS/shellumi-app"

cp packaging/macos/Info.plist "${APP_BUNDLE}/Contents/Info.plist"
if [ -f "resources/logo.png" ]; then
    cp resources/logo.png "${APP_BUNDLE}/Contents/Resources/AppIcon.png"
fi

echo "==> Application bundle created at ${APP_BUNDLE}"

if command -v hdiutil >/dev/null 2>&1; then
    echo "==> Generating disk image (${DMG_NAME})..."
    hdiutil create -volname "Shellumi" \
        -srcfolder "${APP_BUNDLE}" \
        -ov -format UDZO \
        "${OUTPUT_DIR}/${DMG_NAME}"
    echo "==> Successfully created ${OUTPUT_DIR}/${DMG_NAME}"
else
    echo "==> hdiutil not found, packaging as ZIP archive..."
    (cd "${OUTPUT_DIR}" && zip -r "Shellumi-${VERSION}-macos-${ARCH}.zip" "${APP_NAME}")
fi
