use crate::identity::generate_ed25519_key;
use crate::parse_quick_connect_target;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Target mobile operating system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobilePlatform {
    Ios,
    Android,
    Simulator,
}

impl MobilePlatform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ios => "iOS",
            Self::Android => "Android",
            Self::Simulator => "Simulator",
        }
    }
}

/// Modifiers that can alter virtual key behaviors on mobile touch keyboards.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VirtualKeyModifier {
    Ctrl,
    Alt,
    Shift,
}

/// A touch-friendly virtual key designed for mobile virtual keyboard accessory bars.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VirtualKey {
    Escape,
    Tab,
    ControlKey(char),
    AltKey(char),
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Pipe,
    Tilde,
    Slash,
    Backslash,
    Hyphen,
    Colon,
    Backtick,
    Function(u8),
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
}

impl VirtualKey {
    /// Human-readable label for rendering on mobile touch buttons.
    pub fn label(&self) -> String {
        match self {
            Self::Escape => "Esc".to_string(),
            Self::Tab => "Tab".to_string(),
            Self::ControlKey(c) => format!("^{}", c.to_ascii_uppercase()),
            Self::AltKey(c) => format!("⌥{}", c.to_ascii_uppercase()),
            Self::ArrowUp => "↑".to_string(),
            Self::ArrowDown => "↓".to_string(),
            Self::ArrowLeft => "←".to_string(),
            Self::ArrowRight => "→".to_string(),
            Self::Pipe => "|".to_string(),
            Self::Tilde => "~".to_string(),
            Self::Slash => "/".to_string(),
            Self::Backslash => "\\".to_string(),
            Self::Hyphen => "-".to_string(),
            Self::Colon => ":".to_string(),
            Self::Backtick => "`".to_string(),
            Self::Function(n) => format!("F{}", n),
            Self::Home => "Home".to_string(),
            Self::End => "End".to_string(),
            Self::PageUp => "PgUp".to_string(),
            Self::PageDown => "PgDn".to_string(),
            Self::Insert => "Ins".to_string(),
            Self::Delete => "Del".to_string(),
        }
    }

    /// Converts this virtual key into its standard VT/ANSI byte sequence.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Escape => vec![0x1B],
            Self::Tab => vec![0x09],
            Self::ControlKey(c) => {
                let ascii = c.to_ascii_lowercase() as u8;
                if ascii.is_ascii_lowercase() {
                    vec![ascii - b'a' + 1]
                } else if ascii == b' ' || ascii == b'@' {
                    vec![0x00]
                } else if ascii == b'[' {
                    vec![0x1B]
                } else if ascii == b'\\' {
                    vec![0x1C]
                } else if ascii == b']' {
                    vec![0x1D]
                } else {
                    vec![ascii]
                }
            }
            Self::AltKey(c) => {
                vec![0x1B, *c as u8]
            }
            Self::ArrowUp => b"\x1b[A".to_vec(),
            Self::ArrowDown => b"\x1b[B".to_vec(),
            Self::ArrowRight => b"\x1b[C".to_vec(),
            Self::ArrowLeft => b"\x1b[D".to_vec(),
            Self::Pipe => b"|".to_vec(),
            Self::Tilde => b"~".to_vec(),
            Self::Slash => b"/".to_vec(),
            Self::Backslash => b"\\".to_vec(),
            Self::Hyphen => b"-".to_vec(),
            Self::Colon => b":".to_vec(),
            Self::Backtick => b"`".to_vec(),
            Self::Function(n) => match n {
                1 => b"\x1bOP".to_vec(),
                2 => b"\x1bOQ".to_vec(),
                3 => b"\x1bOR".to_vec(),
                4 => b"\x1bOS".to_vec(),
                5 => b"\x1b[15~".to_vec(),
                6 => b"\x1b[17~".to_vec(),
                7 => b"\x1b[18~".to_vec(),
                8 => b"\x1b[19~".to_vec(),
                9 => b"\x1b[20~".to_vec(),
                10 => b"\x1b[21~".to_vec(),
                11 => b"\x1b[23~".to_vec(),
                12 => b"\x1b[24~".to_vec(),
                _ => Vec::new(),
            },
            Self::Home => b"\x1b[H".to_vec(),
            Self::End => b"\x1b[F".to_vec(),
            Self::PageUp => b"\x1b[5~".to_vec(),
            Self::PageDown => b"\x1b[6~".to_vec(),
            Self::Insert => b"\x1b[2~".to_vec(),
            Self::Delete => b"\x1b[3~".to_vec(),
        }
    }
}

