use std::collections::HashMap;
use std::sync::{Mutex, RwLock};
use std::time::Instant;

use crate::config::RateLimitConfig;

/// Result of a rate limit check.
pub struct RateLimitInfo {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Requests remaining in the current window.
    pub remaining: u32,
    /// The rate limit for this window.
    pub limit: u32,
    /// Seconds until the window resets (approximate).
    pub reset_secs: u64,
}

/// Trait for a single rate limit dimension.
pub trait RateLimitDimension: Send + Sync {
    fn check(&self, key: Option<&str>) -> RateLimitInfo;
    fn record(&self, key: Option<&str>, amount: u64);
    fn dimension_name(&self) -> &str;
}

// ─── Sliding Window Limiter (reused for RPM and TPM) ─────────────────────

struct SlidingWindow {
    timestamps: Vec<(Instant, u64)>,
}

impl SlidingWindow {
    fn new() -> Self {
        Self {
            timestamps: Vec::new(),
        }
    }

    fn count_and_prune(&mut self, now: Instant, window_secs: u64) -> u64 {
        let cutoff = now - std::time::Duration::from_secs(window_secs);
        self.timestamps.retain(|&(t, _)| t > cutoff);
        self.timestamps.iter().map(|&(_, amount)| amount).sum()
    }

    fn record(&mut self, now: Instant, amount: u64) {
        self.timestamps.push((now, amount));
    }

    fn estimate_reset(&self, now: Instant, window_secs: u64) -> u64 {
        if let Some(&(oldest, _)) = self.timestamps.first() {
            let age = now.duration_since(oldest);
            window_secs.saturating_sub(age.as_secs())
        } else {
            window_secs
        }
    }
}

/// Sliding window limiter — reusable for RPM and TPM.
pub struct SlidingWindowLimiter {
    name: String,
    window_secs: u64,
    global_limit: RwLock<u64>,
    per_key_limit: RwLock<u64>,
    global: Mutex<SlidingWindow>,
    per_key: RwLock<HashMap<String, Mutex<SlidingWindow>>>,
}

impl SlidingWindowLimiter {
    pub fn new(name: &str, window_secs: u64, global_limit: u64, per_key_limit: u64) -> Self {
        Self {
            name: name.to_string(),
            window_secs,
            global_limit: RwLock::new(global_limit),
            per_key_limit: RwLock::new(per_key_limit),
            global: Mutex::new(SlidingWindow::new()),
            per_key: RwLock::new(HashMap::new()),
        }
    }

    pub fn update_limits(&self, global_limit: u64, per_key_limit: u64) {
        if let Ok(mut g) = self.global_limit.write() {
            *g = global_limit;
        }
        if let Ok(mut p) = self.per_key_limit.write() {
            *p = per_key_limit;
        }
    }

    /// Check a specific key against a custom limit (ignoring the configured per-key limit).
    pub fn check_key_with_limit(&self, key: &str, limit: u64) -> RateLimitInfo {
        if limit == 0 {
            return RateLimitInfo {
                allowed: true,
                remaining: u32::MAX,
                limit: 0,
                reset_secs: self.window_secs,
            };
        }
        let now = Instant::now();
        let Ok(per_key) = self.per_key.read() else {
            return RateLimitInfo {
                allowed: true,
                remaining: u32::MAX,
                limit: 0,
                reset_secs: self.window_secs,
            };
        };
        if let Some(window) = per_key.get(key) {
            let Ok(mut window) = window.lock() else {
                return RateLimitInfo {
                    allowed: true,
                    remaining: u32::MAX,
                    limit: 0,
                    reset_secs: self.window_secs,
                };
            };
            let count = window.count_and_prune(now, self.window_secs);
            let remaining = limit.saturating_sub(count) as u32;
            if count >= limit {
                return RateLimitInfo {
                    allowed: false,
                    remaining: 0,
                    limit: limit as u32,
                    reset_secs: window.estimate_reset(now, self.window_secs),
                };
            }
            RateLimitInfo {
                allowed: true,
                remaining,
                limit: limit as u32,
                reset_secs: self.window_secs,
            }
        } else {
            RateLimitInfo {
                allowed: true,
                remaining: limit as u32,
                limit: limit as u32,
                reset_secs: self.window_secs,
            }
        }
    }
}

