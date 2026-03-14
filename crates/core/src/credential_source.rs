use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CredentialSource {
    /// Static API key (default, current behavior).
    Static { key: String },
    /// Read credentials from a file on disk.
    AuthFile {
        path: PathBuf,
        #[serde(default)]
        format: AuthFileFormat,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthFileFormat {
    /// Plain text file containing just the API key.
    #[default]
    Plain,
    /// Claude CLI credentials format (~/.claude/credentials.json).
    ClaudeCli,
    /// JSON file with a `key` or `api_key` field.
    JsonKey,
}

impl CredentialSource {
    /// Resolve the credential source to an API key string.
    pub fn resolve(&self) -> Result<String, String> {
        match self {
            CredentialSource::Static { key } => Ok(key.clone()),
            CredentialSource::AuthFile { path, format } => {
                let content = std::fs::read_to_string(path)
                    .map_err(|e| format!("failed to read auth file {:?}: {}", path, e))?;
                let content = content.trim().to_string();
                match format {
                    AuthFileFormat::Plain => Ok(content),
                    AuthFileFormat::ClaudeCli => {
                        let val: serde_json::Value =
                            serde_json::from_str(&content).map_err(|e| {
                                format!("failed to parse Claude CLI credentials: {}", e)
                            })?;
                        val.get("token")
                            .or(val.get("api_key"))
                            .or(val.get("key"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .ok_or_else(|| {
                                "no token/api_key/key field found in Claude CLI credentials"
                                    .to_string()
                            })
                    }
                    AuthFileFormat::JsonKey => {
                        let val: serde_json::Value = serde_json::from_str(&content)
                            .map_err(|e| format!("failed to parse JSON key file: {}", e))?;
                        val.get("api_key")
                            .or(val.get("key"))
                            .or(val.get("token"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .ok_or_else(|| {
                                "no api_key/key/token field found in JSON key file".to_string()
                            })
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_static_resolve() {
        let source = CredentialSource::Static {
            key: "sk-ant-test-key".to_string(),
        };
        assert_eq!(source.resolve().unwrap(), "sk-ant-test-key");
    }

    #[test]
    fn test_plain_auth_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api-key.txt");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "  sk-file-key-123  ").unwrap();

        let source = CredentialSource::AuthFile {
            path,
            format: AuthFileFormat::Plain,
        };
        assert_eq!(source.resolve().unwrap(), "sk-file-key-123");
    }

    #[test]
    fn test_json_key_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("creds.json");
        std::fs::write(&path, r#"{"api_key": "sk-json-key-456"}"#).unwrap();

        let source = CredentialSource::AuthFile {
            path,
            format: AuthFileFormat::JsonKey,
        };
        assert_eq!(source.resolve().unwrap(), "sk-json-key-456");
    }

    #[test]
    fn test_json_key_file_key_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("creds.json");
        std::fs::write(&path, r#"{"key": "sk-key-field-789"}"#).unwrap();

        let source = CredentialSource::AuthFile {
            path,
            format: AuthFileFormat::JsonKey,
        };
        assert_eq!(source.resolve().unwrap(), "sk-key-field-789");
    }

    #[test]
    fn test_claude_cli_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        std::fs::write(&path, r#"{"token": "sk-ant-claude-cli-token"}"#).unwrap();

        let source = CredentialSource::AuthFile {
            path,
            format: AuthFileFormat::ClaudeCli,
        };
        assert_eq!(source.resolve().unwrap(), "sk-ant-claude-cli-token");
    }

    #[test]
    fn test_claude_cli_format_api_key_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        std::fs::write(&path, r#"{"api_key": "sk-ant-api-key"}"#).unwrap();

        let source = CredentialSource::AuthFile {
            path,
            format: AuthFileFormat::ClaudeCli,
        };
        assert_eq!(source.resolve().unwrap(), "sk-ant-api-key");
    }

    #[test]
    fn test_auth_file_not_found() {
        let source = CredentialSource::AuthFile {
            path: PathBuf::from("/nonexistent/path/credentials.json"),
            format: AuthFileFormat::Plain,
        };
        let result = source.resolve();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to read auth file"));
    }

    #[test]
    fn test_json_key_file_missing_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, r#"{"other_field": "value"}"#).unwrap();

        let source = CredentialSource::AuthFile {
            path,
            format: AuthFileFormat::JsonKey,
        };
        let result = source.resolve();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("no api_key/key/token field found")
        );
    }

    #[test]
    fn test_claude_cli_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not json").unwrap();

        let source = CredentialSource::AuthFile {
            path,
            format: AuthFileFormat::ClaudeCli,
        };
        let result = source.resolve();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("failed to parse Claude CLI credentials")
        );
    }

    #[test]
    fn test_serde_round_trip_static() {
        let source = CredentialSource::Static {
            key: "test-key".to_string(),
        };
        let json = serde_json::to_string(&source).unwrap();
        let deserialized: CredentialSource = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.resolve().unwrap(), "test-key");
    }

    #[test]
    fn test_serde_round_trip_auth_file() {
        let source = CredentialSource::AuthFile {
            path: PathBuf::from("/tmp/test.txt"),
            format: AuthFileFormat::ClaudeCli,
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("auth-file"));
        assert!(json.contains("claude-cli"));
        let deserialized: CredentialSource = serde_json::from_str(&json).unwrap();
        match deserialized {
            CredentialSource::AuthFile { path, format } => {
                assert_eq!(path, PathBuf::from("/tmp/test.txt"));
                assert!(matches!(format, AuthFileFormat::ClaudeCli));
            }
            _ => panic!("expected AuthFile variant"),
        }
    }
}
