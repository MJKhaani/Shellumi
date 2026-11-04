//! Snippet and command automation engine.
//!
//! Provides UI-independent abstractions for:
//! - Reusable scripts and command templates (`Snippet`)
//! - Template variable extraction (`{{var}}`, `{{var:-default}}`)
//! - Safe variable interpolation and parameter substitution
//! - Secret redaction in command previews
//! - POSIX shell quoting and argument escaping
//! - Library serialization and deserialization (JSON format)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnippetError {
    InvalidName,
    InvalidCommand,
    InvalidVariableName(String),
    MissingVariable(String),
    Serialization(String),
    Deserialization(String),
}

impl fmt::Display for SnippetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName => {
                f.write_str("snippet name cannot be empty or contain control characters")
            }
            Self::InvalidCommand => f.write_str("snippet command cannot be empty"),
            Self::InvalidVariableName(name) => write!(
                f,
                "invalid variable name '{name}': must be alphanumeric or underscore"
            ),
            Self::MissingVariable(name) => write!(f, "missing required snippet variable: '{name}'"),
            Self::Serialization(err) => write!(f, "snippet serialization error: {err}"),
            Self::Deserialization(err) => write!(f, "snippet deserialization error: {err}"),
        }
    }
}

impl std::error::Error for SnippetError {}

/// A declared variable within a snippet template.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetVariable {
    pub name: String,
    pub default_value: Option<String>,
    pub description: Option<String>,
    pub is_secret: bool,
}

impl SnippetVariable {
    pub fn new(name: impl Into<String>) -> Result<Self, SnippetError> {
        let name = name.into();
        validate_var_name(&name)?;
        Ok(Self {
            name,
            default_value: None,
            description: None,
            is_secret: false,
        })
    }

    pub fn with_default(mut self, default: impl Into<String>) -> Self {
        self.default_value = Some(default.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn secret(mut self, is_secret: bool) -> Self {
        self.is_secret = is_secret;
        self
    }
}

/// A reusable snippet / command template.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snippet {
    pub name: String,
    pub command: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub variables: Vec<SnippetVariable>,
}

impl Snippet {
    pub fn new(
        name: impl Into<String>,
        command: impl Into<String>,
        description: Option<String>,
        tags: &[String],
    ) -> Result<Self, SnippetError> {
        let name = name.into();
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() || trimmed_name.chars().any(char::is_control) {
            return Err(SnippetError::InvalidName);
        }
        let command = command.into();
        let trimmed_command = command.trim();
        if trimmed_command.is_empty() {
            return Err(SnippetError::InvalidCommand);
        }

        let clean_tags: Vec<String> = tags
            .iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();

        let extracted_vars = extract_variables(&command)?;

        Ok(Self {
            name: trimmed_name.to_string(),
            command,
            description: description.map(|d| d.trim().to_string()),
            tags: clean_tags,
            variables: extracted_vars,
        })
    }

    /// Renders the executable command with the given parameter map.
    pub fn render(&self, params: &HashMap<String, String>) -> Result<String, SnippetError> {
        render_command(&self.command, params)
    }

    /// Renders a preview of the command with secret variables redacted (`[REDACTED]`).
    pub fn render_preview(&self, params: &HashMap<String, String>) -> Result<String, SnippetError> {
        render_preview(&self.command, params, &self.variables)
    }
}

fn validate_var_name(name: &str) -> Result<(), SnippetError> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(SnippetError::InvalidVariableName(name.to_string()));
    }
    Ok(())
}

