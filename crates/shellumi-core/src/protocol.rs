use crate::SshDestination;
use serde::{Deserialize, Serialize};

/// High-level communication protocol supported by Shellumi.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionProtocol {
    #[default]
    Ssh,
    Mosh,
    Telnet,
    Serial,
    Local,
}

impl ConnectionProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Mosh => "mosh",
            Self::Telnet => "telnet",
            Self::Serial => "serial",
            Self::Local => "local",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "ssh" => Some(Self::Ssh),
            "mosh" => Some(Self::Mosh),
            "telnet" => Some(Self::Telnet),
            "serial" => Some(Self::Serial),
            "local" => Some(Self::Local),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Ssh => "SSH",
            Self::Mosh => "Mosh",
            Self::Telnet => "Telnet",
            Self::Serial => "Serial",
            Self::Local => "Local Terminal",
        }
    }

    pub fn badge_icon(&self) -> &'static str {
        match self {
            Self::Ssh => "🔒",
            Self::Mosh => "⚡",
            Self::Telnet => "⚠️",
            Self::Serial => "🔌",
            Self::Local => "💻",
        }
    }

    /// Whether data transmitted over this protocol is cryptographically encrypted.
    pub fn is_encrypted(&self) -> bool {
        match self {
            Self::Ssh | Self::Mosh | Self::Local => true,
            Self::Telnet | Self::Serial => false,
        }
    }

    /// Whether this protocol supports SFTP file transfer sessions.
    pub fn supports_sftp(&self) -> bool {
        matches!(self, Self::Ssh)
    }

    /// Whether this protocol supports SSH port forwarding tunnels (-L, -R, -D).
    pub fn supports_tunnels(&self) -> bool {
        matches!(self, Self::Ssh)
    }

    /// Whether this protocol supports jump host chaining (-J).
    pub fn supports_jump_host(&self) -> bool {
        matches!(self, Self::Ssh | Self::Mosh)
    }

    /// Whether this protocol supports automatic startup snippets.
    pub fn supports_startup_snippet(&self) -> bool {
        matches!(self, Self::Ssh | Self::Local | Self::Mosh)
    }

    /// Default TCP port associated with this protocol, if applicable.
    pub fn default_port(&self) -> Option<u16> {
        match self {
            Self::Ssh => Some(22),
            Self::Telnet => Some(23),
            Self::Mosh | Self::Serial | Self::Local => None,
        }
    }

    /// Whether this protocol requires an IP address or hostname destination.
    pub fn requires_network_destination(&self) -> bool {
        matches!(self, Self::Ssh | Self::Mosh | Self::Telnet)
    }
}

impl std::fmt::Display for ConnectionProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl std::str::FromStr for ConnectionProtocol {
    type Err = ProtocolError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| ProtocolError::InvalidProtocol(s.to_string()))
    }
}

/// Mosh predictive echo mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoshPredictMode {
    #[default]
    Adaptive,
    Always,
    Never,
    Experimental,
}

impl MoshPredictMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Adaptive => "adaptive",
            Self::Always => "always",
            Self::Never => "never",
            Self::Experimental => "experimental",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "adaptive" => Some(Self::Adaptive),
            "always" => Some(Self::Always),
            "never" => Some(Self::Never),
            "experimental" => Some(Self::Experimental),
            _ => None,
        }
    }
}

impl std::str::FromStr for MoshPredictMode {
    type Err = ProtocolError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| ProtocolError::InvalidPredictMode(s.to_string()))
    }
}

/// Configuration parameters for Mosh (Mobile Shell) connections.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoshConfig {
    pub udp_port_start: Option<u16>,
    pub udp_port_end: Option<u16>,
    pub server_binary: Option<String>,
    pub predict_mode: MoshPredictMode,
    pub ssh_port: Option<u16>,
    pub no_init: bool,
}

impl Default for MoshConfig {
    fn default() -> Self {
        Self {
            udp_port_start: Some(60000),
            udp_port_end: Some(61000),
            server_binary: None,
            predict_mode: MoshPredictMode::Adaptive,
            ssh_port: None,
            no_init: false,
        }
    }
}

