# Sentiment Evaluator Plugin

A Agent Replay plugin that evaluates the sentiment of LLM outputs.

## Description

This plugin analyzes the sentiment of text outputs from LLM traces using keyword-based sentiment analysis. It returns a score from 0 (negative) to 1 (positive), with 0.5 being neutral.

## Installation

```bash
# Build the plugin
agentreplay plugin build ./plugins/sentiment-evaluator

# Install the plugin
agentreplay plugin install ./plugins/sentiment-evaluator
```

## Usage

Once installed, the sentiment evaluator will be available for use in evaluations:

```python
# Via the Agent Replay API
from agentreplay import Agent Replay

ft = Agent Replay()

# Run sentiment evaluation on traces
results = ft.evaluate(
    trace_id="...",
    evaluators=["sentiment-evaluator"]
)

print(results["sentiment-evaluator"])
# {
#   "passed": True,
#   "confidence": 0.8,
#   "explanation": "Sentiment: positive (score: 0.75)",
#   "metrics": {
#     "sentiment_score": 0.75,
#     "positive_words": 5,
#     "negative_words": 1
#   }
# }
```

## Configuration

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `positive_threshold` | float | 0.6 | Score threshold for positive sentiment |
| `negative_threshold` | float | 0.4 | Score threshold for negative sentiment |

Configure in your Agent Replay settings:

```toml
[plugins.sentiment-evaluator]
positive_threshold = 0.7
negative_threshold = 0.3
```

## How it Works

1. Extracts text from trace output and LLM span outputs
2. Tokenizes and converts to lowercase
3. Counts occurrences of positive and negative sentiment words
4. Calculates a sentiment score: `(positive + 0.5*total) / (2*total)`
5. Classifies as positive (≥0.6), negative (≤0.4), or neutral

## Sentiment Word Lists

### Positive Words
good, great, excellent, amazing, wonderful, fantastic, helpful, thank, perfect, love, best, awesome, appreciate, happy, pleased, satisfied, brilliant, superb, outstanding, remarkable, exceptional, positive, success

### Negative Words
bad, terrible, awful, horrible, wrong, error, fail, hate, worst, poor, disappointed, frustrating, annoying, useless, broken, stupid, pathetic, disgusting, negative, problem, issue, bug, crash

## Building from Source

```bash
# Install dependencies
pip install agentreplay-plugin-sdk[build]

# Build to WASM
agentreplay-plugin-build evaluator.py -o sentiment_evaluator.wasm
```

## License

Apache-2.0
