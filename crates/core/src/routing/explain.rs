use super::types::*;

/// Convert a RoutePlan into a structured RouteExplanation for API responses.
pub fn explain(plan: &RoutePlan) -> RouteExplanation {
    let selected = plan.attempts.first().map(|a| SelectedRoute {
        provider: format!("{:?}", a.provider).to_lowercase(),
        credential_name: a.credential_name.clone(),
        model: a.model.clone(),
        score: a.score.clone(),
    });

    let alternates = plan
        .attempts
        .iter()
        .skip(1)
        .map(|a| SelectedRoute {
            provider: format!("{:?}", a.provider).to_lowercase(),
            credential_name: a.credential_name.clone(),
            model: a.model.clone(),
            score: a.score.clone(),
        })
        .collect();

    RouteExplanation {
        profile: plan.profile.clone(),
        matched_rule: plan.trace.matched_rule.clone(),
        model_chain: plan.model_chain.clone(),
        selected,
        alternates,
        rejections: plan.trace.rejections.clone(),
        model_resolution: plan.trace.model_resolution_steps.clone(),
        scoring: plan.trace.scoring.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Format;

    #[test]
    fn test_explain_empty_plan() {
        let plan = RoutePlan {
            profile: "balanced".to_string(),
            model_chain: vec!["gpt-4".to_string()],
            attempts: vec![],
            trace: RouteTrace {
                resolved_profile: "balanced".to_string(),
                ..Default::default()
            },
        };
        let explanation = explain(&plan);
        assert_eq!(explanation.profile, "balanced");
        assert!(explanation.selected.is_none());
        assert!(explanation.alternates.is_empty());
    }

    #[test]
    fn test_explain_with_attempts() {
        let plan = RoutePlan {
            profile: "balanced".to_string(),
            model_chain: vec!["gpt-4".to_string()],
            attempts: vec![
                RouteAttemptPlan {
                    model: "gpt-4".to_string(),
                    provider: Format::OpenAI,
                    credential_id: "cred-1".to_string(),
                    credential_name: "prod-1".to_string(),
                    rank: 1,
                    score: RouteScore {
                        weight: 100.0,
                        ..Default::default()
                    },
                },
                RouteAttemptPlan {
                    model: "gpt-4".to_string(),
                    provider: Format::OpenAI,
                    credential_id: "cred-2".to_string(),
                    credential_name: "prod-2".to_string(),
                    rank: 2,
                    score: RouteScore {
                        weight: 50.0,
                        ..Default::default()
                    },
                },
            ],
            trace: RouteTrace {
                resolved_profile: "balanced".to_string(),
                ..Default::default()
            },
        };
        let explanation = explain(&plan);
        assert!(explanation.selected.is_some());
        assert_eq!(
            explanation.selected.as_ref().unwrap().credential_name,
            "prod-1"
        );
        assert_eq!(explanation.alternates.len(), 1);
        assert_eq!(explanation.alternates[0].credential_name, "prod-2");
    }

    #[test]
    fn test_explain_preserves_rejections() {
        let plan = RoutePlan {
            profile: "stable".to_string(),
            model_chain: vec!["gpt-4".to_string()],
            attempts: vec![],
            trace: RouteTrace {
                resolved_profile: "stable".to_string(),
                rejections: vec![
                    RouteRejection {
                        candidate: "claude/prod".to_string(),
                        reason: RejectReason::ModelNotSupported,
                    },
                    RouteRejection {
                        candidate: "openai/prod".to_string(),
                        reason: RejectReason::CircuitBreakerOpen,
                    },
                ],
                ..Default::default()
            },
        };
        let explanation = explain(&plan);
        assert_eq!(explanation.rejections.len(), 2);
        assert_eq!(
            explanation.rejections[0].reason,
            RejectReason::ModelNotSupported
        );
    }

    #[test]
    fn test_explain_preserves_matched_rule() {
        let plan = RoutePlan {
            profile: "stable".to_string(),
            model_chain: vec!["gpt-4".to_string()],
            attempts: vec![],
            trace: RouteTrace {
                matched_rule: Some("enterprise-latency".to_string()),
                resolved_profile: "stable".to_string(),
                ..Default::default()
            },
        };
        let explanation = explain(&plan);
        assert_eq!(
            explanation.matched_rule,
            Some("enterprise-latency".to_string())
        );
    }

    #[test]
    fn test_explain_preserves_model_resolution() {
        let plan = RoutePlan {
            profile: "balanced".to_string(),
            model_chain: vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()],
            attempts: vec![],
            trace: RouteTrace {
                resolved_profile: "balanced".to_string(),
                model_resolution_steps: vec![ModelResolutionStep::FallbackChainBuilt {
                    primary: "gpt-4".to_string(),
                    fallbacks: vec!["gpt-3.5-turbo".to_string()],
                }],
                ..Default::default()
            },
        };
        let explanation = explain(&plan);
        assert_eq!(explanation.model_resolution.len(), 1);
        assert_eq!(explanation.model_chain.len(), 2);
    }
}
