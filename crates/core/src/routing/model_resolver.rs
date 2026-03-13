use super::config::ModelResolution;
use super::types::ModelResolutionStep;
use crate::glob::glob_match;

/// Result of model resolution.
#[derive(Debug, Clone)]
pub struct ResolvedModel {
    /// Primary model followed by fallback models.
    pub model_chain: Vec<String>,
    /// If set, only these providers may serve the matched models.
    pub pinned_providers: Option<Vec<String>>,
    /// Trace of resolution steps applied.
    pub resolution_steps: Vec<ModelResolutionStep>,
}

/// Resolve a requested model name through the model resolution pipeline.
///
/// Resolution order (single pass each):
/// 1. Alias — exact match only, no chaining
/// 2. Rewrite — glob match, first match wins
/// 3. Fallback chain — glob match on resolved model
/// 4. Provider pin — glob match on resolved model
pub fn resolve_model(requested: &str, resolution: &ModelResolution) -> ResolvedModel {
    let mut steps = Vec::new();
    let mut model = requested.to_string();

    // 1. Alias (exact match only, single pass — no chaining)
    for alias in &resolution.aliases {
        if alias.from == model {
            steps.push(ModelResolutionStep::AliasResolved {
                from: model.clone(),
                to: alias.to.clone(),
            });
            model = alias.to.clone();
            break;
        }
    }

    // 2. Rewrite (glob match, first match wins)
    for rewrite in &resolution.rewrites {
        if glob_match(&rewrite.pattern, &model) {
            steps.push(ModelResolutionStep::RewriteApplied {
                from: model.clone(),
                to: rewrite.to.clone(),
                rule: rewrite.pattern.clone(),
            });
            model = rewrite.to.clone();
            break;
        }
    }

    // 3. Fallback chain (glob match, primary model is first)
    let mut model_chain = vec![model.clone()];
    for fb in &resolution.fallbacks {
        if glob_match(&fb.pattern, &model) {
            let fallbacks: Vec<String> = fb
                .to
                .iter()
                .filter(|m| **m != model) // Don't duplicate primary
                .cloned()
                .collect();
            if !fallbacks.is_empty() {
                steps.push(ModelResolutionStep::FallbackChainBuilt {
                    primary: model.clone(),
                    fallbacks: fallbacks.clone(),
                });
                model_chain.extend(fallbacks);
            }
            break;
        }
    }

    // 4. Provider pin (glob match)
    let mut pinned_providers = None;
    for pin in &resolution.provider_pins {
        if glob_match(&pin.pattern, &model) {
            steps.push(ModelResolutionStep::ProviderPinned {
                model: model.clone(),
                providers: pin.providers.clone(),
            });
            pinned_providers = Some(pin.providers.clone());
            break;
        }
    }

    ResolvedModel {
        model_chain,
        pinned_providers,
        resolution_steps: steps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::config::*;

    fn empty_resolution() -> ModelResolution {
        ModelResolution::default()
    }

    #[test]
    fn test_no_resolution_returns_original() {
        let r = resolve_model("gpt-4", &empty_resolution());
        assert_eq!(r.model_chain, vec!["gpt-4"]);
        assert!(r.pinned_providers.is_none());
        assert!(r.resolution_steps.is_empty());
    }

    #[test]
    fn test_alias_exact_match() {
        let res = ModelResolution {
            aliases: vec![ModelAlias {
                from: "gpt-5-default".to_string(),
                to: "gpt-5".to_string(),
            }],
            ..Default::default()
        };
        let r = resolve_model("gpt-5-default", &res);
        assert_eq!(r.model_chain, vec!["gpt-5"]);
        assert_eq!(r.resolution_steps.len(), 1);
        assert!(matches!(
            &r.resolution_steps[0],
            ModelResolutionStep::AliasResolved { from, to }
            if from == "gpt-5-default" && to == "gpt-5"
        ));
    }

    #[test]
    fn test_alias_no_chaining() {
        // alias A->B and B->C should not chain: requesting A yields B, not C
        let res = ModelResolution {
            aliases: vec![
                ModelAlias {
                    from: "a".to_string(),
                    to: "b".to_string(),
                },
                ModelAlias {
                    from: "b".to_string(),
                    to: "c".to_string(),
                },
            ],
            ..Default::default()
        };
        let r = resolve_model("a", &res);
        assert_eq!(r.model_chain, vec!["b"]);
    }

    #[test]
    fn test_alias_no_glob() {
        // Aliases are exact match only; glob patterns should not match
        let res = ModelResolution {
            aliases: vec![ModelAlias {
                from: "gpt-*".to_string(),
                to: "gpt-4".to_string(),
            }],
            ..Default::default()
        };
        let r = resolve_model("gpt-5", &res);
        // Should NOT match — "gpt-*" is literal, not a glob
        assert_eq!(r.model_chain, vec!["gpt-5"]);
        assert!(r.resolution_steps.is_empty());
    }

    #[test]
    fn test_rewrite_glob() {
        let res = ModelResolution {
            rewrites: vec![ModelRewrite {
                pattern: "claude-*-preview".to_string(),
                to: "claude-3.5-sonnet".to_string(),
            }],
            ..Default::default()
        };
        let r = resolve_model("claude-next-preview", &res);
        assert_eq!(r.model_chain, vec!["claude-3.5-sonnet"]);
    }

    #[test]
    fn test_rewrite_first_match_wins() {
        let res = ModelResolution {
            rewrites: vec![
                ModelRewrite {
                    pattern: "gpt-*".to_string(),
                    to: "gpt-4o".to_string(),
                },
                ModelRewrite {
                    pattern: "gpt-*".to_string(),
                    to: "gpt-4".to_string(),
                },
            ],
            ..Default::default()
        };
        let r = resolve_model("gpt-latest", &res);
        assert_eq!(r.model_chain, vec!["gpt-4o"]);
    }

    #[test]
    fn test_rewrite_no_match() {
        let res = ModelResolution {
            rewrites: vec![ModelRewrite {
                pattern: "claude-*".to_string(),
                to: "claude-3.5-sonnet".to_string(),
            }],
            ..Default::default()
        };
        let r = resolve_model("gpt-4", &res);
        assert_eq!(r.model_chain, vec!["gpt-4"]);
    }

    #[test]
    fn test_fallback_chain() {
        let res = ModelResolution {
            fallbacks: vec![ModelFallback {
                pattern: "gpt-4".to_string(),
                to: vec!["gpt-4-turbo".to_string(), "gpt-3.5-turbo".to_string()],
            }],
            ..Default::default()
        };
        let r = resolve_model("gpt-4", &res);
        assert_eq!(r.model_chain, vec!["gpt-4", "gpt-4-turbo", "gpt-3.5-turbo"]);
    }

    #[test]
    fn test_fallback_no_duplicate_primary() {
        let res = ModelResolution {
            fallbacks: vec![ModelFallback {
                pattern: "gpt-4".to_string(),
                to: vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()],
            }],
            ..Default::default()
        };
        let r = resolve_model("gpt-4", &res);
        // Primary should not be duplicated in chain
        assert_eq!(r.model_chain, vec!["gpt-4", "gpt-3.5-turbo"]);
    }

    #[test]
    fn test_provider_pin() {
        let res = ModelResolution {
            provider_pins: vec![ProviderPin {
                pattern: "claude-*".to_string(),
                providers: vec!["claude".to_string()],
            }],
            ..Default::default()
        };
        let r = resolve_model("claude-3-opus", &res);
        assert_eq!(r.pinned_providers, Some(vec!["claude".to_string()]));
    }

    #[test]
    fn test_alias_then_fallback() {
        let res = ModelResolution {
            aliases: vec![ModelAlias {
                from: "smart".to_string(),
                to: "gpt-4".to_string(),
            }],
            fallbacks: vec![ModelFallback {
                pattern: "gpt-4".to_string(),
                to: vec!["gpt-3.5-turbo".to_string()],
            }],
            ..Default::default()
        };
        let r = resolve_model("smart", &res);
        // alias resolves to gpt-4, then fallback chain adds gpt-3.5-turbo
        assert_eq!(r.model_chain, vec!["gpt-4", "gpt-3.5-turbo"]);
        assert_eq!(r.resolution_steps.len(), 2);
    }

    #[test]
    fn test_full_pipeline() {
        let res = ModelResolution {
            aliases: vec![ModelAlias {
                from: "latest".to_string(),
                to: "gpt-4o".to_string(),
            }],
            rewrites: vec![], // no rewrites
            fallbacks: vec![ModelFallback {
                pattern: "gpt-4o".to_string(),
                to: vec!["gpt-4-turbo".to_string()],
            }],
            provider_pins: vec![ProviderPin {
                pattern: "gpt-*".to_string(),
                providers: vec!["openai".to_string()],
            }],
        };
        let r = resolve_model("latest", &res);
        assert_eq!(r.model_chain, vec!["gpt-4o", "gpt-4-turbo"]);
        assert_eq!(r.pinned_providers, Some(vec!["openai".to_string()]));
        assert_eq!(r.resolution_steps.len(), 3); // alias + fallback + pin
    }
}