impl MoshConfig {
    pub fn prerequisites_explanation() -> &'static str {
        "Mosh requires 'mosh-server' installed on the remote machine and inbound UDP ports (default 60000-61000) open in any intermediate firewalls."
    }

    /// Builds command-line arguments for invoking the system `mosh` client.
    pub fn build_arguments(
        &self,
        destination: &SshDestination,
        jump_host: Option<&str>,
    ) -> Vec<String> {
        let mut args = Vec::new();

        // Predictive echo mode
        args.push(format!("--predict={}", self.predict_mode.as_str()));

        // Custom server binary path
        if let Some(binary) = &self.server_binary {
            let trimmed = binary.trim();
            if !trimmed.is_empty() {
                args.push(format!("--server={trimmed}"));
            }
        }

        // UDP port selection
        if let Some(start) = self.udp_port_start {
            if let Some(end) = self.udp_port_end {
                if start == end {
                    args.push(format!("-p={start}"));
                } else {
                    args.push(format!("-p={start}:{end}"));
                }
            } else {
                args.push(format!("-p={start}"));
            }
        }

        // SSH sub-invocation flags (port, jump host)
        let mut ssh_opts = Vec::new();
        let port = self.ssh_port.unwrap_or(destination.port().get());
        if port != 22 {
            ssh_opts.push(format!("-p {port}"));
        }
        if let Some(jump) = jump_host {
            let trimmed = jump.trim();
            if !trimmed.is_empty() {
                ssh_opts.push(format!("-J {trimmed}"));
            }
        }
        if !ssh_opts.is_empty() {
            args.push(format!("--ssh=ssh {}", ssh_opts.join(" ")));
        }

        if self.no_init {
            args.push("--no-init".to_string());
        }

        // Target: [user@]host
        let target = format!("{}@{}", destination.username(), destination.host());
        args.push(target);

        args
    }
}

/// Configuration parameters for Telnet connections.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelnetConfig {
    pub host: String,
    pub port: u16,
    pub terminal_type: String,
}

impl Default for TelnetConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 23,
            terminal_type: "xterm-256color".to_string(),
        }
    }
}

impl TelnetConfig {
    pub fn new(host: impl Into<String>, port: u16) -> Result<Self, ProtocolError> {
        let host_str = host.into();
        let trimmed = host_str.trim().to_string();
        if trimmed.is_empty() {
            return Err(ProtocolError::EmptyHostname);
        }
        if port == 0 {
            return Err(ProtocolError::InvalidPort(0));
        }
        Ok(Self {
            host: trimmed,
            port,
            terminal_type: "xterm-256color".to_string(),
        })
    }

    pub fn security_warning() -> &'static str {
        "⚠️ UNENCRYPTED PROTOCOL: Telnet transmits all credentials, keystrokes, and session output in cleartext over the network. Passwords can be intercepted or stolen by network eavesdroppers."
    }

    pub fn build_arguments(&self) -> Vec<String> {
        vec![self.host.clone(), self.port.to_string()]
    }
}

/// RFC 854 / RFC 1091 / RFC 1073 Telnet Protocol constants and framing codec.
pub mod telnet_constants {
    pub const IAC: u8 = 255;
    pub const DONT: u8 = 254;
    pub const DO: u8 = 253;
    pub const WONT: u8 = 252;
    pub const WILL: u8 = 251;
    pub const SB: u8 = 250;
    pub const GA: u8 = 249;
    pub const EL: u8 = 248;
    pub const EC: u8 = 247;
    pub const AYT: u8 = 246;
    pub const AO: u8 = 245;
    pub const IP: u8 = 244;
    pub const BREAK: u8 = 243;
    pub const DM: u8 = 242;
    pub const NOP: u8 = 241;
    pub const SE: u8 = 240;

    // Telnet options
    pub const OPT_BINARY: u8 = 0;
    pub const OPT_ECHO: u8 = 1;
    pub const OPT_SUPPRESS_GO_AHEAD: u8 = 3;
    pub const OPT_STATUS: u8 = 5;
    pub const OPT_TIMING_MARK: u8 = 6;
    pub const OPT_TERMINAL_TYPE: u8 = 24;
    pub const OPT_NAWS: u8 = 31;
    pub const OPT_TERMINAL_SPEED: u8 = 32;
    pub const OPT_LINEMODE: u8 = 34;
    pub const OPT_NEW_ENVIRON: u8 = 39;
    pub const OPT_CHARSET: u8 = 42;