/// Scans a command template and extracts declared `{{var}}` or `{{var:-default}}` tokens.
pub fn extract_variables(command: &str) -> Result<Vec<SnippetVariable>, SnippetError> {
    let mut vars = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut rest = command;
    while let Some(start_idx) = rest.find("{{") {
        let after_start = &rest[start_idx + 2..];
        if let Some(end_idx) = after_start.find("}}") {
            let token = after_start[..end_idx].trim();
            if !token.is_empty() {
                let (name, default_val) = match token.find(":-") {
                    Some(split_idx) => {
                        let n = token[..split_idx].trim();
                        let d = token[split_idx + 2..].trim();
                        (n, Some(d.to_string()))
                    }
                    None => (token, None),
                };

                validate_var_name(name)?;

                if seen.insert(name.to_string()) {
                    let is_secret = name.to_lowercase().contains("pass")
                        || name.to_lowercase().contains("secret")
                        || name.to_lowercase().contains("token")
                        || name.to_lowercase().contains("key");

                    vars.push(SnippetVariable {
                        name: name.to_string(),
                        default_value: default_val,
                        description: None,
                        is_secret,
                    });
                }
            }
            rest = &after_start[end_idx + 2..];
        } else {
            break;
        }
    }

    Ok(vars)
}

/// Replaces `{{var}}` tokens in a command string with supplied parameters or defaults.
pub fn render_command(
    command: &str,
    params: &HashMap<String, String>,
) -> Result<String, SnippetError> {
    let mut result = String::with_capacity(command.len());
    let mut rest = command;

    while let Some(start_idx) = rest.find("{{") {
        result.push_str(&rest[..start_idx]);
        let after_start = &rest[start_idx + 2..];
        if let Some(end_idx) = after_start.find("}}") {
            let token = after_start[..end_idx].trim();
            let (name, default_val) = match token.find(":-") {
                Some(split_idx) => {
                    let n = token[..split_idx].trim();
                    let d = token[split_idx + 2..].trim();
                    (n, Some(d))
                }
                None => (token, None),
            };

            if let Some(val) = params.get(name) {
                result.push_str(val);
            } else if let Some(def) = default_val {
                result.push_str(def);
            } else {
                return Err(SnippetError::MissingVariable(name.to_string()));
            }

            rest = &after_start[end_idx + 2..];
        } else {
            result.push_str("{{");
            rest = after_start;
        }
    }

    result.push_str(rest);
    Ok(result)
}

/// Renders a preview with any secret variable replaced by `[REDACTED]`.
pub fn render_preview(
    command: &str,
    params: &HashMap<String, String>,
    variables: &[SnippetVariable],
) -> Result<String, SnippetError> {
    let secret_names: std::collections::HashSet<&str> = variables
        .iter()
        .filter(|v| v.is_secret)
        .map(|v| v.name.as_str())
        .collect();

    let mut preview_params = params.clone();
    for v in variables {
        if secret_names.contains(v.name.as_str()) {
            preview_params.insert(v.name.clone(), "[REDACTED]".to_string());
        }
    }

    render_command(command, &preview_params)
}

