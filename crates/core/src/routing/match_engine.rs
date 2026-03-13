use super::config::{RouteMatch, RouteRule, RoutingConfig};
use super::types::RouteRequestFeatures;
use crate::glob::glob_match;
use crate::routing::config::RouteProfile;

/// Find the best matching rule for the given request features.
/// Rules are ranked by specificity, then explicit priority, then declaration order.
/// Returns `None` if no rule matches.
pub fn match_rule<'a>(
    features: &RouteRequestFeatures,
    rules: &'a [RouteRule],
) -> Option<&'a RouteRule> {
    let mut best: Option<(usize, i32, i32)> = None; // (index, specificity, priority)

    for (i, rule) in rules.iter().enumerate() {
        if !matches_rule(features, &rule.match_conditions) {
            continue;
        }
        let specificity = compute_specificity(features, &rule.match_conditions);
        let priority = rule.priority.unwrap_or(0);

        if let Some((_, best_spec, best_pri)) = best {
            if specificity > best_spec || (specificity == best_spec && priority > best_pri) {
                best = Some((i, specificity, priority));
            }
            // On full tie, earlier declaration wins (keep current best)
        } else {
            best = Some((i, specificity, priority));
        }
    }

    best.map(|(i, _, _)| &rules[i])
}

/// Resolve the effective profile for the given request features.
/// First tries rule matching; falls back to the default profile.
pub fn resolve_profile<'a>(
    features: &RouteRequestFeatures,
    config: &'a RoutingConfig,
) -> (&'a str, &'a RouteProfile) {
    if let Some(rule) =
        match_rule(features, &config.rules).filter(|r| config.profiles.contains_key(&r.use_profile))
    {
        let profile = &config.profiles[&rule.use_profile];
        return (&rule.use_profile, profile);
    }
    let profile = config
        .profiles
        .get(&config.default_profile)
        .expect("default profile must exist (validated at config load)");
    (&config.default_profile, profile)
}

/// Check whether a rule's match conditions are satisfied by the features.
fn matches_rule(features: &RouteRequestFeatures, cond: &RouteMatch) -> bool {
    // Model match
    if !cond.models.is_empty()
        && !cond
            .models
            .iter()
            .any(|p| glob_match(p, &features.requested_model))
    {
        return false;
    }

    // Tenant match
    if !cond.tenants.is_empty() {
        match &features.tenant_id {
            Some(tid) => {
                if !cond.tenants.iter().any(|p| glob_match(p, tid)) {
                    return false;
                }
            }
            None => return false,
        }
    }

    // Endpoint match
    if !cond.endpoints.is_empty() {
        let ep_str = serde_json::to_value(features.endpoint)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        if !cond.endpoints.iter().any(|e| e == ep_str.as_str()) {
            return false;
        }
    }

    // Region match
    if !cond.regions.is_empty() {
        match &features.region {
            Some(r) => {
                if !cond.regions.iter().any(|p| glob_match(p, r)) {
                    return false;
                }
            }
            None => return false,
        }
    }

    // Stream match
    if cond.stream.is_some_and(|stream| features.stream != stream) {
        return false;
    }

    // Header match
    for (key, patterns) in &cond.headers {
        match features.headers.get(key) {
            Some(val) => {
                if !patterns.iter().any(|p| glob_match(p, val)) {
                    return false;
                }
            }
            None => return false,
        }
    }

    true
}

