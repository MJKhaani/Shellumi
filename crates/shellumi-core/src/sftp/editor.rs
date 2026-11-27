//! Minimal syntax-aware administrative text/code editor engine:
//! - Language detection & manual override across 15 admin/config/script modes
//! - Revision-stamped syntax tokenization (keywords, strings, comments, keys, directives, vars)
//! - Conservative semantic-preserving formatter (strict JSON, safe YAML/TOML, honest fallback)
//! - Multiline UTF-8 buffer with undo/redo, line endings, BOM, find/replace, and safe save state

use super::endpoint::{EditorId, EndpointKind, VaultScope};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ops::Range;
use std::path::Path;

pub const DEFAULT_EDITABLE_LIMIT_BYTES: u64 = 2 * 1024 * 1024;
pub const DEFAULT_PREVIEW_LIMIT_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EditorLanguage {
    PlainText,
    Json,
    Jsonc,
    Yaml,
    Toml,
    IniSystemd,
    Dotenv,
    SshConfig,
    NginxApache,
    Shell,
    Python,
    PowerShell,
    Dockerfile,
    Xml,
    Properties,
}

impl EditorLanguage {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::PlainText => "Plain Text",
            Self::Json => "JSON",
            Self::Jsonc => "JSONC",
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
            Self::IniSystemd => "INI / systemd",
            Self::Dotenv => "dotenv",
            Self::SshConfig => "SSH Config",
            Self::NginxApache => "nginx / Apache",
            Self::Shell => "Shell",
            Self::Python => "Python",
            Self::PowerShell => "PowerShell",
            Self::Dockerfile => "Dockerfile",
            Self::Xml => "XML",
            Self::Properties => "Properties",
        }
    }

    pub fn line_comment_prefix(&self) -> Option<&'static str> {
        match self {
            Self::PlainText | Self::Json | Self::Xml => None,
            Self::Jsonc => Some("//"),
            Self::IniSystemd => Some("#"),
            Self::Yaml
            | Self::Toml
            | Self::Dotenv
            | Self::SshConfig
            | Self::NginxApache
            | Self::Shell
            | Self::Python
            | Self::PowerShell
            | Self::Dockerfile
            | Self::Properties => Some("#"),
        }
    }

    pub fn supports_safe_format(&self) -> bool {
        matches!(self, Self::Json | Self::Yaml | Self::Toml)
    }
}