/// Quotes a string safely for POSIX shell execution (`'string'` with single quotes escaped).
pub fn shell_escape(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/')
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Serializes a slice of snippets to formatted JSON.
pub fn export_snippets_json(snippets: &[Snippet]) -> Result<String, SnippetError> {
    serde_json::to_string_pretty(snippets).map_err(|e| SnippetError::Serialization(e.to_string()))
}

/// Deserializes a slice of snippets from JSON.
pub fn parse_snippets_json(json_str: &str) -> Result<Vec<Snippet>, SnippetError> {
    serde_json::from_str(json_str).map_err(|e| SnippetError::Deserialization(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_snippet_creation_and_rejections() {
        assert!(matches!(
            Snippet::new("", "echo hi", None, &[]),
            Err(SnippetError::InvalidName)
        ));
        assert!(matches!(
            Snippet::new("bad\nname", "echo hi", None, &[]),
            Err(SnippetError::InvalidName)
        ));
        assert!(matches!(
            Snippet::new("Valid", "   ", None, &[]),
            Err(SnippetError::InvalidCommand)
        ));

        let s = Snippet::new(
            "Restart Nginx",
            "sudo systemctl restart nginx",
            Some("Restarts the web server".into()),
            &["web".into(), "prod".into()],
        )
        .unwrap();
        assert_eq!(s.name, "Restart Nginx");
        assert_eq!(s.tags, vec!["web", "prod"]);
        assert!(s.variables.is_empty());
    }

    #[test]
    fn extracts_variables_and_detects_secrets() {
        let cmd = "docker run -d --name {{app_name}} -e DB_PASS={{db_password}} -p {{port:-8080}}:80 {{image}}";
        let vars = extract_variables(cmd).unwrap();
        assert_eq!(vars.len(), 4);

        assert_eq!(vars[0].name, "app_name");
        assert_eq!(vars[0].default_value, None);
        assert!(!vars[0].is_secret);

        assert_eq!(vars[1].name, "db_password");
        assert_eq!(vars[1].default_value, None);
        assert!(vars[1].is_secret); // detected via "password"

        assert_eq!(vars[2].name, "port");
        assert_eq!(vars[2].default_value.as_deref(), Some("8080"));
        assert!(!vars[2].is_secret);

        assert_eq!(vars[3].name, "image");
        assert_eq!(vars[3].default_value, None);
    }

    #[test]
    fn renders_command_with_parameters_and_defaults() {
        let cmd = "curl -u {{user}}:{{token}} https://{{host:-api.example.com}}/v1/{{endpoint}}";
        let mut params = HashMap::new();
        params.insert("user".to_string(), "admin".to_string());
        params.insert("token".to_string(), "supersecret123".to_string());
        params.insert("endpoint".to_string(), "status".to_string());

        // Uses default host
        let rendered = render_command(cmd, &params).unwrap();
        assert_eq!(
            rendered,
            "curl -u admin:supersecret123 https://api.example.com/v1/status"
        );

        // Overrides default host
        params.insert("host".to_string(), "custom.internal".to_string());
        let rendered2 = render_command(cmd, &params).unwrap();
        assert_eq!(
            rendered2,
            "curl -u admin:supersecret123 https://custom.internal/v1/status"
        );

        // Fails when required variable is missing
        params.remove("token");
        assert!(matches!(
            render_command(cmd, &params),
            Err(SnippetError::MissingVariable(v)) if v == "token"
        ));
    }

    #[test]
    fn redacts_secrets_in_preview() {
        let snippet = Snippet::new(
            "Deploy App",
            "deploy.sh --key {{api_key}} --env {{environment:-production}}",
            None,
            &[],
        )
        .unwrap();

        let mut params = HashMap::new();
        params.insert("api_key".to_string(), "sk_live_99887766".to_string());

        let preview = snippet.render_preview(&params).unwrap();
        assert_eq!(preview, "deploy.sh --key [REDACTED] --env production");

        let executable = snippet.render(&params).unwrap();
        assert_eq!(
            executable,
            "deploy.sh --key sk_live_99887766 --env production"
        );
    }

    #[test]
    fn escapes_posix_shell_arguments_safely() {
        assert_eq!(shell_escape(""), "''");
        assert_eq!(
            shell_escape("simple-name_123.tar.gz"),
            "simple-name_123.tar.gz"
        );
        assert_eq!(shell_escape("Hello World!"), "'Hello World!'");
        assert_eq!(
            shell_escape("user's password; rm -rf /"),
            "'user'\\''s password; rm -rf /'"
        );
    }

    #[test]
    fn json_export_and_import_roundtrip() {
        let snippets = vec![
            Snippet::new(
                "Health Check",
                "curl -f http://localhost:{{port:-3000}}/health",
                Some("Checks service health".into()),
                &["monitoring".into()],
            )
            .unwrap(),
            Snippet::new(
                "Backup Database",
                "pg_dump -U {{db_user}} {{db_name}} > {{backup_file}}",
                None,
                &["database".into(), "backup".into()],
            )
            .unwrap(),
        ];

        let json = export_snippets_json(&snippets).unwrap();
        let parsed = parse_snippets_json(&json).unwrap();
        assert_eq!(parsed, snippets);
    }
}