/// Preset layouts for virtual accessory strips above the on-screen keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessoryBarLayout {
    DefaultTerminal,
    CompactNavigation,
    CodeEditing,
}

/// Virtual accessory bar state managing touch keys and sticky modifiers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VirtualAccessoryBar {
    pub layout: AccessoryBarLayout,
    pub keys: Vec<VirtualKey>,
    pub active_modifiers: HashSet<VirtualKeyModifier>,
}

impl VirtualAccessoryBar {
    /// Constructs a virtual accessory bar with the specified layout.
    pub fn new(layout: AccessoryBarLayout) -> Self {
        let keys = match layout {
            AccessoryBarLayout::DefaultTerminal => vec![
                VirtualKey::Escape,
                VirtualKey::Tab,
                VirtualKey::ControlKey('c'),
                VirtualKey::ControlKey('z'),
                VirtualKey::Pipe,
                VirtualKey::Tilde,
                VirtualKey::Slash,
                VirtualKey::ArrowUp,
                VirtualKey::ArrowDown,
                VirtualKey::ArrowLeft,
                VirtualKey::ArrowRight,
            ],
            AccessoryBarLayout::CompactNavigation => vec![
                VirtualKey::Escape,
                VirtualKey::Tab,
                VirtualKey::Home,
                VirtualKey::End,
                VirtualKey::PageUp,
                VirtualKey::PageDown,
                VirtualKey::ArrowUp,
                VirtualKey::ArrowDown,
                VirtualKey::ArrowLeft,
                VirtualKey::ArrowRight,
            ],
            AccessoryBarLayout::CodeEditing => vec![
                VirtualKey::Escape,
                VirtualKey::Tab,
                VirtualKey::Colon,
                VirtualKey::Pipe,
                VirtualKey::Tilde,
                VirtualKey::Backtick,
                VirtualKey::Slash,
                VirtualKey::Backslash,
                VirtualKey::ControlKey('a'),
                VirtualKey::ControlKey('e'),
            ],
        };

        Self {
            layout,
            keys,
            active_modifiers: HashSet::new(),
        }
    }

    /// Toggles a sticky modifier (Ctrl, Alt, or Shift).
    pub fn toggle_modifier(&mut self, modifier: VirtualKeyModifier) -> bool {
        if self.active_modifiers.contains(&modifier) {
            self.active_modifiers.remove(&modifier);
            false
        } else {
            self.active_modifiers.insert(modifier);
            true
        }
    }

    /// Checks if a modifier is currently active.
    pub fn has_modifier(&self, modifier: VirtualKeyModifier) -> bool {
        self.active_modifiers.contains(&modifier)
    }

    /// Clears any currently active modifiers.
    pub fn clear_modifiers(&mut self) {
        self.active_modifiers.clear();
    }

    /// Resolves an incoming character against active sticky modifiers.
    pub fn resolve_char_keystroke(&mut self, c: char) -> Vec<u8> {
        let is_ctrl = self.has_modifier(VirtualKeyModifier::Ctrl);
        let is_alt = self.has_modifier(VirtualKeyModifier::Alt);

        // One-shot modifier semantics: clear sticky flags once applied
        self.clear_modifiers();

        if is_ctrl && is_alt {
            let ctrl_byte = VirtualKey::ControlKey(c).to_bytes();
            let mut out = vec![0x1B];
            out.extend_from_slice(&ctrl_byte);
            out
        } else if is_ctrl {
            VirtualKey::ControlKey(c).to_bytes()
        } else if is_alt {
            VirtualKey::AltKey(c).to_bytes()
        } else {
            let mut b = [0u8; 4];
            c.encode_utf8(&mut b).as_bytes().to_vec()
        }
    }

    /// Dispatches a virtual touch key and returns its byte sequence.
    pub fn press_key(&mut self, key: &VirtualKey) -> Vec<u8> {
        self.clear_modifiers();
        key.to_bytes()
    }
}

impl Default for VirtualAccessoryBar {
    fn default() -> Self {
        Self::new(AccessoryBarLayout::DefaultTerminal)
    }
}