/// Detect language by: special basename/path -> extension -> bounded shebang -> PlainText.
/// Never executes any file content.
pub fn detect_language(path_str: &str, content_sample: &str) -> EditorLanguage {
    let p = Path::new(path_str);
    let file_name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let lower_path = path_str.to_ascii_lowercase();

    // 1. Special basenames & paths
    if file_name == "dockerfile"
        || file_name.starts_with("dockerfile.")
        || file_name == "containerfile"
        || file_name.starts_with("containerfile.")
    {
        return EditorLanguage::Dockerfile;
    }
    if file_name == "ssh_config"
        || file_name == "sshd_config"
        || lower_path.ends_with("/.ssh/config")
        || lower_path.contains("/ssh_config.d/")
        || lower_path.contains("/sshd_config.d/")
    {
        return EditorLanguage::SshConfig;
    }
    if file_name == "nginx.conf"
        || file_name == "httpd.conf"
        || file_name == "apache2.conf"
        || lower_path.contains("/nginx/")
        || lower_path.contains("/apache2/")
        || lower_path.contains("/sites-available/")
        || lower_path.contains("/sites-enabled/")
    {
        return EditorLanguage::NginxApache;
    }
    if file_name == ".env" || file_name.starts_with(".env.") {
        return EditorLanguage::Dotenv;
    }

    // 2. Extension
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "json" => return EditorLanguage::Json,
        "jsonc" => return EditorLanguage::Jsonc,
        "yaml" | "yml" => return EditorLanguage::Yaml,
        "toml" => return EditorLanguage::Toml,
        "ini" | "service" | "socket" | "timer" | "mount" | "target" | "path" => {
            return EditorLanguage::IniSystemd
        }
        "env" => return EditorLanguage::Dotenv,
        "sh" | "bash" | "zsh" => return EditorLanguage::Shell,
        "py" => return EditorLanguage::Python,
        "ps1" | "psm1" | "psd1" => return EditorLanguage::PowerShell,
        "xml" | "plist" => return EditorLanguage::Xml,
        "properties" => return EditorLanguage::Properties,
        "conf" | "cfg" => {
            // Heuristic for generic .conf without executing anything
            if content_sample.contains("[Unit]") || content_sample.contains("[Service]") {
                return EditorLanguage::IniSystemd;
            }
            if content_sample.contains("server {") || content_sample.contains("location ") {
                return EditorLanguage::NginxApache;
            }
            return EditorLanguage::Properties;
        }
        _ => {}
    }

    // 3. Bounded shebang inspection (first line only)
    if let Some(first_line) = content_sample.lines().next() {
        if let Some(shebang) = first_line.strip_prefix("#!") {
            let s = shebang.to_ascii_lowercase();
            if s.contains("python") {
                return EditorLanguage::Python;
            }
            if s.contains("pwsh") || s.contains("powershell") {
                return EditorLanguage::PowerShell;
            }
            if s.contains("sh") || s.contains("bash") || s.contains("zsh") {
                return EditorLanguage::Shell;
            }
        }
    }

    EditorLanguage::PlainText
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxTokenKind {
    Plain,
    Keyword,
    String,
    Comment,
    Number,
    Boolean,
    Key,
    Section,
    Directive,
    Variable,
    Punctuation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxSpan {
    pub range: Range<usize>,
    pub kind: SyntaxTokenKind,
}

/// Tokenize a single line of text into non-overlapping UTF-8 byte-aligned `SyntaxSpan`s.
pub fn highlight_line(lang: EditorLanguage, line: &str) -> Vec<SyntaxSpan> {
    if line.is_empty() || lang == EditorLanguage::PlainText {
        return Vec::new();
    }
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();

    // Full-line comments
    if let Some(prefix) = lang.line_comment_prefix() {
        if trimmed.starts_with(prefix) {
            return vec![SyntaxSpan {
                range: indent..line.len(),
                kind: SyntaxTokenKind::Comment,
            }];
        }
    }
    if lang == EditorLanguage::IniSystemd && trimmed.starts_with(';') {
        return vec![SyntaxSpan {
            range: indent..line.len(),
            kind: SyntaxTokenKind::Comment,
        }];
    }
    if lang == EditorLanguage::Xml && trimmed.starts_with("<!--") {
        return vec![SyntaxSpan {
            range: indent..line.len(),
            kind: SyntaxTokenKind::Comment,
        }];
    }

    // Section headers in INI/TOML/SSH
    if matches!(lang, EditorLanguage::IniSystemd | EditorLanguage::Toml)
        && trimmed.starts_with('[')
        && trimmed.ends_with(']')
    {
        return vec![SyntaxSpan {
            range: indent..line.len(),
            kind: SyntaxTokenKind::Section,
        }];
    }

    let mut spans = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        // Skip whitespace
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        // Inline comments
        if (b == b'#'
            && !matches!(
                lang,
                EditorLanguage::Json | EditorLanguage::Jsonc | EditorLanguage::Xml
            ))
            || (b == b'/'
                && i + 1 < bytes.len()
                && bytes[i + 1] == b'/'
                && lang == EditorLanguage::Jsonc)
        {
            spans.push(SyntaxSpan {
                range: i..bytes.len(),
                kind: SyntaxTokenKind::Comment,
            });
            break;
        }
        // Quoted strings
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            // Check if JSON/YAML key (followed by ':')
            let mut rest = i;
            while rest < bytes.len() && bytes[rest].is_ascii_whitespace() {
                rest += 1;
            }
            let kind = if matches!(
                lang,
                EditorLanguage::Json | EditorLanguage::Jsonc | EditorLanguage::Yaml
            ) && rest < bytes.len()
                && bytes[rest] == b':'
            {
                SyntaxTokenKind::Key
            } else {
                SyntaxTokenKind::String
            };
            spans.push(SyntaxSpan {
                range: start..i,
                kind,
            });
            continue;
        }
        // Variables ($VAR, ${VAR})
        if b == b'$'
            && matches!(
                lang,
                EditorLanguage::Shell
                    | EditorLanguage::Dotenv
                    | EditorLanguage::PowerShell
                    | EditorLanguage::NginxApache
            )
        {
            let start = i;
            i += 1;
            if i < bytes.len() && bytes[i] == b'{' {
                while i < bytes.len() && bytes[i] != b'}' {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
            } else {
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
            }
            spans.push(SyntaxSpan {
                range: start..i,
                kind: SyntaxTokenKind::Variable,
            });
            continue;
        }
        // Punctuation
        if matches!(b, b'{' | b'}' | b'[' | b']' | b':' | b',' | b'=' | b';') {
            spans.push(SyntaxSpan {
                range: i..i + 1,
                kind: SyntaxTokenKind::Punctuation,
            });
            i += 1;
            continue;
        }
        // Word / number / identifier
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.' {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'-' | b'.'))
            {
                i += 1;
            }
            let word = &line[start..i];
            let kind = classify_word(lang, word, &line[i..]);
            if kind != SyntaxTokenKind::Plain {
                spans.push(SyntaxSpan {
                    range: start..i,
                    kind,
                });
            }
            continue;
        }
        // Advance safely by one UTF-8 char
        let ch_len = line[i..].chars().next().map_or(1, char::len_utf8);
        i += ch_len;
    }

    spans
}