impl RateLimitDimension for SlidingWindowLimiter {
    fn check(&self, key: Option<&str>) -> RateLimitInfo {
        let now = Instant::now();
        let global_limit = self.global_limit.read().map(|g| *g).unwrap_or(0);
        let per_key_limit = self.per_key_limit.read().map(|p| *p).unwrap_or(0);

        let mut most_restrictive = RateLimitInfo {
            allowed: true,
            remaining: u32::MAX,
            limit: 0,
            reset_secs: self.window_secs,
        };

        // Check global limit
        if global_limit > 0 {
            let Ok(mut global) = self.global.lock() else {
                return most_restrictive;
            };
            let count = global.count_and_prune(now, self.window_secs);
            let remaining = (global_limit).saturating_sub(count) as u32;
            if count >= global_limit {
                return RateLimitInfo {
                    allowed: false,
                    remaining: 0,
                    limit: global_limit as u32,
                    reset_secs: global.estimate_reset(now, self.window_secs),
                };
            }
            if remaining < most_restrictive.remaining {
                most_restrictive = RateLimitInfo {
                    allowed: true,
                    remaining,
                    limit: global_limit as u32,
                    reset_secs: self.window_secs,
                };
            }
        }

        // Check per-key limit
        if per_key_limit > 0
            && let Some(key) = key
        {
            let Ok(per_key) = self.per_key.read() else {
                return most_restrictive;
            };
            if let Some(window) = per_key.get(key) {
                let Ok(mut window) = window.lock() else {
                    return most_restrictive;
                };
                let count = window.count_and_prune(now, self.window_secs);
                let remaining = (per_key_limit).saturating_sub(count) as u32;
                if count >= per_key_limit {
                    return RateLimitInfo {
                        allowed: false,
                        remaining: 0,
                        limit: per_key_limit as u32,
                        reset_secs: window.estimate_reset(now, self.window_secs),
                    };
                }
                if remaining < most_restrictive.remaining {
                    most_restrictive = RateLimitInfo {
                        allowed: true,
                        remaining,
                        limit: per_key_limit as u32,
                        reset_secs: self.window_secs,
                    };
                }
            }
        }

        most_restrictive
    }

    fn record(&self, key: Option<&str>, amount: u64) {
        let now = Instant::now();
        let global_limit = self.global_limit.read().map(|g| *g).unwrap_or(0);

        if global_limit > 0
            && let Ok(mut global) = self.global.lock()
        {
            global.record(now, amount);
        }

        // Always record per-key data when a key is provided, since per-key
        // overrides from AuthKeyEntry may use it even if the global per-key limit is 0.
        if let Some(key) = key {
            // Fast path: read lock
            {
                if let Ok(per_key) = self.per_key.read()
                    && let Some(window) = per_key.get(key)
                {
                    if let Ok(mut window) = window.lock() {
                        window.record(now, amount);
                    }
                    return;
                }
            }
            // Slow path: write lock
            {
                if let Ok(mut per_key) = self.per_key.write() {
                    let window = per_key
                        .entry(key.to_string())
                        .or_insert_with(|| Mutex::new(SlidingWindow::new()));
                    if let Ok(window) = window.get_mut() {
                        window.record(now, amount);
                    }
                }
            }
        }
    }

    fn dimension_name(&self) -> &str {
        &self.name
    }
}

type CostEntries = Vec<(Instant, f64)>;

/// Cost limiter — daily sliding window for per-key cost limits.
pub struct CostLimiter {
    per_key_daily_limit: RwLock<f64>,
    per_key: RwLock<HashMap<String, Mutex<CostEntries>>>,
}

impl CostLimiter {
    pub fn new(per_key_daily_limit: f64) -> Self {
        Self {
            per_key_daily_limit: RwLock::new(per_key_daily_limit),
            per_key: RwLock::new(HashMap::new()),
        }
    }

    pub fn update_limit(&self, limit: f64) {
        if let Ok(mut l) = self.per_key_daily_limit.write() {
            *l = limit;
        }
    }
}

impl RateLimitDimension for CostLimiter {
    fn check(&self, key: Option<&str>) -> RateLimitInfo {
        let limit = self.per_key_daily_limit.read().map(|l| *l).unwrap_or(0.0);
        let Some(key) = key else {
            return RateLimitInfo {
                allowed: true,
                remaining: u32::MAX,
                limit: 0,
                reset_secs: 0,
            };
        };
        self.check_cost_within_window(key, limit, 86400)
    }