    // Subnegotiation qualifier codes
    pub const SUB_IS: u8 = 0;
    pub const SUB_SEND: u8 = 1;
}

/// Parsed Telnet protocol negotiation action frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TelnetCommandFrame {
    Do(u8),
    Dont(u8),
    Will(u8),
    Wont(u8),
    Subnegotiation { option: u8, data: Vec<u8> },
    Simple(u8),
}

/// Strips Telnet IAC commands from a raw byte stream and returns the clean terminal data
/// alongside parsed negotiation frames (RFC 854).
pub fn parse_telnet_stream(input: &[u8]) -> (Vec<u8>, Vec<TelnetCommandFrame>) {
    use telnet_constants::*;

    let mut clean_data = Vec::with_capacity(input.len());
    let mut frames = Vec::new();
    let mut i = 0;

    while i < input.len() {
        if input[i] == IAC {
            if i + 1 >= input.len() {
                break;
            }
            let cmd = input[i + 1];
            match cmd {
                IAC => {
                    // Escaped IAC byte (255, 255) -> literal 255 in terminal stream
                    clean_data.push(IAC);
                    i += 2;
                }
                DO => {
                    if i + 2 < input.len() {
                        frames.push(TelnetCommandFrame::Do(input[i + 2]));
                        i += 3;
                    } else {
                        break;
                    }
                }
                DONT => {
                    if i + 2 < input.len() {
                        frames.push(TelnetCommandFrame::Dont(input[i + 2]));
                        i += 3;
                    } else {
                        break;
                    }
                }
                WILL => {
                    if i + 2 < input.len() {
                        frames.push(TelnetCommandFrame::Will(input[i + 2]));
                        i += 3;
                    } else {
                        break;
                    }
                }
                WONT => {
                    if i + 2 < input.len() {
                        frames.push(TelnetCommandFrame::Wont(input[i + 2]));
                        i += 3;
                    } else {
                        break;
                    }
                }
                SB => {
                    // Subnegotiation: read until IAC SE
                    let start = i + 2;
                    let mut found_end = None;
                    let mut j = start;
                    while j + 1 < input.len() {
                        if input[j] == IAC && input[j + 1] == SE {
                            found_end = Some(j);
                            break;
                        }
                        j += 1;
                    }
                    if let Some(end_idx) = found_end {
                        if start < end_idx {
                            let opt = input[start];
                            let data = input[start + 1..end_idx].to_vec();
                            frames.push(TelnetCommandFrame::Subnegotiation { option: opt, data });
                        }
                        i = end_idx + 2;
                    } else {
                        // Incomplete subnegotiation buffer
                        break;
                    }
                }
                simple => {
                    frames.push(TelnetCommandFrame::Simple(simple));
                    i += 2;
                }
            }
        } else {
            clean_data.push(input[i]);
            i += 1;
        }
    }

    (clean_data, frames)
}

/// Builds an outgoing Telnet IAC DO/DONT/WILL/WONT command frame.
pub fn build_telnet_command(cmd: u8, option: u8) -> [u8; 3] {
    [telnet_constants::IAC, cmd, option]
}

/// Builds an RFC 1073 Negotiate About Window Size (NAWS) subnegotiation payload.
pub fn build_telnet_naws(cols: u16, rows: u16) -> Vec<u8> {
    use telnet_constants::*;
    let c_hi = (cols >> 8) as u8;
    let c_lo = (cols & 0xff) as u8;
    let r_hi = (rows >> 8) as u8;
    let r_lo = (rows & 0xff) as u8;

    let mut payload = vec![IAC, SB, OPT_NAWS];
    // Byte stuffing: if any coordinate byte is 255 (IAC), it must be escaped as IAC IAC
    for b in [c_hi, c_lo, r_hi, r_lo] {
        if b == IAC {
            payload.push(IAC);
        }
        payload.push(b);
    }
    payload.push(IAC);
    payload.push(SE);
    payload
}

