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
    Vnc,
    Rdp,
}

impl ConnectionProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Mosh => "mosh",
            Self::Telnet => "telnet",
            Self::Serial => "serial",
            Self::Local => "local",
            Self::Vnc => "vnc",
            Self::Rdp => "rdp",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "ssh" => Some(Self::Ssh),
            "mosh" => Some(Self::Mosh),
            "telnet" => Some(Self::Telnet),
            "serial" => Some(Self::Serial),
            "local" => Some(Self::Local),
            "vnc" | "rfb" => Some(Self::Vnc),
            "rdp" => Some(Self::Rdp),
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
            Self::Vnc => "VNC",
            Self::Rdp => "RDP",
        }
    }

    pub fn badge_icon(&self) -> &'static str {
        match self {
            Self::Ssh => "🔒",
            Self::Mosh => "⚡",
            Self::Telnet => "⚠️",
            Self::Serial => "🔌",
            Self::Local => "💻",
            Self::Vnc => "🖥️",
            Self::Rdp => "🪟",
        }
    }

    /// Whether data transmitted over this protocol is cryptographically encrypted.
    pub fn is_encrypted(&self) -> bool {
        match self {
            Self::Ssh | Self::Mosh | Self::Local | Self::Rdp => true,
            Self::Telnet | Self::Serial | Self::Vnc => false,
        }
    }

    /// Whether this protocol represents a graphical remote desktop session (VNC/RDP).
    pub fn is_graphical(&self) -> bool {
        matches!(self, Self::Vnc | Self::Rdp)
    }

    /// Whether this protocol supports remote desktop viewports.
    pub fn supports_remote_desktop(&self) -> bool {
        self.is_graphical()
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
            Self::Vnc => Some(5900),
            Self::Rdp => Some(3389),
            Self::Mosh | Self::Serial | Self::Local => None,
        }
    }

    /// Whether this protocol requires an IP address or hostname destination.
    pub fn requires_network_destination(&self) -> bool {
        matches!(
            self,
            Self::Ssh | Self::Mosh | Self::Telnet | Self::Vnc | Self::Rdp
        )
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

/// Configuration parameters for VNC (Virtual Network Computing / RFB) connections.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VncConfig {
    pub host: String,
    pub port: u16,
    pub password_secret_ref: Option<String>,
    pub view_only: bool,
    pub shared: bool,
    pub color_depth: u8,
    pub quality: u8,
    pub enable_clipboard: bool,
}

impl Default for VncConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 5900,
            password_secret_ref: None,
            view_only: false,
            shared: true,
            color_depth: 24,
            quality: 8,
            enable_clipboard: true,
        }
    }
}

impl VncConfig {
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
            password_secret_ref: None,
            view_only: false,
            shared: true,
            color_depth: 24,
            quality: 8,
            enable_clipboard: true,
        })
    }

    pub fn summary(&self) -> String {
        format!(
            "{}:{} (VNC · {}bpp{})",
            self.host,
            self.port,
            self.color_depth,
            if self.view_only { " · view-only" } else { "" }
        )
    }
}

/// Configuration parameters for RDP (Remote Desktop Protocol) connections.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RdpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub domain: Option<String>,
    pub password_secret_ref: Option<String>,
    pub screen_width: u16,
    pub screen_height: u16,
    pub color_depth: u8,
    pub enable_audio: bool,
    pub enable_clipboard: bool,
    pub enable_font_smoothing: bool,
}

impl Default for RdpConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 3389,
            username: None,
            domain: None,
            password_secret_ref: None,
            screen_width: 1920,
            screen_height: 1080,
            color_depth: 32,
            enable_audio: false,
            enable_clipboard: true,
            enable_font_smoothing: true,
        }
    }
}

