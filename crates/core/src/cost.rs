use std::collections::HashMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::request_record::TokenUsage;

/// Price per 1M tokens (input, output, and cache tiers).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ModelPrice {
    /// Cost per 1M input tokens in USD.
    pub input: f64,
    /// Cost per 1M output tokens in USD.
    pub output: f64,
    /// Cost per 1M cache-read tokens in USD. Falls back to `input` if absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    /// Cost per 1M cache-creation tokens in USD. Falls back to `input` if absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
}

/// Cost calculator with built-in price table and user overrides.
pub struct CostCalculator {
    prices: RwLock<HashMap<String, ModelPrice>>,
}

impl CostCalculator {
    pub fn new(overrides: &HashMap<String, ModelPrice>) -> Self {
        let mut prices = built_in_prices();
        for (model, price) in overrides {
            prices.insert(model.clone(), price.clone());
        }
        Self {
            prices: RwLock::new(prices),
        }
    }

    /// Update prices (called on hot-reload).
    pub fn update_prices(&self, overrides: &HashMap<String, ModelPrice>) {
        let mut prices = built_in_prices();
        for (model, price) in overrides {
            prices.insert(model.clone(), price.clone());
        }
        if let Ok(mut p) = self.prices.write() {
            *p = prices;
        }
    }

    /// Calculate cost for a request in USD.
    /// Returns None if the model is not in the price table.
    pub fn calculate(&self, model: &str, usage: &TokenUsage) -> Option<f64> {
        let prices = self.prices.read().ok()?;
        let price = lookup_price(&prices, model)?;

        let cache_read_price = price.cache_read.unwrap_or(price.input);
        let cache_write_price = price.cache_write.unwrap_or(price.input);

        let cost = (usage.input_tokens as f64 / 1_000_000.0) * price.input
            + (usage.output_tokens as f64 / 1_000_000.0) * price.output
            + (usage.cache_read_tokens as f64 / 1_000_000.0) * cache_read_price
            + (usage.cache_creation_tokens as f64 / 1_000_000.0) * cache_write_price;

        Some(cost)
    }
}

/// Look up price by exact match, then by stripping provider prefix (e.g. "openai/gpt-4o" → "gpt-4o").
fn lookup_price<'a>(
    prices: &'a HashMap<String, ModelPrice>,
    model: &str,
) -> Option<&'a ModelPrice> {
    prices.get(model).or_else(|| {
        let stripped = model.split('/').next_back().unwrap_or(model);
        prices.get(stripped)
    })
}

