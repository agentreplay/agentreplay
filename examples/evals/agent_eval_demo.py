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
Agentreplay Agent Evaluation Examples

Runnable examples showing agent evaluation with happy and unhappy paths.
No external dependencies required - uses mock data.

Usage:
    python agent_eval_demo.py
"""

from dataclasses import dataclass
from typing import List, Dict, Any, Optional
from enum import Enum
import json

# =============================================================================
# METRIC FORMULAS (matching agentreplay-evals/src/metrics/formulas.rs)
# =============================================================================

EPSILON = 1e-9

def tool_precision(correct_calls: int, total_calls: int) -> float:
    """Fraction of tool calls that were correct."""
    if total_calls == 0:
        return 0.0
    return correct_calls / total_calls

def tool_recall(correct_calls: int, expected_calls: int) -> float:
    """Fraction of expected tools that were called."""
    if expected_calls == 0:
        return 1.0  # Vacuously true
    return correct_calls / expected_calls

def tool_f1(precision: float, recall: float) -> float:
    """Harmonic mean of precision and recall."""
    if precision + recall < EPSILON:
        return 0.0
    return 2 * precision * recall / (precision + recall)

def step_efficiency(optimal_steps: int, actual_steps: int) -> float:
    """How efficiently agent reached goal."""
    if actual_steps == 0:
        return 1.0 if optimal_steps == 0 else 0.0
    return min(1.0, optimal_steps / actual_steps)

def trajectory_efficiency(
    redundancy: float,
    backtrack_rate: float,
    step_eff: Optional[float] = None,
    backtrack_penalty: float = 0.2
) -> float:
    """Overall trajectory efficiency score."""
    efficiency = 1.0 - redundancy
    efficiency -= backtrack_rate * backtrack_penalty
    if step_eff is not None:
        efficiency = (efficiency + step_eff) / 2.0
    return max(0.0, min(1.0, efficiency))

def hallucination_score(unsupported: int, total_claims: int) -> float:
    """Fraction of claims not supported by evidence."""
    if total_claims == 0:
        return 0.0
    return unsupported / total_claims

# =============================================================================
# DATA STRUCTURES
# =============================================================================

class TaskDifficulty(Enum):
    SIMPLE = "simple"
    MEDIUM = "medium"
    COMPLEX = "complex"

@dataclass
class AgentTestCase:
    """A test case for agent evaluation."""
    id: str
    name: str
    task: str
    difficulty: TaskDifficulty
    expected_tools: List[str]
    optimal_steps: int
    expected_output_contains: List[str]
    ground_truth: str
    tags: List[str]

@dataclass
class AgentTrace:
    """Captured trace from agent execution."""
    trace_id: str
    task: str
    expected_tools: List[str]
    actual_tools: List[str]
    tool_calls: List[Dict[str, str]]
    optimal_steps: int
    actual_steps: int
    redundant_steps: int
    backtrack_steps: int
    output: str
    completed: bool
    claims: Optional[List[Dict[str, Any]]] = None

@dataclass
class EvalResult:
    """Result of evaluating an agent trace."""
    test_case_id: str
    passed: bool
    metrics: Dict[str, float]
    diagnosis: List[str]

# =============================================================================
# EVALUATION LOGIC
# =============================================================================

def evaluate_agent_trace(trace: AgentTrace, thresholds: Dict[str, float] = None) -> EvalResult:
    """
    Evaluate an agent trace against expected behavior.
    
    Thresholds:
        tool_precision: 0.80
        tool_recall: 0.80
        trajectory_efficiency: 0.60
        task_completion: 0.90
        hallucination: 0.20 (lower is better)
    """
    thresholds = thresholds or {
        "tool_precision": 0.80,
        "tool_recall": 0.80,
        "trajectory_efficiency": 0.60,
        "task_completion": 0.90,
        "hallucination": 0.20,
    }
    
    # Calculate tool metrics
    # For tools, we count how many of the expected tool types were called
    # If expected = [weather, weather], we check if weather was called at least twice
    from collections import Counter
    
    expected_counts = Counter(trace.expected_tools)
    actual_counts = Counter(trace.actual_tools)
    
    # Correct = minimum of expected and actual for each tool type
    correct = 0
    for tool, expected_count in expected_counts.items():
        correct += min(expected_count, actual_counts.get(tool, 0))
    
    total_expected = len(trace.expected_tools)
    total_actual = len(trace.actual_tools)
    
    precision = tool_precision(correct, total_actual)
    recall = tool_recall(correct, total_expected)
    f1 = tool_f1(precision, recall)
    
    # Calculate efficiency metrics
    step_eff = step_efficiency(trace.optimal_steps, trace.actual_steps)
    redundancy = trace.redundant_steps / max(trace.actual_steps, 1)
    backtrack_rate = trace.backtrack_steps / max(trace.actual_steps, 1)
    traj_eff = trajectory_efficiency(redundancy, backtrack_rate, step_eff)
    
    # Task completion
    task_completion = 1.0 if trace.completed else 0.0
    
    # Hallucination (if claims provided)
    hall_score = 0.0
    if trace.claims:
        unsupported = sum(1 for c in trace.claims if not c.get("supported", False))
        hall_score = hallucination_score(unsupported, len(trace.claims))
    
    metrics = {
        "tool_precision": precision,
        "tool_recall": recall,
        "tool_f1": f1,
        "step_efficiency": step_eff,
        "trajectory_efficiency": traj_eff,
        "task_completion": task_completion,
        "hallucination_score": hall_score,
    }
    
    # Determine pass/fail
    diagnosis = []
    passed = True
    
    if precision < thresholds["tool_precision"]:
        passed = False
        diagnosis.append(f"Tool precision {precision:.2f} < {thresholds['tool_precision']}")
    
    if recall < thresholds["tool_recall"]:
        passed = False
        diagnosis.append(f"Tool recall {recall:.2f} < {thresholds['tool_recall']}")
    
    if traj_eff < thresholds["trajectory_efficiency"]:
        passed = False
        diagnosis.append(f"Trajectory efficiency {traj_eff:.2f} < {thresholds['trajectory_efficiency']}")
    
    if task_completion < thresholds["task_completion"]:
        passed = False
        diagnosis.append(f"Task completion {task_completion:.2f} < {thresholds['task_completion']}")
    
    if hall_score > thresholds.get("hallucination_score", 0.20):
        passed = False
        diagnosis.append(f"Hallucination score {hall_score:.2f} > {thresholds.get('hallucination_score', 0.20)}")
    
    return EvalResult(
        test_case_id=trace.trace_id,
        passed=passed,
        metrics=metrics,
        diagnosis=diagnosis
    )

# =============================================================================
# EXAMPLE TRACES
# =============================================================================

# Happy Path Examples
HAPPY_TRACES = [
    AgentTrace(
        trace_id="happy-001",
        task="What's the weather in Paris?",
        expected_tools=["weather"],
        actual_tools=["weather"],
        tool_calls=[{"tool": "weather", "input": "Paris", "output": "Paris: 18°C, partly cloudy"}],
        optimal_steps=1,
        actual_steps=1,
        redundant_steps=0,
        backtrack_steps=0,
        output="The weather in Paris is currently 18°C and partly cloudy.",
        completed=True
    ),
    AgentTrace(
        trace_id="happy-002",
        task="Is it warmer in Tokyo or London?",
        expected_tools=["weather", "weather"],
        actual_tools=["weather", "weather"],
        tool_calls=[
            {"tool": "weather", "input": "Tokyo", "output": "Tokyo: 22°C, sunny"},
            {"tool": "weather", "input": "London", "output": "London: 12°C, rainy"}
        ],
        optimal_steps=2,
        actual_steps=2,
        redundant_steps=0,
        backtrack_steps=0,
        output="Tokyo is warmer at 22°C compared to London at 12°C.",
        completed=True
    ),
    AgentTrace(
        trace_id="happy-003",
        task="Calculate 25 * 4 + 50",
        expected_tools=["calculate"],
        actual_tools=["calculate"],
        tool_calls=[{"tool": "calculate", "input": "25 * 4 + 50", "output": "Result: 150"}],
        optimal_steps=1,
        actual_steps=1,
        redundant_steps=0,
        backtrack_steps=0,
        output="The result is 150.",
        completed=True
    ),
]

# Unhappy Path Examples
UNHAPPY_TRACES = [
    # Wrong tool selection
    AgentTrace(
        trace_id="unhappy-001",
        task="What's the weather in Paris?",
        expected_tools=["weather"],
        actual_tools=["search", "search"],  # Wrong!
        tool_calls=[
            {"tool": "search", "input": "weather Paris", "output": "Paris weather: 18°C"},
            {"tool": "search", "input": "Paris temperature", "output": "Current temp: 18°C"}
        ],
        optimal_steps=1,
        actual_steps=2,
        redundant_steps=1,
        backtrack_steps=0,
        output="The weather in Paris is 18°C.",
        completed=True
    ),
    # Inefficient - too many steps
    AgentTrace(
        trace_id="unhappy-002",
        task="What is 25 * 4?",
        expected_tools=["calculate"],
        actual_tools=["search", "calculate", "search", "calculate"],
        tool_calls=[
            {"tool": "search", "input": "25 times 4", "output": "No results"},
            {"tool": "calculate", "input": "25 * 4", "output": "100"},
            {"tool": "search", "input": "verify 25 * 4", "output": "Math verified"},
            {"tool": "calculate", "input": "25 * 4", "output": "100"}
        ],
        optimal_steps=1,
        actual_steps=4,
        redundant_steps=2,
        backtrack_steps=0,
        output="25 * 4 = 100",
        completed=True
    ),
    # Missing required tool
    AgentTrace(
        trace_id="unhappy-003",
        task="Get weather for Paris and London, then email the summary",
        expected_tools=["weather", "weather", "email"],
        actual_tools=["weather", "weather"],  # Missing email!
        tool_calls=[
            {"tool": "weather", "input": "Paris", "output": "Paris: 18°C"},
            {"tool": "weather", "input": "London", "output": "London: 12°C"}
        ],
        optimal_steps=3,
        actual_steps=2,
        redundant_steps=0,
        backtrack_steps=0,
        output="Paris is 18°C and London is 12°C.",
        completed=False  # Task incomplete
    ),
    # Hallucination
    AgentTrace(
        trace_id="unhappy-004",
        task="What's the weather in Atlantis?",
        expected_tools=["weather"],
        actual_tools=["weather"],
        tool_calls=[{"tool": "weather", "input": "Atlantis", "output": "Weather data not available"}],
        optimal_steps=1,
        actual_steps=1,
        redundant_steps=0,
        backtrack_steps=0,
        output="The weather in Atlantis is 25°C and sunny with light underwater currents.",  # Made up!
        completed=True,
        claims=[
            {"claim": "Weather in Atlantis is 25°C", "supported": False},
            {"claim": "Weather is sunny", "supported": False},
            {"claim": "Light underwater currents", "supported": False}
        ]
    ),
    # Backtracking
    AgentTrace(
        trace_id="unhappy-005",
        task="Search for Python tutorials",
        expected_tools=["search"],
        actual_tools=["calculate", "weather", "search"],  # Wrong tools first
        tool_calls=[
            {"tool": "calculate", "input": "Python", "output": "Invalid expression"},
            {"tool": "weather", "input": "Python", "output": "No location found"},
            {"tool": "search", "input": "Python tutorials", "output": "Found tutorials..."}
        ],
        optimal_steps=1,
        actual_steps=3,
        redundant_steps=0,
        backtrack_steps=2,
        output="Here are some Python tutorials...",
        completed=True
    ),
]

# =============================================================================
# DEMO RUNNER
# =============================================================================

def print_separator(char: str = "=", width: int = 70):
    print(char * width)

def print_metrics(metrics: Dict[str, float], thresholds: Dict[str, float]):
    """Pretty print metrics with pass/fail indicators."""
    for name, value in metrics.items():
        threshold = thresholds.get(name)
        if threshold is None:
            status = ""
        elif name == "hallucination_score":
            status = "✅" if value <= threshold else "❌"
        else:
            status = "✅" if value >= threshold else "❌"
        print(f"    {name:25s}: {value:.3f} {status}")

def run_demo():
    """Run the complete evaluation demo."""
    
    thresholds = {
        "tool_precision": 0.80,
        "tool_recall": 0.80,
        "trajectory_efficiency": 0.60,
        "task_completion": 0.90,
        "hallucination_score": 0.20,
    }
    
    print_separator()
    print("           AGENTREPLAY AGENT EVALUATION DEMO")
    print_separator()
    print()
    
    # =========================================================================
    # HAPPY PATH EXAMPLES
    # =========================================================================
    
    print_separator("-")
    print("HAPPY PATH EXAMPLES (Expected to PASS)")
    print_separator("-")
    print()
    
    happy_results = []
    for trace in HAPPY_TRACES:
        result = evaluate_agent_trace(trace, thresholds)
        happy_results.append(result)
        
        status = "✅ PASSED" if result.passed else "❌ FAILED"
        print(f"[{trace.trace_id}] {trace.task}")
        print(f"  Expected Tools: {trace.expected_tools}")
        print(f"  Actual Tools:   {trace.actual_tools}")
        print(f"  Steps: {trace.actual_steps}/{trace.optimal_steps} optimal")
        print(f"  Result: {status}")
        print()
    
    # =========================================================================
    # UNHAPPY PATH EXAMPLES
    # =========================================================================
    
    print_separator("-")
    print("UNHAPPY PATH EXAMPLES (Expected to FAIL)")
    print_separator("-")
    print()
    
    unhappy_results = []
    for trace in UNHAPPY_TRACES:
        result = evaluate_agent_trace(trace, thresholds)
        unhappy_results.append(result)
        
        status = "✅ PASSED" if result.passed else "❌ FAILED"
        print(f"[{trace.trace_id}] {trace.task}")
        print(f"  Expected Tools: {trace.expected_tools}")
        print(f"  Actual Tools:   {trace.actual_tools}")
        print(f"  Steps: {trace.actual_steps}/{trace.optimal_steps} optimal")
        print(f"  Result: {status}")
        
        if result.diagnosis:
            print("  Diagnosis:")
            for d in result.diagnosis:
                print(f"    ⚠️  {d}")
        print()
    
    # =========================================================================
    # DETAILED METRICS VIEW
    # =========================================================================
    
    print_separator("-")
    print("DETAILED METRICS")
    print_separator("-")
    print()
    
    print("Happy Path - Trace happy-001 (Simple Weather):")
    print_metrics(happy_results[0].metrics, thresholds)
    print()
    
    print("Unhappy Path - Trace unhappy-002 (Inefficient):")
    print_metrics(unhappy_results[1].metrics, thresholds)
    print()
    
    print("Unhappy Path - Trace unhappy-004 (Hallucination):")
    print_metrics(unhappy_results[3].metrics, thresholds)
    print()
    
    # =========================================================================
    # SUMMARY
    # =========================================================================
    
    print_separator()
    print("SUMMARY")
    print_separator()
    print()
    
    total_happy = len(happy_results)
    passed_happy = sum(1 for r in happy_results if r.passed)
    
    total_unhappy = len(unhappy_results)
    passed_unhappy = sum(1 for r in unhappy_results if r.passed)
    
    print(f"Happy Path Tests:   {passed_happy}/{total_happy} passed ({passed_happy/total_happy:.0%})")
    print(f"Unhappy Path Tests: {passed_unhappy}/{total_unhappy} passed ({passed_unhappy/total_unhappy:.0%})")
    print()
    
    all_results = happy_results + unhappy_results
    avg_metrics = {}
    for metric in ["tool_precision", "tool_recall", "tool_f1", "trajectory_efficiency"]:
        avg = sum(r.metrics[metric] for r in all_results) / len(all_results)
        avg_metrics[metric] = avg
    
    print("Average Metrics (all tests):")
    for metric, value in avg_metrics.items():
        print(f"  {metric}: {value:.3f}")
    print()
    
    print_separator()
    print("Thresholds Used:")
    for name, value in thresholds.items():
        direction = "≤" if name == "hallucination_score" else "≥"
        print(f"  {name}: {direction} {value}")
    print_separator()

if __name__ == "__main__":
    run_demo()
