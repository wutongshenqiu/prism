use crate::provider::Format;
use prism_domain::capability::UpstreamProtocol;
use prism_domain::operation::ExecutionMode;
use prism_domain::request::RequiredCapabilities;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ─── Request features ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RouteRequestFeatures {
    pub requested_model: String,
    pub endpoint: RouteEndpoint,
    pub source_format: Format,
    pub tenant_id: Option<String>,
    pub api_key_id: Option<String>,
    pub region: Option<String>,
    pub stream: bool,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// Capabilities required by the canonical request.
    /// When set, the planner filters out providers that cannot satisfy them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_capabilities: Option<RequiredCapabilities>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouteEndpoint {
    ChatCompletions,
    Messages,
    Responses,
    GenerateContent,
    StreamGenerateContent,
    Models,
}

// ─── Route plan ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RoutePlan {
    pub profile: String,
    pub model_chain: Vec<String>,
    pub attempts: Vec<RouteAttemptPlan>,
    pub trace: RouteTrace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RouteAttemptPlan {
    pub model: String,
    pub provider: Format,
    pub credential_id: String,
    pub credential_name: String,
    pub rank: u32,
    pub score: RouteScore,
    /// How this route will execute relative to the ingress protocol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_mode: Option<ExecutionMode>,
    /// The provider's upstream protocol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_protocol: Option<UpstreamProtocol>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RouteScore {
    pub weight: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inflight: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_cost: Option<f64>,
    #[serde(default)]
    pub health_penalty: f64,
}

// ─── Route trace ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RouteTrace {
    pub matched_rule: Option<String>,
    pub resolved_profile: String,
    #[serde(default)]
    pub model_resolution_steps: Vec<ModelResolutionStep>,
    #[serde(default)]
    pub candidates: Vec<RouteCandidate>,
    #[serde(default)]
    pub rejections: Vec<RouteRejection>,
    #[serde(default)]
    pub scoring: Vec<RouteScoringEntry>,
    #[serde(default)]
    pub fallback_events: Vec<RouteFallbackEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RouteCandidate {
    pub provider: String,
    pub credential_id: String,
    pub credential_name: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "step")]
pub enum ModelResolutionStep {
    AliasResolved {
        from: String,
        to: String,
    },
    RewriteApplied {
        from: String,
        to: String,
        rule: String,
    },
    FallbackChainBuilt {
        primary: String,
        fallbacks: Vec<String>,
    },
    ProviderPinned {
        model: String,
        providers: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RouteRejection {
    pub candidate: String,
    pub reason: RejectReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    ModelNotSupported,
    RegionMismatch,
    ProviderPinExcluded,
    CircuitBreakerOpen,
    OutlierEjected,
    CredentialDisabled,
    AccessDenied,
    CooldownActive,
    /// Provider is missing one or more required capabilities.
    MissingCapability {
        capabilities: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RouteScoringEntry {
    pub candidate: String,
    pub score: RouteScore,
    pub rank: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RouteFallbackEvent {
    pub from_model: String,
    pub to_model: String,
    pub reason: String,
}

// ─── Route explanation (API response) ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RouteExplanation {
    pub profile: String,
    pub matched_rule: Option<String>,
    pub model_chain: Vec<String>,
    pub selected: Option<SelectedRoute>,
    #[serde(default)]
    pub alternates: Vec<SelectedRoute>,
    #[serde(default)]
    pub rejections: Vec<RouteRejection>,
    #[serde(default)]
    pub model_resolution: Vec<ModelResolutionStep>,
    #[serde(default)]
    pub scoring: Vec<RouteScoringEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SelectedRoute {
    pub provider: String,
    pub credential_name: String,
    pub model: String,
    pub score: RouteScore,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_endpoint_serde() {
        let yaml = r#""chat-completions""#;
        let ep: RouteEndpoint = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(ep, RouteEndpoint::ChatCompletions);
    }

    #[test]
    fn test_reject_reason_serde() {
        let yaml = r#""circuit_breaker_open""#;
        let r: RejectReason = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(r, RejectReason::CircuitBreakerOpen);
    }

    #[test]
    fn test_route_trace_default() {
        let trace = RouteTrace::default();
        assert!(trace.matched_rule.is_none());
        assert!(trace.candidates.is_empty());
        assert!(trace.rejections.is_empty());
    }

    #[test]
    fn test_route_score_serialization() {
        let score = RouteScore {
            weight: 100.0,
            latency_ms: Some(245.3),
            inflight: None,
            estimated_cost: None,
            health_penalty: 0.0,
        };
        let json = serde_json::to_string(&score).unwrap();
        assert!(json.contains("245.3"));
        // inflight and estimated_cost should be skipped
        assert!(!json.contains("inflight"));
        assert!(!json.contains("estimated_cost"));
    }

    #[test]
    fn test_model_resolution_step_tagged() {
        let step = ModelResolutionStep::AliasResolved {
            from: "gpt-5-default".to_string(),
            to: "gpt-5".to_string(),
        };
        let json = serde_json::to_string(&step).unwrap();
        assert!(json.contains("alias_resolved"));
    }

    #[test]
    fn test_route_plan_serialization_round_trip() {
        let plan = RoutePlan {
            profile: "balanced".to_string(),
            model_chain: vec!["gpt-5".to_string()],
            attempts: vec![RouteAttemptPlan {
                model: "gpt-5".to_string(),
                provider: Format::OpenAI,
                credential_id: "cred-1".to_string(),
                credential_name: "prod-openai-1".to_string(),
                rank: 1,
                score: RouteScore {
                    weight: 100.0,
                    ..Default::default()
                },
                execution_mode: None,
                upstream_protocol: None,
            }],
            trace: RouteTrace {
                resolved_profile: "balanced".to_string(),
                ..Default::default()
            },
        };
        let json = serde_json::to_string(&plan).unwrap();
        let parsed: RoutePlan = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.profile, "balanced");
        assert_eq!(parsed.attempts.len(), 1);
    }

    #[test]
    fn test_route_explanation_serialization() {
        let explanation = RouteExplanation {
            profile: "balanced".to_string(),
            matched_rule: Some("enterprise-latency".to_string()),
            model_chain: vec!["gpt-5".to_string()],
            selected: Some(SelectedRoute {
                provider: "openai".to_string(),
                credential_name: "prod-1".to_string(),
                model: "gpt-5".to_string(),
                score: RouteScore::default(),
            }),
            alternates: vec![],
            rejections: vec![RouteRejection {
                candidate: "gemini/eu-1".to_string(),
                reason: RejectReason::RegionMismatch,
            }],
            model_resolution: vec![],
            scoring: vec![],
        };
        let json = serde_json::to_string(&explanation).unwrap();
        assert!(json.contains("enterprise-latency"));
        assert!(json.contains("region_mismatch"));
    }
}