impl RdpConfig {
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
            username: None,
            domain: None,
            password_secret_ref: None,
            screen_width: 1920,
            screen_height: 1080,
            color_depth: 32,
            enable_audio: false,
            enable_clipboard: true,
            enable_font_smoothing: true,
        })
    }

    pub fn summary(&self) -> String {
        let user_part = self.username.as_deref().unwrap_or("default");
        format!(
            "{}:{} (RDP · {}x{} · user: {})",
            self.host, self.port, self.screen_width, self.screen_height, user_part
        )
    }

    /// Builds command-line arguments compatible with standard xfreerdp / wlfreerdp.
    pub fn build_arguments(&self) -> Vec<String> {
        let mut args = Vec::new();
        args.push(format!("/v:{}:{}", self.host, self.port));
        if let Some(user) = &self.username {
            let trimmed = user.trim();
            if !trimmed.is_empty() {
                args.push(format!("/u:{trimmed}"));
            }
        }
        if let Some(domain) = &self.domain {
            let trimmed = domain.trim();
            if !trimmed.is_empty() {
                args.push(format!("/d:{trimmed}"));
            }
        }
        args.push(format!(
            "/size:{}x{}",
            self.screen_width, self.screen_height
        ));
        args.push(format!("/bpp:{}", self.color_depth));
        if self.enable_clipboard {
            args.push("+clipboard".to_string());
        }
        if self.enable_font_smoothing {
            args.push("/fonts".to_string());
        }
        if self.enable_audio {
            args.push("/sound".to_string());
        }
        args
    }
}

/// RFC 6143 The Remote Framebuffer Protocol (RFB / VNC) constants and framing codecs.
pub mod rfb_constants {
    pub const RFB_VERSION_3_8: &[u8; 12] = b"RFB 003.008\n";
    pub const RFB_VERSION_3_7: &[u8; 12] = b"RFB 003.007\n";
    pub const RFB_VERSION_3_3: &[u8; 12] = b"RFB 003.003\n";

    // Security Types
    pub const SEC_INVALID: u8 = 0;
    pub const SEC_NONE: u8 = 1;
    pub const SEC_VNC_AUTH: u8 = 2;
    pub const SEC_RA2: u8 = 5;
    pub const SEC_RA2NE: u8 = 6;
    pub const SEC_TIGHT: u8 = 16;
    pub const SEC_TLS: u8 = 18;
    pub const SEC_VENCRYPT: u8 = 19;

    // Client to Server Message Types
    pub const MSG_SET_PIXEL_FORMAT: u8 = 0;
    pub const MSG_SET_ENCODINGS: u8 = 2;
    pub const MSG_FRAMEBUFFER_UPDATE_REQ: u8 = 3;
    pub const MSG_KEY_EVENT: u8 = 4;
    pub const MSG_POINTER_EVENT: u8 = 5;
    pub const MSG_CLIENT_CUT_TEXT: u8 = 6;

    // Server to Client Message Types
    pub const MSG_FRAMEBUFFER_UPDATE: u8 = 0;
    pub const MSG_SET_COLOUR_MAP_ENTRIES: u8 = 1;
    pub const MSG_BELL: u8 = 2;
    pub const MSG_SERVER_CUT_TEXT: u8 = 3;

    // Encodings
    pub const ENCODING_RAW: i32 = 0;
    pub const ENCODING_COPYRECT: i32 = 1;
    pub const ENCODING_RRE: i32 = 2;
    pub const ENCODING_HEXTILE: i32 = 5;
    pub const ENCODING_ZRLE: i32 = 16;
    pub const ENCODING_DESKTOP_SIZE: i32 = -223;
    pub const ENCODING_LAST_RECT: i32 = -224;
    pub const ENCODING_CURSOR: i32 = -239;

    // Pointer Event Button Masks
    pub const BUTTON_LEFT: u8 = 0b00000001;
    pub const BUTTON_MIDDLE: u8 = 0b00000010;
    pub const BUTTON_RIGHT: u8 = 0b00000100;
    pub const BUTTON_WHEEL_UP: u8 = 0b00001000;
    pub const BUTTON_WHEEL_DOWN: u8 = 0b00010000;