/// Builds an RFC 1091 Terminal-Type subnegotiation response payload.
pub fn build_telnet_terminal_type_response(term: &str) -> Vec<u8> {
    use telnet_constants::*;
    let mut payload = vec![IAC, SB, OPT_TERMINAL_TYPE, SUB_IS];
    payload.extend_from_slice(term.as_bytes());
    payload.push(IAC);
    payload.push(SE);
    payload
}

/// Serial communication data bits configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SerialDataBits {
    Five,
    Six,
    Seven,
    #[default]
    Eight,
}

impl SerialDataBits {
    pub fn as_u8(&self) -> u8 {
        match self {
            Self::Five => 5,
            Self::Six => 6,
            Self::Seven => 7,
            Self::Eight => 8,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "5" => Some(Self::Five),
            "6" => Some(Self::Six),
            "7" => Some(Self::Seven),
            "8" => Some(Self::Eight),
            _ => None,
        }
    }
}

/// Serial communication parity configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SerialParity {
    #[default]
    None,
    Odd,
    Even,
    Mark,
    Space,
}

impl SerialParity {
    pub fn as_char(&self) -> char {
        match self {
            Self::None => 'N',
            Self::Odd => 'O',
            Self::Even => 'E',
            Self::Mark => 'M',
            Self::Space => 'S',
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_uppercase().as_str() {
            "N" | "NONE" => Some(Self::None),
            "O" | "ODD" => Some(Self::Odd),
            "E" | "EVEN" => Some(Self::Even),
            "M" | "MARK" => Some(Self::Mark),
            "S" | "SPACE" => Some(Self::Space),
            _ => None,
        }
    }
}

/// Serial communication stop bits configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SerialStopBits {
    #[default]
    One,
    Two,
}

impl SerialStopBits {
    pub fn as_u8(&self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "1" => Some(Self::One),
            "2" => Some(Self::Two),
            _ => None,
        }
    }
}

/// Serial communication flow control configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SerialFlowControl {
    #[default]
    None,
    Software,
    Hardware,
}

impl SerialFlowControl {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Software => "software",
            Self::Hardware => "hardware",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "none" | "off" => Some(Self::None),
            "software" | "xon/xoff" | "xon_xoff" => Some(Self::Software),
            "hardware" | "rts/cts" | "rts_cts" => Some(Self::Hardware),
            _ => None,
        }
    }
}

/// Configuration parameters for hardware serial / console port connections.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialConfig {
    pub port: String,
    pub baud_rate: u32,
    pub data_bits: SerialDataBits,
    pub parity: SerialParity,
    pub stop_bits: SerialStopBits,
    pub flow_control: SerialFlowControl,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            port: String::new(),
            baud_rate: 115200,
            data_bits: SerialDataBits::Eight,
            parity: SerialParity::None,
            stop_bits: SerialStopBits::One,
            flow_control: SerialFlowControl::None,
        }
    }
}