fn classify_word(lang: EditorLanguage, word: &str, after: &str) -> SyntaxTokenKind {
    if word == "true"
        || word == "false"
        || word == "null"
        || word == "None"
        || word == "True"
        || word == "False"
    {
        return SyntaxTokenKind::Boolean;
    }
    if word.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && word.bytes().any(|b| b.is_ascii_digit())
    {
        return SyntaxTokenKind::Number;
    }
    let after_trim = after.trim_start();
    if matches!(
        lang,
        EditorLanguage::Yaml
            | EditorLanguage::Toml
            | EditorLanguage::IniSystemd
            | EditorLanguage::Dotenv
            | EditorLanguage::Properties
    ) && (after_trim.starts_with(':') || after_trim.starts_with('='))
    {
        return SyntaxTokenKind::Key;
    }
    match lang {
        EditorLanguage::SshConfig => {
            if matches!(
                word,
                "Host"
                    | "Match"
                    | "HostName"
                    | "User"
                    | "Port"
                    | "IdentityFile"
                    | "ProxyJump"
                    | "ForwardAgent"
                    | "ServerAliveInterval"
                    | "Include"
                    | "PermitRootLogin"
                    | "PasswordAuthentication"
                    | "PubkeyAuthentication"
                    | "Subsystem"
            ) {
                return SyntaxTokenKind::Directive;
            }
        }
        EditorLanguage::NginxApache => {
            if matches!(
                word,
                "server"
                    | "location"
                    | "http"
                    | "events"
                    | "upstream"
                    | "listen"
                    | "server_name"
                    | "proxy_pass"
                    | "root"
                    | "index"
                    | "include"
                    | "VirtualHost"
                    | "ServerName"
                    | "DocumentRoot"
            ) {
                return SyntaxTokenKind::Directive;
            }
        }
        EditorLanguage::Dockerfile => {
            if matches!(
                word.to_ascii_uppercase().as_str(),
                "FROM"
                    | "RUN"
                    | "CMD"
                    | "ENTRYPOINT"
                    | "COPY"
                    | "ADD"
                    | "ENV"
                    | "ARG"
                    | "WORKDIR"
                    | "EXPOSE"
                    | "USER"
                    | "VOLUME"
                    | "LABEL"
            ) {
                return SyntaxTokenKind::Keyword;
            }
        }
        EditorLanguage::Shell => {
            if matches!(
                word,
                "if" | "then"
                    | "else"
                    | "elif"
                    | "fi"
                    | "for"
                    | "in"
                    | "do"
                    | "done"
                    | "while"
                    | "case"
                    | "esac"
                    | "function"
                    | "export"
                    | "local"
                    | "return"
                    | "exit"
            ) {
                return SyntaxTokenKind::Keyword;
            }
        }
        EditorLanguage::Python => {
            if matches!(
                word,
                "def"
                    | "class"
                    | "if"
                    | "elif"
                    | "else"
                    | "for"
                    | "while"
                    | "import"
                    | "from"
                    | "as"
                    | "return"
                    | "with"
                    | "try"
                    | "except"
                    | "finally"
                    | "raise"
                    | "pass"
                    | "yield"
                    | "async"
                    | "await"
            ) {
                return SyntaxTokenKind::Keyword;
            }
        }
        EditorLanguage::PowerShell => {
            if matches!(
                word.to_ascii_lowercase().as_str(),
                "function"
                    | "param"
                    | "if"
                    | "else"
                    | "elseif"
                    | "foreach"
                    | "for"
                    | "while"
                    | "switch"
                    | "return"
                    | "try"
                    | "catch"
                    | "finally"
            ) {
                return SyntaxTokenKind::Keyword;
            }
        }
        _ => {}
    }
    SyntaxTokenKind::Plain
}

