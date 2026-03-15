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
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

const CODEX_AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
const CODEX_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CODEX_DEVICE_USER_CODE_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/usercode";
const CODEX_DEVICE_TOKEN_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/token";
const CODEX_DEVICE_VERIFICATION_URL: &str = "https://auth.openai.com/codex/device";
const CODEX_DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";
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
pub struct PendingCodexDeviceSession {
    pub provider: String,
    pub profile_id: String,
    pub device_auth_id: String,
    pub user_code: String,
    pub interval_secs: u64,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CodexDeviceStart {
    pub device_auth_id: String,
    pub user_code: String,
    pub verification_url: String,
    pub interval_secs: u64,
    pub expires_in_secs: u64,
}

#[derive(Debug, Clone)]
pub enum CodexDevicePollResult {
    Pending,
    Complete(CodexOAuthTokens),
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
    version: u32,
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

pub struct AuthRuntimeManager {
    refresh_skew_seconds: i64,
    codex_auth_url: String,
    codex_token_url: String,
    codex_client_id: String,
    codex_device_user_code_url: String,
    codex_device_token_url: String,
    codex_device_verification_url: String,
    env_codex_auth_file: Option<PathBuf>,
    config_path: RwLock<Option<PathBuf>>,
    storage_dir: RwLock<Option<PathBuf>>,
    configured_codex_auth_file: RwLock<Option<PathBuf>>,
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
        let codex_device_user_code_url = std::env::var("PRISM_CODEX_DEVICE_USER_CODE_URL")
            .unwrap_or_else(|_| CODEX_DEVICE_USER_CODE_URL.to_string());
        let codex_device_token_url = std::env::var("PRISM_CODEX_DEVICE_TOKEN_URL")
            .unwrap_or_else(|_| CODEX_DEVICE_TOKEN_URL.to_string());
        let codex_device_verification_url = std::env::var("PRISM_CODEX_DEVICE_VERIFICATION_URL")
            .unwrap_or_else(|_| CODEX_DEVICE_VERIFICATION_URL.to_string());
        let env_codex_auth_file = std::env::var_os("PRISM_CODEX_AUTH_FILE").map(PathBuf::from);
        Self {
            refresh_skew_seconds: 120,
            codex_auth_url,
            codex_token_url,
            codex_client_id,
            codex_device_user_code_url,
            codex_device_token_url,
            codex_device_verification_url,
            env_codex_auth_file,
            config_path: RwLock::new(None),
            storage_dir: RwLock::new(None),
            configured_codex_auth_file: RwLock::new(None),
            persist_lock: Mutex::new(()),
            oauth_profiles: DashMap::new(),
        }
    }

    pub fn with_codex_endpoints(auth_url: String, token_url: String, client_id: String) -> Self {
        Self::with_codex_runtime_endpoints(
            auth_url,
            token_url,
            client_id,
            CODEX_DEVICE_USER_CODE_URL.to_string(),
            CODEX_DEVICE_TOKEN_URL.to_string(),
            CODEX_DEVICE_VERIFICATION_URL.to_string(),
        )
    }

    pub fn with_codex_runtime_endpoints(
        auth_url: String,
        token_url: String,
        client_id: String,
        device_user_code_url: String,
        device_token_url: String,
        device_verification_url: String,
    ) -> Self {
        Self {
            refresh_skew_seconds: 120,
            codex_auth_url: auth_url,
            codex_token_url: token_url,
            codex_client_id: client_id,
            codex_device_user_code_url: device_user_code_url,
            codex_device_token_url: device_token_url,
            codex_device_verification_url: device_verification_url,
            env_codex_auth_file: None,
            config_path: RwLock::new(None),
            storage_dir: RwLock::new(None),
            configured_codex_auth_file: RwLock::new(None),
            persist_lock: Mutex::new(()),
            oauth_profiles: DashMap::new(),
        }
    }

    pub fn with_codex_auth_file(mut self, path: PathBuf) -> Self {
        self.env_codex_auth_file = Some(path);
        self
    }

    pub fn initialize(&self, config_path: &str, config: &Config) -> Result<(), String> {
        {
            let mut guard = self
                .config_path
                .write()
                .map_err(|e| format!("auth runtime config path lock poisoned: {e}"))?;
            *guard = Some(PathBuf::from(config_path));
        }
        self.sync_with_config(config)
    }

    pub fn sync_with_config(&self, config: &Config) -> Result<(), String> {
        self.refresh_runtime_paths(config)?;
        self.load_store_dir()?;

        let mut valid_keys = HashSet::new();
        let mut imported_states = Vec::new();

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
                        .insert(key, Arc::new(RwLock::new(state.clone())));
                    imported_states.push((entry.name.clone(), profile.id.clone(), state));
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
            if let Some((provider, profile_id)) = split_profile_key(&key) {
                self.remove_persisted_state(provider, profile_id)?;
            }
        }

