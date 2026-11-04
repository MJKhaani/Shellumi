//! CSV import and export parser and serializer (RFC 4180).

use crate::{HostConfigEntry, SshDestination};

/// Serializes host entries to RFC 4180 CSV format with header `name,hostname,username,port`.
pub fn export_csv(entries: &[HostConfigEntry]) -> String {
    let mut out = String::from("name,hostname,username,port\n");
    for entry in entries {
        out.push_str(&escape_csv_field(&entry.name));
        out.push(',');
        out.push_str(&escape_csv_field(entry.destination.host()));
        out.push(',');
        out.push_str(&escape_csv_field(entry.destination.username()));
        out.push(',');
        out.push_str(&entry.destination.port().to_string());
        out.push('\n');
    }
    out
}

fn escape_csv_field(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        let escaped = field.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        field.to_string()
    }
}

/// Parses an RFC 4180 CSV string into host entries.
/// If the first row is a header (`name,hostname,username,port`), it is automatically skipped.
pub fn parse_csv(content: &str) -> Result<Vec<HostConfigEntry>, String> {
    let mut entries = Vec::new();
    let mut is_first_line = true;

    for (line_num, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let fields = parse_csv_line(line);
        if fields.len() != 4 {
            return Err(format!(
                "Line {}: expected 4 columns (name, hostname, username, port), got {}",
                line_num + 1,
                fields.len()
            ));
        }

        // Check if header row
        if is_first_line {
            is_first_line = false;
            if fields[0].eq_ignore_ascii_case("name")
                && fields[1].eq_ignore_ascii_case("hostname")
                && fields[2].eq_ignore_ascii_case("username")
                && fields[3].eq_ignore_ascii_case("port")
            {
                continue;
            }
        }

        let name = &fields[0];
        let host = &fields[1];
        let user = &fields[2];
        let port: u16 = fields[3]
            .parse()
            .map_err(|_| format!("Line {}: invalid port '{}'", line_num + 1, fields[3]))?;

        let destination = SshDestination::new(host, user, port)
            .map_err(|e| format!("Line {}: invalid destination: {e}", line_num + 1))?;

        entries.push(HostConfigEntry {
            name: name.clone(),
            destination,
        });
    }

    Ok(entries)
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(c);
            }
        } else if c == '"' {
            in_quotes = true;
        } else if c == ',' {
            fields.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
    }
    fields.push(current.trim().to_string());
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_exports_csv_roundtrip() {
        let original = vec![
            HostConfigEntry::new(
                "Prod Server",
                SshDestination::new("10.0.0.1", "root", 22).unwrap(),
            ),
            HostConfigEntry::new(
                "Dev, Test",
                SshDestination::new("10.0.0.2", "dev", 2222).unwrap(),
            ),
        ];

        let csv = export_csv(&original);
        assert!(csv.contains("\"Dev, Test\"")); // escaped comma

        let parsed = parse_csv(&csv).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn rejects_invalid_csv_rows() {
        assert!(parse_csv("name,hostname,username\nfoo,bar,baz").is_err());
        assert!(parse_csv("name,hostname,username,port\nfoo,bar,baz,notaport").is_err());
    }
}
