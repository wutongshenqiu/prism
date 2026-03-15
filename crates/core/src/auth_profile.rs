use crate::presentation::UpstreamPresentationConfig;
use crate::provider::Format;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthMode {
    #[default]
    ApiKey,
    BearerToken,
    OpenaiCodexOauth,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthHeaderKind {
    #[default]
    Auto,
    Bearer,
    XApiKey,
    XGoogApiKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct AuthProfileEntry {
    pub id: String,
    pub mode: AuthMode,
    pub header: AuthHeaderKind,
    pub secret: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub expires_at: Option<String>,
    pub account_id: Option<String>,
    pub email: Option<String>,
    pub last_refresh: Option<String>,
    pub headers: HashMap<String, String>,
    pub disabled: bool,
    pub weight: u32,
    pub region: Option<String>,
    pub prefix: Option<String>,
    pub upstream_presentation: UpstreamPresentationConfig,
}

impl Default for AuthProfileEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            mode: AuthMode::ApiKey,
            header: AuthHeaderKind::Auto,
            secret: None,
            access_token: None,
            refresh_token: None,
            id_token: None,
            expires_at: None,
            account_id: None,
            email: None,
            last_refresh: None,
            headers: HashMap::new(),
            disabled: false,
            weight: 1,
            region: None,
            prefix: None,
            upstream_presentation: UpstreamPresentationConfig::default(),
        }
    }
}

impl AuthProfileEntry {
    pub fn normalize(&mut self) {
        let headers = self
            .headers
            .drain()
            .map(|(k, v)| (k.to_lowercase(), v))
            .collect();
        self.headers = headers;
        if self.weight == 0 {
            self.weight = 1;
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("auth profile id must not be empty".to_string());
        }
        match self.mode {
            AuthMode::ApiKey | AuthMode::BearerToken => {
                let has_secret = self.secret.as_deref().is_some_and(|s| !s.trim().is_empty());
                if !has_secret {
                    return Err(format!(
                        "auth profile '{}' requires a non-empty secret",
                        self.id
                    ));
                }
            }
            AuthMode::OpenaiCodexOauth => {}
        }
        Ok(())
    }

    pub fn resolve_secrets(&mut self) -> Result<(), String> {
        fn resolve_optional(v: &mut Option<String>) -> Result<(), String> {
            if let Some(value) = v.clone() {
                *v = Some(crate::secret::resolve(&value).map_err(|e| e.to_string())?);
            }
            Ok(())
        }

        resolve_optional(&mut self.secret)?;
        resolve_optional(&mut self.access_token)?;
        resolve_optional(&mut self.refresh_token)?;
        resolve_optional(&mut self.id_token)?;
        Ok(())
    }

    pub fn resolved_header_kind(
        &self,
        format: Format,
        vertex: bool,
        base_url: Option<&str>,
    ) -> AuthHeaderKind {
        match self.header {
            AuthHeaderKind::Auto => match self.mode {
                AuthMode::BearerToken | AuthMode::OpenaiCodexOauth => AuthHeaderKind::Bearer,
                AuthMode::ApiKey => match format {
                    Format::OpenAI => AuthHeaderKind::Bearer,
                    Format::Gemini => {
                        if vertex {
                            AuthHeaderKind::Bearer
                        } else {
                            AuthHeaderKind::XGoogApiKey
                        }
                    }
                    Format::Claude => {
                        if base_url
                            .unwrap_or(format.default_base_url())
                            .contains("anthropic.com")
                        {
                            AuthHeaderKind::XApiKey
                        } else {
                            AuthHeaderKind::Bearer
                        }
                    }
                },
            },
            explicit => explicit,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OAuthTokenState {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: Option<String>,
    pub account_id: Option<String>,
    pub email: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_refresh: Option<DateTime<Utc>>,
}

impl OAuthTokenState {
    pub fn from_profile(profile: &AuthProfileEntry) -> Option<Self> {
        if profile.mode != AuthMode::OpenaiCodexOauth {
            return None;
        }
        Some(Self {
            access_token: profile.access_token.clone().unwrap_or_default(),
            refresh_token: profile.refresh_token.clone().unwrap_or_default(),
            id_token: profile.id_token.clone(),
            account_id: profile.account_id.clone(),
            email: profile.email.clone(),
            expires_at: profile
                .expires_at
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            last_refresh: profile
                .last_refresh
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
        })
    }

    pub fn expires_soon(&self, skew_seconds: i64) -> bool {
        match self.expires_at {
            Some(expires_at) => expires_at <= Utc::now() + chrono::Duration::seconds(skew_seconds),
            None => false,
        }
    }
}

pub type SharedOAuthTokenState = Arc<RwLock<OAuthTokenState>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolved_header_kind_defaults() {
        let profile = AuthProfileEntry {
            id: "p1".into(),
            mode: AuthMode::ApiKey,
            secret: Some("sk".into()),
            ..Default::default()
        };
        assert_eq!(
            profile.resolved_header_kind(Format::OpenAI, false, None),
            AuthHeaderKind::Bearer
        );
        assert_eq!(
            profile.resolved_header_kind(Format::Claude, false, Some("https://api.anthropic.com")),
            AuthHeaderKind::XApiKey
        );
        assert_eq!(
            profile.resolved_header_kind(Format::Claude, false, Some("https://proxy.example.com")),
            AuthHeaderKind::Bearer
        );
    }

    #[test]
    fn test_validate_oauth_profile() {
        let profile = AuthProfileEntry {
            id: "codex".into(),
            mode: AuthMode::OpenaiCodexOauth,
            disabled: false,
            ..Default::default()
        };
        assert!(profile.validate().is_ok());
    }
}
