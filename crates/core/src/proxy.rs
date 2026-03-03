use reqwest::{Client, Proxy};
use std::time::Duration;

/// Default User-Agent for upstream requests.
/// Can be overridden per-credential via the `headers` config field:
///
/// ```yaml
/// claude-api-key:
///   - api-key: "sk-..."
///     headers:
///       user-agent: "claude-code/2.1.62"
/// ```
const DEFAULT_USER_AGENT: &str = "prism/0.1.0";

/// Build an HTTP client with optional proxy support.
///
/// Proxy selection logic:
/// - `entry_proxy` is `Some(url)` with non-empty url → use per-provider proxy
/// - `entry_proxy` is `Some("")` → direct connection (no proxy)
/// - `entry_proxy` is `None` → fall back to `global_proxy`
/// - `global_proxy` is also `None` → direct connection
pub fn build_http_client(
    entry_proxy: Option<&str>,
    global_proxy: Option<&str>,
) -> Result<Client, anyhow::Error> {
    build_http_client_with_timeout(entry_proxy, global_proxy, 30, 300)
}

/// Build an HTTP client with explicit timeout settings.
pub fn build_http_client_with_timeout(
    entry_proxy: Option<&str>,
    global_proxy: Option<&str>,
    connect_timeout_secs: u64,
    request_timeout_secs: u64,
) -> Result<Client, anyhow::Error> {
    let proxy_url = match entry_proxy {
        Some("") => None,       // Explicit direct connection
        Some(url) => Some(url), // Per-provider proxy
        None => global_proxy,   // Fall back to global
    };

    let mut builder = Client::builder()
        .user_agent(DEFAULT_USER_AGENT)
        .connect_timeout(Duration::from_secs(connect_timeout_secs))
        .timeout(Duration::from_secs(request_timeout_secs));

    if let Some(url) = proxy_url {
        let proxy = Proxy::all(url)?; // reqwest auto-detects http/https/socks5
        builder = builder.proxy(proxy);
    } else {
        builder = builder.no_proxy(); // Don't read system proxy env vars
    }

    Ok(builder.build()?)
}

/// Resolve the effective proxy URL for a given entry.
pub fn resolve_proxy_url<'a>(
    entry_proxy: Option<&'a str>,
    global_proxy: Option<&'a str>,
) -> Option<&'a str> {
    match entry_proxy {
        Some("") => None,
        Some(url) => Some(url),
        None => global_proxy,
    }
}

/// Validate that a proxy URL is well-formed.
pub fn validate_proxy_url(url: &str) -> Result<(), anyhow::Error> {
    if url.is_empty() {
        return Ok(());
    }
    let parsed =
        url::Url::parse(url).map_err(|e| anyhow::anyhow!("invalid proxy URL '{url}': {e}"))?;
    match parsed.scheme() {
        "http" | "https" | "socks5" => Ok(()),
        scheme => Err(anyhow::anyhow!(
            "unsupported proxy scheme '{scheme}' in URL '{url}', expected http/https/socks5"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_proxy_url() {
        // Per-provider proxy takes precedence
        assert_eq!(
            resolve_proxy_url(Some("http://proxy:8080"), Some("socks5://global:1080")),
            Some("http://proxy:8080")
        );

        // Empty string means direct
        assert_eq!(
            resolve_proxy_url(Some(""), Some("socks5://global:1080")),
            None
        );

        // None falls back to global
        assert_eq!(
            resolve_proxy_url(None, Some("socks5://global:1080")),
            Some("socks5://global:1080")
        );

        // Both None means direct
        assert_eq!(resolve_proxy_url(None, None), None);
    }

    #[test]
    fn test_validate_proxy_url() {
        assert!(validate_proxy_url("http://proxy:8080").is_ok());
        assert!(validate_proxy_url("https://proxy:8080").is_ok());
        assert!(validate_proxy_url("socks5://user:pass@proxy:1080").is_ok());
        assert!(validate_proxy_url("").is_ok());
        assert!(validate_proxy_url("ftp://proxy:21").is_err());
        assert!(validate_proxy_url("not-a-url").is_err());
    }
}
