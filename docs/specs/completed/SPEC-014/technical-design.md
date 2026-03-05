# Technical Design: Cost Tracking

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-014       |
| Title     | Cost Tracking  |
| Author    | AI Proxy Team  |
| Status    | Completed      |
| Created   | 2026-03-01     |
| Updated   | 2026-03-01     |

## Overview

Per-request cost calculation using built-in model prices + user overrides. Cost data flows into request logs, metrics snapshots, and response extensions for downstream consumption.

Reference: [SPEC-014 PRD](prd.md)

## API Design

### Metrics Snapshot (GET /metrics)

```json
{
  "total_cost_usd": 12.34,
  "cost_by_model": {
    "gpt-4o": 8.50,
    "claude-sonnet-4-6": 3.84
  }
}
```

### Request Log Entry

```json
{
  "request_id": "...",
  "model": "gpt-4o",
  "input_tokens": 1500,
  "output_tokens": 200,
  "cost": 0.00575
}
```

## Backend Implementation

### Module Structure

```
crates/core/src/
├── cost.rs            # CostCalculator, ModelPrice, built-in price table
├── config.rs          # model_prices: HashMap<String, ModelPrice>
├── metrics.rs         # total_cost_micro, model_costs, record_cost()
└── request_log.rs     # RequestLogEntry.cost

crates/server/src/
├── lib.rs             # AppState.cost_calculator
└── dispatch.rs        # inject_dispatch_meta() — calculate + record cost

src/
└── app.rs             # CostCalculator initialization + hot-reload
```

### Key Types

```rust
// crates/core/src/cost.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ModelPrice {
    pub input: f64,     // USD per 1M input tokens
    pub output: f64,    // USD per 1M output tokens
}

pub struct CostCalculator {
    prices: RwLock<HashMap<String, ModelPrice>>,
}

// crates/core/src/metrics.rs
pub struct Metrics {
    total_cost_micro: AtomicU64,               // Micro-USD for lock-free atomic ops
    model_costs: Mutex<HashMap<String, f64>>,  // Per-model aggregation
}

// crates/core/src/request_log.rs
pub struct RequestLogEntry {
    pub cost: Option<f64>,  // USD
    // ...
}

// crates/server/src/dispatch.rs
pub struct DispatchMeta {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost: Option<f64>,
}
```

### Flow

1. `CostCalculator::new(overrides)` loads built-in prices (30+ models), applies user overrides
2. Request dispatched to provider, response received
3. `extract_usage(payload)` parses tokens from response (OpenAI, Claude, Gemini formats)
4. `CostCalculator::calculate(model, input_tokens, output_tokens)` → `Option<f64>`
5. `Metrics::record_cost(model, cost)` — atomic micro-USD accumulation
6. `DispatchMeta { cost, .. }` inserted into response extensions
7. Request logging middleware reads `DispatchMeta`, writes `RequestLogEntry.cost`
8. Dashboard `/metrics` endpoint returns `total_cost_usd` and `cost_by_model`

### Built-in Price Table

| Model Family | Models | Input $/1M | Output $/1M |
|-------------|--------|-----------|------------|
| Claude 4.x | opus-4-6, sonnet-4-6, haiku-4-5 | $0.80–$15 | $4–$75 |
| Claude 3.x | 3-5-sonnet, 3-opus, 3-haiku | $0.25–$15 | $1.25–$75 |
| OpenAI | gpt-4o, gpt-4o-mini, o1, o3 | $0.15–$15 | $0.60–$60 |
| Gemini | 2.5-pro, 2.0-flash | $0.10–$1.25 | $0.40–$10 |
| DeepSeek | chat, reasoner | $0.27–$0.55 | $1.10–$2.19 |
| Groq | llama-3.3-70b, llama-3.1-8b | $0.05–$0.59 | $0.08–$0.79 |

### Cost Calculation Formula

```
cost = (input_tokens / 1,000,000) × input_price + (output_tokens / 1,000,000) × output_price
```

### Token Extraction (Multi-Format)

```rust
fn extract_usage(payload: &str) -> (Option<u64>, Option<u64>) {
    // OpenAI:  usage.prompt_tokens, usage.completion_tokens
    // Claude:  usage.input_tokens, usage.output_tokens
    // Gemini:  usageMetadata.promptTokenCount, usageMetadata.candidatesTokenCount
}
```

## Configuration Changes

```yaml
model-prices:
  my-custom-model:
    input: 1.0              # USD per 1M input tokens
    output: 2.0             # USD per 1M output tokens
  gpt-4o:
    input: 2.50             # Override built-in price
    output: 10.0
```

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes       | `usage.prompt_tokens` / `usage.completion_tokens` |
| Claude   | Yes       | `usage.input_tokens` / `usage.output_tokens` |
| Gemini   | Yes       | `usageMetadata.promptTokenCount` / `candidatesTokenCount` |
| OpenAI-compat | Yes  | Same as OpenAI format |

## Task Breakdown

- [x] T1: `CostCalculator` + built-in price table in `cost.rs`
- [x] T2: `ModelPrice` config struct + `model_prices` in `Config`
- [x] T3: `extract_usage()` multi-format token extraction in `dispatch.rs`
- [x] T4: `inject_dispatch_meta()` — calculate cost + record metrics
- [x] T5: `RequestLogEntry.cost` field + logging integration
- [x] T6: `Metrics.total_cost_micro` + `cost_by_model` + `record_cost()`
- [x] T7: `config.example.yaml` documentation

## Test Strategy

- **Unit tests:** Cost calculation accuracy, price override priority, prefix stripping
- **Integration tests:** Dashboard logs API returns cost field
- **Manual verification:** Send requests, verify cost in request logs and metrics snapshot
