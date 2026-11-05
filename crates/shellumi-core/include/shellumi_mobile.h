/**
 * @file shellumi_mobile.h
 * @brief C-ABI Foreign Function Interface definitions for Shellumi Core.
 *
 * This header defines the stable C-ABI surface for integrating Shellumi's
 * portable Rust core into native mobile frontends:
 * - iOS / iPadOS (Swift / SwiftUI via Swift Package Manager & XCFramework)
 * - Android (Kotlin / Jetpack Compose via JNI & Android AAR)
 *
 * @copyright (c) 2026 Shellumi Project Contributors. MIT License.
 */

#ifndef SHELLUMI_MOBILE_H
#define SHELLUMI_MOBILE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* -------------------------------------------------------------------------
 * Modifier Bitmasks
 * ------------------------------------------------------------------------- */

/** Modifier flag indicating Control key is active */
#define SHELLUMI_MODIFIER_CTRL  0x01

/** Modifier flag indicating Alt/Option key is active */
#define SHELLUMI_MODIFIER_ALT   0x02

/** Modifier flag indicating Shift key is active */
#define SHELLUMI_MODIFIER_SHIFT 0x04

/* -------------------------------------------------------------------------
 * Version & Platform Discovery
 * ------------------------------------------------------------------------- */

/**
 * @brief Returns the static semantic version string of Shellumi Core.
 *
 * @return Statically allocated null-terminated C string (e.g. "0.1.0").
 *         Must NOT be freed by the caller.
 */
const char* shellumi_core_version(void);

/**
 * @brief Checks if a given platform identifier is supported.
 *
 * @param platform_name Null-terminated string ("ios", "android", "simulator").
 * @return true if platform is supported, false otherwise.
 */
bool shellumi_mobile_platform_supported(const char* platform_name);

/* -------------------------------------------------------------------------
 * Connection & Cryptography
 * ------------------------------------------------------------------------- */

/**
 * @brief Parses an SSH destination string into structured JSON.
 *
 * Formats accepted: `user@host`, `user@host:port`, `user@[ipv6]:port`.
 * Rejects command injection characters, leading hyphens, and invalid ports.
 *
 * @param target Null-terminated destination string.
 * @return Heap-allocated JSON string:
 *         `{"ok": true, "host": "...", "username": "...", "port": 22}`
 *         or `{"ok": false, "error": "..."}`.
 *         Must be deallocated using shellumi_string_free().
 */
char* shellumi_parse_destination_c(const char* target);

/**
 * @brief Generates an Ed25519 SSH keypair with zeroized memory on drop.
 *
 * @param comment Optional comment string for the public key (can be NULL).
 * @return Heap-allocated JSON string:
 *         `{"ok": true, "key": {"public_key": "...", "fingerprint": "...", "comment": "..."}}`
 *         or `{"ok": false, "error": "..."}`.
 *         Must be deallocated using shellumi_string_free().
 */
char* shellumi_generate_keypair_c(const char* comment);

/* -------------------------------------------------------------------------
 * Virtual Accessory Bar & Terminal Keystroke Translation
 * ------------------------------------------------------------------------- */

/**
 * @brief Translates an accessory key name and modifier bitmask to ANSI bytes.
 *
 * Supported key names: "esc", "tab", "arrow_up", "arrow_down", "arrow_left",
 * "arrow_right", "pipe", "tilde", "slash", "backslash", "home", "end",
 * "pageup", "pagedown", "insert", "delete", "f1".."f12", or individual characters.
 *
 * @param key_name Null-terminated key identifier string.
 * @param modifier_flags Bitmask of active modifiers (SHELLUMI_MODIFIER_*).
 * @return Heap-allocated JSON string:
 *         `{"ok": true, "key": "...", "bytes": [27, 91, 65], "hex": "1B 5B 41"}`
 *         or `{"ok": false, "error": "..."}`.
 *         Must be deallocated using shellumi_string_free().
 */
char* shellumi_translate_virtual_key_c(const char* key_name, uint32_t modifier_flags);

/* -------------------------------------------------------------------------
 * Mobile Lifecycle State Machine
 * ------------------------------------------------------------------------- */

/**
 * @brief Advances the mobile session lifecycle state machine.
 *
 * Handles iOS background grace periods (~30s limit), foreground resumption,
 * network connectivity events, and exponential backoff calculations.
 *
 * @param current_state_tag Current state ("connecting", "connected", "grace", "suspended", "reconnecting", "disconnected").
 * @param event_name Event trigger ("foreground", "background", "tick", "expired", "net_lost", "net_restored", "disconnect").
 * @param param Parameter integer (e.g. grace seconds or elapsed timer seconds).
 * @param epoch_seconds Current Unix timestamp in seconds.
 * @return Heap-allocated JSON string:
 *         `{"ok": true, "state": "...", "is_active": true/false, "backoff_or_grace": ...}`.
 *         Must be deallocated using shellumi_string_free().
 */
char* shellumi_lifecycle_transition_c(
    const char* current_state_tag,
    const char* event_name,
    uint64_t param,
    uint64_t epoch_seconds
);

/* -------------------------------------------------------------------------
 * Mobile Hardware Credential Specifications
 * ------------------------------------------------------------------------- */

/**
 * @brief Retrieves platform-specific hardware credential storage requirements.
 *
 * iOS: Keychain Services (`kSecClassGenericPassword`, `kSecAccessControlBiometryAny`).
 * Android: Android Keystore (`setUserAuthenticationRequired(true)`, StrongBox backing).
 *
 * @param platform Platform string ("ios" or "android").
 * @param service_name Keychain service identifier (e.g. "org.shellumi.app").
 * @param account_name Account / host key identifier.
 * @param require_biometric Whether hardware biometric authentication is required.
 * @return Heap-allocated JSON string detailing platform security parameters.
 *         Must be deallocated using shellumi_string_free().
 */
char* shellumi_get_credential_spec_c(
    const char* platform,
    const char* service_name,
    const char* account_name,
    bool require_biometric
);

/* -------------------------------------------------------------------------
 * Memory Management
 * ------------------------------------------------------------------------- */

/**
 * @brief Frees a string previously allocated by Shellumi C-ABI functions.
 *
 * @param ptr Pointer to string allocated by Shellumi.
 *            Passing NULL is a safe no-op.
 */
void shellumi_string_free(char* ptr);

#ifdef __cplusplus
}
#endif

#endif /* SHELLUMI_MOBILE_H */