    fn record(&self, key: Option<&str>, _amount: u64) {
        // Cost recording is done via record_cost() below
        let _ = key;
    }

    fn dimension_name(&self) -> &str {
        "cost"
    }
}

impl CostLimiter {
    /// Check a specific key against a cost limit within a sliding window.
    pub fn check_cost_within_window(
        &self,
        key: &str,
        limit: f64,
        window_secs: u64,
    ) -> RateLimitInfo {
        if limit <= 0.0 {
            return RateLimitInfo {
                allowed: true,
                remaining: u32::MAX,
                limit: 0,
                reset_secs: 0,
            };
        }
        let now = Instant::now();
        let cutoff = now - std::time::Duration::from_secs(window_secs);
        let Ok(per_key) = self.per_key.read() else {
            return RateLimitInfo {
                allowed: true,
                remaining: u32::MAX,
                limit: 0,
                reset_secs: 0,
            };
        };
        if let Some(entries) = per_key.get(key) {
            let Ok(mut entries) = entries.lock() else {
                return RateLimitInfo {
                    allowed: true,
                    remaining: u32::MAX,
                    limit: 0,
                    reset_secs: 0,
                };
            };
            entries.retain(|&(t, _)| t > cutoff);
            let total_cost: f64 = entries.iter().map(|&(_, c)| c).sum();
            if total_cost >= limit {
                return RateLimitInfo {
                    allowed: false,
                    remaining: 0,
                    limit: (limit * 100.0) as u32,
                    reset_secs: entries
                        .first()
                        .map(|&(t, _)| window_secs.saturating_sub(now.duration_since(t).as_secs()))
                        .unwrap_or(window_secs),
                };
            }
        }
        RateLimitInfo {
            allowed: true,
            remaining: u32::MAX,
            limit: 0,
            reset_secs: window_secs,
        }
    }

    /// Check a specific key against a custom daily cost limit.
    pub fn check_key_with_limit(&self, key: &str, limit: f64) -> RateLimitInfo {
        self.check_cost_within_window(key, limit, 86400)
    }

    /// Record cost for a key (in USD).
    pub fn record_cost(&self, key: &str, cost: f64) {
        let now = Instant::now();
        // Fast path
        {
            if let Ok(per_key) = self.per_key.read()
                && let Some(entries) = per_key.get(key)
            {
                if let Ok(mut entries) = entries.lock() {
                    entries.push((now, cost));
                }
                return;
            }
        }
        // Slow path
        {
            if let Ok(mut per_key) = self.per_key.write() {
                let entries = per_key
                    .entry(key.to_string())
                    .or_insert_with(|| Mutex::new(Vec::new()));
                if let Ok(entries) = entries.get_mut() {
                    entries.push((now, cost));
                }
            }
        }
    }
}

/// Composite rate limiter — checks all dimensions, returns most restrictive.
pub struct CompositeRateLimiter {
    rpm: SlidingWindowLimiter,
    tpm: SlidingWindowLimiter,
    cost: CostLimiter,
    enabled: RwLock<bool>,
}

impl CompositeRateLimiter {
    pub fn new(config: &RateLimitConfig) -> Self {
        Self {
            rpm: SlidingWindowLimiter::new(
                "rpm",
                60,
                config.global_rpm as u64,
                config.per_key_rpm as u64,
            ),
            tpm: SlidingWindowLimiter::new("tpm", 60, config.global_tpm, config.per_key_tpm),
            cost: CostLimiter::new(config.per_key_cost_per_day_usd),
            enabled: RwLock::new(config.enabled),
        }
    }

    /// Update configuration (called on hot-reload).
    pub fn update_config(&self, config: &RateLimitConfig) {
        if let Ok(mut e) = self.enabled.write() {
            *e = config.enabled;
        }
        self.rpm
            .update_limits(config.global_rpm as u64, config.per_key_rpm as u64);
        self.tpm
            .update_limits(config.global_tpm, config.per_key_tpm);
        self.cost.update_limit(config.per_key_cost_per_day_usd);
    }

