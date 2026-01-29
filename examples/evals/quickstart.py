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
Agentreplay Evals Quick Start

A complete example showing the full evaluation workflow:
1. Create a dataset
2. Add test cases
3. Run evaluations
4. View results

Usage:
    python examples/evals/quickstart.py

This example uses no external dependencies (only Python stdlib).
"""

import json
import sys
import time
import urllib.request
import urllib.error
from datetime import datetime
from typing import Any


# Configuration
API_URL = "http://localhost:8000"
TIMEOUT = 30


def api_request(method: str, path: str, data: dict = None) -> dict:
    """Make API request."""
    url = f"{API_URL}{path}"
    headers = {"Content-Type": "application/json"}
    
    if data:
        body = json.dumps(data).encode("utf-8")
        req = urllib.request.Request(url, data=body, headers=headers, method=method)
    else:
        req = urllib.request.Request(url, headers=headers, method=method)
    
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as response:
            text = response.read().decode("utf-8")
            return json.loads(text) if text else {}
    except urllib.error.HTTPError as e:
        error_body = e.read().decode("utf-8")
        raise Exception(f"HTTP {e.code}: {error_body}")
    except urllib.error.URLError as e:
        raise Exception(f"Connection error: {e.reason}")


def check_server():
    """Check if Agentreplay server is running."""
    try:
        api_request("GET", "/health")
        return True
    except:
        return False


def create_sample_dataset():
    """Create a sample dataset for testing."""
    
    dataset_data = {
        "name": f"Quick Start Dataset {datetime.now().strftime('%H:%M:%S')}",
        "description": "Sample dataset for Agentreplay evals quick start",
        "test_cases": [
            {
                "input": "What is 2 + 2?",
                "expected_output": "4",
                "metadata": {"category": "math", "difficulty": "easy"}
            },
            {
                "input": "What is the largest planet in our solar system?",
                "expected_output": "Jupiter",
                "metadata": {"category": "science", "difficulty": "easy"}
            },
            {
                "input": "Translate 'hello' to Spanish",
                "expected_output": "hola",
                "metadata": {"category": "language", "difficulty": "easy"}
            },
            {
                "input": "What year did World War II end?",
                "expected_output": "1945",
                "metadata": {"category": "history", "difficulty": "medium"}
            },
            {
                "input": "What is the chemical symbol for gold?",
                "expected_output": "Au",
                "metadata": {"category": "chemistry", "difficulty": "medium"}
            }
        ]
    }
    
    return dataset_data


def simple_evaluate(expected: str, actual: str) -> tuple[bool, float, str]:
    """
    Simple evaluation: check if expected answer is in actual response.
    
    Returns (passed, score, explanation)
    """
    expected_lower = expected.lower().strip()
    actual_lower = actual.lower().strip()
    
    # Exact match
    if expected_lower == actual_lower:
        return True, 1.0, "Exact match"
    
    # Contains match
    if expected_lower in actual_lower:
        return True, 0.9, "Contains expected answer"
    
    # Word overlap
    expected_words = set(expected_lower.split())
    actual_words = set(actual_lower.split())
    overlap = len(expected_words & actual_words)
    
    if overlap > 0:
        score = overlap / len(expected_words)
        return score >= 0.5, score, f"Partial match ({overlap}/{len(expected_words)} words)"
    
    return False, 0.0, "No match"


def mock_agent(prompt: str) -> str:
    """Mock agent that returns simple answers."""
    answers = {
        "2 + 2": "The answer is 4.",
        "largest planet": "Jupiter is the largest planet in our solar system.",
        "hello": "The Spanish translation of 'hello' is 'hola'.",
        "world war ii": "World War II ended in 1945.",
        "gold": "The chemical symbol for gold is Au (from Latin 'aurum')."
    }
    
    prompt_lower = prompt.lower()
    for key, answer in answers.items():
        if key in prompt_lower:
            return answer
    
    return "I'm not sure about that."


def run_quickstart():
    """Run the quick start demo."""
    
    print("=" * 60)
    print("🚀 Agentreplay Evals Quick Start")
    print("=" * 60)
    
    # Check server
    print("\n1️⃣  Checking Agentreplay server...")
    if not check_server():
        print("   ❌ Server not running at", API_URL)
        print("\n   To start the server:")
        print("   cargo run --bin agentreplay-server")
        print("\n   Or run in demo mode without server (see below)")
        demo_mode = True
    else:
        print("   ✅ Server is running")
        demo_mode = False
    
    # Create dataset
    print("\n2️⃣  Creating sample dataset...")
    dataset_data = create_sample_dataset()
    print(f"   Name: {dataset_data['name']}")
    print(f"   Test cases: {len(dataset_data['test_cases'])}")
    
    if not demo_mode:
        try:
            result = api_request("POST", "/api/v1/evals/datasets", dataset_data)
            dataset_id = result.get("id", "unknown")
            print(f"   ✅ Created with ID: {dataset_id}")
        except Exception as e:
            print(f"   ⚠️  API call failed: {e}")
            print("   Running in demo mode...")
            demo_mode = True
    
    # Run evaluations
    print("\n3️⃣  Running evaluations...")
    print("   Using mock agent for demonstration\n")
    
    results = []
    for i, tc in enumerate(dataset_data["test_cases"]):
        # Get agent response
        response = mock_agent(tc["input"])
        
        # Evaluate
        passed, score, explanation = simple_evaluate(tc["expected_output"], response)
        
        result = {
            "input": tc["input"],
            "expected": tc["expected_output"],
            "actual": response,
            "passed": passed,
            "score": score,
            "explanation": explanation
        }
        results.append(result)
        
        # Print result
        status = "✅" if passed else "❌"
        print(f"   {status} Test {i+1}: {tc['input'][:35]}...")
        print(f"      Expected: {tc['expected_output']}")
        print(f"      Got: {response[:50]}...")
        print(f"      Score: {score:.2f} ({explanation})\n")
    
    # Summary
    passed_count = sum(1 for r in results if r["passed"])
    avg_score = sum(r["score"] for r in results) / len(results)
    
    print("=" * 60)
    print("📊 Evaluation Summary")
    print("=" * 60)
    print(f"""
   Total Test Cases: {len(results)}
   Passed: {passed_count} ✅
   Failed: {len(results) - passed_count} ❌
   Pass Rate: {passed_count/len(results)*100:.1f}%
   Average Score: {avg_score:.2f}
    """)
    
    # Save results
    output = {
        "dataset": dataset_data["name"],
        "timestamp": datetime.now().isoformat(),
        "summary": {
            "total": len(results),
            "passed": passed_count,
            "failed": len(results) - passed_count,
            "pass_rate": passed_count / len(results),
            "avg_score": avg_score
        },
        "results": results
    }
    
    output_file = "/tmp/evals_quickstart_results.json"
    with open(output_file, 'w') as f:
        json.dump(output, f, indent=2)
    print(f"   Results saved to: {output_file}")
    
    # Next steps
    print("\n" + "=" * 60)
    print("📚 Next Steps")
    print("=" * 60)
    print("""
   1. Explore more examples:
      python examples/evals/basic_dataset.py
      python examples/evals/custom_evaluator.py
      python examples/evals/prod_to_eval.py

   2. Create your own dataset:
      - Copy examples/datasets/customer-support.json
      - Add your test cases
      - Import with scripts/import_datasets.py

   3. Connect a real LLM:
      - Install: pip install openai
      - Set OPENAI_API_KEY environment variable
      - Run: python examples/evals/run_evaluation.py

   4. View docs:
      - examples/evals/README.md
      - docs/EVALUATION_API_GUIDE.md
    """)
    
    print("=" * 60)
    print("✨ Quick start complete!")
    print("=" * 60)


if __name__ == "__main__":
    run_quickstart()