/// Outcome of formatting a document. Never mutates the buffer on error or unsupported mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatOutcome {
    Formatted {
        text: String,
        changed: bool,
    },
    Unsupported(String),
    SyntaxError {
        line: usize,
        column: usize,
        message: String,
    },
}

/// Format a document conservatively according to §9.3.
pub fn format_document(lang: EditorLanguage, input: &str, indent_spaces: usize) -> FormatOutcome {
    match lang {
        EditorLanguage::Json => format_strict_json(input, indent_spaces),
        EditorLanguage::Yaml => format_yaml_conservative(input),
        EditorLanguage::Toml => format_toml_conservative(input),
        _ => FormatOutcome::Unsupported(format!(
            "Safe full-document formatting is not enabled for {}; use indentation actions instead.",
            lang.display_name()
        )),
    }
}

/// Strict, non-lossy JSON formatter:
/// - Preserves exact key order
/// - Preserves exact numeric tokens
/// - Rejects duplicate keys in any object
/// - Leaves input untouched on syntax error
fn format_strict_json(input: &str, indent_spaces: usize) -> FormatOutcome {
    let mut parser = JsonParser {
        src: input,
        pos: 0,
        line: 1,
        col: 1,
    };
    let val = match parser.parse_value() {
        Ok(v) => v,
        Err((line, col, message)) => {
            return FormatOutcome::SyntaxError {
                line,
                column: col,
                message,
            }
        }
    };
    parser.skip_ws();
    if parser.pos < parser.src.len() {
        return FormatOutcome::SyntaxError {
            line: parser.line,
            column: parser.col,
            message: "Unexpected trailing characters after JSON value".into(),
        };
    }

    let mut out = String::with_capacity(input.len() + 64);
    let indent_unit = " ".repeat(indent_spaces.clamp(2, 8));
    write_json_pretty(&val, &indent_unit, 0, &mut out);
    if input.ends_with('\n') {
        out.push('\n');
    }
    let changed = out != input;
    FormatOutcome::Formatted { text: out, changed }
}

