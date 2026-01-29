# Agentreplay Example Datasets

This directory contains example datasets for common LLM evaluation use cases. These datasets can be used with Agentreplay's evaluation API to test and benchmark your AI agents.

## Available Datasets

### 1. Customer Support QA (`customer-support.json`)
- **Purpose**: Evaluate AI agents handling customer support queries
- **Size**: 10 test cases
- **Categories**: Account management, billing, technical support, integrations
- **Metrics to evaluate**: Accuracy, helpfulness, tone, completeness

### 2. Code Generation (`code-generation.json`)
- **Purpose**: Test code generation capabilities across multiple languages
- **Size**: 10 test cases
- **Languages**: Python, JavaScript, SQL, React
- **Metrics to evaluate**: Syntax correctness, efficiency, completeness, best practices

### 3. Hallucination Detection (`hallucination-detection.json`)
- **Purpose**: Identify factual errors and hallucinations in AI responses
- **Size**: 10 test cases
- **Categories**: Factual questions, anachronisms, fictional vs real, current events
- **Metrics to evaluate**: Factual accuracy, uncertainty handling, grounding

### 4. Sentiment Analysis (`sentiment-analysis.json`)
- **Purpose**: Test sentiment classification accuracy
- **Size**: 10 test cases
- **Sentiments**: Positive, negative, neutral, mixed
- **Metrics to evaluate**: Sentiment accuracy, intensity scoring, aspect detection

## Importing Datasets

### Using the API

```bash
# Import a dataset via HTTP POST
curl -X POST http://localhost:8000/api/v1/evals/datasets \
  -H "Content-Type: application/json" \
  -d @customer-support.json
```

### Using Python SDK

```python
from agentreplay import AgentreplayClient
import json

client = AgentreplayClient(api_url="http://localhost:8000")

# Load dataset
with open("customer-support.json") as f:
    dataset = json.load(f)

# Create dataset
response = client.create_dataset(
    name=dataset["name"],
    description=dataset["description"],
    test_cases=dataset["test_cases"]
)

print(f"Dataset created with ID: {response['id']}")
```

### Using the Import Script

```bash
# Import all datasets
python scripts/import_datasets.py --all

# Import specific dataset
python scripts/import_datasets.py --file customer-support.json
```

## Running Evaluations

### Example: Customer Support Evaluation

```python
from agentreplay import AgentreplayClient

client = AgentreplayClient(api_url="http://localhost:8000")

# Create evaluation run
run = client.create_eval_run(
    dataset_id="<dataset-id>",
    name="Customer Support Agent v1",
    agent_id="support-agent-v1",
    model="gpt-4"
)

# Run tests
for test_case in dataset["test_cases"]:
    # Your agent logic here
    response = your_agent.process(test_case["input"])

    # Record result
    client.add_eval_result(
        run_id=run["id"],
        test_case_id=test_case["id"],
        trace_id=response["trace_id"],
        passed=evaluate_response(response, test_case),
        metrics={
            "accuracy": calculate_accuracy(response, test_case),
            "relevance": calculate_relevance(response, test_case)
        }
    )

# Mark run as completed
client.update_run_status(run["id"], "completed")

# Get results
results = client.get_eval_run(run["id"])
print(f"Pass rate: {results['pass_rate']}")
```

## Dataset Format

All datasets follow this JSON structure:

```json
{
  "name": "Dataset Name",
  "description": "Dataset description",
  "test_cases": [
    {
      "input": "The input text or prompt",
      "expected_output": "Expected response (optional)",
      "metadata": {
        "category": "categorization",
        "difficulty": "easy|medium|hard",
        "custom_field": "custom value"
      }
    }
  ]
}
```

## Creating Custom Datasets

You can create custom datasets by following the same format:

```python
import json

custom_dataset = {
    "name": "My Custom Dataset",
    "description": "Description of what this dataset tests",
    "test_cases": [
        {
            "input": "Test input 1",
            "expected_output": "Expected output 1",
            "metadata": {
                "category": "category1",
                "tags": ["tag1", "tag2"]
            }
        }
    ]
}

# Save to file
with open("my-dataset.json", "w") as f:
    json.dump(custom_dataset, f, indent=2)
```

## Best Practices

1. **Diverse Test Cases**: Include a variety of scenarios to thoroughly test your agent
2. **Clear Expected Outputs**: Provide clear expected outputs when possible
3. **Rich Metadata**: Add metadata to help analyze results by category, difficulty, etc.
4. **Regular Updates**: Keep datasets updated as your use cases evolve
5. **Version Control**: Track dataset versions to compare results over time

## Evaluation Metrics

Common metrics to track:

- **Accuracy**: How often does the agent produce correct responses?
- **Relevance**: Are responses relevant to the input?
- **Hallucination Rate**: How often does the agent make up facts?
- **Consistency**: Do similar inputs produce similar outputs?
- **Latency**: Response time for each test case
- **Cost**: Token usage and estimated cost per test case

## CI/CD Integration

See the `.github/workflows/` directory for examples of running evaluations in CI/CD pipelines.

## Contributing

To contribute new example datasets:

1. Follow the dataset format above
2. Include at least 10 diverse test cases
3. Add clear descriptions and metadata
4. Submit a pull request with your dataset

## License

These example datasets are provided under the same license as Agentreplay.