    // Standard Keysyms for RFB KeyEvent
    pub const KEYSYM_BACKSPACE: u32 = 0xFF08;
    pub const KEYSYM_TAB: u32 = 0xFF09;
    pub const KEYSYM_RETURN: u32 = 0xFF0D;
    pub const KEYSYM_ESCAPE: u32 = 0xFF1B;
    pub const KEYSYM_DELETE: u32 = 0xFFFF;
    pub const KEYSYM_HOME: u32 = 0xFF50;
    pub const KEYSYM_LEFT: u32 = 0xFF51;
    pub const KEYSYM_UP: u32 = 0xFF52;
    pub const KEYSYM_RIGHT: u32 = 0xFF53;
    pub const KEYSYM_DOWN: u32 = 0xFF54;
    pub const KEYSYM_PAGE_UP: u32 = 0xFF55;
    pub const KEYSYM_PAGE_DOWN: u32 = 0xFF56;
    pub const KEYSYM_END: u32 = 0xFF57;
    pub const KEYSYM_CONTROL_L: u32 = 0xFFE3;
    pub const KEYSYM_CONTROL_R: u32 = 0xFFE4;
    pub const KEYSYM_ALT_L: u32 = 0xFFE9;
    pub const KEYSYM_ALT_R: u32 = 0xFFEA;
    pub const KEYSYM_SUPER_L: u32 = 0xFFEB;
    pub const KEYSYM_SUPER_R: u32 = 0xFFEC;
}

/// RFB Pixel Format specification (RFC 6143 Section 7.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RfbPixelFormat {
    pub bits_per_pixel: u8,
    pub depth: u8,
    pub big_endian_flag: u8,
    pub true_colour_flag: u8,
    pub red_max: u16,
    pub green_max: u16,
    pub blue_max: u16,
    pub red_shift: u8,
    pub green_shift: u8,
    pub blue_shift: u8,
}

impl Default for RfbPixelFormat {
    fn default() -> Self {
        Self::default_32bpp_rgba()
    }
}

impl RfbPixelFormat {
    pub fn default_32bpp_rgba() -> Self {
        Self {
            bits_per_pixel: 32,
            depth: 24,
            big_endian_flag: 0,
            true_colour_flag: 1,
            red_max: 255,
            green_max: 255,
            blue_max: 255,
            red_shift: 0,
            green_shift: 8,
            blue_shift: 16,
        }
    }

    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0] = self.bits_per_pixel;
        buf[1] = self.depth;
        buf[2] = self.big_endian_flag;
        buf[3] = self.true_colour_flag;
        buf[4..6].copy_from_slice(&self.red_max.to_be_bytes());
        buf[6..8].copy_from_slice(&self.green_max.to_be_bytes());
        buf[8..10].copy_from_slice(&self.blue_max.to_be_bytes());
        buf[10] = self.red_shift;
        buf[11] = self.green_shift;
        buf[12] = self.blue_shift;
        buf
    }

    pub fn from_bytes(buf: &[u8; 16]) -> Self {
        Self {
            bits_per_pixel: buf[0],
            depth: buf[1],
            big_endian_flag: buf[2],
            true_colour_flag: buf[3],
            red_max: u16::from_be_bytes([buf[4], buf[5]]),
            green_max: u16::from_be_bytes([buf[6], buf[7]]),
            blue_max: u16::from_be_bytes([buf[8], buf[9]]),
            red_shift: buf[10],
            green_shift: buf[11],
            blue_shift: buf[12],
        }
    }
}

/// RFB ServerInit message payload (RFC 6143 Section 7.3.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RfbServerInit {
    pub framebuffer_width: u16,
    pub framebuffer_height: u16,
    pub pixel_format: RfbPixelFormat,
    pub name: String,
}

/// RFB Rectangle header in FramebufferUpdate message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RfbRectangleHeader {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub encoding_type: i32,
}

/// Parses the server protocol version string (e.g. "RFB 003.008\n").
pub fn parse_rfb_version(buf: &[u8]) -> Result<(u32, u32), ProtocolError> {
    if buf.len() < 12 {
        return Err(ProtocolError::IncompleteRfbMessage);
    }
    if !buf.starts_with(b"RFB ") || buf[11] != b'\n' || buf[7] != b'.' {
        let preview = String::from_utf8_lossy(&buf[..buf.len().min(12)]).to_string();
        return Err(ProtocolError::InvalidRfbVersion(preview));
    }
    let major_str = std::str::from_utf8(&buf[4..7])
        .map_err(|_| ProtocolError::InvalidRfbVersion("bad utf8".into()))?;
    let minor_str = std::str::from_utf8(&buf[8..11])
        .map_err(|_| ProtocolError::InvalidRfbVersion("bad utf8".into()))?;
    let major = major_str
        .parse::<u32>()
        .map_err(|_| ProtocolError::InvalidRfbVersion(major_str.into()))?;
    let minor = minor_str
        .parse::<u32>()
        .map_err(|_| ProtocolError::InvalidRfbVersion(minor_str.into()))?;
    Ok((major, minor))
}