enum JsonAst<'a> {
    Null,
    Bool(bool),
    Number(&'a str),
    String(&'a str),
    Array(Vec<JsonAst<'a>>),
    Object(Vec<(&'a str, JsonAst<'a>)>),
}

struct JsonParser<'a> {
    src: &'a str,
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> JsonParser<'a> {
    fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.pos).copied()
    }

    fn advance(&mut self, n: usize) {
        for ch in self.src[self.pos..self.pos + n].chars() {
            if ch == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        self.pos += n;
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if matches!(b, b' ' | b'\t' | b'\r' | b'\n') {
                self.advance(1);
            } else {
                break;
            }
        }
    }

    fn err(&self, msg: impl Into<String>) -> (usize, usize, String) {
        (self.line, self.col, msg.into())
    }

    fn parse_value(&mut self) -> Result<JsonAst<'a>, (usize, usize, String)> {
        self.skip_ws();
        match self.peek() {
            Some(b'"') => self.parse_string_raw().map(JsonAst::String),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b't') if self.src[self.pos..].starts_with("true") => {
                self.advance(4);
                Ok(JsonAst::Bool(true))
            }
            Some(b'f') if self.src[self.pos..].starts_with("false") => {
                self.advance(5);
                Ok(JsonAst::Bool(false))
            }
            Some(b'n') if self.src[self.pos..].starts_with("null") => {
                self.advance(4);
                Ok(JsonAst::Null)
            }
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number_raw().map(JsonAst::Number),
            _ => Err(self.err("Expected JSON value")),
        }
    }

    fn parse_string_raw(&mut self) -> Result<&'a str, (usize, usize, String)> {
        let start = self.pos;
        self.advance(1); // opening '"'
        while self.pos < self.src.len() {
            let b = self.src.as_bytes()[self.pos];
            if b == b'"' {
                self.advance(1);
                return Ok(&self.src[start..self.pos]);
            }
            if b == b'\\' {
                self.advance(1);
                if self.pos >= self.src.len() {
                    return Err(self.err("Unterminated string escape"));
                }
                let esc = self.src.as_bytes()[self.pos];
                if esc == b'u' {
                    if self.pos + 5 > self.src.len() {
                        return Err(self.err("Invalid \\u escape"));
                    }
                    self.advance(5);
                } else {
                    self.advance(1);
                }
                continue;
            }
            if b < 0x20 {
                return Err(self.err("Unescaped control character in JSON string"));
            }
            let ch_len = self.src[self.pos..]
                .chars()
                .next()
                .map_or(1, char::len_utf8);
            self.advance(ch_len);
        }
        Err(self.err("Unterminated JSON string"))
    }

    fn parse_number_raw(&mut self) -> Result<&'a str, (usize, usize, String)> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.advance(1);
        }
        match self.peek() {
            Some(b'0') => self.advance(1),
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.advance(1);
                }
            }
            _ => return Err(self.err("Invalid JSON number")),
        }
        if self.peek() == Some(b'.') {
            self.advance(1);
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err("Expected digit after decimal point"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.advance(1);
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.advance(1);
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.advance(1);
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err("Expected exponent digit"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.advance(1);
            }
        }
        Ok(&self.src[start..self.pos])
    }

    fn parse_array(&mut self) -> Result<JsonAst<'a>, (usize, usize, String)> {
        self.advance(1); // '['
        self.skip_ws();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.advance(1);
            return Ok(JsonAst::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.advance(1),
                Some(b']') => {
                    self.advance(1);
                    return Ok(JsonAst::Array(items));
                }
                _ => return Err(self.err("Expected ',' or ']' in array")),
            }
        }
    }

    fn parse_object(&mut self) -> Result<JsonAst<'a>, (usize, usize, String)> {
        self.advance(1); // '{'
        self.skip_ws();
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        if self.peek() == Some(b'}') {
            self.advance(1);
            return Ok(JsonAst::Object(entries));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("Expected string key in JSON object"));
            }
            let key_raw = self.parse_string_raw()?;
            // Unescape via serde_json::from_str::<String> only for duplicate-key detection
            let decoded_key: String = serde_json::from_str(key_raw)
                .map_err(|e| self.err(format!("Invalid JSON key: {e}")))?;
            if !seen.insert(decoded_key.clone()) {
                return Err(self.err(format!("Duplicate JSON key '{decoded_key}'")));
            }
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.err("Expected ':' after JSON object key"));
            }
            self.advance(1);
            let val = self.parse_value()?;
            entries.push((key_raw, val));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.advance(1),
                Some(b'}') => {
                    self.advance(1);
                    return Ok(JsonAst::Object(entries));
                }
                _ => return Err(self.err("Expected ',' or '}' in object")),
            }
        }
    }
}