impl SerialConfig {
    pub const STANDARD_BAUD_RATES: &'static [u32] = &[
        300, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600,
    ];

    pub fn new(port: impl Into<String>, baud_rate: u32) -> Result<Self, ProtocolError> {
        let port_str = port.into();
        let trimmed = port_str.trim().to_string();
        if trimmed.is_empty() {
            return Err(ProtocolError::EmptyPortPath);
        }
        if !Self::is_valid_baud_rate(baud_rate) {
            return Err(ProtocolError::InvalidBaudRate(baud_rate));
        }
        Ok(Self {
            port: trimmed,
            baud_rate,
            data_bits: SerialDataBits::Eight,
            parity: SerialParity::None,
            stop_bits: SerialStopBits::One,
            flow_control: SerialFlowControl::None,
        })
    }

    pub fn is_valid_baud_rate(baud: u32) -> bool {
        baud > 0 && baud <= 4_000_000
    }

    /// Formats the serial mode string (e.g., "115200 8N1").
    pub fn mode_string(&self) -> String {
        format!(
            "{} {}{}{}",
            self.baud_rate,
            self.data_bits.as_u8(),
            self.parity.as_char(),
            self.stop_bits.as_u8()
        )
    }

    /// Comprehensive human-readable summary of port and settings.
    pub fn summary(&self) -> String {
        format!(
            "{} · {} (Flow: {})",
            self.port,
            self.mode_string(),
            self.flow_control.as_str()
        )
    }

    /// Builds command-line arguments for standard screen/cu serial dispatch.
    pub fn build_arguments(&self) -> Vec<String> {
        vec![self.port.clone(), self.baud_rate.to_string()]
    }

    /// Discovers candidate serial device ports on the local machine.
    pub fn discover_serial_ports() -> Vec<String> {
        let mut ports = Vec::new();

        #[cfg(unix)]
        {
            if let Ok(entries) = std::fs::read_dir("/dev") {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        // macOS USB/serial ports: cu.usbserial*, cu.usbmodem*, tty.usb*
                        // Linux USB/serial ports: ttyUSB*, ttyACM*, ttyS*
                        if name.starts_with("cu.usb")
                            || name.starts_with("tty.usb")
                            || name.starts_with("ttyUSB")
                            || name.starts_with("ttyACM")
                            || name.starts_with("cu.SLAB")
                            || name.starts_with("cu.wchusb")
                        {
                            ports.push(path.to_string_lossy().into_owned());
                        }
                    }
                }
            }
        }

        #[cfg(windows)]
        {
            for i in 1..=32 {
                ports.push(format!("COM{i}"));
            }
        }

        ports.sort();
        ports
    }
}

/// Unified endpoint representing any protocol connection target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolEndpoint {
    Ssh(SshDestination),
    Mosh {
        destination: SshDestination,
        config: MoshConfig,
    },
    Telnet(TelnetConfig),
    Serial(SerialConfig),
    Local,
}

impl ProtocolEndpoint {
    pub fn protocol(&self) -> ConnectionProtocol {
        match self {
            Self::Ssh(_) => ConnectionProtocol::Ssh,
            Self::Mosh { .. } => ConnectionProtocol::Mosh,
            Self::Telnet(_) => ConnectionProtocol::Telnet,
            Self::Serial(_) => ConnectionProtocol::Serial,
            Self::Local => ConnectionProtocol::Local,
        }
    }

    pub fn format_target(&self) -> String {
        match self {
            Self::Ssh(dest) => format!("ssh://{}@{}:{}", dest.username(), dest.host(), dest.port()),
            Self::Mosh { destination, .. } => {
                format!("mosh://{}@{}", destination.username(), destination.host())
            }
            Self::Telnet(t) => format!("telnet://{}:{}", t.host, t.port),
            Self::Serial(s) => format!("serial://{}", s.summary()),
            Self::Local => "local://shell".to_string(),
        }
    }

    pub fn security_warning(&self) -> Option<&'static str> {
        match self {
            Self::Telnet(_) => Some(TelnetConfig::security_warning()),
            Self::Serial(_) => Some(
                "ℹ️ Serial connection: Physical line communication without network encryption.",
            ),
            _ => None,
        }
    }

    pub fn supports_sftp(&self) -> bool {
        self.protocol().supports_sftp()
    }

    pub fn supports_tunnels(&self) -> bool {
        self.protocol().supports_tunnels()
    }
}

