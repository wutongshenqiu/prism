/// Simple glob pattern matching supporting `*` wildcards.
///
/// `*` matches zero or more characters. Multiple `*` are supported.
///
/// Examples:
/// - `"gemini-*"` matches `"gemini-2.5-pro"`
/// - `"*-preview"` matches `"gpt-4-preview"`
/// - `"*flash*"` matches `"gemini-2.0-flash-exp"`
/// - `"exact"` matches only `"exact"`
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();

    let mut px = 0; // pattern index
    let mut tx = 0; // text index
    let mut star_px = usize::MAX; // last '*' position in pattern
    let mut star_tx = 0; // text position at last '*' match

    while tx < text.len() {
        if px < pattern.len() && (pattern[px] == text[tx] || pattern[px] == b'?') {
            px += 1;
            tx += 1;
        } else if px < pattern.len() && pattern[px] == b'*' {
            star_px = px;
            star_tx = tx;
            px += 1; // try matching '*' with empty string first
        } else if star_px != usize::MAX {
            // Backtrack: make '*' match one more character
            star_tx += 1;
            tx = star_tx;
            px = star_px + 1;
        } else {
            return false;
        }
    }

    // Consume trailing '*'s in pattern
    while px < pattern.len() && pattern[px] == b'*' {
        px += 1;
    }

    px == pattern.len()
}

/// Look up a value in a HashMap by key, trying exact match first, then glob patterns.
/// Returns `None` if no match is found.
pub fn glob_lookup<'a, V>(
    map: &'a std::collections::HashMap<String, V>,
    key: &str,
) -> Option<&'a V> {
    if let Some(v) = map.get(key) {
        return Some(v);
    }
    for (pattern, v) in map {
        if glob_match(pattern, key) {
            return Some(v);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(glob_match("hello", "hello"));
        assert!(!glob_match("hello", "world"));
    }

    #[test]
    fn test_star_suffix() {
        assert!(glob_match("gemini-*", "gemini-2.5-pro"));
        assert!(glob_match("gemini-*", "gemini-"));
        assert!(!glob_match("gemini-*", "openai-gpt4"));
    }

    #[test]
    fn test_star_prefix() {
        assert!(glob_match("*-preview", "gpt-4-preview"));
        assert!(glob_match("*-preview", "-preview"));
        assert!(!glob_match("*-preview", "gpt-4-stable"));
    }

    #[test]
    fn test_star_middle() {
        assert!(glob_match("*flash*", "gemini-2.0-flash-exp"));
        assert!(glob_match("*flash*", "flash"));
        assert!(glob_match("*flash*", "xflashy"));
    }

    #[test]
    fn test_multiple_stars() {
        assert!(glob_match("*-*-*", "a-b-c"));
        assert!(glob_match("g*-*-pro", "gemini-2.5-pro"));
    }

    #[test]
    fn test_single_star() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn test_no_wildcard() {
        assert!(glob_match("exact", "exact"));
        assert!(!glob_match("exact", "exactx"));
        assert!(!glob_match("exact", "xexact"));
    }

    #[test]
    fn test_empty() {
        assert!(glob_match("", ""));
        assert!(!glob_match("", "x"));
        assert!(glob_match("*", ""));
    }
}