fn write_json_pretty(val: &JsonAst<'_>, unit: &str, depth: usize, out: &mut String) {
    match val {
        JsonAst::Null => out.push_str("null"),
        JsonAst::Bool(true) => out.push_str("true"),
        JsonAst::Bool(false) => out.push_str("false"),
        JsonAst::Number(n) => out.push_str(n),
        JsonAst::String(s) => out.push_str(s),
        JsonAst::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (idx, item) in items.iter().enumerate() {
                for _ in 0..=depth {
                    out.push_str(unit);
                }
                write_json_pretty(item, unit, depth + 1, out);
                if idx + 1 < items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            for _ in 0..depth {
                out.push_str(unit);
            }
            out.push(']');
        }
        JsonAst::Object(entries) => {
            if entries.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            for (idx, (k, v)) in entries.iter().enumerate() {
                for _ in 0..=depth {
                    out.push_str(unit);
                }
                out.push_str(k);
                out.push_str(": ");
                write_json_pretty(v, unit, depth + 1, out);
                if idx + 1 < entries.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            for _ in 0..depth {
                out.push_str(unit);
            }
            out.push('}');
        }
    }
}

/// Conservative YAML formatter: validates tab indentation & basic structure,
/// preserves comments, anchors, aliases, key order, and block scalars (`|` / `>`).
fn format_yaml_conservative(input: &str) -> FormatOutcome {
    for (idx, line) in input.lines().enumerate() {
        if line.starts_with('\t') {
            return FormatOutcome::SyntaxError {
                line: idx + 1,
                column: 1,
                message: "YAML forbids leading tabs for indentation".into(),
            };
        }
    }
    // Preserve exact line endings and block scalars; only normalize trailing spaces on non-block lines
    let line_ending = if input.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out_lines = Vec::new();
    let mut in_block_scalar_indent: Option<usize> = None;

    for line in input.lines() {
        let indent = line.len() - line.trim_start_matches(' ').len();
        if let Some(block_indent) = in_block_scalar_indent {
            if !line.trim().is_empty() && indent <= block_indent {
                in_block_scalar_indent = None;
            } else {
                // Inside block scalar: preserve verbatim!
                out_lines.push(line.to_string());
                continue;
            }
        }
        let trimmed_end = line.trim_end_matches([' ', '\t']);
        out_lines.push(trimmed_end.to_string());
        if trimmed_end.ends_with(": |")
            || trimmed_end.ends_with(": >")
            || trimmed_end.ends_with(": |-")
            || trimmed_end.ends_with(": >-")
        {
            in_block_scalar_indent = Some(indent);
        }
    }

    let mut out = out_lines.join(line_ending);
    if input.ends_with('\n') && !out.ends_with('\n') {
        out.push_str(line_ending);
    }
    let changed = out != input;
    FormatOutcome::Formatted { text: out, changed }
}

/// Conservative TOML formatter: preserves comments, multiline strings, and key order;
/// validates section headers and string quotes.
fn format_toml_conservative(input: &str) -> FormatOutcome {
    let line_ending = if input.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out_lines = Vec::new();
    let mut in_multiline = false;

    for (idx, line) in input.lines().enumerate() {
        if line.contains("\"\"\"") || line.contains("'''") {
            in_multiline = !in_multiline;
            out_lines.push(line.to_string());
            continue;
        }
        if in_multiline {
            out_lines.push(line.to_string());
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with('[') && !trimmed.contains('#') && !trimmed.ends_with(']') {
            return FormatOutcome::SyntaxError {
                line: idx + 1,
                column: 1,
                message: "Unterminated TOML section header".into(),
            };
        }
        out_lines.push(line.trim_end_matches([' ', '\t']).to_string());
    }

    if in_multiline {
        return FormatOutcome::SyntaxError {
            line: input.lines().count().max(1),
            column: 1,
            message: "Unterminated multiline TOML string".into(),
        };
    }

    let mut out = out_lines.join(line_ending);
    if input.ends_with('\n') && !out.ends_with('\n') {
        out.push_str(line_ending);
    }
    let changed = out != input;
    FormatOutcome::Formatted { text: out, changed }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

#[derive(Clone, Debug)]
struct UndoEntry {
    text: String,
    cursor: usize,
}

/// Multiline UTF-8 text buffer with undo/redo, line ending & BOM preservation,
/// find/replace, and revision tracking.
#[derive(Clone, Debug)]
pub struct EditorBuffer {
    text: String,
    cursor: usize,
    selection_anchor: Option<usize>,
    revision: u64,
    saved_revision: u64,
    line_ending: LineEnding,
    has_bom: bool,
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
}

impl EditorBuffer {
    pub fn from_bytes(raw: &[u8]) -> Result<Self, String> {
        if raw.len() as u64 > DEFAULT_PREVIEW_LIMIT_BYTES {
            return Err("File exceeds maximum preview/editor size limit".into());
        }
        if raw.contains(&0) {
            return Err("Binary file detected (contains NUL bytes)".into());
        }
        let (has_bom, slice) = if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
            (true, &raw[3..])
        } else {
            (false, raw)
        };
        let text = std::str::from_utf8(slice)
            .map_err(|_| "File is not valid UTF-8 text".to_string())?
            .to_string();
        let line_ending = if text.contains("\r\n") {
            LineEnding::Crlf
        } else {
            LineEnding::Lf
        };
        Ok(Self {
            text,
            cursor: 0,
            selection_anchor: None,
            revision: 1,
            saved_revision: 1,
            line_ending,
            has_bom,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    /// Mark `saved_rev` as saved. If the user typed while save was running (`self.revision > saved_rev`),
    /// the buffer remains dirty!
    pub fn mark_saved_revision(&mut self, saved_rev: u64) {
        if self.saved_revision < saved_rev {
            self.saved_revision = saved_rev;
        }
    }

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    pub fn has_bom(&self) -> bool {
        self.has_bom
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.text.len() + 3);
        if self.has_bom {
            out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        out.extend_from_slice(self.text.as_bytes());
        out
    }

    pub fn sha256_hex(&self) -> String {
        hex::encode(Sha256::digest(self.to_bytes()))
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_cursor(&mut self, pos: usize) {
        let clamped = pos.min(self.text.len());
        self.cursor = self.snap_char_boundary(clamped);
    }

    fn snap_char_boundary(&self, mut pos: usize) -> usize {
        pos = pos.min(self.text.len());
        while pos > 0 && !self.text.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }

    pub fn apply_edit(&mut self, range: Range<usize>, replacement: &str) {
        let start = self.snap_char_boundary(range.start);
        let end = self.snap_char_boundary(range.end).max(start);
        self.undo_stack.push(UndoEntry {
            text: self.text.clone(),
            cursor: self.cursor,
        });
        if self.undo_stack.len() > 200 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
        self.text.replace_range(start..end, replacement);
        self.cursor = start + replacement.len();
        self.selection_anchor = None;
        self.revision += 1;
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(UndoEntry {
                text: self.text.clone(),
                cursor: self.cursor,
            });
            self.text = prev.text;
            self.cursor = prev.cursor.min(self.text.len());
            self.revision += 1;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(UndoEntry {
                text: self.text.clone(),
                cursor: self.cursor,
            });
            self.text = next.text;
            self.cursor = next.cursor.min(self.text.len());
            self.revision += 1;
            true
        } else {
            false
        }
    }

    pub fn line_col(&self) -> (usize, usize) {
        let prefix = &self.text[..self.snap_char_boundary(self.cursor)];
        let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
        let col = prefix
            .rsplit('\n')
            .next()
            .map_or(1, |s| s.chars().count() + 1);
        (line, col)
    }

    pub fn find_all(&self, query: &str, case_sensitive: bool) -> Vec<Range<usize>> {
        if query.is_empty() {
            return Vec::new();
        }
        let mut matches = Vec::new();
        if case_sensitive {
            let mut start = 0;
            while let Some(pos) = self.text[start..].find(query) {
                let abs = start + pos;
                matches.push(abs..abs + query.len());
                start = abs + query.len();
            }
        } else {
            let hay = self.text.to_lowercase();
            let needle = query.to_lowercase();
            let mut start = 0;
            while let Some(pos) = hay[start..].find(&needle) {
                let abs = start + pos;
                if self.text.is_char_boundary(abs) && self.text.is_char_boundary(abs + needle.len())
                {
                    matches.push(abs..abs + needle.len());
                }
                start = abs + needle.len().max(1);
            }
        }
        matches
    }

    pub fn replace_all(&mut self, query: &str, replacement: &str, case_sensitive: bool) -> usize {
        let matches = self.find_all(query, case_sensitive);
        if matches.is_empty() {
            return 0;
        }
        self.undo_stack.push(UndoEntry {
            text: self.text.clone(),
            cursor: self.cursor,
        });
        self.redo_stack.clear();
        let count = matches.len();
        let mut new_text = String::with_capacity(self.text.len());
        let mut last = 0;
        for r in matches {
            new_text.push_str(&self.text[last..r.start]);
            new_text.push_str(replacement);
            last = r.end;
        }
        new_text.push_str(&self.text[last..]);
        self.text = new_text;
        self.cursor = self.cursor.min(self.text.len());
        self.revision += 1;
        count
    }
}

/// Hardened metadata for an open integrated or external editor session,
/// scoped by `VaultScope`, `EndpointKind`, and exact path.
#[derive(Clone, Debug)]
pub struct EditorSessionMeta {
    pub editor_id: EditorId,
    pub vault_scope: VaultScope,
    pub endpoint: EndpointKind,
    pub path_display: String,
    pub language: EditorLanguage,
    pub original_sha256: String,
    pub original_size: u64,
    pub original_mtime: Option<u64>,
    pub original_mode: Option<u32>,
    pub read_only: bool,
}
