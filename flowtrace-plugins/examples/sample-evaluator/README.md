# Sample Evaluator Plugin

A sample evaluator plugin that demonstrates how to create custom evaluators for Flowtrace.

## Overview

This plugin provides three sample evaluators:

1. **sentiment-check** - Checks the sentiment of LLM responses (returns score 0-1)
2. **length-check** - Ensures response length is within expected bounds (binary pass/fail)
3. **toxicity-filter** - Detects potentially toxic or harmful content (returns score 0-1)

## Installation

### From Local Directory

```bash
flowtrace plugin install ./plugins/sample-evaluator
```

### From Registry (once published)

```bash
flowtrace plugin install sample-evaluator
```

## Usage

### CLI

Run evaluation on a specific trace:

```bash
flowtrace sample-evaluator evaluate <trace-id>
```

Run batch evaluation on all traces in a session:

```bash
flowtrace sample-evaluator batch-evaluate --session <session-id>
```

### API

The evaluators are automatically registered and can be used through the Flowtrace API:

```rust
use flowtrace_plugins::PluginManager;
use flowtrace_core::Trace;

// Get the plugin manager
let manager = PluginManager::new(plugins_dir).await?;

// Run an evaluator from the plugin
let result = manager.run_evaluator(
    "sample-evaluator",
    "sentiment-check",
    &trace
).await?;

println!("Sentiment score: {}", result.score);
```

## Configuration

The evaluators can be configured in your Flowtrace configuration:

```toml
[plugins.sample-evaluator]
enabled = true

[plugins.sample-evaluator.evaluators.length-check]
min_length = 50
max_length = 2000

[plugins.sample-evaluator.evaluators.toxicity-filter]
threshold = 0.7
```

## Capabilities

This plugin requires the following capabilities:

- `trace_read` - Read access to traces and spans
- `eval_write` - Write access to evaluation results
- `env_vars` - Access to OPENAI_API_KEY, ANTHROPIC_API_KEY (optional, for LLM-based evaluators)

## Development

### Building

```bash
cargo build --release
```

### Testing

```bash
cargo test
```

### Packaging

```bash
flowtrace plugin pack ./plugins/sample-evaluator
```

This creates `sample-evaluator-0.1.0.ftplugin` that can be distributed.

## License

MIT