/// Compute specificity score for a matching rule.
/// Higher score = more specific match.
fn compute_specificity(features: &RouteRequestFeatures, cond: &RouteMatch) -> i32 {
    let mut score = 0i32;

    // Each non-empty dimension adds base points
    if !cond.models.is_empty() {
        // Exact model match > glob match
        if cond.models.iter().any(|p| p == &features.requested_model) {
            score += 10; // exact
        } else {
            score += 5; // glob
        }
    }
    if let Some(tid) = features
        .tenant_id
        .as_ref()
        .filter(|_| !cond.tenants.is_empty())
    {
        if cond.tenants.iter().any(|p| p == tid) {
            score += 10;
        } else {
            score += 5;
        }
    }
    if !cond.endpoints.is_empty() {
        score += 10; // endpoints are always exact
    }
    if let Some(r) = features
        .region
        .as_ref()
        .filter(|_| !cond.regions.is_empty())
    {
        if cond.regions.iter().any(|p| p == r) {
            score += 10;
        } else {
            score += 5;
        }
    }
    if cond.stream.is_some() {
        score += 5;
    }
    // Each header constraint adds points
    score += (cond.headers.len() as i32) * 5;

    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Format;
    use crate::routing::types::RouteEndpoint;
    use std::collections::BTreeMap;

    fn features(model: &str) -> RouteRequestFeatures {
        RouteRequestFeatures {
            requested_model: model.to_string(),
            endpoint: RouteEndpoint::ChatCompletions,
            source_format: Format::OpenAI,
            tenant_id: None,
            api_key_id: None,
            region: None,
            stream: false,
            headers: BTreeMap::new(),
        }
    }

    fn rule(name: &str, models: &[&str], profile: &str) -> RouteRule {
        RouteRule {
            name: name.to_string(),
            priority: None,
            match_conditions: RouteMatch {
                models: models.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            },
            use_profile: profile.to_string(),
        }
    }

    #[test]
    fn test_no_rules_returns_none() {
        let f = features("gpt-4");
        assert!(match_rule(&f, &[]).is_none());
    }

    #[test]
    fn test_exact_model_match() {
        let rules = vec![rule("r1", &["gpt-4"], "stable")];
        let f = features("gpt-4");
        let matched = match_rule(&f, &rules).unwrap();
        assert_eq!(matched.name, "r1");
    }

    #[test]
    fn test_glob_model_match() {
        let rules = vec![rule("r1", &["gpt-*"], "stable")];
        let f = features("gpt-4");
        let matched = match_rule(&f, &rules).unwrap();
        assert_eq!(matched.name, "r1");
    }

    #[test]
    fn test_no_match() {
        let rules = vec![rule("r1", &["claude-*"], "stable")];
        let f = features("gpt-4");
        assert!(match_rule(&f, &rules).is_none());
    }

    #[test]
    fn test_exact_model_beats_glob() {
        let rules = vec![
            rule("glob", &["gpt-*"], "balanced"),
            rule("exact", &["gpt-4"], "stable"),
        ];
        let f = features("gpt-4");
        let matched = match_rule(&f, &rules).unwrap();
        assert_eq!(matched.name, "exact");
    }

    #[test]
    fn test_more_dimensions_beats_fewer() {
        let r1 = RouteRule {
            name: "model-only".to_string(),
            priority: None,
            match_conditions: RouteMatch {
                models: vec!["gpt-4".to_string()],
                ..Default::default()
            },
            use_profile: "balanced".to_string(),
        };
        let r2 = RouteRule {
            name: "model-and-tenant".to_string(),
            priority: None,
            match_conditions: RouteMatch {
                models: vec!["gpt-4".to_string()],
                tenants: vec!["enterprise-*".to_string()],
                ..Default::default()
            },
            use_profile: "stable".to_string(),
        };
        let mut f = features("gpt-4");
        f.tenant_id = Some("enterprise-acme".to_string());
        let rules = [r1, r2];
        let matched = match_rule(&f, &rules).unwrap();
        assert_eq!(matched.name, "model-and-tenant");
    }

    #[test]
    fn test_priority_overrides_specificity_tie() {
        let r1 = rule("low", &["gpt-4"], "balanced");
        let mut r2 = rule("high", &["gpt-4"], "stable");
        r2.priority = Some(10);
        let f = features("gpt-4");
        let rules = [r1, r2];
        let matched = match_rule(&f, &rules).unwrap();
        assert_eq!(matched.name, "high");
    }

    #[test]
    fn test_declaration_order_breaks_tie() {
        let r1 = rule("first", &["gpt-4"], "balanced");
        let r2 = rule("second", &["gpt-4"], "stable");
        let f = features("gpt-4");
        let rules = [r1, r2];
        let matched = match_rule(&f, &rules).unwrap();
        assert_eq!(matched.name, "first");
    }

    #[test]
    fn test_stream_filter() {
        let r = RouteRule {
            name: "streaming".to_string(),
            priority: None,
            match_conditions: RouteMatch {
                stream: Some(true),
                ..Default::default()
            },
            use_profile: "balanced".to_string(),
        };
        let mut f = features("gpt-4");
        assert!(match_rule(&f, std::slice::from_ref(&r)).is_none());
        f.stream = true;
        assert!(match_rule(&f, &[r]).is_some());
    }

    #[test]
    fn test_header_match() {
        let mut headers_cond = std::collections::HashMap::new();
        headers_cond.insert("x-priority".to_string(), vec!["high".to_string()]);
        let r = RouteRule {
            name: "priority".to_string(),
            priority: None,
            match_conditions: RouteMatch {
                headers: headers_cond,
                ..Default::default()
            },
            use_profile: "stable".to_string(),
        };
        let mut f = features("gpt-4");
        assert!(match_rule(&f, std::slice::from_ref(&r)).is_none());
        f.headers
            .insert("x-priority".to_string(), "high".to_string());
        assert!(match_rule(&f, &[r]).is_some());
    }

    #[test]
    fn test_resolve_profile_default_no_rules() {
        let config = RoutingConfig::default();
        let f = features("gpt-4");
        let (name, _profile) = resolve_profile(&f, &config);
        assert_eq!(name, "balanced");
    }

    #[test]
    fn test_resolve_profile_matched_rule() {
        let mut config = RoutingConfig::default();
        config.rules.push(RouteRule {
            name: "stable-for-claude".to_string(),
            priority: None,
            match_conditions: RouteMatch {
                models: vec!["claude-*".to_string()],
                ..Default::default()
            },
            use_profile: "stable".to_string(),
        });
        let f = features("claude-3-opus");
        let (name, _profile) = resolve_profile(&f, &config);
        assert_eq!(name, "stable");
    }

    #[test]
    fn test_tenant_required_when_specified() {
        let r = RouteRule {
            name: "enterprise".to_string(),
            priority: None,
            match_conditions: RouteMatch {
                tenants: vec!["ent-*".to_string()],
                ..Default::default()
            },
            use_profile: "stable".to_string(),
        };
        // No tenant_id -> no match
        let f = features("gpt-4");
        assert!(match_rule(&f, &[r]).is_none());
    }

    #[test]
    fn test_region_match() {
        let r = RouteRule {
            name: "us-only".to_string(),
            priority: None,
            match_conditions: RouteMatch {
                regions: vec!["us-*".to_string()],
                ..Default::default()
            },
            use_profile: "stable".to_string(),
        };
        let mut f = features("gpt-4");
        f.region = Some("eu-west-1".to_string());
        assert!(match_rule(&f, std::slice::from_ref(&r)).is_none());
        f.region = Some("us-east-1".to_string());
        assert!(match_rule(&f, &[r]).is_some());
    }

    #[test]
    fn test_endpoint_match() {
        let r = RouteRule {
            name: "messages-only".to_string(),
            priority: None,
            match_conditions: RouteMatch {
                endpoints: vec!["messages".to_string()],
                ..Default::default()
            },
            use_profile: "stable".to_string(),
        };
        let f = features("gpt-4"); // endpoint = ChatCompletions
        assert!(match_rule(&f, std::slice::from_ref(&r)).is_none());
        let mut f2 = features("gpt-4");
        f2.endpoint = RouteEndpoint::Messages;
        assert!(match_rule(&f2, &[r]).is_some());
    }
}
