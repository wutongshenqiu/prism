/// Protected headers that cannot be set by profiles or custom headers.
/// Auth and transport headers are protocol invariants owned by executors.
static PROTECTED_HEADERS: &[&str] = &[
    "authorization",
    "x-api-key",
    "x-goog-api-key",
    "originator",
    "chatgpt-account-id",
    "version",
    "session_id",
    "content-type",
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
];

/// Check if a header name is protected (case-insensitive).
pub fn is_protected(name: &str) -> bool {
    let lower = name.to_lowercase();
    PROTECTED_HEADERS.contains(&lower.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protected_headers() {
        assert!(is_protected("authorization"));
        assert!(is_protected("Authorization"));
        assert!(is_protected("AUTHORIZATION"));
        assert!(is_protected("x-api-key"));
        assert!(is_protected("X-API-KEY"));
        assert!(is_protected("originator"));
        assert!(is_protected("chatgpt-account-id"));
        assert!(is_protected("version"));
        assert!(is_protected("session_id"));
        assert!(is_protected("content-type"));
        assert!(is_protected("host"));
        assert!(is_protected("content-length"));
        assert!(is_protected("transfer-encoding"));
        assert!(is_protected("connection"));
        assert!(is_protected("x-goog-api-key"));
    }

    #[test]
    fn test_non_protected_headers() {
        assert!(!is_protected("user-agent"));
        assert!(!is_protected("x-custom-header"));
        assert!(!is_protected("x-goog-api-client"));
        assert!(!is_protected("anthropic-beta"));
    }
}