/// Mobile session connection and lifecycle states.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobileSessionState {
    Connecting,
    Connected,
    BackgroundGracePeriod { remaining_secs: u32 },
    Suspended { saved_at_epoch: u64 },
    Reconnecting { attempt: u32, backoff_secs: u32 },
    Disconnected { reason: String },
}

impl MobileSessionState {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Connected | Self::BackgroundGracePeriod { .. })
    }

    pub fn status_label(&self) -> &'static str {
        match self {
            Self::Connecting => "Connecting...",
            Self::Connected => "Connected",
            Self::BackgroundGracePeriod { .. } => "Background (Grace Period)",
            Self::Suspended { .. } => "Suspended",
            Self::Reconnecting { .. } => "Reconnecting...",
            Self::Disconnected { .. } => "Disconnected",
        }
    }
}

/// Lifecycle events delivered by the mobile operating system.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobileLifecycleEvent {
    AppForegrounded,
    AppBackgrounded { grace_seconds: u32 },
    BackgroundTimerTick { elapsed_seconds: u32 },
    BackgroundExpired,
    NetworkLost,
    NetworkRestored,
    UserDisconnect,
}

/// State machine managing mobile connection lifecycles across OS background events.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MobileLifecycleManager {
    pub platform: MobilePlatform,
    pub state: MobileSessionState,
    pub max_reconnect_attempts: u32,
    pub current_reconnect_attempt: u32,
}

impl MobileLifecycleManager {
    pub fn new(platform: MobilePlatform) -> Self {
        Self {
            platform,
            state: MobileSessionState::Connecting,
            max_reconnect_attempts: 5,
            current_reconnect_attempt: 0,
        }
    }

    pub fn mark_connected(&mut self) {
        self.state = MobileSessionState::Connected;
        self.current_reconnect_attempt = 0;
    }

    /// Calculates exponential backoff seconds with a 30-second ceiling.
    pub fn calculate_backoff(attempt: u32) -> u32 {
        let base = 2u32.saturating_pow(attempt.min(5));
        base.min(30)
    }

    /// Deterministically transitions session state in response to mobile OS events.
    pub fn transition(
        &mut self,
        event: MobileLifecycleEvent,
        current_epoch: u64,
    ) -> &MobileSessionState {
        match (&self.state, event) {
            (
                MobileSessionState::Connected,
                MobileLifecycleEvent::AppBackgrounded { grace_seconds },
            ) => {
                self.state = MobileSessionState::BackgroundGracePeriod {
                    remaining_secs: grace_seconds,
                };
            }
            (
                MobileSessionState::BackgroundGracePeriod { remaining_secs },
                MobileLifecycleEvent::BackgroundTimerTick { elapsed_seconds },
            ) => {
                if *remaining_secs <= elapsed_seconds {
                    self.state = MobileSessionState::Suspended {
                        saved_at_epoch: current_epoch,
                    };
                } else {
                    self.state = MobileSessionState::BackgroundGracePeriod {
                        remaining_secs: remaining_secs.saturating_sub(elapsed_seconds),
                    };
                }
            }
            (
                MobileSessionState::BackgroundGracePeriod { .. },
                MobileLifecycleEvent::BackgroundExpired,
            ) => {
                self.state = MobileSessionState::Suspended {
                    saved_at_epoch: current_epoch,
                };
            }
            (
                MobileSessionState::BackgroundGracePeriod { .. }
                | MobileSessionState::Suspended { .. },
                MobileLifecycleEvent::AppForegrounded,
            ) => {
                // When resuming to foreground, if suspended, initiate seamless reconnect
                if matches!(self.state, MobileSessionState::Suspended { .. }) {
                    self.current_reconnect_attempt = 1;
                    let backoff = Self::calculate_backoff(self.current_reconnect_attempt);
                    self.state = MobileSessionState::Reconnecting {
                        attempt: self.current_reconnect_attempt,
                        backoff_secs: backoff,
                    };
                } else {
                    self.state = MobileSessionState::Connected;
                }
            }
            (MobileSessionState::Connected, MobileLifecycleEvent::NetworkLost) => {
                self.current_reconnect_attempt = 1;
                let backoff = Self::calculate_backoff(self.current_reconnect_attempt);
                self.state = MobileSessionState::Reconnecting {
                    attempt: self.current_reconnect_attempt,
                    backoff_secs: backoff,
                };
            }
            (MobileSessionState::Reconnecting { .. }, MobileLifecycleEvent::NetworkRestored) => {
                self.state = MobileSessionState::Connected;
                self.current_reconnect_attempt = 0;
            }
            (
                MobileSessionState::Reconnecting { attempt, .. },
                MobileLifecycleEvent::NetworkLost,
            ) => {
                let next_attempt = attempt + 1;
                if next_attempt > self.max_reconnect_attempts {
                    self.state = MobileSessionState::Disconnected {
                        reason: "Exceeded maximum reconnection attempts".to_string(),
                    };
                } else {
                    self.current_reconnect_attempt = next_attempt;
                    let backoff = Self::calculate_backoff(next_attempt);
                    self.state = MobileSessionState::Reconnecting {
                        attempt: next_attempt,
                        backoff_secs: backoff,
                    };
                }
            }
            (_, MobileLifecycleEvent::UserDisconnect) => {
                self.state = MobileSessionState::Disconnected {
                    reason: "User requested disconnect".to_string(),
                };
            }
            _ => {}
        }

        &self.state
    }
}