/// Builds client protocol version handshake (RFB 003.008).
pub fn build_rfb_version() -> Vec<u8> {
    rfb_constants::RFB_VERSION_3_8.to_vec()
}

/// Builds ClientInit message (RFC 6143 Section 7.3.1).
pub fn build_rfb_client_init(shared: bool) -> [u8; 1] {
    [if shared { 1 } else { 0 }]
}

/// Parses ServerInit message from raw bytes (RFC 6143 Section 7.3.2).
pub fn parse_rfb_server_init(buf: &[u8]) -> Result<(RfbServerInit, usize), ProtocolError> {
    if buf.len() < 24 {
        return Err(ProtocolError::IncompleteRfbMessage);
    }
    let width = u16::from_be_bytes([buf[0], buf[1]]);
    let height = u16::from_be_bytes([buf[2], buf[3]]);
    let mut pf_bytes = [0u8; 16];
    pf_bytes.copy_from_slice(&buf[4..20]);
    let pixel_format = RfbPixelFormat::from_bytes(&pf_bytes);
    let name_len = u32::from_be_bytes([buf[20], buf[21], buf[22], buf[23]]) as usize;
    if buf.len() < 24 + name_len {
        return Err(ProtocolError::IncompleteRfbMessage);
    }
    let name = String::from_utf8_lossy(&buf[24..24 + name_len]).into_owned();
    Ok((
        RfbServerInit {
            framebuffer_width: width,
            framebuffer_height: height,
            pixel_format,
            name,
        },
        24 + name_len,
    ))
}

/// Builds SetPixelFormat client-to-server message (RFC 6143 Section 7.5.1).
pub fn build_rfb_set_pixel_format(format: &RfbPixelFormat) -> [u8; 20] {
    let mut msg = [0u8; 20];
    msg[0] = rfb_constants::MSG_SET_PIXEL_FORMAT;
    msg[4..20].copy_from_slice(&format.to_bytes());
    msg
}

/// Builds SetEncodings client-to-server message (RFC 6143 Section 7.5.2).
pub fn build_rfb_set_encodings(encodings: &[i32]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(4 + encodings.len() * 4);
    msg.push(rfb_constants::MSG_SET_ENCODINGS);
    msg.push(0);
    msg.extend_from_slice(&(encodings.len() as u16).to_be_bytes());
    for enc in encodings {
        msg.extend_from_slice(&enc.to_be_bytes());
    }
    msg
}