        for (provider, profile_id, state) in imported_states {
            self.persist_state(&provider, &profile_id, &state)?;
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

    pub fn storage_dir(&self) -> Result<Option<PathBuf>, String> {
        self.storage_dir
            .read()
            .map_err(|e| format!("auth runtime storage dir lock poisoned: {e}"))
            .map(|value| value.clone())
    }

    pub fn codex_auth_file_path(&self) -> Result<Option<PathBuf>, String> {
        self.configured_codex_auth_file
            .read()
            .map_err(|e| format!("auth runtime auth file lock poisoned: {e}"))
            .map(|value| value.clone())
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
            *guard = state.clone();
        } else {
            self.oauth_profiles
                .insert(key, Arc::new(RwLock::new(state.clone())));
        }
        self.persist_state(provider, profile_id, &state)
    }

    pub fn clear_profile_state(&self, provider: &str, profile_id: &str) -> Result<(), String> {
        let key = Self::profile_key(provider, profile_id);
        self.oauth_profiles.remove(&key);
        self.remove_persisted_state(provider, profile_id)
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

        let auth_proxy = state.config.load().managed_auth.proxy_url.clone();
        let refreshed = self
            .refresh_codex_tokens(
                &state.http_client_pool,
                auth_proxy.as_deref(),
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

    pub async fn start_codex_device_flow(
        &self,
        pool: &prism_core::proxy::HttpClientPool,
        global_proxy: Option<&str>,
    ) -> Result<CodexDeviceStart, String> {
        #[derive(Serialize)]
        struct DeviceUserCodeRequest<'a> {
            client_id: &'a str,
        }

        #[derive(Deserialize)]
        struct DeviceUserCodeResponse {
            device_auth_id: Option<String>,
            user_code: Option<String>,
            usercode: Option<String>,
            interval: Option<Value>,
        }

        let client = pool
            .get_or_create(None, global_proxy, 30, 30)
            .map_err(|e| format!("failed to build oauth client: {e}"))?;
        let resp = client
            .post(&self.codex_device_user_code_url)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .json(&DeviceUserCodeRequest {
                client_id: self.codex_client_id.as_str(),
            })
            .send()
            .await
            .map_err(|e| format!("device auth request failed: {e}"))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| format!("device auth read failed: {e}"))?;
        if status != StatusCode::OK {
            return Err(format!(
                "device auth request failed with status {}: {}",
                status, body
            ));
        }

        let parsed: DeviceUserCodeResponse = serde_json::from_str(&body)
            .map_err(|e| format!("invalid device auth response: {e}"))?;
        let device_auth_id = parsed
            .device_auth_id
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "device auth response missing device_auth_id".to_string())?;
        let user_code = parsed
            .user_code
            .or(parsed.usercode)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "device auth response missing user_code".to_string())?;
        Ok(CodexDeviceStart {
            device_auth_id,
            user_code,
            verification_url: self.codex_device_verification_url.clone(),
            interval_secs: parse_device_interval(parsed.interval.as_ref()).unwrap_or(5),
            expires_in_secs: 15 * 60,
        })
    }

    pub async fn poll_codex_device_flow(
        &self,
        pool: &prism_core::proxy::HttpClientPool,
        global_proxy: Option<&str>,
        session: &PendingCodexDeviceSession,
    ) -> Result<CodexDevicePollResult, String> {
        #[derive(Serialize)]
        struct DeviceTokenRequest<'a> {
            device_auth_id: &'a str,
            user_code: &'a str,
        }

        #[derive(Deserialize)]
        struct DeviceTokenResponse {
            authorization_code: Option<String>,
            code_verifier: Option<String>,
        }

        let client = pool
            .get_or_create(None, global_proxy, 30, 30)
            .map_err(|e| format!("failed to build oauth client: {e}"))?;
        let resp = client
            .post(&self.codex_device_token_url)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .json(&DeviceTokenRequest {
                device_auth_id: session.device_auth_id.as_str(),
                user_code: session.user_code.as_str(),
            })
            .send()
            .await
            .map_err(|e| format!("device token polling failed: {e}"))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| format!("device token read failed: {e}"))?;
        if status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND {
            return Ok(CodexDevicePollResult::Pending);
        }
        if status != StatusCode::OK {
            return Err(format!(
                "device token polling failed with status {}: {}",
                status, body
            ));
        }

        let parsed: DeviceTokenResponse = serde_json::from_str(&body)
            .map_err(|e| format!("invalid device token response: {e}"))?;
        let authorization_code = parsed
            .authorization_code
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "device token response missing authorization_code".to_string())?;
        let code_verifier = parsed
            .code_verifier
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "device token response missing code_verifier".to_string())?;
        let tokens = self
            .exchange_codex_code(
                pool,
                global_proxy,
                &authorization_code,
                CODEX_DEVICE_REDIRECT_URI,
                &code_verifier,
            )
            .await?;
        Ok(CodexDevicePollResult::Complete(tokens))
    }

    pub fn load_codex_cli_tokens(
        &self,
        path_override: Option<&Path>,
    ) -> Result<CodexOAuthTokens, String> {
        #[derive(Deserialize)]
        struct CodexAuthFile {
            tokens: Option<CodexAuthTokens>,
            last_refresh: Option<String>,
        }

        #[derive(Deserialize)]
        struct CodexAuthTokens {
            access_token: Option<String>,
            refresh_token: Option<String>,
            id_token: Option<String>,
            account_id: Option<String>,
        }

        let path = path_override
            .map(PathBuf::from)
            .or_else(|| {
                self.configured_codex_auth_file
                    .read()
                    .ok()
                    .and_then(|value| value.clone())
            })
            .or_else(|| self.env_codex_auth_file.clone())
            .or_else(default_codex_auth_path)
            .ok_or_else(|| "unable to resolve ~/.codex/auth.json".to_string())?;
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read '{}': {e}", path.display()))?;
        let parsed: CodexAuthFile =
            serde_json::from_str(&raw).map_err(|e| format!("invalid auth.json: {e}"))?;
        let tokens = parsed
            .tokens
            .ok_or_else(|| "auth.json missing tokens object".to_string())?;
        let access_token = tokens
            .access_token
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "auth.json missing tokens.access_token".to_string())?;
        let refresh_token = tokens.refresh_token.unwrap_or_default();
        let id_token = tokens.id_token;
        let (account_id_from_id_token, email) = id_token
            .as_deref()
            .and_then(parse_id_token_claims)
            .unwrap_or_default();
        let last_refresh = parsed
            .last_refresh
            .as_deref()
            .and_then(|value| parse_timestamp(Some(value)))
            .unwrap_or_else(Utc::now);

        Ok(CodexOAuthTokens {
            access_token: access_token.clone(),
            refresh_token,
            id_token,
            expires_at: parse_exp_from_jwt(&access_token),
            account_id: tokens.account_id.or(account_id_from_id_token),
            email,
            last_refresh,
        })
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

    pub fn default_storage_dir_for_config(config_path: &Path) -> PathBuf {
        let file_name = config_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("config.yaml");
        config_path.with_file_name(format!("{file_name}.managed-auth.d"))
    }

    fn refresh_runtime_paths(&self, config: &Config) -> Result<(), String> {
        let config_path = self
            .config_path
            .read()
            .map_err(|e| format!("auth runtime config path lock poisoned: {e}"))?
            .clone()
            .ok_or_else(|| "auth runtime config path not initialized".to_string())?;
        let storage_dir = config
            .managed_auth
            .storage_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| Self::default_storage_dir_for_config(&config_path));
        let codex_auth_file = config
            .managed_auth
            .codex_auth_file
            .as_deref()
            .map(PathBuf::from)
            .or_else(|| self.env_codex_auth_file.clone())
            .or_else(default_codex_auth_path);
        let previous_storage_dir = self
            .storage_dir
            .read()
            .map_err(|e| format!("auth runtime storage dir lock poisoned: {e}"))?
            .clone();
        if previous_storage_dir.as_ref() != Some(&storage_dir) {
            migrate_storage_dir(previous_storage_dir.as_deref(), &storage_dir)?;
        }

        {
            let mut guard = self
                .storage_dir
                .write()
                .map_err(|e| format!("auth runtime storage dir lock poisoned: {e}"))?;
            *guard = Some(storage_dir);
        }
        {
            let mut guard = self
                .configured_codex_auth_file
                .write()
                .map_err(|e| format!("auth runtime auth file lock poisoned: {e}"))?;
            *guard = codex_auth_file;
        }
        Ok(())
    }

    fn load_store_dir(&self) -> Result<(), String> {
        let Some(dir) = self
            .storage_dir
            .read()
            .map_err(|e| format!("auth runtime storage dir lock poisoned: {e}"))?
            .clone()
        else {
            return Ok(());
        };

        self.oauth_profiles.clear();
        if !dir.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(&dir)
            .map_err(|e| format!("failed to read auth runtime dir '{}': {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("failed to inspect auth runtime dir: {e}"))?;
            let path = entry.path();
            if path.extension() != Some(OsStr::new("json")) {
                continue;
            }
            let contents = std::fs::read_to_string(&path).map_err(|e| {
                format!(
                    "failed to read auth runtime store '{}': {e}",
                    path.display()
                )
            })?;
            let profile: PersistedOAuthProfileState =
                serde_json::from_str(&contents).map_err(|e| {
                    format!(
                        "failed to parse auth runtime store '{}': {e}",
                        path.display()
                    )
                })?;
            if profile.version != AUTH_STORE_VERSION {
                return Err(format!(
                    "unsupported auth runtime store version {} in '{}'",
                    profile.version,
                    path.display()
                ));
            }
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

    fn persist_state(
        &self,
        provider: &str,
        profile_id: &str,
        state: &OAuthTokenState,
    ) -> Result<(), String> {
        let Some(dir) = self
            .storage_dir
            .read()
            .map_err(|e| format!("auth runtime storage dir lock poisoned: {e}"))?
            .clone()
        else {
            return Ok(());
        };

        let _guard = self
            .persist_lock
            .lock()
            .map_err(|e| format!("auth runtime persist lock poisoned: {e}"))?;
        ensure_secure_dir(&dir)?;
        let file = PersistedOAuthProfileState {
            version: AUTH_STORE_VERSION,
            provider: provider.to_string(),
            profile_id: profile_id.to_string(),
            access_token: state.access_token.clone(),
            refresh_token: state.refresh_token.clone(),
            id_token: state.id_token.clone(),
            expires_at: state.expires_at.map(|dt| dt.to_rfc3339()),
            account_id: state.account_id.clone(),
            email: state.email.clone(),
            last_refresh: state.last_refresh.map(|dt| dt.to_rfc3339()),
        };
        let bytes = serde_json::to_vec_pretty(&file)
            .map_err(|e| format!("failed to serialize auth runtime store: {e}"))?;
        let path = self.profile_store_path(&dir, provider, profile_id);
        write_atomic(&path, &bytes)
    }

    fn remove_persisted_state(&self, provider: &str, profile_id: &str) -> Result<(), String> {
        let Some(dir) = self
            .storage_dir
            .read()
            .map_err(|e| format!("auth runtime storage dir lock poisoned: {e}"))?
            .clone()
        else {
            return Ok(());
        };
        let _guard = self
            .persist_lock
            .lock()
            .map_err(|e| format!("auth runtime persist lock poisoned: {e}"))?;
        let path = self.profile_store_path(&dir, provider, profile_id);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                format!(
                    "failed to remove auth runtime store '{}': {e}",
                    path.display()
                )
            })?;
        }
        Ok(())
    }

    fn profile_store_path(&self, storage_dir: &Path, provider: &str, profile_id: &str) -> PathBuf {
        storage_dir.join(profile_store_file_name(provider, profile_id))
    }
}

