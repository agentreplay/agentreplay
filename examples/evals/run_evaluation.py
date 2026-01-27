#!/usr/bin/env python3

# Copyright 2025 Sushanth (https://github.com/sushanthpy)
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""
Run Evaluation Example

This example demonstrates how to:
1. Load a dataset
2. Run an AI agent against each test case
3. Evaluate results using built-in evaluators
4. Aggregate and report metrics

Usage:
    python examples/evals/run_evaluation.py --dataset-id <id>
    
    # With mock agent (no API key needed):
    python examples/evals/run_evaluation.py --dataset-id <id> --mock

Requirements:
    - Flowtrace server running
    - openai package (optional, for real LLM calls)
"""

import argparse
import json
import sys
import time
from dataclasses import dataclass
from datetime import datetime
from typing import Any, Optional

try:
    import httpx
except ImportError:
    print("Please install httpx: pip install httpx")
    sys.exit(1)


@dataclass
class EvalResult:
    """Result of a single test case evaluation."""
    test_case_id: str
    input: str
    expected_output: str
    actual_output: str
    passed: bool
    score: float
    latency_ms: float
    metrics: dict
    explanation: str = ""


class FlowtraceEvalsClient:
    """Client for Flowtrace Evals API."""
    
    def __init__(self, base_url: str = "http://localhost:8000"):
        self.base_url = base_url.rstrip("/")
        self.client = httpx.Client(timeout=60.0)
    
    def get_dataset(self, dataset_id: str) -> dict:
        """Get dataset by ID."""
        response = self.client.get(f"{self.base_url}/api/v1/evals/datasets/{dataset_id}")
        response.raise_for_status()
        return response.json()
    
    def get_items(self, dataset_id: str) -> list:
        """Get all items in a dataset."""
        response = self.client.get(f"{self.base_url}/api/v1/evals/datasets/{dataset_id}/items")
        response.raise_for_status()
        return response.json()
    
    def create_run(
        self,
        dataset_id: str,
        name: str,
        config: dict = None
    ) -> dict:
        """Create a new evaluation run."""
        response = self.client.post(
            f"{self.base_url}/api/v1/evals/runs",
            json={
                "dataset_id": dataset_id,
                "name": name,
                "config": config or {},
                "status": "running"
            }
        )
        response.raise_for_status()
        return response.json()
    
    def add_result(
        self,
        run_id: str,
        result: EvalResult
    ) -> dict:
        """Add evaluation result to a run."""
        response = self.client.post(
            f"{self.base_url}/api/v1/evals/runs/{run_id}/results",
            json={
                "test_case_id": result.test_case_id,
                "passed": result.passed,
                "score": result.score,
                "latency_ms": result.latency_ms,
                "metrics": result.metrics,
                "explanation": result.explanation,
                "actual_output": result.actual_output
            }
        )
        response.raise_for_status()
        return response.json()
    
    def complete_run(self, run_id: str, summary: dict) -> dict:
        """Mark run as complete with summary."""
        response = self.client.patch(
            f"{self.base_url}/api/v1/evals/runs/{run_id}",
            json={
                "status": "completed",
                "summary": summary
            }
        )
        response.raise_for_status()
        return response.json()


class MockAgent:
    """Mock agent for testing without API keys."""
    
    def __init__(self):
        self.responses = {
            "capital": "Paris is the capital of France.",
            "relativity": "Einstein's theory of relativity describes how space and time are linked.",
            "python": "Python is great for backend and data science, JavaScript runs in browsers.",
            "photosynthesis": "Plants convert sunlight and CO2 into glucose and oxygen.",
            "quantum": "Quantum computers use qubits that can be 0, 1, or both at once!"
        }
    
    def generate(self, prompt: str) -> tuple[str, float]:
        """Generate response with simulated latency."""
        time.sleep(0.1)  # Simulate API call
        
        # Match based on keywords
        prompt_lower = prompt.lower()
        for key, response in self.responses.items():
            if key in prompt_lower:
                return response, 100 + (len(response) * 2)  # Simulated latency
        
        return "I don't know the answer to that question.", 150


class OpenAIAgent:
    """Real OpenAI agent for production use."""
    
    def __init__(self, model: str = "gpt-4o-mini"):
        try:
            import openai
            self.client = openai.OpenAI()
            self.model = model
        except ImportError:
            raise ImportError("Please install openai: pip install openai")
    
    def generate(self, prompt: str) -> tuple[str, float]:
        """Generate response with actual API call."""
        start = time.time()
        
        response = self.client.chat.completions.create(
            model=self.model,
            messages=[{"role": "user", "content": prompt}],
            max_tokens=200
        )
        
        latency_ms = (time.time() - start) * 1000
        return response.choices[0].message.content, latency_ms


class Evaluator:
    """Base class for evaluators."""
    
    def evaluate(
        self,
        input_text: str,
        expected: str,
        actual: str,
        metadata: dict = None
    ) -> tuple[float, str]:
        """Evaluate and return (score, explanation)."""
        raise NotImplementedError


class ExactMatchEvaluator(Evaluator):
    """Exact string match evaluator."""
    
    def evaluate(self, input_text: str, expected: str, actual: str, metadata: dict = None) -> tuple[float, str]:
        if expected.strip().lower() == actual.strip().lower():
            return 1.0, "Exact match"
        return 0.0, "No match"


class ContainsEvaluator(Evaluator):
    """Check if expected keywords are in actual output."""
    
    def evaluate(self, input_text: str, expected: str, actual: str, metadata: dict = None) -> tuple[float, str]:
        expected_words = set(expected.lower().split())
        actual_words = set(actual.lower().split())
        
        overlap = len(expected_words & actual_words)
        total = len(expected_words)
        
        if total == 0:
            return 1.0, "No expected words to match"
        
        score = overlap / total
        return score, f"Matched {overlap}/{total} expected words"


class SemanticSimilarityEvaluator(Evaluator):
    """Semantic similarity using word overlap (simple version)."""
    
    def evaluate(self, input_text: str, expected: str, actual: str, metadata: dict = None) -> tuple[float, str]:
        # Simple Jaccard similarity
        expected_words = set(expected.lower().split())
        actual_words = set(actual.lower().split())
        
        intersection = len(expected_words & actual_words)
        union = len(expected_words | actual_words)
        
        if union == 0:
            return 1.0, "Both empty"
        
        score = intersection / union
        return score, f"Jaccard similarity: {score:.2f}"


def run_evaluation(
    client: FlowtraceEvalsClient,
    dataset_id: str,
    agent: Any,
    evaluators: list[Evaluator],
    run_name: str = None
) -> dict:
    """Run evaluation on dataset."""
    
    # Get dataset info
    print(f"\n📦 Loading dataset {dataset_id}...")
    try:
        dataset = client.get_dataset(dataset_id)
        items = client.get_items(dataset_id)
        print(f"   Dataset: {dataset['name']}")
        print(f"   Items: {len(items)}")
    except Exception as e:
        print(f"❌ Failed to load dataset: {e}")
        return {}
    
    # Create run (optional - depends on API availability)
    run_id = None
    run_name = run_name or f"Eval Run {datetime.now().strftime('%Y%m%d_%H%M%S')}"
    
    # Run evaluations
    results: list[EvalResult] = []
    
    print(f"\n🏃 Running evaluation...")
    for i, item in enumerate(items):
        print(f"   [{i+1}/{len(items)}] {item['input'][:40]}...")
        
        # Generate response
        actual_output, latency_ms = agent.generate(item['input'])
        
        # Run all evaluators
        scores = {}
        explanations = []
        for evaluator in evaluators:
            name = evaluator.__class__.__name__
            score, explanation = evaluator.evaluate(
                item['input'],
                item.get('expected_output', ''),
                actual_output,
                item.get('metadata', {})
            )
            scores[name] = score
            explanations.append(f"{name}: {explanation}")
        
        # Calculate overall score
        overall_score = sum(scores.values()) / len(scores) if scores else 0
        passed = overall_score >= 0.5
        
        result = EvalResult(
            test_case_id=item.get('id', str(i)),
            input=item['input'],
            expected_output=item.get('expected_output', ''),
            actual_output=actual_output,
            passed=passed,
            score=overall_score,
            latency_ms=latency_ms,
            metrics=scores,
            explanation="; ".join(explanations)
        )
        results.append(result)
        
        # Report result
        status = "✅" if passed else "❌"
        print(f"      {status} Score: {overall_score:.2f} | Latency: {latency_ms:.0f}ms")
    
    # Aggregate results
    summary = {
        "total": len(results),
        "passed": sum(1 for r in results if r.passed),
        "failed": sum(1 for r in results if not r.passed),
        "pass_rate": sum(1 for r in results if r.passed) / len(results) if results else 0,
        "avg_score": sum(r.score for r in results) / len(results) if results else 0,
        "avg_latency_ms": sum(r.latency_ms for r in results) / len(results) if results else 0,
        "evaluators_used": [e.__class__.__name__ for e in evaluators]
    }
    
    return {
        "run_name": run_name,
        "dataset_id": dataset_id,
        "dataset_name": dataset['name'],
        "summary": summary,
        "results": [
            {
                "input": r.input[:50] + "..." if len(r.input) > 50 else r.input,
                "passed": r.passed,
                "score": r.score,
                "latency_ms": r.latency_ms,
                "metrics": r.metrics
            }
            for r in results
        ]
    }


def print_summary(evaluation: dict):
    """Print evaluation summary."""
    summary = evaluation.get("summary", {})
    
    print("\n" + "=" * 60)
    print("📊 EVALUATION SUMMARY")
    print("=" * 60)
    print(f"Run: {evaluation['run_name']}")
    print(f"Dataset: {evaluation['dataset_name']}")
    print(f"\nResults:")
    print(f"  Total: {summary['total']}")
    print(f"  Passed: {summary['passed']} ✅")
    print(f"  Failed: {summary['failed']} ❌")
    print(f"  Pass Rate: {summary['pass_rate']:.1%}")
    print(f"\nMetrics:")
    print(f"  Avg Score: {summary['avg_score']:.2f}")
    print(f"  Avg Latency: {summary['avg_latency_ms']:.0f}ms")
    print(f"\nEvaluators: {', '.join(summary['evaluators_used'])}")
    print("=" * 60)


def main():
    parser = argparse.ArgumentParser(description="Run Flowtrace evaluation")
    parser.add_argument("--dataset-id", required=True, help="Dataset ID to evaluate")
    parser.add_argument("--api-url", default="http://localhost:8000", help="Flowtrace API URL")
    parser.add_argument("--mock", action="store_true", help="Use mock agent instead of OpenAI")
    parser.add_argument("--model", default="gpt-4o-mini", help="OpenAI model to use")
    parser.add_argument("--output", help="Output file for results JSON")
    
    args = parser.parse_args()
    
    # Initialize client
    client = FlowtraceEvalsClient(args.api_url)
    
    # Initialize agent
    if args.mock:
        print("Using mock agent")
        agent = MockAgent()
    else:
        print(f"Using OpenAI agent ({args.model})")
        try:
            agent = OpenAIAgent(args.model)
        except ImportError as e:
            print(f"❌ {e}")
            print("Use --mock flag to run without OpenAI")
            sys.exit(1)
    
    # Initialize evaluators
    evaluators = [
        ContainsEvaluator(),
        SemanticSimilarityEvaluator()
    ]
    
    # Run evaluation
    evaluation = run_evaluation(
        client=client,
        dataset_id=args.dataset_id,
        agent=agent,
        evaluators=evaluators
    )
    
    # Print summary
    print_summary(evaluation)
    
    # Save results
    if args.output:
        with open(args.output, 'w') as f:
            json.dump(evaluation, f, indent=2)
        print(f"\n📁 Results saved to {args.output}")


if __name__ == "__main__":
    main()