/// Builds FramebufferUpdateRequest client-to-server message (RFC 6143 Section 7.5.3).
pub fn build_rfb_framebuffer_update_request(
    incremental: bool,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> [u8; 10] {
    let mut msg = [0u8; 10];
    msg[0] = rfb_constants::MSG_FRAMEBUFFER_UPDATE_REQ;
    msg[1] = if incremental { 1 } else { 0 };
    msg[2..4].copy_from_slice(&x.to_be_bytes());
    msg[4..6].copy_from_slice(&y.to_be_bytes());
    msg[6..8].copy_from_slice(&width.to_be_bytes());
    msg[8..10].copy_from_slice(&height.to_be_bytes());
    msg
}

/// Builds KeyEvent client-to-server message (RFC 6143 Section 7.5.4).
pub fn build_rfb_key_event(down: bool, key: u32) -> [u8; 8] {
    let mut msg = [0u8; 8];
    msg[0] = rfb_constants::MSG_KEY_EVENT;
    msg[1] = if down { 1 } else { 0 };
    msg[4..8].copy_from_slice(&key.to_be_bytes());
    msg
}

/// Builds PointerEvent client-to-server message (RFC 6143 Section 7.5.5).
pub fn build_rfb_pointer_event(button_mask: u8, x: u16, y: u16) -> [u8; 6] {
    let mut msg = [0u8; 6];
    msg[0] = rfb_constants::MSG_POINTER_EVENT;
    msg[1] = button_mask;
    msg[2..4].copy_from_slice(&x.to_be_bytes());
    msg[4..6].copy_from_slice(&y.to_be_bytes());
    msg
}

/// Builds ClientCutText client-to-server message (RFC 6143 Section 7.5.6).
pub fn build_rfb_client_cut_text(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut msg = Vec::with_capacity(8 + bytes.len());
    msg.push(rfb_constants::MSG_CLIENT_CUT_TEXT);
    msg.extend_from_slice(&[0, 0, 0]);
    msg.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    msg.extend_from_slice(bytes);
    msg
}

/// Parses ServerCutText message (RFC 6143 Section 7.6.4).
pub fn parse_rfb_server_cut_text(buf: &[u8]) -> Result<(String, usize), ProtocolError> {
    if buf.len() < 8 {
        return Err(ProtocolError::IncompleteRfbMessage);
    }
    if buf[0] != rfb_constants::MSG_SERVER_CUT_TEXT {
        return Err(ProtocolError::InvalidRfbData(format!(
            "Expected ServerCutText(3), got {}",
            buf[0]
        )));
    }
    let len = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
    if buf.len() < 8 + len {
        return Err(ProtocolError::IncompleteRfbMessage);
    }
    let text = String::from_utf8_lossy(&buf[8..8 + len]).into_owned();
    Ok((text, 8 + len))
}

/// Parses FramebufferUpdate message header (RFC 6143 Section 7.6.1).
pub fn parse_rfb_framebuffer_update_header(buf: &[u8]) -> Result<(u16, usize), ProtocolError> {
    if buf.len() < 4 {
        return Err(ProtocolError::IncompleteRfbMessage);
    }
    if buf[0] != rfb_constants::MSG_FRAMEBUFFER_UPDATE {
        return Err(ProtocolError::InvalidRfbData(format!(
            "Expected FramebufferUpdate(0), got {}",
            buf[0]
        )));
    }
    let rect_count = u16::from_be_bytes([buf[2], buf[3]]);
    Ok((rect_count, 4))
}

/// Parses a single rectangle header in a FramebufferUpdate message.
pub fn parse_rfb_rectangle_header(
    buf: &[u8],
) -> Result<(RfbRectangleHeader, usize), ProtocolError> {
    if buf.len() < 12 {
        return Err(ProtocolError::IncompleteRfbMessage);
    }
    let x = u16::from_be_bytes([buf[0], buf[1]]);
    let y = u16::from_be_bytes([buf[2], buf[3]]);
    let width = u16::from_be_bytes([buf[4], buf[5]]);
    let height = u16::from_be_bytes([buf[6], buf[7]]);
    let encoding_type = i32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
    Ok((
        RfbRectangleHeader {
            x,
            y,
            width,
            height,
            encoding_type,
        },
        12,
    ))
}

/// Builds an RFC 1006 TPKT packet wrapping payload.
pub fn build_tpkt_packet(payload: &[u8]) -> Vec<u8> {
    let total_len = (payload.len() + 4) as u16;
    let mut packet = Vec::with_capacity(total_len as usize);
    packet.push(3); // TPKT version 3
    packet.push(0); // Reserved
    packet.extend_from_slice(&total_len.to_be_bytes());
    packet.extend_from_slice(payload);
    packet
}

/// Parses a TPKT packet. Returns Some((payload, consumed_bytes)) or None if incomplete.
pub fn parse_tpkt_packet(buf: &[u8]) -> Result<Option<(&[u8], usize)>, ProtocolError> {
    if buf.len() < 4 {
        return Ok(None);
    }
    if buf[0] != 3 {
        return Err(ProtocolError::InvalidTpktPacket);
    }
    let total_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    if total_len < 4 {
        return Err(ProtocolError::InvalidTpktPacket);
    }
    if buf.len() < total_len {
        return Ok(None);
    }
    Ok(Some((&buf[4..total_len], total_len)))
}

/// Builds an RDP X.224 Connection Request (CR) wrapped in TPKT (MS-RDPBCGR Section 2.2.1.1).
pub fn build_rdp_connection_request(routing_token_user: &str, requested_protocols: u32) -> Vec<u8> {
    let mut x224 = Vec::new();
    x224.push(0); // Length indicator placeholder
    x224.push(0xE0); // PDU type: Connection Request
    x224.extend_from_slice(&[0x00, 0x00]); // DST-REF
    x224.extend_from_slice(&[0x12, 0x34]); // SRC-REF
    x224.push(0x00); // Class 0

    if !routing_token_user.is_empty() {
        let cookie = format!("Cookie: mstshash={routing_token_user}\r\n");
        x224.extend_from_slice(cookie.as_bytes());
    }

    // RDP Negotiation Request (RDP_NEG_REQ)
    x224.push(0x01); // TYPE_RDP_NEG_REQ
    x224.push(0x00); // flags
    x224.extend_from_slice(&8u16.to_le_bytes()); // length: 8 bytes
    x224.extend_from_slice(&requested_protocols.to_le_bytes());

    let len_indicator = (x224.len() - 1) as u8;
    x224[0] = len_indicator;

    build_tpkt_packet(&x224)
}

/// Parses an RDP Negotiation Response (RDP_NEG_RSP). Returns the selected protocol flag.
pub fn parse_rdp_negotiation_response(payload: &[u8]) -> Result<u32, ProtocolError> {
    for i in 0..payload.len().saturating_sub(7) {
        if payload[i] == 0x02 {
            let len = u16::from_le_bytes([payload[i + 2], payload[i + 3]]);
            if len == 8 && i + 8 <= payload.len() {
                let selected = u32::from_le_bytes([
                    payload[i + 4],
                    payload[i + 5],
                    payload[i + 6],
                    payload[i + 7],
                ]);
                return Ok(selected);
            }
        }
    }
    Err(ProtocolError::InvalidRfbData(
        "No valid RDP negotiation response found".into(),
    ))
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
    Vnc(VncConfig),
    Rdp(RdpConfig),
}

impl ProtocolEndpoint {
    pub fn protocol(&self) -> ConnectionProtocol {
        match self {
            Self::Ssh(_) => ConnectionProtocol::Ssh,
            Self::Mosh { .. } => ConnectionProtocol::Mosh,
            Self::Telnet(_) => ConnectionProtocol::Telnet,
            Self::Serial(_) => ConnectionProtocol::Serial,
            Self::Local => ConnectionProtocol::Local,
            Self::Vnc(_) => ConnectionProtocol::Vnc,
            Self::Rdp(_) => ConnectionProtocol::Rdp,
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
            Self::Vnc(v) => format!("vnc://{}:{}", v.host, v.port),
            Self::Rdp(r) => format!(
                "rdp://{}@{}:{}",
                r.username.as_deref().unwrap_or(""),
                r.host,
                r.port
            ),
        }
    }

    pub fn security_warning(&self) -> Option<&'static str> {
        match self {
            Self::Telnet(_) => Some(TelnetConfig::security_warning()),
            Self::Serial(_) => Some(
                "ℹ️ Serial connection: Physical line communication without network encryption.",
            ),
            Self::Vnc(_) => Some(
                "ℹ️ Standard VNC transmits raw unencrypted framebuffer data unless tunneled over SSH.",
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

    pub fn is_graphical(&self) -> bool {
        self.protocol().is_graphical()
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
    InvalidRfbVersion(String),
    IncompleteRfbMessage,
    InvalidRfbData(String),
    InvalidTpktPacket,
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
            Self::InvalidRfbVersion(v) => write!(f, "Invalid RFB protocol version string: '{v}'"),
            Self::IncompleteRfbMessage => write!(f, "Incomplete RFB message buffer"),
            Self::InvalidRfbData(msg) => write!(f, "Invalid RFB data: {msg}"),
            Self::InvalidTpktPacket => write!(f, "Invalid TPKT packet header or length"),
            Self::UnsupportedFeature { protocol, feature } => {
                write!(f, "{protocol} does not support {feature}")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

#[cfg(test)]
mod tests;
