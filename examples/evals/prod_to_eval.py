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
Production to Evaluation Pipeline Example

This example demonstrates how to:
1. Capture production traces
2. Flag poor quality responses
3. Add flagged traces to evaluation datasets
4. Run regression tests with improved agents

This creates a feedback loop where production failures become
test cases for future agent improvements.

Usage:
    python examples/evals/prod_to_eval.py
"""

import json
import sys
import time
import uuid
from dataclasses import dataclass, asdict
from datetime import datetime
from typing import Optional


@dataclass
class ProductionTrace:
    """Represents a production trace."""
    trace_id: str
    input: str
    output: str
    latency_ms: float
    model: str
    timestamp: str
    user_feedback: Optional[str] = None  # "positive", "negative", None
    quality_score: Optional[float] = None
    error: Optional[str] = None


class ProductionMonitor:
    """
    Monitors production traces and flags poor quality responses.
    
    In a real system, this would:
    - Connect to your trace storage
    - Apply quality evaluators in real-time
    - Alert on quality degradation
    """
    
    def __init__(self, quality_threshold: float = 0.7):
        self.quality_threshold = quality_threshold
        self.flagged_traces: list[ProductionTrace] = []
    
    def evaluate_trace(self, trace: ProductionTrace) -> float:
        """
        Evaluate trace quality.
        
        In production, this would use sophisticated evaluators.
        Here we use simple heuristics for demonstration.
        """
        score = 1.0
        
        # Check for empty or very short responses
        if len(trace.output) < 10:
            score -= 0.3
        
        # Check for error responses
        if trace.error:
            score -= 0.5
        
        # Check for uncertainty patterns
        uncertainty_phrases = [
            "i don't know",
            "i'm not sure",
            "i cannot help",
            "i apologize, but"
        ]
        output_lower = trace.output.lower()
        for phrase in uncertainty_phrases:
            if phrase in output_lower:
                score -= 0.2
                break
        
        # Penalize high latency
        if trace.latency_ms > 5000:
            score -= 0.1
        
        # Factor in user feedback
        if trace.user_feedback == "negative":
            score -= 0.4
        elif trace.user_feedback == "positive":
            score += 0.1
        
        return max(0, min(1, score))
    
    def process_trace(self, trace: ProductionTrace) -> bool:
        """
        Process a production trace and flag if low quality.
        
        Returns True if trace was flagged.
        """
        score = self.evaluate_trace(trace)
        trace.quality_score = score
        
        if score < self.quality_threshold:
            self.flagged_traces.append(trace)
            return True
        
        return False
    
    def get_flagged_traces(self) -> list[ProductionTrace]:
        """Get all flagged traces."""
        return self.flagged_traces


class EvalDatasetManager:
    """
    Manages evaluation datasets.
    
    In production, this would connect to Agentreplay API.
    Here we simulate the workflow.
    """
    
    def __init__(self):
        self.datasets: dict[str, list[dict]] = {}
    
    def create_dataset(self, name: str, description: str = "") -> str:
        """Create a new dataset."""
        dataset_id = str(uuid.uuid4())[:8]
        self.datasets[dataset_id] = {
            "name": name,
            "description": description,
            "items": []
        }
        return dataset_id
    
    def add_from_trace(
        self,
        dataset_id: str,
        trace: ProductionTrace,
        correct_output: Optional[str] = None
    ):
        """
        Add a production trace to an evaluation dataset.
        
        Args:
            dataset_id: Target dataset
            trace: The production trace to add
            correct_output: Human-corrected expected output (optional)
        """
        if dataset_id not in self.datasets:
            raise ValueError(f"Dataset {dataset_id} not found")
        
        item = {
            "id": str(uuid.uuid4())[:8],
            "input": trace.input,
            "expected_output": correct_output or trace.output,
            "metadata": {
                "source": "production",
                "original_trace_id": trace.trace_id,
                "original_model": trace.model,
                "quality_score": trace.quality_score,
                "user_feedback": trace.user_feedback,
                "flagged_at": datetime.now().isoformat()
            }
        }
        self.datasets[dataset_id]["items"].append(item)
    
    def get_dataset(self, dataset_id: str) -> dict:
        """Get dataset by ID."""
        return self.datasets.get(dataset_id)
    
    def export_dataset(self, dataset_id: str) -> dict:
        """Export dataset in Agentreplay format."""
        if dataset_id not in self.datasets:
            raise ValueError(f"Dataset {dataset_id} not found")
        
        ds = self.datasets[dataset_id]
        return {
            "name": ds["name"],
            "description": ds["description"],
            "test_cases": ds["items"]
        }


def simulate_production_traces() -> list[ProductionTrace]:
    """Generate simulated production traces for demo."""
    
    traces = [
        # Good responses
        ProductionTrace(
            trace_id=str(uuid.uuid4()),
            input="What is the capital of France?",
            output="The capital of France is Paris. It's the largest city in France and serves as the country's political, economic, and cultural center.",
            latency_ms=245,
            model="gpt-4o-mini",
            timestamp=datetime.now().isoformat(),
            user_feedback="positive"
        ),
        ProductionTrace(
            trace_id=str(uuid.uuid4()),
            input="How do I create a Python list?",
            output="You can create a Python list using square brackets: my_list = [1, 2, 3]. Lists can contain any type of element and are mutable.",
            latency_ms=312,
            model="gpt-4o-mini",
            timestamp=datetime.now().isoformat(),
            user_feedback="positive"
        ),
        
        # Poor responses - should be flagged
        ProductionTrace(
            trace_id=str(uuid.uuid4()),
            input="Explain quantum computing",
            output="I don't know much about that topic.",
            latency_ms=189,
            model="gpt-4o-mini",
            timestamp=datetime.now().isoformat(),
            user_feedback="negative"
        ),
        ProductionTrace(
            trace_id=str(uuid.uuid4()),
            input="What are the key features of Rust?",
            output="Error",
            latency_ms=5200,
            model="gpt-4o-mini",
            timestamp=datetime.now().isoformat(),
            error="API timeout"
        ),
        ProductionTrace(
            trace_id=str(uuid.uuid4()),
            input="How do I deploy a Docker container?",
            output="I apologize, but I cannot help with that request.",
            latency_ms=156,
            model="gpt-4o-mini",
            timestamp=datetime.now().isoformat(),
            user_feedback="negative"
        ),
        
        # Edge cases
        ProductionTrace(
            trace_id=str(uuid.uuid4()),
            input="Write a haiku about programming",
            output="",
            latency_ms=89,
            model="gpt-4o-mini",
            timestamp=datetime.now().isoformat()
        ),
        ProductionTrace(
            trace_id=str(uuid.uuid4()),
            input="Calculate 2+2",
            output="4",
            latency_ms=102,
            model="gpt-4o-mini",
            timestamp=datetime.now().isoformat(),
            user_feedback="positive"
        ),
    ]
    
    return traces


def main():
    print("=" * 60)
    print("🔄 Production to Evaluation Pipeline Demo")
    print("=" * 60)
    
    # Step 1: Initialize components
    print("\n📦 Initializing components...")
    monitor = ProductionMonitor(quality_threshold=0.7)
    dataset_manager = EvalDatasetManager()
    
    # Step 2: Simulate production traces
    print("\n📊 Processing production traces...")
    traces = simulate_production_traces()
    
    flagged_count = 0
    for trace in traces:
        is_flagged = monitor.process_trace(trace)
        status = "🚩 FLAGGED" if is_flagged else "✅ OK"
        print(f"   [{status}] {trace.input[:40]}... (score: {trace.quality_score:.2f})")
        if is_flagged:
            flagged_count += 1
    
    print(f"\n   Total: {len(traces)} traces, {flagged_count} flagged")
    
    # Step 3: Create evaluation dataset from flagged traces
    print("\n📝 Creating evaluation dataset from flagged traces...")
    
    dataset_id = dataset_manager.create_dataset(
        name=f"Production Issues {datetime.now().strftime('%Y-%m-%d')}",
        description="Traces flagged from production with quality issues"
    )
    print(f"   Created dataset: {dataset_id}")
    
    # Human corrections for some traces
    corrections = {
        "Explain quantum computing": "Quantum computing uses quantum mechanics principles like superposition and entanglement to process information. Unlike classical bits that are 0 or 1, quantum bits (qubits) can be in multiple states simultaneously, enabling parallel computation.",
        "What are the key features of Rust?": "Rust is a systems programming language known for: 1) Memory safety without garbage collection, 2) Zero-cost abstractions, 3) Fearless concurrency, 4) Rich type system, 5) Excellent tooling with Cargo.",
        "How do I deploy a Docker container?": "To deploy a Docker container: 1) Build your image: docker build -t myapp . 2) Tag it: docker tag myapp registry/myapp:v1 3) Push to registry: docker push registry/myapp:v1 4) Deploy: docker run -d -p 8080:80 registry/myapp:v1"
    }
    
    flagged_traces = monitor.get_flagged_traces()
    for trace in flagged_traces:
        correct_output = corrections.get(trace.input)
        dataset_manager.add_from_trace(
            dataset_id,
            trace,
            correct_output=correct_output
        )
        has_correction = "✏️ corrected" if correct_output else "📋 original"
        print(f"   Added: {trace.input[:40]}... ({has_correction})")
    
    # Step 4: Export dataset
    print("\n📤 Exporting dataset...")
    exported = dataset_manager.export_dataset(dataset_id)
    print(f"   Name: {exported['name']}")
    print(f"   Test cases: {len(exported['test_cases'])}")
    
    # Show sample
    print("\n   Sample test case:")
    sample = exported['test_cases'][0]
    print(f"     Input: {sample['input'][:50]}...")
    print(f"     Expected: {sample['expected_output'][:50]}...")
    print(f"     Source: {sample['metadata']['source']}")
    
    # Step 5: Save to file
    output_file = "/tmp/production_issues_dataset.json"
    with open(output_file, 'w') as f:
        json.dump(exported, f, indent=2)
    print(f"\n   💾 Saved to {output_file}")
    
    # Summary
    print("\n" + "=" * 60)
    print("📊 Pipeline Summary")
    print("=" * 60)
    print(f"""
   Production Traces Processed: {len(traces)}
   Traces Flagged (< 0.7 quality): {flagged_count}
   Test Cases Created: {len(exported['test_cases'])}
   Human Corrections Applied: {len([t for t in flagged_traces if t.input in corrections])}
   
   Next Steps:
   1. Review flagged traces for accuracy
   2. Add human corrections where needed
   3. Import dataset to Agentreplay:
      curl -X POST http://localhost:8000/api/v1/evals/datasets \\
           -H "Content-Type: application/json" \\
           -d @{output_file}
   4. Run regression tests with improved agent
   5. Compare results to baseline
    """)
    
    print("=" * 60)
    print("✨ Pipeline demo complete!")
    print("=" * 60)


if __name__ == "__main__":
    main()