    /// Check rate limits. Returns info about the most restrictive limit.
    pub fn check(&self, api_key: Option<&str>) -> RateLimitInfo {
        if !self.enabled.read().map(|e| *e).unwrap_or(false) {
            return RateLimitInfo {
                allowed: true,
                remaining: u32::MAX,
                limit: 0,
                reset_secs: 0,
            };
        }

        let rpm_info = self.rpm.check(api_key);
        if !rpm_info.allowed {
            return rpm_info;
        }

        let tpm_info = self.tpm.check(api_key);
        if !tpm_info.allowed {
            return tpm_info;
        }

        let cost_info = self.cost.check(api_key);
        if !cost_info.allowed {
            return cost_info;
        }

        // Return the most restrictive remaining
        let mut result = rpm_info;
        if tpm_info.remaining < result.remaining {
            result = tpm_info;
        }
        result
    }

    /// Record a request (RPM dimension). Call after check() returns allowed=true.
    pub fn record_request(&self, api_key: Option<&str>) {
        if !self.enabled.read().map(|e| *e).unwrap_or(false) {
            return;
        }
        self.rpm.record(api_key, 1);
    }

    /// Record tokens (TPM dimension). Call after response is received.
    pub fn record_tokens(&self, api_key: Option<&str>, tokens: u64) {
        if !self.enabled.read().map(|e| *e).unwrap_or(false) {
            return;
        }
        self.tpm.record(api_key, tokens);
    }

    /// Check per-key rate limit overrides from AuthKeyEntry config.
    pub fn check_key_overrides(
        &self,
        key: &str,
        rl: &crate::auth_key::KeyRateLimitConfig,
    ) -> RateLimitInfo {
        if let Some(rpm) = rl.rpm {
            let info = self.rpm.check_key_with_limit(key, rpm as u64);
            if !info.allowed {
                return info;
            }
        }
        if let Some(tpm) = rl.tpm {
            let info = self.tpm.check_key_with_limit(key, tpm);
            if !info.allowed {
                return info;
            }
        }
        if let Some(cost_limit) = rl.cost_per_day_usd {
            let info = self.cost.check_key_with_limit(key, cost_limit);
            if !info.allowed {
                return info;
            }
        }
        RateLimitInfo {
            allowed: true,
            remaining: u32::MAX,
            limit: 0,
            reset_secs: 0,
        }
    }

    /// Check per-key budget limits from AuthKeyEntry config.
    pub fn check_budget(&self, key: &str, budget: &crate::auth_key::BudgetConfig) -> RateLimitInfo {
        let window_secs = match budget.period {
            crate::auth_key::BudgetPeriod::Daily => 86400u64,
            crate::auth_key::BudgetPeriod::Monthly => 30 * 86400u64,
        };
        self.cost
            .check_cost_within_window(key, budget.total_usd, window_secs)
    }