fn ensure_secure_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|e| format!("failed to create auth runtime store dir: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o700);
        let _ = std::fs::set_permissions(path, perms);
    }
    Ok(())
}

fn migrate_storage_dir(previous: Option<&Path>, next: &Path) -> Result<(), String> {
    let Some(previous) = previous else {
        return Ok(());
    };
    if previous == next || !previous.exists() {
        return Ok(());
    }
    ensure_secure_dir(next)?;
    let entries = std::fs::read_dir(previous).map_err(|e| {
        format!(
            "failed to read previous auth runtime dir '{}': {e}",
            previous.display()
        )
    })?;
    for entry in entries {
        let entry =
            entry.map_err(|e| format!("failed to inspect previous auth runtime dir: {e}"))?;
        let source = entry.path();
        if source.extension() != Some(OsStr::new("json")) {
            continue;
        }
        let target = next.join(entry.file_name());
        if target.exists() {
            continue;
        }
        std::fs::copy(&source, &target).map_err(|e| {
            format!(
                "failed to migrate auth runtime store '{}' to '{}': {e}",
                source.display(),
                target.display()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&target, perms);
        }
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        ensure_secure_dir(parent)?;
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

fn profile_store_file_name(provider: &str, profile_id: &str) -> String {
    let key = AuthRuntimeManager::profile_key(provider, profile_id);
    let digest = URL_SAFE_NO_PAD.encode(Sha256::digest(key.as_bytes()));
    format!(
        "{}--{}--{}.json",
        sanitize_file_component(provider),
        sanitize_file_component(profile_id),
        &digest[..16]
    )
}

fn sanitize_file_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .take(48)
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "profile".to_string()
    } else {
        sanitized
    }
}

fn default_codex_auth_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).map(|home| {
        if home.ends_with("auth.json") {
            home
        } else {
            home.join(".codex").join("auth.json")
        }
    })
}

fn parse_timestamp(value: Option<&str>) -> Option<chrono::DateTime<Utc>> {
    value
        .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_device_interval(value: Option<&Value>) -> Option<u64> {
    match value {
        Some(Value::Number(number)) => number.as_u64(),
        Some(Value::String(text)) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
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

fn parse_exp_from_jwt(token: &str) -> Option<chrono::DateTime<Utc>> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    let exp = value.get("exp")?.as_i64()?;
    chrono::DateTime::<Utc>::from_timestamp(exp, 0)
}