/// Errors related to protocol specification, configuration, or codecs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidProtocol(String),
    InvalidPredictMode(String),
    InvalidBaudRate(u32),
    EmptyPortPath,
    EmptyHostname,
    InvalidPort(u16),
    UnsupportedFeature {
        protocol: ConnectionProtocol,
        feature: &'static str,
    },
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProtocol(p) => write!(f, "Invalid connection protocol: '{p}'"),
            Self::InvalidPredictMode(m) => write!(f, "Invalid mosh predict mode: '{m}'"),
            Self::InvalidBaudRate(b) => write!(f, "Invalid serial baud rate: {b}"),
            Self::EmptyPortPath => write!(f, "Serial device port path cannot be empty"),
            Self::EmptyHostname => write!(f, "Hostname cannot be empty"),
            Self::InvalidPort(p) => write!(f, "Invalid port number: {p}"),
            Self::UnsupportedFeature { protocol, feature } => {
                write!(f, "{protocol} does not support {feature}")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_classifies_protocols_correctly() {
        assert_eq!(
            ConnectionProtocol::parse("ssh"),
            Some(ConnectionProtocol::Ssh)
        );
        assert_eq!(
            ConnectionProtocol::parse("MOSH"),
            Some(ConnectionProtocol::Mosh)
        );
        assert_eq!(
            ConnectionProtocol::parse("Telnet"),
            Some(ConnectionProtocol::Telnet)
        );
        assert_eq!(
            ConnectionProtocol::parse("serial"),
            Some(ConnectionProtocol::Serial)
        );
        assert_eq!(
            ConnectionProtocol::parse("local"),
            Some(ConnectionProtocol::Local)
        );
        assert_eq!(ConnectionProtocol::parse("unknown"), None);

        // Security & features
        assert!(ConnectionProtocol::Ssh.is_encrypted());
        assert!(ConnectionProtocol::Mosh.is_encrypted());
        assert!(!ConnectionProtocol::Telnet.is_encrypted());
        assert!(!ConnectionProtocol::Serial.is_encrypted());

        assert!(ConnectionProtocol::Ssh.supports_sftp());
        assert!(!ConnectionProtocol::Mosh.supports_sftp());
        assert!(!ConnectionProtocol::Telnet.supports_sftp());
        assert!(!ConnectionProtocol::Serial.supports_sftp());

        assert!(ConnectionProtocol::Ssh.supports_tunnels());
        assert!(!ConnectionProtocol::Telnet.supports_tunnels());

        assert_eq!(ConnectionProtocol::Ssh.default_port(), Some(22));
        assert_eq!(ConnectionProtocol::Telnet.default_port(), Some(23));
        assert_eq!(ConnectionProtocol::Serial.default_port(), None);
    }

    #[test]
    fn builds_mosh_arguments_properly() {
        let dest = SshDestination::new("10.0.0.5", "admin", 2222).unwrap();
        let config = MoshConfig {
            udp_port_start: Some(60010),
            udp_port_end: Some(60010),
            server_binary: Some("/usr/local/bin/mosh-server".into()),
            predict_mode: MoshPredictMode::Always,
            ssh_port: Some(2222),
            no_init: false,
        };

        let args = config.build_arguments(&dest, Some("bastion.corp"));
        assert!(args.contains(&"--predict=always".to_string()));
        assert!(args.contains(&"--server=/usr/local/bin/mosh-server".to_string()));
        assert!(args.contains(&"-p=60010".to_string()));
        assert!(args.iter().any(|a| a.starts_with("--ssh=ssh")
            && a.contains("-p 2222")
            && a.contains("-J bastion.corp")));
        assert_eq!(args.last().unwrap(), "admin@10.0.0.5");
    }

    #[test]
    fn verifies_telnet_config_and_security_warning() {
        let telnet = TelnetConfig::new("switch-01.corp", 23).unwrap();
        assert_eq!(telnet.host, "switch-01.corp");
        assert_eq!(telnet.port, 23);
        assert_eq!(telnet.build_arguments(), vec!["switch-01.corp", "23"]);
        assert!(TelnetConfig::security_warning().contains("UNENCRYPTED PROTOCOL"));

        assert_eq!(TelnetConfig::new("", 23), Err(ProtocolError::EmptyHostname));
        assert_eq!(
            TelnetConfig::new("switch", 0),
            Err(ProtocolError::InvalidPort(0))
        );
    }

    #[test]
    fn parses_telnet_iac_stream_and_escapes() {
        use telnet_constants::*;

        // Raw stream: "Hello", IAC DO OPT_ECHO, " World", IAC IAC (escaped 255), "!"
        let mut raw = Vec::new();
        raw.extend_from_slice(b"Hello");
        raw.extend_from_slice(&[IAC, DO, OPT_ECHO]);
        raw.extend_from_slice(b" World");
        raw.extend_from_slice(&[IAC, IAC]);
        raw.extend_from_slice(b"!");

        let (clean, frames) = parse_telnet_stream(&raw);
        assert_eq!(clean, b"Hello World\xff!");
        assert_eq!(frames, vec![TelnetCommandFrame::Do(OPT_ECHO)]);

        // Subnegotiation: IAC SB OPT_NAWS 0 80 0 24 IAC SE
        let mut naws_raw = vec![IAC, SB, OPT_NAWS, 0, 80, 0, 24, IAC, SE];
        naws_raw.extend_from_slice(b"Logged in.\n");

        let (clean2, frames2) = parse_telnet_stream(&naws_raw);
        assert_eq!(clean2, b"Logged in.\n");
        assert_eq!(
            frames2,
            vec![TelnetCommandFrame::Subnegotiation {
                option: OPT_NAWS,
                data: vec![0, 80, 0, 24]
            }]
        );

        // Codec builders
        let naws_bytes = build_telnet_naws(80, 24);
        assert_eq!(naws_bytes, vec![IAC, SB, OPT_NAWS, 0, 80, 0, 24, IAC, SE]);

        let term_bytes = build_telnet_terminal_type_response("xterm-256color");
        assert_eq!(
            term_bytes,
            vec![
                IAC,
                SB,
                OPT_TERMINAL_TYPE,
                SUB_IS,
                b'x',
                b't',
                b'e',
                b'r',
                b'm',
                b'-',
                b'2',
                b'5',
                b'6',
                b'c',
                b'o',
                b'l',
                b'o',
                b'r',
                IAC,
                SE
            ]
        );
    }

    #[test]
    fn verifies_serial_config_validation_and_formatting() {
        let serial = SerialConfig::new("/dev/ttyUSB0", 115200).unwrap();
        assert_eq!(serial.port, "/dev/ttyUSB0");
        assert_eq!(serial.baud_rate, 115200);
        assert_eq!(serial.mode_string(), "115200 8N1");
        assert_eq!(serial.summary(), "/dev/ttyUSB0 · 115200 8N1 (Flow: none)");
        assert_eq!(
            serial.build_arguments(),
            vec!["/dev/ttyUSB0".to_string(), "115200".to_string()]
        );

        assert_eq!(
            SerialConfig::new("", 115200),
            Err(ProtocolError::EmptyPortPath)
        );
        assert_eq!(
            SerialConfig::new("/dev/ttyS0", 0),
            Err(ProtocolError::InvalidBaudRate(0))
        );

        // Serial data bits & parity parsing
        assert_eq!(SerialDataBits::parse("8"), Some(SerialDataBits::Eight));
        assert_eq!(SerialParity::parse("E"), Some(SerialParity::Even));
        assert_eq!(SerialStopBits::parse("1"), Some(SerialStopBits::One));
        assert_eq!(
            SerialFlowControl::parse("hardware"),
            Some(SerialFlowControl::Hardware)
        );
    }

    #[test]
    fn verifies_protocol_endpoint_unification() {
        let dest = SshDestination::new("router.internal", "cisco", 22).unwrap();
        let ep_ssh = ProtocolEndpoint::Ssh(dest.clone());
        assert_eq!(ep_ssh.protocol(), ConnectionProtocol::Ssh);
        assert_eq!(ep_ssh.format_target(), "ssh://cisco@router.internal:22");
        assert_eq!(ep_ssh.security_warning(), None);
        assert!(ep_ssh.supports_sftp());
        assert!(ep_ssh.supports_tunnels());

        let telnet_cfg = TelnetConfig::new("router.internal", 23).unwrap();
        let ep_telnet = ProtocolEndpoint::Telnet(telnet_cfg);
        assert_eq!(ep_telnet.protocol(), ConnectionProtocol::Telnet);
        assert_eq!(ep_telnet.format_target(), "telnet://router.internal:23");
        assert!(ep_telnet.security_warning().is_some());
        assert!(!ep_telnet.supports_sftp());
        assert!(!ep_telnet.supports_tunnels());

        let serial_cfg = SerialConfig::new("/dev/cu.usbserial-10", 9600).unwrap();
        let ep_serial = ProtocolEndpoint::Serial(serial_cfg);
        assert_eq!(ep_serial.protocol(), ConnectionProtocol::Serial);
        assert!(ep_serial.security_warning().is_some());
    }
}
