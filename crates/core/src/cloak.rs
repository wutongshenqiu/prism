use rand::RngExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// Cloak configuration per Claude API key entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct CloakConfig {
    /// auto: cloak unless User-Agent starts with "claude-cli"
    /// always: always cloak
    /// never: never cloak
    pub mode: CloakMode,
    /// If true, replace user's system prompt entirely; if false, prepend cloak system prompt.
    pub strict_mode: bool,
    /// Words to obfuscate by inserting zero-width spaces.
    pub sensitive_words: Vec<String>,
    /// Whether to cache the generated user_id per API key.
    pub cache_user_id: bool,
}

impl Default for CloakConfig {
    fn default() -> Self {
        Self {
            mode: CloakMode::Never,
            strict_mode: false,
            sensitive_words: Vec::new(),
            cache_user_id: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CloakMode {
    Auto,
    Always,
    #[default]
    Never,
}

/// Cached user IDs per API key.
static USER_ID_CACHE: std::sync::LazyLock<Mutex<HashMap<String, String>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Claude Code system prompt snippet used for cloaking.
const CLOAK_SYSTEM_PROMPT: &str = "You are Claude Code, Anthropic's official CLI for Claude. \
You are an interactive agent specialized in software engineering tasks. \
You help users with coding, debugging, and software development.";

/// Determine whether cloaking should be applied.
pub fn should_cloak(cloak_cfg: &CloakConfig, user_agent: Option<&str>) -> bool {
    match cloak_cfg.mode {
        CloakMode::Always => true,
        CloakMode::Never => false,
        CloakMode::Auto => {
            // Don't cloak native Claude CLI clients
            !user_agent
                .map(|ua| ua.starts_with("claude-cli") || ua.starts_with("claude-code"))
                .unwrap_or(false)
        }
    }
}

/// Generate a fake user_id in the format: user_{64hex}_account__session_{uuid}
pub fn generate_user_id(api_key: &str, cache: bool) -> String {
    if cache {
        let mut map = USER_ID_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cached) = map.get(api_key) {
            return cached.clone();
        }
        let id = make_user_id();
        map.insert(api_key.to_string(), id.clone());
        id
    } else {
        make_user_id()
    }
}

fn make_user_id() -> String {
    let mut rng = rand::rng();
    let hex: String = (0..64)
        .map(|_| format!("{:x}", rng.random_range(0..16u8)))
        .collect();
    let session_uuid = uuid::Uuid::new_v4();
    format!("user_{hex}_account__session_{session_uuid}")
}

/// Apply cloaking to a Claude Messages API request body.
/// Injects system prompt, user_id, and obfuscates sensitive words.
pub fn apply_cloak(body: &mut serde_json::Value, cloak_cfg: &CloakConfig, api_key: &str) {
    let obj = match body.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    // 1. Inject/modify system prompt
    if cloak_cfg.strict_mode {
        obj.insert(
            "system".to_string(),
            serde_json::Value::String(CLOAK_SYSTEM_PROMPT.to_string()),
        );
    } else {
        // Prepend cloak system prompt to existing
        let existing = obj
            .get("system")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let combined = if existing.is_empty() {
            CLOAK_SYSTEM_PROMPT.to_string()
        } else {
            format!("{CLOAK_SYSTEM_PROMPT}\n\n{existing}")
        };
        obj.insert("system".to_string(), serde_json::Value::String(combined));
    }

    // 2. Inject metadata with fake user_id
    let user_id = generate_user_id(api_key, cloak_cfg.cache_user_id);
    let metadata = obj
        .entry("metadata")
        .or_insert_with(|| serde_json::json!({}));
    if let Some(meta_obj) = metadata.as_object_mut() {
        meta_obj.insert("user_id".to_string(), serde_json::Value::String(user_id));
    }

    // 3. Obfuscate sensitive words in messages
    if !cloak_cfg.sensitive_words.is_empty() {
        obfuscate_sensitive_words(body, &cloak_cfg.sensitive_words);
    }
}

/// Insert zero-width space after the first character of each sensitive word match.
fn obfuscate_sensitive_words(body: &mut serde_json::Value, words: &[String]) {
    if words.is_empty() {
        return;
    }

    // Build a single regex pattern for all words (case-insensitive)
    let pattern = words
        .iter()
        .map(|w| regex::escape(w))
        .collect::<Vec<_>>()
        .join("|");
    let re = match Regex::new(&format!("(?i)({pattern})")) {
        Ok(r) => r,
        Err(_) => return,
    };

    // Walk through all string values in messages
    if let Some(messages) = body.get_mut("messages") {
        obfuscate_in_value(messages, &re);
    }
    if let Some(system) = body.get_mut("system") {
        obfuscate_in_value(system, &re);
    }
}

fn obfuscate_in_value(value: &mut serde_json::Value, re: &Regex) {
    match value {
        serde_json::Value::String(s) => {
            *s = obfuscate_string(s, re);
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                obfuscate_in_value(item, re);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                // Only obfuscate text content, not structural keys
                if key == "text" || key == "content" {
                    obfuscate_in_value(val, re);
                }
            }
        }
        _ => {}
    }
}

fn obfuscate_string(s: &str, re: &Regex) -> String {
    re.replace_all(s, |caps: &regex::Captures| {
        let matched = &caps[0];
        let mut chars = matched.chars();
        match chars.next() {
            Some(first) => format!("{first}\u{200B}{}", chars.collect::<String>()),
            None => String::new(),
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_should_cloak_auto() {
        let cfg = CloakConfig {
            mode: CloakMode::Auto,
            ..Default::default()
        };
        assert!(!should_cloak(&cfg, Some("claude-cli/2.1.58")));
        assert!(should_cloak(&cfg, Some("python-requests/2.31.0")));
        assert!(should_cloak(&cfg, None));
    }

    #[test]
    fn test_should_cloak_always_never() {
        let always = CloakConfig {
            mode: CloakMode::Always,
            ..Default::default()
        };
        assert!(should_cloak(&always, Some("claude-cli/2.1.58")));

        let never = CloakConfig {
            mode: CloakMode::Never,
            ..Default::default()
        };
        assert!(!should_cloak(&never, None));
    }

    #[test]
    fn test_generate_user_id_format() {
        let id = generate_user_id("test-key", false);
        assert!(id.starts_with("user_"));
        assert!(id.contains("_account__session_"));
    }

    #[test]
    fn test_generate_user_id_caching() {
        let id1 = generate_user_id("cache-test-key", true);
        let id2 = generate_user_id("cache-test-key", true);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_apply_cloak_system_prompt() {
        let cfg = CloakConfig {
            mode: CloakMode::Always,
            strict_mode: false,
            ..Default::default()
        };
        let mut body = json!({
            "model": "claude-sonnet-4-20250514",
            "messages": [{"role": "user", "content": "hello"}],
            "system": "You are a helpful assistant."
        });
        apply_cloak(&mut body, &cfg, "test-key");
        let system = body["system"].as_str().unwrap();
        assert!(system.starts_with("You are Claude Code"));
        assert!(system.contains("You are a helpful assistant."));
    }

    #[test]
    fn test_apply_cloak_strict_mode() {
        let cfg = CloakConfig {
            mode: CloakMode::Always,
            strict_mode: true,
            ..Default::default()
        };
        let mut body = json!({
            "model": "claude-sonnet-4-20250514",
            "messages": [{"role": "user", "content": "hello"}],
            "system": "You are a helpful assistant."
        });
        apply_cloak(&mut body, &cfg, "test-key");
        let system = body["system"].as_str().unwrap();
        assert!(system.starts_with("You are Claude Code"));
        assert!(!system.contains("You are a helpful assistant."));
    }

    #[test]
    fn test_obfuscate_sensitive_words() {
        let cfg = CloakConfig {
            mode: CloakMode::Always,
            sensitive_words: vec!["API".into(), "proxy".into()],
            ..Default::default()
        };
        let mut body = json!({
            "model": "claude-sonnet-4-20250514",
            "messages": [{"role": "user", "content": "This API proxy is great"}]
        });
        apply_cloak(&mut body, &cfg, "test-key");
        let content = body["messages"][0]["content"].as_str().unwrap();
        // Should contain zero-width spaces
        assert!(content.contains('\u{200B}'));
        assert!(!content.contains("API"));
        assert!(!content.contains("proxy"));
    }

    #[test]
    fn test_user_id_in_metadata() {
        let cfg = CloakConfig {
            mode: CloakMode::Always,
            ..Default::default()
        };
        let mut body = json!({
            "model": "claude-sonnet-4-20250514",
            "messages": [{"role": "user", "content": "hello"}]
        });
        apply_cloak(&mut body, &cfg, "test-key");
        assert!(body["metadata"]["user_id"].is_string());
        let user_id = body["metadata"]["user_id"].as_str().unwrap();
        assert!(user_id.starts_with("user_"));
    }
}