/// Mobile hardware biometric unlock requirements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BiometricRequirement {
    None,
    BiometricAny,
    DevicePasscodeFallback,
}

/// Target platform credential storage backend on mobile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformKeyStoreTarget {
    IosKeychainSecClassGenericPassword,
    AndroidKeystoreAesGcm,
}

/// Specifications for hardware-backed mobile credential storage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileCredentialSpec {
    pub target: PlatformKeyStoreTarget,
    pub biometric: BiometricRequirement,
    pub service_name: String,
    pub account_name: String,
}

impl MobileCredentialSpec {
    pub fn new_ios(
        service_name: impl Into<String>,
        account_name: impl Into<String>,
        biometric: BiometricRequirement,
    ) -> Self {
        Self {
            target: PlatformKeyStoreTarget::IosKeychainSecClassGenericPassword,
            biometric,
            service_name: service_name.into(),
            account_name: account_name.into(),
        }
    }

    pub fn new_android(
        service_name: impl Into<String>,
        account_name: impl Into<String>,
        biometric: BiometricRequirement,
    ) -> Self {
        Self {
            target: PlatformKeyStoreTarget::AndroidKeystoreAesGcm,
            biometric,
            service_name: service_name.into(),
            account_name: account_name.into(),
        }
    }

    /// Formats the platform-native security flags required to query or store credentials.
    pub fn platform_flags_summary(&self) -> String {
        match self.target {
            PlatformKeyStoreTarget::IosKeychainSecClassGenericPassword => match self.biometric {
                BiometricRequirement::None => {
                    "kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly".to_string()
                }
                BiometricRequirement::BiometricAny => {
                    "kSecAccessControlBiometryAny | kSecAttrAccessibleWhenUnlockedThisDeviceOnly"
                        .to_string()
                }
                BiometricRequirement::DevicePasscodeFallback => {
                    "kSecAccessControlUserPresence".to_string()
                }
            },
            PlatformKeyStoreTarget::AndroidKeystoreAesGcm => match self.biometric {
                BiometricRequirement::None => {
                    "KeyProperties.PURPOSE_ENCRYPT | PURPOSE_DECRYPT".to_string()
                }
                BiometricRequirement::BiometricAny => {
                    "setUserAuthenticationRequired(true) | AUTH_BIOMETRIC_STRONG".to_string()
                }
                BiometricRequirement::DevicePasscodeFallback => {
                    "setUserAuthenticationRequired(true) | AUTH_DEVICE_CREDENTIAL".to_string()
                }
            },
        }
    }
}

/// Portable FFI serialization bridge for mobile frontends (Swift, Kotlin, or C-ABI).
pub struct MobileFfiBridge;

impl MobileFfiBridge {
    /// Validates an incoming destination string through core validation rules.
    pub fn parse_destination_json(target: &str) -> String {
        match parse_quick_connect_target(target) {
            Ok(dest) => serde_json::json!({
                "ok": true,
                "data": {
                    "host": dest.host,
                    "username": dest.username,
                    "port": dest.port,
                }
            })
            .to_string(),
            Err(e) => serde_json::json!({
                "ok": false,
                "error": e.to_string(),
            })
            .to_string(),
        }
    }

