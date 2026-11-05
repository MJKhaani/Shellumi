#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Shellumi — Build Android Native Libraries (JNI) from Rust Core
# Target ABIs:
#   - arm64-v8a (aarch64-linux-android)
#   - x86_64    (x86_64-linux-android)
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
JNI_LIBS_DIR="${SCRIPT_DIR}/src/main/jniLibs"

echo "=== Building Shellumi Mobile Core for Android ==="
echo "Repo root:    ${REPO_ROOT}"
echo "JNI output:   ${JNI_LIBS_DIR}"

mkdir -p "${JNI_LIBS_DIR}/arm64-v8a"
mkdir -p "${JNI_LIBS_DIR}/x86_64"

# Check if cargo-ndk is installed
if ! command -v cargo-ndk &> /dev/null; then
    echo "Notice: cargo-ndk is not installed."
    echo "To compile for Android, install cargo-ndk:"
    echo "  cargo install cargo-ndk"
    echo "  rustup target add aarch64-linux-android x86_64-linux-android"
    echo "Skipping direct compilation step."
    exit 0
fi

# Build for arm64-v8a
echo "--> Compiling for aarch64-linux-android (arm64-v8a)..."
cargo ndk -t arm64-v8a -o "${JNI_LIBS_DIR}" build --release --package shellumi-core

# Build for x86_64 emulator
echo "--> Compiling for x86_64-linux-android (x86_64)..."
cargo ndk -t x86_64 -o "${JNI_LIBS_DIR}" build --release --package shellumi-core

echo "=== Android native libraries successfully compiled into ${JNI_LIBS_DIR} ==="
