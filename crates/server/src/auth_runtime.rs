use axum::http::StatusCode;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration, Utc};
use dashmap::DashMap;
use prism_core::auth_profile::{AuthProfileEntry, OAuthTokenState, SharedOAuthTokenState};
use prism_core::config::Config;
use prism_core::error::ProxyError;
use prism_core::provider::AuthRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

const CODEX_AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
const CODEX_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const AUTH_STORE_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct PendingCodexOauthSession {
    pub provider: String,
    pub profile_id: String,
    pub code_verifier: String,
    pub redirect_uri: String,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CodexOAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: Option<String>,
    pub expires_at: Option<chrono::DateTime<Utc>>,
    pub account_id: Option<String>,
    pub email: Option<String>,
    pub last_refresh: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedOAuthProfileState {
    provider: String,
    profile_id: String,
    access_token: String,
    refresh_token: String,
    id_token: Option<String>,
    expires_at: Option<String>,
    account_id: Option<String>,
    email: Option<String>,
    last_refresh: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RuntimeAuthStoreFile {
    version: u32,
    oauth_profiles: Vec<PersistedOAuthProfileState>,
}

pub struct AuthRuntimeManager {
    refresh_skew_seconds: i64,
    codex_auth_url: String,
    codex_token_url: String,
    codex_client_id: String,
    store_path: RwLock<Option<PathBuf>>,
    persist_lock: Mutex<()>,
    oauth_profiles: DashMap<String, SharedOAuthTokenState>,
}

impl Default for AuthRuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthRuntimeManager {
    pub fn new() -> Self {
        let codex_auth_url =
            std::env::var("PRISM_CODEX_AUTH_URL").unwrap_or_else(|_| CODEX_AUTH_URL.to_string());
        let codex_token_url =
            std::env::var("PRISM_CODEX_TOKEN_URL").unwrap_or_else(|_| CODEX_TOKEN_URL.to_string());
        let codex_client_id =
            std::env::var("PRISM_CODEX_CLIENT_ID").unwrap_or_else(|_| CODEX_CLIENT_ID.to_string());
        Self {
            refresh_skew_seconds: 120,
            codex_auth_url,
            codex_token_url,
            codex_client_id,
            store_path: RwLock::new(None),
            persist_lock: Mutex::new(()),
            oauth_profiles: DashMap::new(),
        }
    }

    pub fn with_codex_endpoints(auth_url: String, token_url: String, client_id: String) -> Self {
        Self {
            refresh_skew_seconds: 120,
            codex_auth_url: auth_url,
            codex_token_url: token_url,
            codex_client_id: client_id,
            store_path: RwLock::new(None),
            persist_lock: Mutex::new(()),
            oauth_profiles: DashMap::new(),
        }
    }

    pub fn initialize(&self, config_path: &str, config: &Config) -> Result<(), String> {
        {
            let mut guard = self
                .store_path
                .write()
                .map_err(|e| format!("auth runtime store path lock poisoned: {e}"))?;
            *guard = Some(Self::store_path_for_config(config_path));
        }
        self.load_store_file()?;
        self.sync_with_config(config)
    }

    pub fn sync_with_config(&self, config: &Config) -> Result<(), String> {
        let mut valid_keys = HashSet::new();
        let mut dirty = false;

        for entry in &config.providers {
            for profile in &entry.auth_profiles {
                if !profile.mode.is_managed() {
                    continue;
                }
                let key = Self::profile_key(&entry.name, &profile.id);
                valid_keys.insert(key.clone());
                if !self.oauth_profiles.contains_key(&key)
                    && let Some(state) = OAuthTokenState::from_profile(profile)
                {
                    self.oauth_profiles
                        .insert(key, Arc::new(RwLock::new(state)));
                    dirty = true;
                }
            }
        }

        let stale_keys = self
            .oauth_profiles
            .iter()
            .filter(|entry| !valid_keys.contains(entry.key()))
            .map(|entry| entry.key().clone())
            .collect::<Vec<_>>();
        for key in stale_keys {
            self.oauth_profiles.remove(&key);
            dirty = true;
        }

        if dirty {
            self.persist_store_file()?;
        }
        Ok(())
    }

    pub fn profile_key(provider: &str, profile_id: &str) -> String {
        format!("{provider}/{profile_id}")
    }

    pub fn oauth_snapshot(&self) -> HashMap<String, OAuthTokenState> {
        self.oauth_profiles
            .iter()
            .filter_map(|entry| {
                entry
                    .value()
                    .read()
                    .ok()
                    .map(|state| (entry.key().clone(), state.clone()))
            })
            .collect()
    }

    pub fn state_for_profile(
        &self,
        provider: &str,
        profile_id: &str,
    ) -> Result<Option<OAuthTokenState>, String> {
        let key = Self::profile_key(provider, profile_id);
        let Some(entry) = self.oauth_profiles.get(&key) else {
            return Ok(None);
        };
        let guard = entry
            .read()
            .map_err(|e| format!("oauth state lock poisoned: {e}"))?;
        Ok(Some(guard.clone()))
    }

    pub fn apply_runtime_state(
        &self,
        provider: &str,
        profile: &AuthProfileEntry,
    ) -> Result<AuthProfileEntry, String> {
        if !profile.mode.is_managed() {
            return Ok(profile.clone());
        }
        let Some(state) = self.state_for_profile(provider, &profile.id)? else {
            return Ok(profile.clone());
        };
        let mut hydrated = profile.clone();
        hydrated.access_token = Some(state.access_token);
        hydrated.refresh_token = Some(state.refresh_token);
        hydrated.id_token = state.id_token;
        hydrated.expires_at = state.expires_at.map(|dt| dt.to_rfc3339());
        hydrated.account_id = state.account_id;
        hydrated.email = state.email;
        hydrated.last_refresh = state.last_refresh.map(|dt| dt.to_rfc3339());
        Ok(hydrated)
    }

    pub fn ensure_profile_placeholder(
        &self,
        provider: &str,
        profile_id: &str,
    ) -> Result<(), String> {
        let key = Self::profile_key(provider, profile_id);
        self.oauth_profiles.entry(key).or_insert_with(|| {
            Arc::new(RwLock::new(OAuthTokenState {
                access_token: String::new(),
                refresh_token: String::new(),
                id_token: None,
                account_id: None,
                email: None,
                expires_at: None,
                last_refresh: None,
            }))
        });
        self.persist_store_file()
    }

    pub fn store_codex_tokens(
        &self,
        provider: &str,
        profile_id: &str,
        tokens: &CodexOAuthTokens,
    ) -> Result<(), String> {
        self.store_state(
            provider,
            profile_id,
            OAuthTokenState {
                access_token: tokens.access_token.clone(),
                refresh_token: tokens.refresh_token.clone(),
                id_token: tokens.id_token.clone(),
                account_id: tokens.account_id.clone(),
                email: tokens.email.clone(),
                expires_at: tokens.expires_at,
                last_refresh: Some(tokens.last_refresh),
            },
        )
    }

    pub fn store_anthropic_subscription_token(
        &self,
        provider: &str,
        profile_id: &str,
        token: &str,
    ) -> Result<(), String> {
        self.store_state(
            provider,
            profile_id,
            OAuthTokenState {
                access_token: token.to_string(),
                refresh_token: String::new(),
                id_token: None,
                account_id: None,
                email: None,
                expires_at: None,
                last_refresh: Some(Utc::now()),
            },
        )
    }

    pub fn store_state(
        &self,
        provider: &str,
        profile_id: &str,
        state: OAuthTokenState,
    ) -> Result<(), String> {
        let key = Self::profile_key(provider, profile_id);
        if let Some(existing) = self.oauth_profiles.get(&key) {
            let mut guard = existing
                .write()
                .map_err(|e| format!("oauth state lock poisoned: {e}"))?;
            *guard = state;
        } else {
            self.oauth_profiles
                .insert(key, Arc::new(RwLock::new(state)));
        }
        self.persist_store_file()
    }

    pub fn clear_profile_state(&self, provider: &str, profile_id: &str) -> Result<(), String> {
        let key = Self::profile_key(provider, profile_id);
        self.oauth_profiles.remove(&key);
        self.persist_store_file()
    }

    pub async fn prepare_auth(
        &self,
        state: &crate::AppState,
        auth: &AuthRecord,
    ) -> Result<(), ProxyError> {
        if !auth.auth_mode.supports_refresh() {
            return Ok(());
        }
        let Some(shared) = auth.oauth_state.clone() else {
            return Ok(());
        };
        let skip_refresh = {
            let guard = shared
                .read()
                .map_err(|e| ProxyError::Internal(format!("oauth state lock poisoned: {e}")))?;
            guard.refresh_token.is_empty() || !guard.expires_soon(self.refresh_skew_seconds)
        };
        if skip_refresh {
            return Ok(());
        }

        let global_proxy = state.config.load().proxy_url.clone();
        let refreshed = self
            .refresh_codex_tokens(
                &state.http_client_pool,
                global_proxy.as_deref(),
                shared.clone(),
            )
            .await?;
        {
            let mut guard = shared
                .write()
                .map_err(|e| ProxyError::Internal(format!("oauth state lock poisoned: {e}")))?;
            guard.access_token = refreshed.access_token.clone();
            guard.refresh_token = refreshed.refresh_token.clone();
            guard.id_token = refreshed.id_token.clone();
            guard.expires_at = refreshed.expires_at;
            guard.account_id = refreshed.account_id.clone();
            guard.email = refreshed.email.clone();
            guard.last_refresh = Some(refreshed.last_refresh);
        }
        self.store_codex_tokens(&auth.provider_name, &auth.auth_profile_id, &refreshed)
            .map_err(ProxyError::Internal)?;
        Ok(())
    }

    pub fn generate_pkce() -> Result<(String, String), ProxyError> {
        let random: [u8; 96] = rand::random();
        let verifier = URL_SAFE_NO_PAD.encode(random);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Ok((verifier, challenge))
    }

    pub fn build_codex_auth_url(&self, state: &str, challenge: &str, redirect_uri: &str) -> String {
        let params = [
            ("client_id", self.codex_client_id.as_str()),
            ("response_type", "code"),
            ("redirect_uri", redirect_uri),
            ("scope", "openid email profile offline_access"),
            ("state", state),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("prompt", "login"),
            ("id_token_add_organizations", "true"),
            ("codex_cli_simplified_flow", "true"),
        ];
        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        format!("{}?{query}", self.codex_auth_url)
    }

    pub async fn exchange_codex_code(
        &self,
        pool: &prism_core::proxy::HttpClientPool,
        global_proxy: Option<&str>,
        code: &str,
        redirect_uri: &str,
        code_verifier: &str,
    ) -> Result<CodexOAuthTokens, String> {
        let client = pool
            .get_or_create(None, global_proxy, 30, 30)
            .map_err(|e| format!("failed to build oauth client: {e}"))?;
        self.exchange_codex_form(
            &client,
            &[
                ("grant_type", "authorization_code"),
                ("client_id", self.codex_client_id.as_str()),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("code_verifier", code_verifier),
            ],
        )
        .await
    }

    pub async fn refresh_codex_tokens(
        &self,
        pool: &prism_core::proxy::HttpClientPool,
        global_proxy: Option<&str>,
        shared: SharedOAuthTokenState,
    ) -> Result<CodexOAuthTokens, ProxyError> {
        let refresh_token = {
            let guard = shared
                .read()
                .map_err(|e| ProxyError::Internal(format!("oauth state lock poisoned: {e}")))?;
            guard.refresh_token.clone()
        };
        if refresh_token.is_empty() {
            return Err(ProxyError::Auth(
                "codex oauth profile missing refresh token".to_string(),
            ));
        }

        let client = pool
            .get_or_create(None, global_proxy, 30, 30)
            .map_err(|e| ProxyError::Internal(format!("failed to build oauth client: {e}")))?;
        self.exchange_codex_form(
            &client,
            &[
                ("client_id", self.codex_client_id.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
                ("scope", "openid profile email"),
            ],
        )
        .await
        .map_err(ProxyError::Auth)
    }

    async fn exchange_codex_form(
        &self,
        client: &reqwest::Client,
        params: &[(&str, &str)],
    ) -> Result<CodexOAuthTokens, String> {
        #[derive(Deserialize)]
        struct TokenResp {
            access_token: String,
            refresh_token: Option<String>,
            id_token: Option<String>,
            expires_in: Option<i64>,
        }

        let form = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let resp = client
            .post(&self.codex_token_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .header("accept", "application/json")
            .body(form)
            .send()
            .await
            .map_err(|e| format!("oauth request failed: {e}"))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| format!("oauth read failed: {e}"))?;
        if status != StatusCode::OK {
            return Err(format!(
                "oauth exchange failed with status {}: {}",
                status, body
            ));
        }
        let token: TokenResp =
            serde_json::from_str(&body).map_err(|e| format!("invalid oauth response: {e}"))?;
        let (account_id, email) = token
            .id_token
            .as_deref()
            .and_then(parse_id_token_claims)
            .unwrap_or_default();
        Ok(CodexOAuthTokens {
            access_token: token.access_token,
            refresh_token: token.refresh_token.unwrap_or_default(),
            id_token: token.id_token,
            expires_at: token
                .expires_in
                .map(|secs| Utc::now() + Duration::seconds(secs)),
            account_id,
            email,
            last_refresh: Utc::now(),
        })
    }

    fn store_path_for_config(config_path: &str) -> PathBuf {
        let path = PathBuf::from(config_path);
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("config.yaml");
        path.with_file_name(format!("{file_name}.auth-runtime.json"))
    }

    fn load_store_file(&self) -> Result<(), String> {
        let Some(path) = self
            .store_path
            .read()
            .map_err(|e| format!("auth runtime store path lock poisoned: {e}"))?
            .clone()
        else {
            return Ok(());
        };

        self.oauth_profiles.clear();
        if !path.exists() {
            return Ok(());
        }

        let contents = std::fs::read_to_string(&path).map_err(|e| {
            format!(
                "failed to read auth runtime store '{}': {e}",
                path.display()
            )
        })?;
        let file: RuntimeAuthStoreFile = serde_json::from_str(&contents).map_err(|e| {
            format!(
                "failed to parse auth runtime store '{}': {e}",
                path.display()
            )
        })?;
        for profile in file.oauth_profiles {
            let key = Self::profile_key(&profile.provider, &profile.profile_id);
            self.oauth_profiles.insert(
                key,
                Arc::new(RwLock::new(OAuthTokenState {
                    access_token: profile.access_token,
                    refresh_token: profile.refresh_token,
                    id_token: profile.id_token,
                    account_id: profile.account_id,
                    email: profile.email,
                    expires_at: parse_timestamp(profile.expires_at.as_deref()),
                    last_refresh: parse_timestamp(profile.last_refresh.as_deref()),
                })),
            );
        }
        Ok(())
    }

    fn persist_store_file(&self) -> Result<(), String> {
        let Some(path) = self
            .store_path
            .read()
            .map_err(|e| format!("auth runtime store path lock poisoned: {e}"))?
            .clone()
        else {
            return Ok(());
        };

        let _guard = self
            .persist_lock
            .lock()
            .map_err(|e| format!("auth runtime persist lock poisoned: {e}"))?;

        let profiles = self
            .oauth_profiles
            .iter()
            .filter_map(|entry| {
                let state = entry.value().read().ok()?;
                let (provider, profile_id) = split_profile_key(entry.key())?;
                Some(PersistedOAuthProfileState {
                    provider: provider.to_string(),
                    profile_id: profile_id.to_string(),
                    access_token: state.access_token.clone(),
                    refresh_token: state.refresh_token.clone(),
                    id_token: state.id_token.clone(),
                    expires_at: state.expires_at.map(|dt| dt.to_rfc3339()),
                    account_id: state.account_id.clone(),
                    email: state.email.clone(),
                    last_refresh: state.last_refresh.map(|dt| dt.to_rfc3339()),
                })
            })
            .collect::<Vec<_>>();
        let file = RuntimeAuthStoreFile {
            version: AUTH_STORE_VERSION,
            oauth_profiles: profiles,
        };

        let bytes = serde_json::to_vec_pretty(&file)
            .map_err(|e| format!("failed to serialize auth runtime store: {e}"))?;
        write_atomic(&path, &bytes)
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create auth runtime store dir: {e}"))?;
    }
    let tmp_path = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp_path, bytes)
        .map_err(|e| format!("failed to write auth runtime temp file: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(&tmp_path, perms);
    }
    std::fs::rename(&tmp_path, path)
        .map_err(|e| format!("failed to rename auth runtime store: {e}"))?;
    Ok(())
}

fn split_profile_key(value: &str) -> Option<(&str, &str)> {
    let (provider, profile_id) = value.split_once('/')?;
    Some((provider, profile_id))
}

fn parse_timestamp(value: Option<&str>) -> Option<chrono::DateTime<Utc>> {
    value
        .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_id_token_claims(id_token: &str) -> Option<(Option<String>, Option<String>)> {
    #[derive(Deserialize)]
    struct Claims {
        email: Option<String>,
        account_id: Option<String>,
        sub: Option<String>,
    }

    let payload = id_token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: Claims = serde_json::from_slice(&bytes).ok()?;
    Some((claims.account_id.or(claims.sub), claims.email))
}