    /// Converts a virtual key name into its ANSI escape byte array.
    pub fn translate_virtual_key_json(key_name: &str) -> String {
        let key = match key_name.to_lowercase().as_str() {
            "esc" | "escape" => VirtualKey::Escape,
            "tab" => VirtualKey::Tab,
            "up" | "arrowup" => VirtualKey::ArrowUp,
            "down" | "arrowdown" => VirtualKey::ArrowDown,
            "left" | "arrowleft" => VirtualKey::ArrowLeft,
            "right" | "arrowright" => VirtualKey::ArrowRight,
            "pipe" | "|" => VirtualKey::Pipe,
            "tilde" | "~" => VirtualKey::Tilde,
            "slash" | "/" => VirtualKey::Slash,
            "backslash" | "\\" => VirtualKey::Backslash,
            "home" => VirtualKey::Home,
            "end" => VirtualKey::End,
            "pgup" | "pageup" => VirtualKey::PageUp,
            "pgdn" | "pagedown" => VirtualKey::PageDown,
            "ctrl-c" => VirtualKey::ControlKey('c'),
            "ctrl-z" => VirtualKey::ControlKey('z'),
            "ctrl-d" => VirtualKey::ControlKey('d'),
            "ctrl-a" => VirtualKey::ControlKey('a'),
            "ctrl-e" => VirtualKey::ControlKey('e'),
            _ => {
                return serde_json::json!({
                    "ok": false,
                    "error": format!("Unknown virtual key: {}", key_name)
                })
                .to_string();
            }
        };

        let bytes = key.to_bytes();
        serde_json::json!({
            "ok": true,
            "label": key.label(),
            "bytes": bytes,
            "hex": bytes.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ")
        })
        .to_string()
    }

