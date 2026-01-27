# Flowtrace Evals Examples

This directory contains example scripts demonstrating how to use Flowtrace's evaluation system to test and benchmark AI agents.

## Overview

Flowtrace Evals provides:
- **Datasets**: Collections of test cases for evaluating AI agents
- **Evaluators**: Built-in and custom evaluators for measuring quality
- **Runs**: Track evaluation runs with metrics and comparisons

## Examples

### 1. Basic Dataset Workflow (`basic_dataset.py`)
Shows how to create, populate, and query datasets.

```bash
python examples/evals/basic_dataset.py
```

### 2. Running Evaluations (`run_evaluation.py`)
Demonstrates running an agent against a dataset with evaluators.

```bash
python examples/evals/run_evaluation.py \
    --dataset customer-support \
    --evaluators relevance,accuracy
```

### 3. Custom Evaluators (`custom_evaluator.py`)
Shows how to create and register custom evaluators.

```bash
python examples/evals/custom_evaluator.py
```

### 4. Production to Eval Pipeline (`prod_to_eval.py`)
Demonstrates capturing production traces and adding them to evaluation datasets.

```bash
python examples/evals/prod_to_eval.py
```

### 5. Agent Evaluation Scenarios (`agent_eval_scenarios.py`)
Comprehensive test scenarios for agent evaluation with happy and unhappy paths.

```bash
# Run all scenarios
python examples/evals/agent_eval_scenarios.py

# Verbose output with metrics
python examples/evals/agent_eval_scenarios.py --verbose

# Export scenarios to JSON
python examples/evals/agent_eval_scenarios.py --export scenarios.json

# Run specific category
python examples/evals/agent_eval_scenarios.py --category happy_path
```

Categories:
- `happy_path`: Perfect agent behavior (should pass)
- `wrong_tool`: Agent uses incorrect tools (should fail - precision issues)
- `inefficient`: Agent takes unnecessary steps (should fail - efficiency issues)
- `missing_tool`: Agent skips required tools (should fail - recall issues)
- `hallucination`: Agent makes unsupported claims (should fail)
- `backtracking`: Agent tries wrong tools first (should fail)
- `ambiguous`: Queries with multiple interpretations

### 6. Agent Eval Demo (`agent_eval_demo.py`)
Simple demo showing metric calculations and evaluation flow.

```bash
python examples/evals/agent_eval_demo.py
```

## Prerequisites

```bash
# Install Flowtrace Python SDK
pip install flowtrace

# Or install from source
cd sdks/python
pip install -e .
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/evals/datasets` | POST | Create a new dataset |
| `/api/v1/evals/datasets` | GET | List all datasets |
| `/api/v1/evals/datasets/{id}` | GET | Get dataset details |
| `/api/v1/evals/datasets/{id}/items` | POST | Add items to dataset |
| `/api/v1/evals/runs` | POST | Create evaluation run |
| `/api/v1/evals/runs/{id}` | GET | Get run results |
| `/api/v1/evals/runs/{id}/results` | POST | Add evaluation result |

## Dataset Format

```json
{
  "name": "My Dataset",
  "description": "Description of test cases",
  "test_cases": [
    {
      "input": "User query or prompt",
      "expected_output": "Expected response (optional)",
      "metadata": {
        "category": "category_name",
        "difficulty": "easy|medium|hard"
      }
    }
  ]
}
```

## Evaluation Result Format

```json
{
  "test_case_id": "uuid",
  "trace_id": "trace-uuid",
  "passed": true,
  "score": 0.95,
  "metrics": {
    "relevance": 0.92,
    "accuracy": 0.98,
    "latency_ms": 245
  },
  "explanation": "Response matches expected output with high accuracy"
}
```

## Best Practices

1. **Organize datasets by use case**: Create separate datasets for different evaluation scenarios
2. **Include diverse test cases**: Mix easy and hard cases to get a complete picture
3. **Use meaningful metadata**: Categories and tags help filter and analyze results
4. **Track over time**: Run evaluations regularly to detect regressions
5. **Combine evaluators**: Use multiple evaluators for comprehensive assessment