/// Built-in price table for major models (USD per 1M tokens).
///
/// Cache pricing ratios (from official docs):
/// - Claude: cache_read = input × 0.1, cache_write = input × 1.25
/// - OpenAI: cache_read = input × 0.5
/// - Gemini: cache_read = input × 0.25
fn built_in_prices() -> HashMap<String, ModelPrice> {
    let mut prices = HashMap::new();

    // Helper closures for provider-specific cache ratios
    let claude = |model: &str, input: f64, output: f64| {
        (
            model.to_string(),
            ModelPrice {
                input,
                output,
                cache_read: Some(input * 0.1),
                cache_write: Some(input * 1.25),
            },
        )
    };
    let openai = |model: &str, input: f64, output: f64| {
        (
            model.to_string(),
            ModelPrice {
                input,
                output,
                cache_read: Some(input * 0.5),
                cache_write: None,
            },
        )
    };
    let gemini = |model: &str, input: f64, output: f64| {
        (
            model.to_string(),
            ModelPrice {
                input,
                output,
                cache_read: Some(input * 0.25),
                cache_write: None,
            },
        )
    };
    let plain = |model: &str, input: f64, output: f64| {
        (
            model.to_string(),
            ModelPrice {
                input,
                output,
                cache_read: None,
                cache_write: None,
            },
        )
    };

    let entries = vec![
        // Claude 4.x models (latest aliases)
        claude("claude-opus-4-6", 15.0, 75.0),
        claude("claude-sonnet-4-6", 3.0, 15.0),
        claude("claude-opus-4-5", 15.0, 75.0),
        claude("claude-sonnet-4-5", 3.0, 15.0),
        claude("claude-haiku-4-5", 0.80, 4.0),
        // Claude 4.x models (dated versions)
        claude("claude-opus-4-20250514", 15.0, 75.0),
        claude("claude-sonnet-4-20250514", 3.0, 15.0),
        claude("claude-haiku-4-20250514", 0.80, 4.0),
        claude("claude-sonnet-4-5-20250929", 3.0, 15.0),
        claude("claude-opus-4-5-20251101", 15.0, 75.0),
        claude("claude-opus-4-1-20250805", 15.0, 75.0),
        claude("claude-haiku-4-5-20251001", 0.80, 4.0),
        // Claude 3.x models
        claude("claude-3-5-sonnet-20241022", 3.0, 15.0),
        claude("claude-3-5-haiku-20241022", 0.80, 4.0),
        claude("claude-3-opus-20240229", 15.0, 75.0),
        claude("claude-3-sonnet-20240229", 3.0, 15.0),
        claude("claude-3-haiku-20240307", 0.25, 1.25),
        // OpenAI models
        openai("gpt-4o", 2.50, 10.0),
        openai("gpt-4o-mini", 0.15, 0.60),
        openai("gpt-4o-2024-11-20", 2.50, 10.0),
        openai("gpt-4-turbo", 10.0, 30.0),
        openai("gpt-4", 30.0, 60.0),
        openai("gpt-3.5-turbo", 0.50, 1.50),
        openai("o1", 15.0, 60.0),
        openai("o1-mini", 3.0, 12.0),
        openai("o1-pro", 150.0, 600.0),
        openai("o3", 10.0, 40.0),
        openai("o3-mini", 1.10, 4.40),
        openai("o4-mini", 1.10, 4.40),
        // Gemini models
        gemini("gemini-2.5-pro-preview-06-05", 1.25, 10.0),
        gemini("gemini-2.5-flash-preview-05-20", 0.15, 0.60),
        gemini("gemini-2.0-flash", 0.10, 0.40),
        gemini("gemini-2.0-flash-lite", 0.075, 0.30),
        gemini("gemini-1.5-pro", 1.25, 5.0),
        gemini("gemini-1.5-flash", 0.075, 0.30),
        // DeepSeek models (no cache support)
        plain("deepseek-chat", 0.27, 1.10),
        plain("deepseek-reasoner", 0.55, 2.19),
        // Groq models (no cache support)
        plain("llama-3.3-70b-versatile", 0.59, 0.79),
        plain("llama-3.1-8b-instant", 0.05, 0.08),
    ];

    for (model, price) in entries {
        prices.insert(model, price);
    }
    prices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_known_model() {
        let calc = CostCalculator::new(&HashMap::new());
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            ..Default::default()
        };
        // gpt-4o: $2.50/1M input, $10.0/1M output
        let cost = calc.calculate("gpt-4o", &usage);
        assert!(cost.is_some());
        // $2.50 (input) + $5.00 (output) = $7.50
        assert!((cost.unwrap() - 7.50).abs() < 0.001);
    }

    #[test]
    fn test_calculate_unknown_model() {
        let calc = CostCalculator::new(&HashMap::new());
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            ..Default::default()
        };
        let cost = calc.calculate("unknown-model-xyz", &usage);
        assert!(cost.is_none());
    }

    #[test]
    fn test_prefix_stripping() {
        let calc = CostCalculator::new(&HashMap::new());
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            ..Default::default()
        };
        let cost = calc.calculate("openai/gpt-4o", &usage);
        assert!(cost.is_some());
        assert!((cost.unwrap() - 2.50).abs() < 0.001);
    }

    #[test]
    fn test_user_override() {
        let mut overrides = HashMap::new();
        overrides.insert(
            "my-custom-model".to_string(),
            ModelPrice {
                input: 1.0,
                output: 2.0,
                cache_read: None,
                cache_write: None,
            },
        );
        let calc = CostCalculator::new(&overrides);
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            ..Default::default()
        };
        let cost = calc.calculate("my-custom-model", &usage);
        assert!(cost.is_some());
        assert!((cost.unwrap() - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_override_built_in() {
        let mut overrides = HashMap::new();
        overrides.insert(
            "gpt-4o".to_string(),
            ModelPrice {
                input: 100.0,
                output: 200.0,
                cache_read: None,
                cache_write: None,
            },
        );
        let calc = CostCalculator::new(&overrides);
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            ..Default::default()
        };
        let cost = calc.calculate("gpt-4o", &usage);
        assert!(cost.is_some());
        assert!((cost.unwrap() - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_update_prices() {
        let calc = CostCalculator::new(&HashMap::new());
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            ..Default::default()
        };
        assert!(calc.calculate("custom-model", &usage).is_none());

        let mut overrides = HashMap::new();
        overrides.insert(
            "custom-model".to_string(),
            ModelPrice {
                input: 5.0,
                output: 10.0,
                cache_read: None,
                cache_write: None,
            },
        );
        calc.update_prices(&overrides);
        assert!(calc.calculate("custom-model", &usage).is_some());
    }

    #[test]
    fn test_zero_tokens() {
        let calc = CostCalculator::new(&HashMap::new());
        let usage = TokenUsage::default();
        let cost = calc.calculate("gpt-4o", &usage);
        assert!(cost.is_some());
        assert!((cost.unwrap()).abs() < 0.001);
    }

    #[test]
    fn test_cache_tokens_claude() {
        let calc = CostCalculator::new(&HashMap::new());
        // claude-sonnet-4-6: input=$3.0, cache_read=$0.30, cache_write=$3.75
        let usage = TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 1_000_000,
            cache_creation_tokens: 1_000_000,
        };
        let cost = calc.calculate("claude-sonnet-4-6", &usage).unwrap();
        // $0.30 (cache_read) + $3.75 (cache_write) = $4.05
        assert!((cost - 4.05).abs() < 0.001);
    }

    #[test]
    fn test_cache_tokens_openai() {
        let calc = CostCalculator::new(&HashMap::new());
        // gpt-4o: input=$2.50, cache_read=$1.25
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 0,
            cache_read_tokens: 1_000_000,
            cache_creation_tokens: 0,
        };
        let cost = calc.calculate("gpt-4o", &usage).unwrap();
        // $2.50 (input) + $1.25 (cache_read) = $3.75
        assert!((cost - 3.75).abs() < 0.001);
    }

    #[test]
    fn test_cache_fallback_to_input_price() {
        // For models with no cache pricing, cache tokens use input price
        let calc = CostCalculator::new(&HashMap::new());
        let usage = TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 1_000_000,
            cache_creation_tokens: 0,
        };
        // deepseek-chat: input=$0.27, no cache_read set → falls back to $0.27
        let cost = calc.calculate("deepseek-chat", &usage).unwrap();
        assert!((cost - 0.27).abs() < 0.001);
    }
}