    /// Generates a standard Ed25519 keypair for mobile client onboarding.
    pub fn generate_keypair_json(comment: &str) -> String {
        match generate_ed25519_key(Some(comment)) {
            Ok((_keypair, ssh_key)) => serde_json::json!({
                "ok": true,
                "data": {
                    "public_key": ssh_key.public_key,
                    "fingerprint": ssh_key.fingerprint,
                    "comment": comment
                }
            })
            .to_string(),
            Err(e) => serde_json::json!({
                "ok": false,
                "error": e.to_string()
            })
            .to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_virtual_key_byte_translations() {
        assert_eq!(VirtualKey::Escape.to_bytes(), vec![0x1B]);
        assert_eq!(VirtualKey::Tab.to_bytes(), vec![0x09]);
        assert_eq!(VirtualKey::ControlKey('c').to_bytes(), vec![0x03]);
        assert_eq!(VirtualKey::ControlKey('z').to_bytes(), vec![0x1A]);
        assert_eq!(VirtualKey::ControlKey('d').to_bytes(), vec![0x04]);
        assert_eq!(VirtualKey::ArrowUp.to_bytes(), b"\x1b[A");
        assert_eq!(VirtualKey::ArrowDown.to_bytes(), b"\x1b[B");
        assert_eq!(VirtualKey::ArrowRight.to_bytes(), b"\x1b[C");
        assert_eq!(VirtualKey::ArrowLeft.to_bytes(), b"\x1b[D");
        assert_eq!(VirtualKey::Home.to_bytes(), b"\x1b[H");
        assert_eq!(VirtualKey::End.to_bytes(), b"\x1b[F");
        assert_eq!(VirtualKey::PageUp.to_bytes(), b"\x1b[5~");
        assert_eq!(VirtualKey::PageDown.to_bytes(), b"\x1b[6~");
        assert_eq!(VirtualKey::Pipe.to_bytes(), b"|");
        assert_eq!(VirtualKey::Tilde.to_bytes(), b"~");
        assert_eq!(VirtualKey::Function(1).to_bytes(), b"\x1bOP");
        assert_eq!(VirtualKey::Function(5).to_bytes(), b"\x1b[15~");
    }

    #[test]
    fn verifies_virtual_accessory_bar_sticky_modifiers() {
        let mut bar = VirtualAccessoryBar::new(AccessoryBarLayout::DefaultTerminal);
        assert!(!bar.has_modifier(VirtualKeyModifier::Ctrl));

        // Toggle Ctrl on
        assert!(bar.toggle_modifier(VirtualKeyModifier::Ctrl));
        assert!(bar.has_modifier(VirtualKeyModifier::Ctrl));

        // Resolving a char should output Ctrl code and clear sticky modifier
        let bytes = bar.resolve_char_keystroke('x');
        assert_eq!(bytes, vec![0x18]); // Ctrl-X is 24 = 0x18
        assert!(!bar.has_modifier(VirtualKeyModifier::Ctrl));

        // Toggle Alt on
        assert!(bar.toggle_modifier(VirtualKeyModifier::Alt));
        let bytes = bar.resolve_char_keystroke('f');
        assert_eq!(bytes, vec![0x1B, b'f']); // Alt-F is Esc + 'f'
        assert!(!bar.has_modifier(VirtualKeyModifier::Alt));
    }

    #[test]
    fn verifies_mobile_lifecycle_transitions() {
        let mut manager = MobileLifecycleManager::new(MobilePlatform::Ios);
        assert_eq!(manager.state, MobileSessionState::Connecting);

        manager.mark_connected();
        assert_eq!(manager.state, MobileSessionState::Connected);
        assert!(manager.state.is_active());

        // App backgrounded with 30s grace
        manager.transition(
            MobileLifecycleEvent::AppBackgrounded { grace_seconds: 30 },
            1000,
        );
        assert_eq!(
            manager.state,
            MobileSessionState::BackgroundGracePeriod { remaining_secs: 30 }
        );
        assert!(manager.state.is_active());

        // Timer ticks 10s
        manager.transition(
            MobileLifecycleEvent::BackgroundTimerTick {
                elapsed_seconds: 10,
            },
            1010,
        );
        assert_eq!(
            manager.state,
            MobileSessionState::BackgroundGracePeriod { remaining_secs: 20 }
        );

        // Timer expires rest of grace
        manager.transition(
            MobileLifecycleEvent::BackgroundTimerTick {
                elapsed_seconds: 25,
            },
            1035,
        );
        assert_eq!(
            manager.state,
            MobileSessionState::Suspended {
                saved_at_epoch: 1035
            }
        );
        assert!(!manager.state.is_active());

        // Resume to foreground from suspended state initiates reconnect
        manager.transition(MobileLifecycleEvent::AppForegrounded, 1100);
        assert_eq!(
            manager.state,
            MobileSessionState::Reconnecting {
                attempt: 1,
                backoff_secs: 2
            }
        );

        // Network restored
        manager.transition(MobileLifecycleEvent::NetworkRestored, 1102);
        assert_eq!(manager.state, MobileSessionState::Connected);
    }

    #[test]
    fn verifies_mobile_credential_spec_flags() {
        let ios_spec = MobileCredentialSpec::new_ios(
            "org.shellumi.app",
            "user@host",
            BiometricRequirement::BiometricAny,
        );
        assert!(ios_spec
            .platform_flags_summary()
            .contains("kSecAccessControlBiometryAny"));

        let android_spec = MobileCredentialSpec::new_android(
            "org.shellumi.app",
            "user@host",
            BiometricRequirement::BiometricAny,
        );
        assert!(android_spec
            .platform_flags_summary()
            .contains("setUserAuthenticationRequired(true)"));
    }

    #[test]
    fn verifies_mobile_ffi_bridge_json_helpers() {
        // Destination validation
        let valid_json = MobileFfiBridge::parse_destination_json("alice@example.org:2222");
        assert!(valid_json.contains("\"ok\":true"));
        assert!(valid_json.contains("\"host\":\"example.org\""));
        assert!(valid_json.contains("\"port\":2222"));

        let invalid_json = MobileFfiBridge::parse_destination_json("invalid-no-at-symbol");
        assert!(invalid_json.contains("\"ok\":false"));

        // Virtual key translation
        let key_json = MobileFfiBridge::translate_virtual_key_json("ctrl-c");
        assert!(key_json.contains("\"ok\":true"));
        assert!(key_json.contains("\"bytes\":[3]"));
        assert!(key_json.contains("\"hex\":\"03\""));

        // Keypair generation
        let keypair_json = MobileFfiBridge::generate_keypair_json("mobile-test");
        assert!(keypair_json.contains("\"ok\":true"));
        assert!(keypair_json.contains("ssh-ed25519"));
        assert!(keypair_json.contains("SHA256:"));
    }
}
