use crate::AppState;

#[derive(Debug)]
pub enum ConfigTxError {
    Conflict { current_version: String },
    Validation(String),
    Internal(String),
}

/// Compute a stable content hash for optimistic concurrency checks.
pub fn sha256_hex(content: &str) -> String {
    use std::hash::Hasher;

    let mut hasher = std::hash::DefaultHasher::new();
    hasher.write(content.as_bytes());
    format!("{:016x}-{}", hasher.finish(), content.len())
}

/// Read the current config file and return `(contents, version_hash)`.
pub fn read_config_versioned(state: &AppState) -> Result<(String, String), String> {
    let config_path = state
        .config_path
        .lock()
        .map_err(|e| format!("Failed to lock config path: {e}"))?
        .clone();
    let contents =
        std::fs::read_to_string(&config_path).map_err(|e| format!("Failed to read config: {e}"))?;
    let version = sha256_hex(&contents);
    Ok((contents, version))
}

fn config_path(state: &AppState) -> Result<String, ConfigTxError> {
    state
        .config_path
        .lock()
        .map_err(|e| ConfigTxError::Internal(format!("Failed to lock config path: {e}")))
        .map(|path| path.clone())
}

fn write_yaml_atomically(config_path: &str, yaml: &str) -> Result<(), ConfigTxError> {
    let config_path = std::path::Path::new(config_path);
    let dir = config_path.parent().unwrap_or(std::path::Path::new("."));
    let tmp_name = format!(
        ".config.yaml.tmp.{}.{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    );
    let tmp_path = dir.join(tmp_name);

    std::fs::write(&tmp_path, yaml).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        ConfigTxError::Internal(format!("Failed to write temp file: {e}"))
    })?;

    std::fs::rename(&tmp_path, config_path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        ConfigTxError::Internal(format!("Failed to rename config file: {e}"))
    })?;

    Ok(())
}

pub fn apply_runtime_config(
    state: &AppState,
    runtime_config: prism_core::config::Config,
) -> Result<(), ConfigTxError> {
    state
        .auth_runtime
        .sync_with_config(&runtime_config)
        .map_err(|e| ConfigTxError::Internal(format!("Failed to sync auth runtime: {e}")))?;
    state
        .router
        .set_oauth_states(state.auth_runtime.oauth_snapshot());
    state.router.update_from_config(&runtime_config);
    state
        .catalog
        .update_from_credentials(&state.router.credential_map());
    state.rate_limiter.update_config(&runtime_config.rate_limit);
    state
        .cost_calculator
        .update_prices(&runtime_config.model_prices);
    state.http_client_pool.clear();
    state.config.store(std::sync::Arc::new(runtime_config));
    Ok(())
}

fn ensure_expected_version(
    contents: &str,
    expected_version: Option<&str>,
) -> Result<(), ConfigTxError> {
    if let Some(expected) = expected_version {
        let current = sha256_hex(contents);
        if current != expected {
            return Err(ConfigTxError::Conflict {
                current_version: current,
            });
        }
    }
    Ok(())
}

pub async fn update_config_file_public(
    state: &AppState,
    mutate: impl FnOnce(&mut prism_core::config::Config),
) -> Result<String, String> {
    update_config_versioned(state, None, mutate)
        .await
        .map_err(|e| match e {
            ConfigTxError::Conflict { .. } => "conflict".to_string(),
            ConfigTxError::Validation(msg) | ConfigTxError::Internal(msg) => msg,
        })
}

/// Read current config from disk, mutate the raw YAML-backed model, persist atomically,
/// then rebuild runtime state from the written config.
pub async fn update_config_versioned(
    state: &AppState,
    expected_version: Option<&str>,
    mutate: impl FnOnce(&mut prism_core::config::Config),
) -> Result<String, ConfigTxError> {
    let path = config_path(state)?;
    let contents = std::fs::read_to_string(&path)
        .map_err(|e| ConfigTxError::Internal(format!("Failed to read config: {e}")))?;

    ensure_expected_version(&contents, expected_version)?;

    let mut raw_config = prism_core::config::Config::from_yaml_raw(&contents)
        .map_err(|e| ConfigTxError::Internal(format!("Failed to parse config: {e}")))?;
    mutate(&mut raw_config);

    let yaml = raw_config
        .to_yaml()
        .map_err(|e| ConfigTxError::Internal(format!("Failed to serialize config: {e}")))?;
    let runtime_config = prism_core::config::Config::load_from_str(&yaml)
        .map_err(|e| ConfigTxError::Validation(format!("Failed to load runtime config: {e}")))?;

    write_yaml_atomically(&path, &yaml)?;
    apply_runtime_config(state, runtime_config)?;

    Ok(sha256_hex(&yaml))
}

/// Validate a full YAML document, persist it atomically, then rebuild runtime state.
pub async fn apply_yaml_versioned(
    state: &AppState,
    yaml: &str,
    expected_version: Option<&str>,
) -> Result<String, ConfigTxError> {
    let runtime_config = prism_core::config::Config::load_from_str(yaml)
        .map_err(|e| ConfigTxError::Validation(e.to_string()))?;
    let path = config_path(state)?;

    if expected_version.is_some() {
        let contents = std::fs::read_to_string(&path)
            .map_err(|e| ConfigTxError::Internal(format!("Failed to read config: {e}")))?;
        ensure_expected_version(&contents, expected_version)?;
    }

    write_yaml_atomically(&path, yaml)?;
    apply_runtime_config(state, runtime_config)?;

    Ok(sha256_hex(yaml))
}

pub async fn reload_config_from_disk(state: &AppState) -> Result<(), ConfigTxError> {
    let path = config_path(state)?;
    let runtime_config = prism_core::config::Config::load(&path)
        .map_err(|e| ConfigTxError::Validation(e.to_string()))?;
    apply_runtime_config(state, runtime_config)
}
