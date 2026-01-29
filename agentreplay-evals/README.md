# Agentreplay Evaluators

A modular, extensible evaluation framework for LLM agent traces. Evaluate your agent outputs for hallucinations, relevance, toxicity, performance, and cost.

[![Tests](https://img.shields.io/badge/tests-22%20passing-brightgreen)](.)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](../LICENSE)

## Features

- **🔍 Built-in Evaluators**: 5 production-ready evaluators for common use cases
- **🚀 High Performance**: 50-100 traces/sec, with caching up to 500 traces/sec
- **💰 Cost Tracking**: Built-in cost analysis for all major LLM providers
- **⚡ Async/Parallel**: Tokio-based async execution with parallel batch processing
- **💾 Smart Caching**: Moka-based LRU cache with configurable TTL
- **🎯 LLM-as-Judge**: Use GPT-4/Claude to evaluate LLM outputs
- **🔧 Extensible**: Easy-to-implement trait for custom evaluators

## Quick Start

```rust
use agentreplay_evals::{EvaluatorRegistry, evaluators::*};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Create registry and register evaluators
    let registry = EvaluatorRegistry::new();
    registry.register(Arc::new(LatencyBenchmark::new())).unwrap();
    registry.register(Arc::new(CostAnalyzer::new())).unwrap();

    // Evaluate a trace
    let results = registry.evaluate_trace(
        &trace,
        vec!["latency_v1".to_string(), "cost_v1".to_string()]
    ).await.unwrap();

    // Check results
    for (id, result) in results {
        println!("{}: {}", id, if result.passed { "✓" } else { "✗" });
    }
}
```

## Built-in Evaluators

| Evaluator | Purpose | Cost | Latency | Use Case |
|-----------|---------|------|---------|----------|
| **HallucinationDetector** | Detect hallucinated info | $0.0001 | ~1500ms | Quality assurance |
| **RelevanceEvaluator** | Measure input/output relevance | Free | ~5ms | Content filtering |
| **ToxicityDetector** | Detect unsafe content | Free | ~2ms | Safety monitoring |
| **LatencyBenchmark** | Analyze performance | Free | ~5ms | Performance tuning |
| **CostAnalyzer** | Track token costs | Free | ~5ms | Budget management |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
agentreplay-evals = { path = "../agentreplay-evals" }
tokio = { version = "1.35", features = ["full"] }
```

## Documentation

- **[Usage Guide](./USAGE.md)** - Comprehensive guide with examples
- **[Implementation Summary](./IMPLEMENTATION_SUMMARY.md)** - Technical details and architecture
- **[Implementation Plan](../IMPLEMENTATION_PLAN_PRIORITIES_5-10.md)** - Roadmap for future features

## Examples

### Hallucination Detection

```rust
use agentreplay_evals::evaluators::HallucinationDetector;
use agentreplay_evals::llm_client::OpenAIClient;

let llm_client = Arc::new(OpenAIClient::new(
    std::env::var("OPENAI_API_KEY").unwrap(),
    "gpt-4o-mini".to_string()
));

let detector = HallucinationDetector::new(llm_client)
    .with_threshold(0.3);  // Fail if >30% hallucinated

registry.register(Arc::new(detector)).unwrap();
```

### Performance Monitoring

```rust
use agentreplay_evals::evaluators::LatencyBenchmark;

let benchmark = LatencyBenchmark::new()
    .with_p99_threshold_ms(5000);  // Alert if p99 > 5s

let result = benchmark.evaluate(&trace).await.unwrap();

println!("p50: {:.0}ms, p95: {:.0}ms, p99: {:.0}ms",
    result.metrics["p50_ms"],
    result.metrics["p95_ms"],
    result.metrics["p99_ms"]
);
```

### Cost Tracking

```rust
use agentreplay_evals::evaluators::CostAnalyzer;

let analyzer = CostAnalyzer::new()
    .with_budget_threshold(1.0);  // Alert if cost > $1

let result = analyzer.evaluate(&trace).await.unwrap();

println!("Total cost: ${:.4}", result.metrics["total_cost_usd"]);
println!("Total tokens: {}", result.metrics["total_tokens"]);
```

### Batch Evaluation

```rust
let evaluators = vec![
    "hallucination_v1".to_string(),
    "relevance_v1".to_string(),
    "latency_v1".to_string(),
];

// Process 100 traces in parallel
let results = registry.evaluate_batch(traces, evaluators).await;

let passed = results.iter()
    .filter(|r| r.as_ref().map(|m| m.values().all(|v| v.passed)).unwrap_or(false))
    .count();

println!("{}/{} traces passed all evaluations", passed, results.len());
```

## Custom Evaluators

Implement the `Evaluator` trait to create custom evaluators:

```rust
use agentreplay_evals::{Evaluator, TraceContext, EvalResult, EvalError};
use async_trait::async_trait;

pub struct CustomEvaluator;

#[async_trait]
impl Evaluator for CustomEvaluator {
    fn id(&self) -> &str {
        "custom_v1"
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        // Your evaluation logic here
        todo!()
    }

    fn metadata(&self) -> EvaluatorMetadata {
        // Evaluator metadata
        todo!()
    }
}
```

See [USAGE.md](./USAGE.md) for complete examples.

## Performance

### Throughput
- **Without LLM evaluators**: 50-100 traces/sec
- **With caching (80% hit rate)**: 200-500 traces/sec
- **LLM evaluators**: Limited by API rate limits (~10-50 req/sec)

### Caching
```rust
use agentreplay_evals::EvalConfig;

let config = EvalConfig {
    enable_cache: true,
    cache_ttl_secs: 3600,     // 1 hour TTL
    max_concurrent: 10,        // Parallel evaluations
    timeout_secs: 30,          // Per-evaluation timeout
};

let registry = EvaluatorRegistry::with_config(config);
```

## Architecture

```
┌─────────────────────────────────────────┐
│      EvaluatorRegistry                  │
│  - Register/manage evaluators           │
│  - Parallel execution                   │
│  - Result caching                       │
└────────────┬────────────────────────────┘
             │
    ┌────────┴────────┐
    │                 │
┌───▼────┐    ┌──────▼─────┐
│ Built-in│    │  Custom    │
│Evaluators│   │Evaluators  │
└────┬────┘    └──────┬─────┘
     │                │
     ├──────┬─────────┼──────────┐
     │      │         │          │
┌────▼──┐ ┌─▼──┐  ┌──▼───┐  ┌──▼────┐
│Halluc.│ │Rel.│  │Toxic.│  │Custom │
└───────┘ └────┘  └──────┘  └───────┘
```

## Testing

Run the test suite:

```bash
cargo test -p agentreplay-evals
```

All 22 tests should pass, covering:
- Individual evaluator functionality
- Registry operations
- Caching behavior
- Error handling
- Batch evaluation

## Contributing

We welcome contributions! Areas for improvement:

1. **New Evaluators**: Factuality, coherence, creativity, etc.
2. **Embeddings Support**: Better semantic similarity in RelevanceEvaluator
3. **ML Models**: Integration with Perspective API for toxicity
4. **Prompt Templates**: Optimized prompts for LLM-as-judge
5. **Benchmarks**: Performance benchmarking suite

See [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

## Roadmap

### Phase 1: Core Evaluators ✅ (Complete)
- [x] Hallucination detection
- [x] Relevance evaluation
- [x] Toxicity detection
- [x] Latency benchmarking
- [x] Cost analysis

### Phase 2: Advanced Features (Planned)
- [ ] Dataset management system
- [ ] Test suite execution engine
- [ ] Prompt templates directory
- [ ] Evaluation API endpoints
- [ ] Dashboard integration

### Phase 3: Enterprise Features (Future)
- [ ] Time-series analytics
- [ ] Anomaly detection
- [ ] Budget alerts
- [ ] A/B testing support
- [ ] Compliance reports

See [IMPLEMENTATION_PLAN_PRIORITIES_5-10.md](../IMPLEMENTATION_PLAN_PRIORITIES_5-10.md) for details.

## License

Apache 2.0 - see [LICENSE](../LICENSE) for details.

## Credits

Built by the Agentreplay team as part of the Agentreplay agent observability platform.

Special thanks to:
- OpenAI for GPT models used in LLM-as-judge
- Anthropic for Claude models
- Tokio for async runtime
- Moka for caching implementation

## Support

- **Documentation**: [USAGE.md](./USAGE.md)
- **Issues**: [GitHub Issues](https://github.com/sochdb/agentreplay/issues)
- **Discussions**: [GitHub Discussions](https://github.com/sochdb/agentreplay/discussions)

---

**Status**: Production-ready for non-LLM evaluators, Beta for LLM-based evaluators.

**Version**: 0.1.0 (Initial release)
