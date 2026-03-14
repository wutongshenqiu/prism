use crate::AppState;
use axum::{extract::State, http::Request, middleware::Next, response::Response};
use prism_core::context::RequestContext;
use prism_core::error::ProxyError;

pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ProxyError> {
    let config = state.config.load();
    if !config.rate_limit.enabled {
        return Ok(next.run(request).await);
    }

    // Extract API key from Authorization: Bearer or x-api-key header
    let api_key = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| {
            request
                .headers()
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
        })
        .map(|s| s.to_string());

    // Global + global per-key check
    let info = state.rate_limiter.check(api_key.as_deref());

    if !info.allowed {
        tracing::warn!(
            api_key = api_key.as_deref().unwrap_or("none"),
            reset_secs = info.reset_secs,
            "Global rate limit exceeded"
        );
        return Err(ProxyError::RateLimited {
            message: format!("Rate limit exceeded. Retry after {}s", info.reset_secs),
            retry_after_secs: info.reset_secs,
        });
    }

    // Per-key rate limit overrides from auth key config
    if let Some(ref key) = api_key
        && let Some(ctx) = request.extensions().get::<RequestContext>()
        && let Some(ref auth_entry) = ctx.auth_key
    {
        // Check per-key rate limit overrides (rpm/tpm/cost_per_day_usd)
        if let Some(ref rl) = auth_entry.rate_limit {
            let key_info = state.rate_limiter.check_key_overrides(key, rl);
            if !key_info.allowed {
                tracing::warn!(
                    api_key = %key,
                    reset_secs = key_info.reset_secs,
                    "Per-key rate limit exceeded"
                );
                return Err(ProxyError::RateLimited {
                    message: format!(
                        "Per-key rate limit exceeded. Retry after {}s",
                        key_info.reset_secs
                    ),
                    retry_after_secs: key_info.reset_secs,
                });
            }
        }
        // Check per-key budget
        if let Some(ref budget) = auth_entry.budget {
            let budget_info = state.rate_limiter.check_budget(key, budget);
            if !budget_info.allowed {
                tracing::warn!(
                    api_key = %key,
                    reset_secs = budget_info.reset_secs,
                    "Per-key budget limit exceeded"
                );
                return Err(ProxyError::RateLimited {
                    message: format!(
                        "Budget limit exceeded. Retry after {}s",
                        budget_info.reset_secs
                    ),
                    retry_after_secs: budget_info.reset_secs,
                });
            }
        }
    }

    // Record the request (RPM dimension)
    state.rate_limiter.record_request(api_key.as_deref());

    let mut response = next.run(request).await;

    // Inject x-ratelimit-* response headers
    let headers = response.headers_mut();
    headers.insert("x-ratelimit-limit", info.limit.to_string().parse().unwrap());
    headers.insert(
        "x-ratelimit-remaining",
        info.remaining
            .saturating_sub(1)
            .to_string()
            .parse()
            .unwrap(),
    );
    headers.insert(
        "x-ratelimit-reset",
        info.reset_secs.to_string().parse().unwrap(),
    );

    Ok(response)
}