    /// Record cost (Cost dimension). Call after response is received.
    pub fn record_cost(&self, api_key: Option<&str>, cost: f64) {
        if !self.enabled.read().map(|e| *e).unwrap_or(false) || cost <= 0.0 {
            return;
        }
        if let Some(key) = api_key {
            self.cost.record_cost(key, cost);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_allows_all() {
        let config = RateLimitConfig {
            enabled: false,
            global_rpm: 10,
            per_key_rpm: 5,
            ..Default::default()
        };
        let limiter = CompositeRateLimiter::new(&config);
        let info = limiter.check(Some("key1"));
        assert!(info.allowed);
    }

    #[test]
    fn test_global_rpm_limit() {
        let config = RateLimitConfig {
            enabled: true,
            global_rpm: 3,
            per_key_rpm: 0,
            ..Default::default()
        };
        let limiter = CompositeRateLimiter::new(&config);

        for _ in 0..3 {
            let info = limiter.check(None);
            assert!(info.allowed);
            limiter.record_request(None);
        }

        let info = limiter.check(None);
        assert!(!info.allowed);
        assert_eq!(info.remaining, 0);
        assert_eq!(info.limit, 3);
    }

    #[test]
    fn test_per_key_rpm_limit() {
        let config = RateLimitConfig {
            enabled: true,
            global_rpm: 0,
            per_key_rpm: 2,
            ..Default::default()
        };
        let limiter = CompositeRateLimiter::new(&config);

        // key1 uses 2 requests
        for _ in 0..2 {
            let info = limiter.check(Some("key1"));
            assert!(info.allowed);
            limiter.record_request(Some("key1"));
        }

        // key1 is now rate limited
        let info = limiter.check(Some("key1"));
        assert!(!info.allowed);

        // key2 still has quota
        let info = limiter.check(Some("key2"));
        assert!(info.allowed);
    }

    #[test]
    fn test_tpm_limit() {
        let config = RateLimitConfig {
            enabled: true,
            global_tpm: 1000,
            ..Default::default()
        };
        let limiter = CompositeRateLimiter::new(&config);

        // Record 1000 tokens
        limiter.tpm.record(None, 1000);

        let info = limiter.check(None);
        assert!(!info.allowed);
    }

    #[test]
    fn test_update_config() {
        let config = RateLimitConfig {
            enabled: true,
            global_rpm: 2,
            per_key_rpm: 0,
            ..Default::default()
        };
        let limiter = CompositeRateLimiter::new(&config);

        limiter.record_request(None);
        limiter.record_request(None);
        assert!(!limiter.check(None).allowed);

        // Increase limit
        limiter.update_config(&RateLimitConfig {
            enabled: true,
            global_rpm: 5,
            per_key_rpm: 0,
            ..Default::default()
        });

        assert!(limiter.check(None).allowed);
    }

    #[test]
    fn test_check_key_with_limit_rpm() {
        let config = RateLimitConfig {
            enabled: true,
            ..Default::default()
        };
        let limiter = CompositeRateLimiter::new(&config);

        // Record some requests for key1
        limiter.rpm.record(Some("key1"), 1);
        limiter.rpm.record(Some("key1"), 1);

        // Check with custom limit of 2 — should be at the limit
        let info = limiter.rpm.check_key_with_limit("key1", 2);
        assert!(!info.allowed);

        // Check with custom limit of 5 — should be allowed
        let info = limiter.rpm.check_key_with_limit("key1", 5);
        assert!(info.allowed);

        // key2 should be fine
        let info = limiter.rpm.check_key_with_limit("key2", 2);
        assert!(info.allowed);
    }

    #[test]
    fn test_check_key_overrides() {
        let config = RateLimitConfig {
            enabled: true,
            ..Default::default()
        };
        let limiter = CompositeRateLimiter::new(&config);

        // Record 3 requests for key1
        for _ in 0..3 {
            limiter.record_request(Some("key1"));
        }

        let rl = crate::auth_key::KeyRateLimitConfig {
            rpm: Some(2),
            tpm: None,
            cost_per_day_usd: None,
        };
        let info = limiter.check_key_overrides("key1", &rl);
        assert!(!info.allowed);

        let rl_high = crate::auth_key::KeyRateLimitConfig {
            rpm: Some(100),
            tpm: None,
            cost_per_day_usd: None,
        };
        let info = limiter.check_key_overrides("key1", &rl_high);
        assert!(info.allowed);
    }

    #[test]
    fn test_check_budget() {
        let config = RateLimitConfig {
            enabled: true,
            ..Default::default()
        };
        let limiter = CompositeRateLimiter::new(&config);

        // Record $5 cost for key1
        limiter.cost.record_cost("key1", 5.0);

        let budget = crate::auth_key::BudgetConfig {
            total_usd: 3.0,
            period: crate::auth_key::BudgetPeriod::Daily,
        };
        let info = limiter.check_budget("key1", &budget);
        assert!(!info.allowed);

        let high_budget = crate::auth_key::BudgetConfig {
            total_usd: 100.0,
            period: crate::auth_key::BudgetPeriod::Monthly,
        };
        let info = limiter.check_budget("key1", &high_budget);
        assert!(info.allowed);
    }

    #[test]
    fn test_record_tokens_and_cost() {
        let config = RateLimitConfig {
            enabled: true,
            global_tpm: 0,
            per_key_tpm: 1000,
            per_key_cost_per_day_usd: 10.0,
            ..Default::default()
        };
        let limiter = CompositeRateLimiter::new(&config);

        limiter.record_tokens(Some("key1"), 500);
        limiter.record_cost(Some("key1"), 5.0);

        // Within limits
        let info = limiter.check(Some("key1"));
        assert!(info.allowed);

        // Record more to exceed
        limiter.record_tokens(Some("key1"), 600);
        let info = limiter.check(Some("key1"));
        assert!(!info.allowed);
    }
}
