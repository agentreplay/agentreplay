# Agentreplay Plugin SDK for Python

Build Agentreplay plugins in Python and compile them to WASM.

## Installation

```bash
pip install agentreplay-plugin-sdk
```

For building plugins to WASM:

```bash
pip install agentreplay-plugin-sdk[build]
```

## Quick Start

```python
from agentreplay_plugin import Evaluator, TraceContext, EvalResult, PluginMetadata, export

class MyEvaluator(Evaluator):
    def evaluate(self, trace: TraceContext) -> EvalResult:
        # Your evaluation logic
        score = self._calculate_score(trace)
        
        return EvalResult(
            evaluator_id="my-evaluator",
            passed=score > 0.7,
            confidence=score,
            explanation=f"Score: {score:.2f}"
        )
    
    def _calculate_score(self, trace: TraceContext) -> float:
        # Example: check completion rate
        completed = sum(1 for s in trace.spans if s.duration_us is not None)
        return completed / max(len(trace.spans), 1)
    
    def get_metadata(self) -> PluginMetadata:
        return PluginMetadata(
            id="my-evaluator",
            name="My Custom Evaluator",
            version="1.0.0",
            description="Evaluates traces using custom logic"
        )

# Export the plugin
export(MyEvaluator())
```

## Building

```bash
agentreplay-plugin-build src/evaluator.py -o my_plugin.wasm
```

Or using componentize-py directly:

```bash
componentize-py -d wit -w agentreplay-plugin componentize src/evaluator.py -o my_plugin.wasm
```

## Using Host Functions

```python
from agentreplay_plugin import Host

class MyEvaluator(Evaluator):
    def evaluate(self, trace: TraceContext) -> EvalResult:
        # Log messages
        Host.info("Starting evaluation")
        
        # Get configuration
        config = Host.get_config()
        threshold = config.get("threshold", 0.7)
        
        # Query other traces (requires trace-read capability)
        similar_traces = Host.query_traces(
            filter_json='{"model": "gpt-4"}',
            limit=10
        )
        
        # Make HTTP request (requires network capability)
        response = Host.http_request(
            method="POST",
            url="https://api.example.com/analyze",
            headers={"Content-Type": "application/json"},
            body=trace.output.encode()
        )
        
        # Generate embeddings (requires embedding capability)
        embedding = Host.embed_text(trace.output or "")
        
        return EvalResult(...)
```

## Plugin Types

### Evaluator

Evaluates traces and returns pass/fail with confidence score.

### EmbeddingProvider

Provides custom text embeddings.

### Exporter

Exports traces to custom formats.

## Configuration

Plugins can define configuration options in their manifest:

```toml
[plugin]
name = "my-evaluator"
version = "1.0.0"

[capabilities]
trace_read = true

[config]
threshold = { type = "float", default = 0.7, description = "Pass threshold" }
```

Access configuration in your plugin:

```python
config = Host.get_config()
threshold = config.get("threshold", 0.7)
```

## License

Apache-2.0
