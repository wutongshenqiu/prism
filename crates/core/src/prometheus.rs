use crate::cache::CacheStats;
use crate::metrics::Metrics;
use std::fmt::Write;

/// Write a Prometheus counter line.
fn write_counter(out: &mut String, name: &str, labels: &str, value: u64) {
    if labels.is_empty() {
        let _ = writeln!(out, "{name} {value}");
    } else {
        let _ = writeln!(out, "{name}{{{labels}}} {value}");
    }
}

/// Write a Prometheus gauge line.
fn write_gauge(out: &mut String, name: &str, labels: &str, value: f64) {
    if labels.is_empty() {
        let _ = writeln!(out, "{name} {value}");
    } else {
        let _ = writeln!(out, "{name}{{{labels}}} {value}");
    }
}

/// Write a Prometheus histogram bucket line.
fn write_histogram_bucket(out: &mut String, name: &str, le: &str, count: u64) {
    let _ = writeln!(out, "{name}_bucket{{le=\"{le}\"}} {count}");
}

/// Render all metrics in Prometheus text exposition format.
pub fn render_metrics(
    metrics: &Metrics,
    cache_stats: Option<&CacheStats>,
    circuit_breaker_states: &[(String, bool)],
) -> String {
    let mut out = String::with_capacity(4096);
    let snap = metrics.snapshot();

    // ── prism_requests_total ──
    let _ = writeln!(out, "# HELP prism_requests_total Total number of requests.");
    let _ = writeln!(out, "# TYPE prism_requests_total counter");
    if let Some(by_model) = snap["by_model"].as_object() {
        for (model, count) in by_model {
            if let Some(c) = count.as_u64() {
                write_counter(
                    &mut out,
                    "prism_requests_total",
                    &format!("model=\"{model}\""),
                    c,
                );
            }
        }
    }
    if let Some(by_provider) = snap["by_provider"].as_object() {
        for (provider, count) in by_provider {
            if let Some(c) = count.as_u64() {
                write_counter(
                    &mut out,
                    "prism_requests_total",
                    &format!("provider=\"{provider}\""),
                    c,
                );
            }
        }
    }

    // ── prism_errors_total ──
    let _ = writeln!(out, "# HELP prism_errors_total Total number of errors.");
    let _ = writeln!(out, "# TYPE prism_errors_total counter");
    write_counter(
        &mut out,
        "prism_errors_total",
        "",
        snap["total_errors"].as_u64().unwrap_or(0),
    );

    // ── prism_tokens_total ──
    let _ = writeln!(out, "# HELP prism_tokens_total Total tokens processed.");
    let _ = writeln!(out, "# TYPE prism_tokens_total counter");
    write_counter(
        &mut out,
        "prism_tokens_total",
        "direction=\"input\"",
        snap["total_input_tokens"].as_u64().unwrap_or(0),
    );
    write_counter(
        &mut out,
        "prism_tokens_total",
        "direction=\"output\"",
        snap["total_output_tokens"].as_u64().unwrap_or(0),
    );

    // ── prism_cost_usd_total ──
    let _ = writeln!(out, "# HELP prism_cost_usd_total Total cost in USD.");
    let _ = writeln!(out, "# TYPE prism_cost_usd_total counter");
    write_gauge(
        &mut out,
        "prism_cost_usd_total",
        "",
        snap["total_cost_usd"].as_f64().unwrap_or(0.0),
    );

    // ── prism_request_duration_seconds ──
    let _ = writeln!(
        out,
        "# HELP prism_request_duration_seconds Request duration histogram."
    );
    let _ = writeln!(out, "# TYPE prism_request_duration_seconds histogram");
    // Map existing ms-based buckets (<100, 100-499, 500-999, 1000-4999, 5000-29999, >=30000)
    // to Prometheus seconds buckets (le=0.1, 0.5, 1, 5, 30, +Inf)
    let bucket_values = metrics.latency_bucket_values();
    let le_values = ["0.1", "0.5", "1", "5", "30", "+Inf"];
    let mut cumulative = 0u64;
    for (i, &count) in bucket_values.iter().enumerate() {
        cumulative += count;
        write_histogram_bucket(
            &mut out,
            "prism_request_duration_seconds",
            le_values[i],
            cumulative,
        );
    }
    let total_reqs = snap["total_requests"].as_u64().unwrap_or(0);
    let _ = writeln!(out, "prism_request_duration_seconds_count {total_reqs}");

    // ── TTFT ──
    let ttft_buckets = metrics.ttft_bucket_values();
    if ttft_buckets.iter().any(|&v| v > 0) {
        let _ = writeln!(
            out,
            "# HELP prism_ttft_seconds Time to first token histogram."
        );
        let _ = writeln!(out, "# TYPE prism_ttft_seconds histogram");
        let ttft_le = ["0.05", "0.1", "0.5", "1", "5", "+Inf"];
        let mut cum = 0u64;
        for (i, &count) in ttft_buckets.iter().enumerate() {
            cum += count;
            write_histogram_bucket(&mut out, "prism_ttft_seconds", ttft_le[i], cum);
        }
    }

    // ── prism_cache_hits_total / misses ──
    if let Some(stats) = cache_stats {
        let _ = writeln!(out, "# HELP prism_cache_hits_total Total cache hits.");
        let _ = writeln!(out, "# TYPE prism_cache_hits_total counter");
        write_counter(&mut out, "prism_cache_hits_total", "", stats.hits);
        let _ = writeln!(out, "# HELP prism_cache_misses_total Total cache misses.");
        let _ = writeln!(out, "# TYPE prism_cache_misses_total counter");
        write_counter(&mut out, "prism_cache_misses_total", "", stats.misses);
    }

    // ── prism_circuit_breaker_open ──
    if !circuit_breaker_states.is_empty() {
        let _ = writeln!(
            out,
            "# HELP prism_circuit_breaker_open Whether circuit breaker is open."
        );
        let _ = writeln!(out, "# TYPE prism_circuit_breaker_open gauge");
        for (credential, is_open) in circuit_breaker_states {
            write_gauge(
                &mut out,
                "prism_circuit_breaker_open",
                &format!("credential=\"{credential}\""),
                if *is_open { 1.0 } else { 0.0 },
            );
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_basic_metrics() {
        let metrics = Metrics::new();
        metrics.record_request("gpt-4", "openai");
        metrics.record_error();

        let output = render_metrics(&metrics, None, &[]);
        assert!(output.contains("prism_requests_total"));
        assert!(output.contains("prism_errors_total"));
        assert!(output.contains("prism_tokens_total"));
        assert!(output.contains("prism_cost_usd_total"));
        assert!(output.contains("prism_request_duration_seconds"));
    }

    #[test]
    fn test_render_with_cache_stats() {
        let metrics = Metrics::new();
        let stats = CacheStats {
            hits: 42,
            misses: 8,
            entries: 100,
            hit_rate: 0.84,
        };
        let output = render_metrics(&metrics, Some(&stats), &[]);
        assert!(output.contains("prism_cache_hits_total 42"));
        assert!(output.contains("prism_cache_misses_total 8"));
    }

    #[test]
    fn test_render_with_circuit_breaker() {
        let metrics = Metrics::new();
        let cb_states = vec![("cred-1".to_string(), true), ("cred-2".to_string(), false)];
        let output = render_metrics(&metrics, None, &cb_states);
        assert!(output.contains("prism_circuit_breaker_open{credential=\"cred-1\"} 1"));
        assert!(output.contains("prism_circuit_breaker_open{credential=\"cred-2\"} 0"));
    }
}
