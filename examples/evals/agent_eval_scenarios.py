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
Flowtrace Agent Evaluation Scenarios

Test cases for evaluating agent behavior with happy and unhappy paths.
Based on metrics formulas from flowtrace-evals/src/evaluators.

Usage:
    python examples/evals/agent_eval_scenarios.py
    
    # Run with verbose output:
    python examples/evals/agent_eval_scenarios.py --verbose
    
    # Export scenarios to JSON:
    python examples/evals/agent_eval_scenarios.py --export scenarios.json

Metrics Evaluated:
    - Tool Precision: Fraction of tool calls that were correct
    - Tool Recall: Fraction of expected tools that were called  
    - Tool F1: Harmonic mean of precision and recall
    - Step Efficiency: How efficiently agent reached goal (optimal/actual)
    - Trajectory Efficiency: Overall path efficiency accounting for redundancy
    - Task Completion: Whether the task was fully completed
    - Hallucination Score: Fraction of claims not supported by evidence
"""

from dataclasses import dataclass, field, asdict
from typing import List, Dict, Any, Optional
from enum import Enum
import json
import argparse


# =============================================================================
# CONSTANTS & THRESHOLDS
# =============================================================================

EPSILON = 1e-9

# Default thresholds (matching flowtrace-evals defaults)
DEFAULT_THRESHOLDS = {
    "tool_precision": 0.80,
    "tool_recall": 0.80,
    "tool_f1": 0.70,
    "trajectory_efficiency": 0.60,
    "task_completion": 0.90,
    "hallucination_score": 0.20,  # Lower is better
    "step_efficiency": 0.60,
}


# =============================================================================
# METRIC FORMULAS (matching flowtrace-evals/src/metrics/formulas.rs)
# =============================================================================

def tool_precision(correct_calls: int, total_calls: int) -> float:
    """
    Precision = |T_actual ∩ T_expected| / |T_actual|
    
    Measures: What fraction of the tools called were correct?
    High precision = agent doesn't call unnecessary tools
    """
    if total_calls == 0:
        return 0.0
    return correct_calls / total_calls


def tool_recall(correct_calls: int, expected_calls: int) -> float:
    """
    Recall = |T_actual ∩ T_expected| / |T_expected|
    
    Measures: What fraction of required tools were called?
    High recall = agent calls all necessary tools
    """
    if expected_calls == 0:
        return 1.0  # Vacuously true - no tools expected
    return correct_calls / expected_calls


def tool_f1(precision: float, recall: float) -> float:
    """
    F1 = 2 * (Precision * Recall) / (Precision + Recall)
    
    Harmonic mean balancing precision and recall.
    """
    if precision + recall < EPSILON:
        return 0.0
    return 2 * precision * recall / (precision + recall)


def step_efficiency(optimal_steps: int, actual_steps: int) -> float:
    """
    Efficiency = optimal_steps / actual_steps
    
    Measures: How close to optimal was the agent's path?
    1.0 = perfect efficiency, <1.0 = took extra steps
    """
    if actual_steps == 0:
        return 1.0 if optimal_steps == 0 else 0.0
    return min(1.0, optimal_steps / actual_steps)


def trajectory_efficiency(
    redundancy: float,
    backtrack_rate: float,
    step_eff: Optional[float] = None,
    backtrack_penalty: float = 0.2
) -> float:
    """
    Trajectory Efficiency = 1 - redundancy - (backtrack_rate * penalty)
    
    Combined efficiency metric accounting for:
    - Redundant operations (repeated tool calls)
    - Backtracking (reverting decisions)
    - Overall step efficiency
    """
    efficiency = 1.0 - redundancy
    efficiency -= backtrack_rate * backtrack_penalty
    if step_eff is not None:
        efficiency = (efficiency + step_eff) / 2.0
    return max(0.0, min(1.0, efficiency))


def hallucination_score(unsupported: int, total_claims: int) -> float:
    """
    Hallucination = unsupported_claims / total_claims
    
    Measures: What fraction of claims lack evidence?
    0.0 = no hallucinations, 1.0 = all hallucinated
    """
    if total_claims == 0:
        return 0.0
    return unsupported / total_claims


# =============================================================================
# DATA STRUCTURES
# =============================================================================

class TaskDifficulty(Enum):
    SIMPLE = "simple"       # Single tool, direct task
    MEDIUM = "medium"       # 2-3 tools, some planning needed
    COMPLEX = "complex"     # Multi-step, requires strategy


class ScenarioCategory(Enum):
    HAPPY_PATH = "happy_path"
    WRONG_TOOL = "wrong_tool"
    INEFFICIENT = "inefficient"
    MISSING_TOOL = "missing_tool"
    HALLUCINATION = "hallucination"
    BACKTRACKING = "backtracking"
    AMBIGUOUS = "ambiguous"


@dataclass
class ToolCall:
    """Record of a tool call in an agent trace."""
    tool: str
    input: str
    output: str


@dataclass
class Claim:
    """A claim made in agent output with evidence status."""
    claim: str
    supported: bool
    evidence: Optional[str] = None


@dataclass
class AgentScenario:
    """
    A test scenario for agent evaluation.
    
    Represents both the expected behavior and simulated actual behavior
    for testing the evaluation system.
    """
    id: str
    name: str
    description: str
    category: ScenarioCategory
    difficulty: TaskDifficulty
    
    # Task specification
    task: str
    available_tools: List[str]
    expected_tools: List[str]
    optimal_steps: int
    expected_output_contains: List[str]
    ground_truth: str
    
    # Simulated agent behavior
    actual_tools: List[str]
    tool_calls: List[ToolCall]
    actual_steps: int
    redundant_steps: int = 0
    backtrack_steps: int = 0
    output: str = ""
    completed: bool = True
    claims: List[Claim] = field(default_factory=list)
    
    # Expected result
    should_pass: bool = True
    expected_failures: List[str] = field(default_factory=list)
    
    # Metadata
    tags: List[str] = field(default_factory=list)


@dataclass
class EvalResult:
    """Result of evaluating a scenario."""
    scenario_id: str
    passed: bool
    metrics: Dict[str, float]
    diagnosis: List[str]
    expected_pass: bool
    result_matches_expected: bool


# =============================================================================
# EVALUATION LOGIC
# =============================================================================

def calculate_tool_metrics(
    expected_tools: List[str],
    actual_tools: List[str]
) -> Dict[str, float]:
    """Calculate precision, recall, and F1 for tool usage."""
    from collections import Counter
    
    expected_counts = Counter(expected_tools)
    actual_counts = Counter(actual_tools)
    
    # Count correct tool calls
    correct = 0
    for tool, expected_count in expected_counts.items():
        correct += min(expected_count, actual_counts.get(tool, 0))
    
    total_expected = len(expected_tools)
    total_actual = len(actual_tools)
    
    prec = tool_precision(correct, total_actual)
    rec = tool_recall(correct, total_expected)
    f1 = tool_f1(prec, rec)
    
    return {
        "tool_precision": prec,
        "tool_recall": rec,
        "tool_f1": f1,
        "correct_tools": correct,
        "expected_tool_count": total_expected,
        "actual_tool_count": total_actual,
    }


def evaluate_scenario(
    scenario: AgentScenario,
    thresholds: Dict[str, float] = None
) -> EvalResult:
    """
    Evaluate an agent scenario against expected behavior.
    
    Returns metrics and pass/fail determination.
    """
    thresholds = thresholds or DEFAULT_THRESHOLDS
    
    # Calculate tool metrics
    tool_metrics = calculate_tool_metrics(
        scenario.expected_tools,
        scenario.actual_tools
    )
    
    # Calculate efficiency metrics
    step_eff = step_efficiency(scenario.optimal_steps, scenario.actual_steps)
    redundancy = scenario.redundant_steps / max(scenario.actual_steps, 1)
    backtrack_rate = scenario.backtrack_steps / max(scenario.actual_steps, 1)
    traj_eff = trajectory_efficiency(redundancy, backtrack_rate, step_eff)
    
    # Task completion
    task_completion = 1.0 if scenario.completed else 0.0
    
    # Hallucination score
    hall_score = 0.0
    if scenario.claims:
        unsupported = sum(1 for c in scenario.claims if not c.supported)
        hall_score = hallucination_score(unsupported, len(scenario.claims))
    
    # Aggregate all metrics
    metrics = {
        **tool_metrics,
        "step_efficiency": step_eff,
        "trajectory_efficiency": traj_eff,
        "task_completion": task_completion,
        "hallucination_score": hall_score,
        "redundancy_rate": redundancy,
        "backtrack_rate": backtrack_rate,
    }
    
    # Determine pass/fail with diagnosis
    diagnosis = []
    passed = True
    
    if tool_metrics["tool_precision"] < thresholds["tool_precision"]:
        passed = False
        diagnosis.append(
            f"Tool precision {tool_metrics['tool_precision']:.2f} < {thresholds['tool_precision']}"
        )
    
    if tool_metrics["tool_recall"] < thresholds["tool_recall"]:
        passed = False
        diagnosis.append(
            f"Tool recall {tool_metrics['tool_recall']:.2f} < {thresholds['tool_recall']}"
        )
    
    if traj_eff < thresholds["trajectory_efficiency"]:
        passed = False
        diagnosis.append(
            f"Trajectory efficiency {traj_eff:.2f} < {thresholds['trajectory_efficiency']}"
        )
    
    if task_completion < thresholds["task_completion"]:
        passed = False
        diagnosis.append(
            f"Task completion {task_completion:.2f} < {thresholds['task_completion']}"
        )
    
    if hall_score > thresholds["hallucination_score"]:
        passed = False
        diagnosis.append(
            f"Hallucination score {hall_score:.2f} > {thresholds['hallucination_score']}"
        )
    
    return EvalResult(
        scenario_id=scenario.id,
        passed=passed,
        metrics=metrics,
        diagnosis=diagnosis,
        expected_pass=scenario.should_pass,
        result_matches_expected=passed == scenario.should_pass
    )


# =============================================================================
# TEST SCENARIOS
# =============================================================================

SCENARIOS: List[AgentScenario] = [
    # =========================================================================
    # HAPPY PATH SCENARIOS - Expected to PASS
    # =========================================================================
    
    AgentScenario(
        id="happy-001",
        name="Simple Weather Lookup",
        description="Single tool call to get weather for one city",
        category=ScenarioCategory.HAPPY_PATH,
        difficulty=TaskDifficulty.SIMPLE,
        task="What's the weather in Paris?",
        available_tools=["weather", "search", "calculate", "email"],
        expected_tools=["weather"],
        optimal_steps=1,
        expected_output_contains=["Paris", "temperature", "weather"],
        ground_truth="Paris: 18°C, partly cloudy",
        actual_tools=["weather"],
        tool_calls=[
            ToolCall("weather", "Paris", "Paris: 18°C, partly cloudy")
        ],
        actual_steps=1,
        output="The weather in Paris is currently 18°C and partly cloudy.",
        completed=True,
        should_pass=True,
        tags=["weather", "simple", "single-tool"]
    ),
    
    AgentScenario(
        id="happy-002",
        name="Weather Comparison",
        description="Compare weather between two cities",
        category=ScenarioCategory.HAPPY_PATH,
        difficulty=TaskDifficulty.MEDIUM,
        task="Is it warmer in Tokyo or London?",
        available_tools=["weather", "search", "calculate"],
        expected_tools=["weather", "weather"],
        optimal_steps=2,
        expected_output_contains=["Tokyo", "London", "warmer"],
        ground_truth="Tokyo: 22°C, London: 12°C. Tokyo is warmer.",
        actual_tools=["weather", "weather"],
        tool_calls=[
            ToolCall("weather", "Tokyo", "Tokyo: 22°C, sunny"),
            ToolCall("weather", "London", "London: 12°C, rainy")
        ],
        actual_steps=2,
        output="Tokyo is warmer at 22°C compared to London at 12°C.",
        completed=True,
        should_pass=True,
        tags=["weather", "comparison", "multi-tool"]
    ),
    
    AgentScenario(
        id="happy-003",
        name="Simple Calculation",
        description="Direct mathematical calculation",
        category=ScenarioCategory.HAPPY_PATH,
        difficulty=TaskDifficulty.SIMPLE,
        task="Calculate 25 * 4 + 50",
        available_tools=["calculate", "search", "weather"],
        expected_tools=["calculate"],
        optimal_steps=1,
        expected_output_contains=["150"],
        ground_truth="25 * 4 + 50 = 150",
        actual_tools=["calculate"],
        tool_calls=[
            ToolCall("calculate", "25 * 4 + 50", "Result: 150")
        ],
        actual_steps=1,
        output="The result of 25 * 4 + 50 is 150.",
        completed=True,
        should_pass=True,
        tags=["math", "simple", "single-tool"]
    ),
    
    AgentScenario(
        id="happy-004",
        name="Multi-Step Research and Email",
        description="Complex task requiring search, analysis, and communication",
        category=ScenarioCategory.HAPPY_PATH,
        difficulty=TaskDifficulty.COMPLEX,
        task="Find the current weather in NYC and Tokyo, then email a summary to team@example.com",
        available_tools=["weather", "search", "email", "calculate"],
        expected_tools=["weather", "weather", "email"],
        optimal_steps=3,
        expected_output_contains=["NYC", "Tokyo", "email", "sent"],
        ground_truth="NYC: 15°C, Tokyo: 22°C. Email sent to team@example.com",
        actual_tools=["weather", "weather", "email"],
        tool_calls=[
            ToolCall("weather", "NYC", "NYC: 15°C, cloudy"),
            ToolCall("weather", "Tokyo", "Tokyo: 22°C, sunny"),
            ToolCall("email", "team@example.com: Weather summary - NYC 15°C, Tokyo 22°C", "Email sent")
        ],
        actual_steps=3,
        output="I've gathered the weather data and sent the summary email. NYC is 15°C and Tokyo is 22°C.",
        completed=True,
        should_pass=True,
        tags=["weather", "email", "complex", "multi-tool"]
    ),

    # =========================================================================
    # WRONG TOOL SCENARIOS - Expected to FAIL (precision issues)
    # =========================================================================
    
    AgentScenario(
        id="wrong-001",
        name="Wrong Tool Selection",
        description="Agent uses search instead of dedicated weather tool",
        category=ScenarioCategory.WRONG_TOOL,
        difficulty=TaskDifficulty.SIMPLE,
        task="What's the weather in Paris?",
        available_tools=["weather", "search", "calculate"],
        expected_tools=["weather"],
        optimal_steps=1,
        expected_output_contains=["Paris", "weather"],
        ground_truth="Use weather tool for weather queries",
        actual_tools=["search", "search"],  # Wrong tool, used twice
        tool_calls=[
            ToolCall("search", "weather Paris", "Paris weather info..."),
            ToolCall("search", "Paris temperature now", "Current temp: 18°C")
        ],
        actual_steps=2,
        redundant_steps=1,
        output="The weather in Paris is 18°C.",
        completed=True,
        should_pass=False,
        expected_failures=["tool_precision", "trajectory_efficiency"],
        tags=["wrong-tool", "precision-fail"]
    ),
    
    AgentScenario(
        id="wrong-002",
        name="Calculation via Search",
        description="Agent tries to search for math answer instead of calculating",
        category=ScenarioCategory.WRONG_TOOL,
        difficulty=TaskDifficulty.SIMPLE,
        task="What is 42 * 17?",
        available_tools=["calculate", "search", "weather"],
        expected_tools=["calculate"],
        optimal_steps=1,
        expected_output_contains=["714"],
        ground_truth="42 * 17 = 714",
        actual_tools=["search"],
        tool_calls=[
            ToolCall("search", "42 times 17", "Result: 714")
        ],
        actual_steps=1,
        output="42 * 17 equals 714.",
        completed=True,
        should_pass=False,
        expected_failures=["tool_precision", "tool_recall"],
        tags=["wrong-tool", "math", "precision-fail"]
    ),

    # =========================================================================
    # INEFFICIENT SCENARIOS - Expected to FAIL (efficiency issues)
    # =========================================================================
    
    AgentScenario(
        id="inefficient-001",
        name="Redundant Tool Calls",
        description="Agent calls same tool multiple times unnecessarily",
        category=ScenarioCategory.INEFFICIENT,
        difficulty=TaskDifficulty.SIMPLE,
        task="What is 25 * 4?",
        available_tools=["calculate", "search"],
        expected_tools=["calculate"],
        optimal_steps=1,
        expected_output_contains=["100"],
        ground_truth="25 * 4 = 100",
        actual_tools=["search", "calculate", "search", "calculate"],
        tool_calls=[
            ToolCall("search", "25 times 4", "No direct answer"),
            ToolCall("calculate", "25 * 4", "Result: 100"),
            ToolCall("search", "verify 25 * 4", "Math verified"),
            ToolCall("calculate", "25 * 4", "Result: 100")  # Redundant
        ],
        actual_steps=4,
        redundant_steps=2,
        output="25 * 4 = 100",
        completed=True,
        should_pass=False,
        expected_failures=["tool_precision", "trajectory_efficiency"],
        tags=["inefficient", "redundant", "efficiency-fail"]
    ),
    
    AgentScenario(
        id="inefficient-002",
        name="Excessive Verification",
        description="Agent over-verifies simple results",
        category=ScenarioCategory.INEFFICIENT,
        difficulty=TaskDifficulty.MEDIUM,
        task="What's the weather in Berlin?",
        available_tools=["weather", "search"],
        expected_tools=["weather"],
        optimal_steps=1,
        expected_output_contains=["Berlin"],
        ground_truth="Berlin: 14°C, windy",
        actual_tools=["weather", "search", "weather", "search"],
        tool_calls=[
            ToolCall("weather", "Berlin", "Berlin: 14°C, windy"),
            ToolCall("search", "Berlin weather verify", "14°C confirmed"),
            ToolCall("weather", "Berlin", "Berlin: 14°C, windy"),
            ToolCall("search", "current weather Berlin", "Berlin is 14°C")
        ],
        actual_steps=4,
        redundant_steps=3,
        output="The weather in Berlin is 14°C and windy.",
        completed=True,
        should_pass=False,
        expected_failures=["trajectory_efficiency", "tool_precision"],
        tags=["inefficient", "over-verification"]
    ),

    # =========================================================================
    # MISSING TOOL SCENARIOS - Expected to FAIL (recall issues)
    # =========================================================================
    
    AgentScenario(
        id="missing-001",
        name="Incomplete Task - Missing Email",
        description="Agent gets data but forgets to send required email",
        category=ScenarioCategory.MISSING_TOOL,
        difficulty=TaskDifficulty.COMPLEX,
        task="Get weather for Paris and London, then email the summary to boss@company.com",
        available_tools=["weather", "email", "search"],
        expected_tools=["weather", "weather", "email"],
        optimal_steps=3,
        expected_output_contains=["Paris", "London", "email", "sent"],
        ground_truth="Weather gathered and email sent",
        actual_tools=["weather", "weather"],  # Missing email!
        tool_calls=[
            ToolCall("weather", "Paris", "Paris: 18°C, sunny"),
            ToolCall("weather", "London", "London: 12°C, rainy")
        ],
        actual_steps=2,
        output="Paris is 18°C and sunny. London is 12°C and rainy.",
        completed=False,  # Task not complete - email not sent
        should_pass=False,
        expected_failures=["tool_recall", "task_completion"],
        tags=["missing-tool", "incomplete", "recall-fail"]
    ),
    
    AgentScenario(
        id="missing-002",
        name="Partial Data Gathering",
        description="Agent only gets weather for one of two cities",
        category=ScenarioCategory.MISSING_TOOL,
        difficulty=TaskDifficulty.MEDIUM,
        task="Compare the weather in Rome and Madrid",
        available_tools=["weather", "search"],
        expected_tools=["weather", "weather"],
        optimal_steps=2,
        expected_output_contains=["Rome", "Madrid"],
        ground_truth="Rome: 20°C, Madrid: 25°C",
        actual_tools=["weather"],  # Only got one city
        tool_calls=[
            ToolCall("weather", "Rome", "Rome: 20°C, sunny")
        ],
        actual_steps=1,
        output="Rome is currently 20°C and sunny. I couldn't get Madrid's weather.",
        completed=False,
        should_pass=False,
        expected_failures=["tool_recall", "task_completion"],
        tags=["missing-tool", "partial", "recall-fail"]
    ),

    # =========================================================================
    # HALLUCINATION SCENARIOS - Expected to FAIL (fabricated information)
    # =========================================================================
    
    AgentScenario(
        id="hallucination-001",
        name="Fabricated Weather Data",
        description="Agent makes up weather for non-existent location",
        category=ScenarioCategory.HALLUCINATION,
        difficulty=TaskDifficulty.SIMPLE,
        task="What's the weather in Atlantis?",
        available_tools=["weather", "search"],
        expected_tools=["weather"],
        optimal_steps=1,
        expected_output_contains=["not available", "no data"],
        ground_truth="Weather data not available for fictional location",
        actual_tools=["weather"],
        tool_calls=[
            ToolCall("weather", "Atlantis", "Location not found - no weather data available")
        ],
        actual_steps=1,
        output="The weather in Atlantis is 25°C and sunny with gentle underwater currents.",
        completed=True,
        claims=[
            Claim("Weather in Atlantis is 25°C", False),
            Claim("Weather is sunny", False),
            Claim("Gentle underwater currents", False)
        ],
        should_pass=False,
        expected_failures=["hallucination_score"],
        tags=["hallucination", "fabricated-data"]
    ),
    
    AgentScenario(
        id="hallucination-002",
        name="Invented Statistics",
        description="Agent fabricates statistics not in tool output",
        category=ScenarioCategory.HALLUCINATION,
        difficulty=TaskDifficulty.MEDIUM,
        task="What's the average temperature in London this week?",
        available_tools=["weather", "search"],
        expected_tools=["weather"],
        optimal_steps=1,
        expected_output_contains=["London", "temperature"],
        ground_truth="Current: 12°C (average not available from this tool)",
        actual_tools=["weather"],
        tool_calls=[
            ToolCall("weather", "London", "London: 12°C, rainy")
        ],
        actual_steps=1,
        output="The average temperature in London this week is 14.5°C with a high of 18°C and low of 8°C.",
        completed=True,
        claims=[
            Claim("Average temperature is 14.5°C", False),
            Claim("High of 18°C", False),
            Claim("Low of 8°C", False),
            Claim("Current temperature is 12°C", True, evidence="Tool output: London: 12°C")
        ],
        should_pass=False,
        expected_failures=["hallucination_score"],
        tags=["hallucination", "fabricated-stats"]
    ),

    # =========================================================================
    # BACKTRACKING SCENARIOS - Expected to FAIL (efficiency issues)
    # =========================================================================
    
    AgentScenario(
        id="backtrack-001",
        name="Wrong Tools First",
        description="Agent tries wrong tools before finding correct one",
        category=ScenarioCategory.BACKTRACKING,
        difficulty=TaskDifficulty.SIMPLE,
        task="Search for Python tutorials",
        available_tools=["search", "calculate", "weather", "email"],
        expected_tools=["search"],
        optimal_steps=1,
        expected_output_contains=["Python", "tutorial"],
        ground_truth="Use search tool for finding tutorials",
        actual_tools=["calculate", "weather", "search"],
        tool_calls=[
            ToolCall("calculate", "Python", "Invalid expression"),
            ToolCall("weather", "Python", "Location not found"),
            ToolCall("search", "Python tutorials", "Found: Python.org tutorial, Real Python...")
        ],
        actual_steps=3,
        backtrack_steps=2,
        output="Here are some Python tutorials: Python.org tutorial, Real Python...",
        completed=True,
        should_pass=False,
        expected_failures=["trajectory_efficiency", "tool_precision"],
        tags=["backtracking", "wrong-first", "efficiency-fail"]
    ),
    
    AgentScenario(
        id="backtrack-002",
        name="Circular Tool Usage",
        description="Agent goes in circles before completing task",
        category=ScenarioCategory.BACKTRACKING,
        difficulty=TaskDifficulty.MEDIUM,
        task="Calculate the area of a circle with radius 5",
        available_tools=["calculate", "search"],
        expected_tools=["calculate"],
        optimal_steps=1,
        expected_output_contains=["78.5", "area"],
        ground_truth="Area = π * r² = 3.14159 * 25 ≈ 78.54",
        actual_tools=["search", "calculate", "search", "calculate"],
        tool_calls=[
            ToolCall("search", "circle area formula", "Area = π * r²"),
            ToolCall("calculate", "3.14159 * 5", "15.708"),  # Wrong formula
            ToolCall("search", "area of circle radius 5", "Area = π * 5² = 78.54"),
            ToolCall("calculate", "3.14159 * 25", "78.54")
        ],
        actual_steps=4,
        backtrack_steps=2,
        redundant_steps=1,
        output="The area of a circle with radius 5 is approximately 78.54 square units.",
        completed=True,
        should_pass=False,
        expected_failures=["trajectory_efficiency", "tool_precision"],
        tags=["backtracking", "circular", "efficiency-fail"]
    ),

    # =========================================================================
    # AMBIGUOUS SCENARIOS - Expected behavior depends on interpretation
    # =========================================================================
    
    AgentScenario(
        id="ambiguous-001",
        name="Ambiguous Query - Weather or Stock",
        description="Query 'What is AMZN?' could be weather or stock lookup",
        category=ScenarioCategory.AMBIGUOUS,
        difficulty=TaskDifficulty.MEDIUM,
        task="What is AMZN?",
        available_tools=["weather", "search", "stock_price"],
        expected_tools=["stock_price"],  # Most likely interpretation
        optimal_steps=1,
        expected_output_contains=["Amazon", "stock", "AMZN"],
        ground_truth="AMZN is Amazon's stock ticker",
        actual_tools=["weather", "search"],  # Agent guessed wrong
        tool_calls=[
            ToolCall("weather", "AMZN", "Location not found"),
            ToolCall("search", "AMZN", "AMZN is Amazon.com Inc stock ticker, currently $150")
        ],
        actual_steps=2,
        backtrack_steps=1,
        output="AMZN is Amazon.com Inc's stock ticker, currently trading at $150.",
        completed=True,
        should_pass=False,  # Should have used stock_price directly
        expected_failures=["tool_precision", "trajectory_efficiency"],
        tags=["ambiguous", "interpretation"]
    ),
]


# =============================================================================
# RUNNER
# =============================================================================

def print_separator(char: str = "=", width: int = 78):
    print(char * width)


def print_metrics(metrics: Dict[str, float], thresholds: Dict[str, float]):
    """Pretty print metrics with pass/fail indicators."""
    for name, value in sorted(metrics.items()):
        if name in ["correct_tools", "expected_tool_count", "actual_tool_count"]:
            print(f"    {name:25s}: {int(value)}")
            continue
        
        threshold = thresholds.get(name)
        if threshold is None:
            status = ""
        elif name in ["hallucination_score", "redundancy_rate", "backtrack_rate"]:
            status = "✅" if value <= threshold else "❌"
        else:
            status = "✅" if value >= threshold else "❌"
        print(f"    {name:25s}: {value:.3f} {status}")


def run_scenarios(
    scenarios: List[AgentScenario],
    thresholds: Dict[str, float] = None,
    verbose: bool = False
) -> Dict[str, Any]:
    """Run all scenarios and return aggregated results."""
    thresholds = thresholds or DEFAULT_THRESHOLDS
    results = []
    
    for scenario in scenarios:
        result = evaluate_scenario(scenario, thresholds)
        results.append((scenario, result))
    
    return {
        "results": results,
        "thresholds": thresholds,
    }


def print_results(
    run_data: Dict[str, Any],
    verbose: bool = False
):
    """Print evaluation results."""
    results = run_data["results"]
    thresholds = run_data["thresholds"]
    
    print_separator()
    print("           FLOWTRACE AGENT EVALUATION SCENARIOS")
    print_separator()
    print()
    
    # Group by category
    by_category: Dict[ScenarioCategory, List] = {}
    for scenario, result in results:
        cat = scenario.category
        if cat not in by_category:
            by_category[cat] = []
        by_category[cat].append((scenario, result))
    
    # Print each category
    for category in ScenarioCategory:
        if category not in by_category:
            continue
        
        items = by_category[category]
        print_separator("-")
        print(f"{category.value.upper().replace('_', ' ')} SCENARIOS ({len(items)} tests)")
        print_separator("-")
        print()
        
        for scenario, result in items:
            status = "✅ PASSED" if result.passed else "❌ FAILED"
            match = "✓" if result.result_matches_expected else "✗ UNEXPECTED"
            
            print(f"[{scenario.id}] {scenario.name}")
            print(f"  Task: {scenario.task}")
            print(f"  Expected Tools: {scenario.expected_tools}")
            print(f"  Actual Tools:   {scenario.actual_tools}")
            print(f"  Steps: {scenario.actual_steps}/{scenario.optimal_steps} optimal")
            print(f"  Result: {status} (expected: {'PASS' if scenario.should_pass else 'FAIL'}) {match}")
            
            if result.diagnosis:
                print("  Diagnosis:")
                for d in result.diagnosis:
                    print(f"    ⚠️  {d}")
            
            if verbose:
                print("  Metrics:")
                print_metrics(result.metrics, thresholds)
            
            print()
    
    # Summary
    print_separator()
    print("SUMMARY")
    print_separator()
    print()
    
    total = len(results)
    passed = sum(1 for _, r in results if r.passed)
    matched_expected = sum(1 for _, r in results if r.result_matches_expected)
    
    by_category_summary = {}
    for scenario, result in results:
        cat = scenario.category.value
        if cat not in by_category_summary:
            by_category_summary[cat] = {"total": 0, "passed": 0, "matched": 0}
        by_category_summary[cat]["total"] += 1
        if result.passed:
            by_category_summary[cat]["passed"] += 1
        if result.result_matches_expected:
            by_category_summary[cat]["matched"] += 1
    
    print("Results by Category:")
    for cat, stats in sorted(by_category_summary.items()):
        print(f"  {cat:20s}: {stats['passed']}/{stats['total']} passed, {stats['matched']}/{stats['total']} matched expected")
    
    print()
    print(f"Total: {passed}/{total} passed ({passed/total*100:.0f}%)")
    print(f"Matched Expected: {matched_expected}/{total} ({matched_expected/total*100:.0f}%)")
    print()
    
    # Average metrics
    all_metrics = [r.metrics for _, r in results]
    avg_metrics = {}
    for key in ["tool_precision", "tool_recall", "tool_f1", "trajectory_efficiency", "task_completion"]:
        avg_metrics[key] = sum(m[key] for m in all_metrics) / len(all_metrics)
    
    print("Average Metrics (all scenarios):")
    for metric, value in avg_metrics.items():
        print(f"  {metric}: {value:.3f}")
    
    print()
    print_separator()
    print("Thresholds Used:")
    for name, value in sorted(thresholds.items()):
        direction = "≤" if name in ["hallucination_score"] else "≥"
        print(f"  {name}: {direction} {value}")
    print_separator()


def export_scenarios(scenarios: List[AgentScenario], filepath: str):
    """Export scenarios to JSON."""
    data = []
    for s in scenarios:
        d = {
            "id": s.id,
            "name": s.name,
            "description": s.description,
            "category": s.category.value,
            "difficulty": s.difficulty.value,
            "task": s.task,
            "available_tools": s.available_tools,
            "expected_tools": s.expected_tools,
            "optimal_steps": s.optimal_steps,
            "expected_output_contains": s.expected_output_contains,
            "ground_truth": s.ground_truth,
            "actual_tools": s.actual_tools,
            "tool_calls": [{"tool": tc.tool, "input": tc.input, "output": tc.output} for tc in s.tool_calls],
            "actual_steps": s.actual_steps,
            "redundant_steps": s.redundant_steps,
            "backtrack_steps": s.backtrack_steps,
            "output": s.output,
            "completed": s.completed,
            "claims": [{"claim": c.claim, "supported": c.supported, "evidence": c.evidence} for c in s.claims],
            "should_pass": s.should_pass,
            "expected_failures": s.expected_failures,
            "tags": s.tags,
        }
        data.append(d)
    
    with open(filepath, "w") as f:
        json.dump({"scenarios": data, "thresholds": DEFAULT_THRESHOLDS}, f, indent=2)
    
    print(f"Exported {len(scenarios)} scenarios to {filepath}")


def main():
    parser = argparse.ArgumentParser(description="Flowtrace Agent Evaluation Scenarios")
    parser.add_argument("--verbose", "-v", action="store_true", help="Show detailed metrics")
    parser.add_argument("--export", metavar="FILE", help="Export scenarios to JSON file")
    parser.add_argument("--category", help="Run only specific category (e.g., happy_path, wrong_tool)")
    args = parser.parse_args()
    
    scenarios = SCENARIOS
    
    if args.category:
        try:
            cat = ScenarioCategory(args.category)
            scenarios = [s for s in scenarios if s.category == cat]
            print(f"Filtering to {len(scenarios)} scenarios in category: {args.category}")
        except ValueError:
            print(f"Invalid category: {args.category}")
            print(f"Valid categories: {[c.value for c in ScenarioCategory]}")
            return 1
    
    if args.export:
        export_scenarios(scenarios, args.export)
        return 0
    
    run_data = run_scenarios(scenarios, verbose=args.verbose)
    print_results(run_data, verbose=args.verbose)
    
    # Exit code based on whether all tests matched expected
    matched = sum(1 for _, r in run_data["results"] if r.result_matches_expected)
    total = len(run_data["results"])
    
    return 0 if matched == total else 1


if __name__ == "__main__":
    exit(main())
