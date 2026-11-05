#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Shellumi — Build iOS XCFramework from Rust Core
# Target Architectures:
#   - aarch64-apple-ios (Physical iOS devices)
#   - aarch64-apple-ios-sim (Apple Silicon iOS Simulator)
#   - x86_64-apple-ios (Intel iOS Simulator)
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
OUTPUT_DIR="${SCRIPT_DIR}/Frameworks"
HEADER_PATH="${REPO_ROOT}/crates/shellumi-core/include/shellumi_mobile.h"

echo "=== Building Shellumi Mobile Core for iOS ==="
echo "Repo root:   ${REPO_ROOT}"
echo "Header path: ${HEADER_PATH}"
echo "Output dir:  ${OUTPUT_DIR}"

if [[ ! -f "${HEADER_PATH}" ]]; then
    echo "ERROR: Header file not found at ${HEADER_PATH}"
    exit 1
fi

mkdir -p "${OUTPUT_DIR}"

# Build static library for iOS device (arm64)
echo "--> Compiling for aarch64-apple-ios..."
cargo build --manifest-path "${REPO_ROOT}/Cargo.toml" \
    --package shellumi-core \
    --lib \
    --release \
    --target aarch64-apple-ios || {
    echo "Notice: aarch64-apple-ios target not installed. Run: rustup target add aarch64-apple-ios"
}

# Build static library for iOS Simulator (arm64)
echo "--> Compiling for aarch64-apple-ios-sim..."
cargo build --manifest-path "${REPO_ROOT}/Cargo.toml" \
    --package shellumi-core \
    --lib \
    --release \
    --target aarch64-apple-ios-sim || {
    echo "Notice: aarch64-apple-ios-sim target not installed. Run: rustup target add aarch64-apple-ios-sim"
}

# Assemble XCFramework if libraries exist
DEVICE_LIB="${REPO_ROOT}/target/aarch64-apple-ios/release/libshellumi_core.a"
SIM_LIB="${REPO_ROOT}/target/aarch64-apple-ios-sim/release/libshellumi_core.a"

if [[ -f "${DEVICE_LIB}" && -f "${SIM_LIB}" ]]; then
    rm -rf "${OUTPUT_DIR}/ShellumiCore.xcframework"
    xcodebuild -create-xcframework \
        -library "${DEVICE_LIB}" \
        -headers "${REPO_ROOT}/crates/shellumi-core/include" \
        -library "${SIM_LIB}" \
        -headers "${REPO_ROOT}/crates/shellumi-core/include" \
        -output "${OUTPUT_DIR}/ShellumiCore.xcframework"
    echo "=== Successfully created ${OUTPUT_DIR}/ShellumiCore.xcframework ==="
else
    echo "=== Skipping XCFramework packaging (requires rustup iOS targets and Xcode toolchain) ==="
    echo "To compile for physical iOS devices and simulators:"
    echo "  rustup target add aarch64-apple-ios aarch64-apple-ios-sim"
    echo "  ./packaging/mobile/ios/build-xcframework.sh"
fi
