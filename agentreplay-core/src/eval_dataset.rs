// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A test case within an evaluation dataset
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestCase {
    /// Unique identifier for the test case
    pub id: u128,

    /// Input data (JSON serialized)
    pub input: String,

    /// Expected output (optional, for reference comparison)
    pub expected_output: Option<String>,

    /// Additional metadata (tags, categories, etc.)
    #[serde(default)]
    pub metadata: HashMap<String, String>,

    /// Optional TaskDefinitionV2 with explicit success criteria and graders
    #[serde(default)]
    pub task_definition_v2: Option<TaskDefinitionV2>,
}

impl TestCase {
    /// Create a new test case
    pub fn new(id: u128, input: String) -> Self {
        Self {
            id,
            input,
            expected_output: None,
            metadata: HashMap::new(),
            task_definition_v2: None,
        }
    }

    /// Create a test case with expected output
    pub fn with_expected_output(id: u128, input: String, expected_output: String) -> Self {
        Self {
            id,
            input,
            expected_output: Some(expected_output),
            metadata: HashMap::new(),
            task_definition_v2: None,
        }
    }

    /// Add metadata to the test case
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    pub fn with_task_definition(mut self, task_definition_v2: TaskDefinitionV2) -> Self {
        self.task_definition_v2 = Some(task_definition_v2);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskDefinitionV2 {
    pub schema_version: String,
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub description: Option<String>,
    pub input: serde_json::Value,
    #[serde(default)]
    pub expected_output: Option<serde_json::Value>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    pub success_criteria: SuccessCriteriaV2,
    #[serde(default)]
    pub tracked_metrics: Vec<String>,
    #[serde(default)]
    pub graders: Vec<GraderSpecV2>,
}

impl TaskDefinitionV2 {
    pub fn new(input: serde_json::Value, success_criteria: SuccessCriteriaV2) -> Self {
        let mut definition = Self {
            schema_version: "task_definition_v2".to_string(),
            task_id: String::new(),
            description: None,
            input,
            expected_output: None,
            metadata: HashMap::new(),
            success_criteria,
            tracked_metrics: Vec::new(),
            graders: Vec::new(),
        };
        definition.ensure_task_id();
        definition
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn ensure_task_id(&mut self) {
        if self.task_id.is_empty() {
            self.task_id = compute_task_id(self);
        }
    }
}

fn compute_task_id(definition: &TaskDefinitionV2) -> String {
    let canonical = serde_json::json!({
        "input": definition.input,
        "expected_output": definition.expected_output,
        "success_criteria": definition.success_criteria,
        "tracked_metrics": definition.tracked_metrics,
        "graders": definition.graders,
    });

    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    let hash = blake3::hash(&bytes);
    format!("task_{}", hash.to_hex())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuccessCriteriaV2 {
    #[serde(default)]
    pub text: Vec<String>,
    #[serde(default)]
    pub state: Vec<StateExpectationV2>,
    #[serde(default)]
    pub side_effects: Vec<SideEffectExpectationV2>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateExpectationV2 {
    pub target: String,
    pub operator: String,
    pub expected: serde_json::Value,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SideEffectExpectationV2 {
    pub effect_type: String,
    pub target: String,
    #[serde(default)]
    pub expected: Option<serde_json::Value>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraderSpecV2 {
    pub grader_id: String,
    #[serde(default)]
    pub grader_type: Option<String>,
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub policy: Option<GraderPolicyV2>,
    #[serde(default)]
    pub thresholds: Vec<GraderThresholdV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraderPolicyV2 {
    pub mode: String,
    #[serde(default)]
    pub quorum: Option<u32>,
    #[serde(default)]
    pub aggregation: Option<String>,
    #[serde(default)]
    pub threshold: Option<f64>,
    #[serde(default)]
    pub tie_breaker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraderThresholdV2 {
    pub metric: String,
    pub operator: String,
    pub value: f64,
}

/// An evaluation dataset containing multiple test cases
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalDataset {
    /// Unique identifier for the dataset
    pub id: u128,

    /// Human-readable name
    pub name: String,

    /// Description of the dataset
    pub description: String,

    /// List of test cases in this dataset
    pub test_cases: Vec<TestCase>,

    /// Creation timestamp (microseconds since Unix epoch)
    pub created_at: u64,

    /// Last update timestamp (microseconds since Unix epoch)
    pub updated_at: u64,
}

impl EvalDataset {
    /// Create a new evaluation dataset
    pub fn new(id: u128, name: String, description: String, created_at: u64) -> Self {
        Self {
            id,
            name,
            description,
            test_cases: Vec::new(),
            created_at,
            updated_at: created_at,
        }
    }

    /// Add a test case to the dataset
    pub fn add_test_case(&mut self, test_case: TestCase) {
        self.test_cases.push(test_case);
        self.updated_at = current_timestamp_us();
    }

    /// Get the number of test cases
    pub fn test_case_count(&self) -> usize {
        self.test_cases.len()
    }

    /// Find a test case by ID
    pub fn find_test_case(&self, id: u128) -> Option<&TestCase> {
        self.test_cases.iter().find(|tc| tc.id == id)
    }
}

/// Status of an evaluation run
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    /// Run is currently in progress
    Running,

    /// Run completed successfully
    Completed,

    /// Run failed with errors
    Failed,

    /// Run was stopped by user
    Stopped,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Completed => "completed",
            RunStatus::Failed => "failed",
            RunStatus::Stopped => "stopped",
        }
    }
}

/// Result of a single assertion within a grader
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssertionResult {
    pub id: String,
    pub passed: bool,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub message: Option<String>,
}

/// Individual judge vote for multi-judge consensus
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JudgeVote {
    pub judge_id: String,
    pub passed: bool,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub rationale: Option<String>,
}

/// Detailed grader result with assertions and votes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraderResult {
    pub grader_id: String,
    pub grader_type: String,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub score: Option<f64>,
    pub passed: bool,
    #[serde(default)]
    pub assertions: Vec<AssertionResult>,
    #[serde(default)]
    pub judge_votes: Vec<JudgeVote>,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

/// Overall evaluation policy and composite score
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OverallResult {
    pub policy: String,
    pub passed: bool,
    #[serde(default)]
    pub composite_score: Option<f64>,
}

/// Result of running a single test case
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunResult {
    /// ID of the test case that was run
    pub test_case_id: u128,

    /// Trial identifier for multi-trial evaluations
    #[serde(default)]
    pub trial_id: u32,

    /// RNG seed or deterministic seed used for this trial
    #[serde(default)]
    pub seed: Option<u64>,

    /// ID of the trace generated during execution
    pub trace_id: Option<u128>,

    /// Evaluation metrics collected (e.g., accuracy, hallucination)
    #[serde(default)]
    pub eval_metrics: HashMap<String, f64>,

    /// Detailed grader outputs (per grader, per assertion)
    #[serde(default)]
    pub grader_results: Vec<GraderResult>,

    /// Overall evaluation policy and composite score
    #[serde(default)]
    pub overall: Option<OverallResult>,

    /// Whether the test case passed
    pub passed: bool,

    /// Error message if the test case failed
    pub error: Option<String>,

    /// Timestamp when this result was recorded
    pub timestamp_us: u64,

    /// Optional cost in USD for this trial
    #[serde(default)]
    pub cost_usd: Option<f64>,

    /// Optional latency in milliseconds for this trial
    #[serde(default)]
    pub latency_ms: Option<u64>,
}

impl RunResult {
    /// Create a new successful run result
    pub fn success(test_case_id: u128, trace_id: u128, timestamp_us: u64) -> Self {
        Self {
            test_case_id,
            trial_id: 0,
            seed: None,
            trace_id: Some(trace_id),
            eval_metrics: HashMap::new(),
            grader_results: Vec::new(),
            overall: None,
            passed: true,
            error: None,
            timestamp_us,
            cost_usd: None,
            latency_ms: None,
        }
    }

    /// Create a new failed run result
    pub fn failure(test_case_id: u128, error: String, timestamp_us: u64) -> Self {
        Self {
            test_case_id,
            trial_id: 0,
            seed: None,
            trace_id: None,
            eval_metrics: HashMap::new(),
            grader_results: Vec::new(),
            overall: None,
            passed: false,
            error: Some(error),
            timestamp_us,
            cost_usd: None,
            latency_ms: None,
        }
    }

    /// Add an evaluation metric to the result
    pub fn with_metric(mut self, name: String, value: f64) -> Self {
        self.eval_metrics.insert(name, value);
        self
    }

    /// Set trial metadata
    pub fn with_trial(mut self, trial_id: u32, seed: Option<u64>) -> Self {
        self.trial_id = trial_id;
        self.seed = seed;
        self
    }

    /// Set cost metadata
    pub fn with_cost(mut self, cost_usd: f64) -> Self {
        self.cost_usd = Some(cost_usd);
        self
    }

    /// Set latency metadata
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }
}

/// Canonical alias for trial results
pub type TrialResult = RunResult;

/// An evaluation run representing an experiment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalRun {
    /// Unique identifier for the run
    pub id: u128,

    /// ID of the dataset used for this run
    pub dataset_id: u128,

    /// Human-readable name for the run
    pub name: String,

    /// Agent ID that was evaluated
    pub agent_id: String,

    /// Model used during the run
    pub model: String,

    /// Results for each test case
    pub results: Vec<RunResult>,

    /// When the run started (microseconds since Unix epoch)
    pub started_at: u64,

    /// When the run completed (None if still running)
    pub completed_at: Option<u64>,

    /// Current status of the run
    pub status: RunStatus,

    /// Additional configuration used for this run
    #[serde(default)]
    pub config: HashMap<String, String>,

    /// Total cost of the run in USD (input + output tokens)
    #[serde(default)]
    pub total_cost: f64,

    /// Token budget limit for the run (None = unlimited)
    #[serde(default)]
    pub token_budget: Option<u64>,

    /// Breakdown of costs by category
    #[serde(default)]
    pub cost_breakdown: CostBreakdown,

    /// Schema version for eval run serialization
    #[serde(default = "default_eval_run_schema_version")]
    pub schema_version: String,
}

fn default_eval_run_schema_version() -> String {
    "eval_run_v1".to_string()
}

/// Cost breakdown by category
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CostBreakdown {
    /// Cost for input tokens
    pub input_tokens_cost: f64,
    /// Cost for output tokens
    pub output_tokens_cost: f64,
    /// Cost for evaluator LLM calls
    pub evaluator_cost: f64,
    /// Total tokens consumed (input)
    pub total_input_tokens: u64,
    /// Total tokens consumed (output)
    pub total_output_tokens: u64,
    /// Number of LLM calls made
    pub llm_call_count: u32,
}

impl EvalRun {
    /// Create a new evaluation run
    pub fn new(
        id: u128,
        dataset_id: u128,
        name: String,
        agent_id: String,
        model: String,
        started_at: u64,
    ) -> Self {
        Self {
            id,
            dataset_id,
            name,
            agent_id,
            model,
            results: Vec::new(),
            started_at,
            completed_at: None,
            status: RunStatus::Running,
            config: HashMap::new(),
            total_cost: 0.0,
            token_budget: None,
            cost_breakdown: CostBreakdown::default(),
            schema_version: default_eval_run_schema_version(),
        }
    }

    /// Add a result to the run
    pub fn add_result(&mut self, result: RunResult) {
        self.results.push(result);
    }

    /// Set the token budget for this run
    pub fn with_token_budget(mut self, budget: u64) -> Self {
        self.token_budget = Some(budget);
        self
    }

    /// Update cost tracking
    pub fn add_cost(
        &mut self,
        input_tokens: u64,
        output_tokens: u64,
        input_cost_per_1k: f64,
        output_cost_per_1k: f64,
    ) {
        let input_cost = (input_tokens as f64 / 1000.0) * input_cost_per_1k;
        let output_cost = (output_tokens as f64 / 1000.0) * output_cost_per_1k;

        self.cost_breakdown.input_tokens_cost += input_cost;
        self.cost_breakdown.output_tokens_cost += output_cost;
        self.cost_breakdown.total_input_tokens += input_tokens;
        self.cost_breakdown.total_output_tokens += output_tokens;
        self.cost_breakdown.llm_call_count += 1;
        self.total_cost += input_cost + output_cost;
    }

    /// Add evaluator cost (separate from agent costs)
    pub fn add_evaluator_cost(&mut self, cost: f64) {
        self.cost_breakdown.evaluator_cost += cost;
        self.total_cost += cost;
    }

    /// Check if the run has exceeded its token budget
    pub fn is_over_budget(&self) -> bool {
        if let Some(budget) = self.token_budget {
            let total_tokens =
                self.cost_breakdown.total_input_tokens + self.cost_breakdown.total_output_tokens;
            total_tokens > budget
        } else {
            false
        }
    }

    /// Mark the run as completed
    pub fn complete(&mut self, timestamp_us: u64) {
        self.status = RunStatus::Completed;
        self.completed_at = Some(timestamp_us);
    }

    /// Mark the run as failed
    pub fn fail(&mut self, timestamp_us: u64) {
        self.status = RunStatus::Failed;
        self.completed_at = Some(timestamp_us);
    }

    /// Mark the run as stopped
    pub fn stop(&mut self, timestamp_us: u64) {
        self.status = RunStatus::Stopped;
        self.completed_at = Some(timestamp_us);
    }

    /// Get the number of test cases that passed
    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|r| r.passed).count()
    }

    /// Get the number of test cases that failed
    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|r| !r.passed).count()
    }

    /// Get the pass rate (0.0 to 1.0)
    pub fn pass_rate(&self) -> f64 {
        if self.results.is_empty() {
            0.0
        } else {
            self.passed_count() as f64 / self.results.len() as f64
        }
    }

    /// Get aggregated metrics across all results
    pub fn aggregate_metrics(&self) -> HashMap<String, f64> {
        let mut sums = HashMap::new();
        let mut counts = HashMap::new();

        for result in &self.results {
            for (metric_name, value) in &result.eval_metrics {
                let sum = sums.entry(metric_name.clone()).or_insert(0.0);
                *sum += value;
                let count = counts.entry(metric_name.clone()).or_insert(0);
                *count += 1;
            }
        }

        // Calculate averages
        let mut aggregated = HashMap::new();
        for (metric_name, sum) in sums {
            if let Some(count) = counts.get(&metric_name) {
                aggregated.insert(metric_name, sum / *count as f64);
            }
        }

        aggregated
    }

    /// Check if the run is still in progress
    pub fn is_running(&self) -> bool {
        matches!(self.status, RunStatus::Running)
    }

    /// Check if the run is finished (completed, failed, or stopped)
    pub fn is_finished(&self) -> bool {
        !self.is_running()
    }

    /// Compute per-test-case aggregates for multi-trial runs
    pub fn task_aggregates(&self, k_values: &[usize]) -> Vec<TaskAggregate> {
        let mut grouped: HashMap<u128, Vec<&RunResult>> = HashMap::new();
        for result in &self.results {
            grouped.entry(result.test_case_id).or_default().push(result);
        }

        let mut aggregates: Vec<TaskAggregate> = grouped
            .into_iter()
            .map(|(test_case_id, trials)| compute_task_aggregate(test_case_id, &trials, k_values))
            .collect();

        aggregates.sort_by(|a, b| a.test_case_id.cmp(&b.test_case_id));
        aggregates
    }
}

/// Binomial pass-rate confidence interval (Wilson)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PassRateCI {
    pub method: String,
    pub lower: f64,
    pub upper: f64,
}

/// Aggregated per-task metrics across trials
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskAggregate {
    pub test_case_id: u128,
    pub trials: usize,
    pub passed: usize,
    pub pass_rate: f64,
    #[serde(default)]
    pub pass_at_k_estimated: HashMap<String, f64>,
    #[serde(default)]
    pub pass_at_k_empirical: HashMap<String, f64>,
    #[serde(default)]
    pub pass_all_k_estimated: HashMap<String, f64>,
    #[serde(default)]
    pub pass_all_k_empirical: HashMap<String, f64>,
    #[serde(default)]
    pub mean_cost_usd: Option<f64>,
    #[serde(default)]
    pub p50_latency_ms: Option<u64>,
    #[serde(default)]
    pub p95_latency_ms: Option<u64>,
    #[serde(default)]
    pub pass_rate_ci: Option<PassRateCI>,
}

fn compute_task_aggregate(
    test_case_id: u128,
    trials: &[&RunResult],
    k_values: &[usize],
) -> TaskAggregate {
    let total = trials.len();
    let passed = trials.iter().filter(|r| r.passed).count();
    let pass_rate = if total == 0 { 0.0 } else { passed as f64 / total as f64 };

    let mut pass_at_k_estimated = HashMap::new();
    let mut pass_at_k_empirical = HashMap::new();
    let mut pass_all_k_estimated = HashMap::new();
    let mut pass_all_k_empirical = HashMap::new();

    let mut ordered_trials: Vec<&RunResult> = trials.to_vec();
    ordered_trials.sort_by(|a, b| {
        a.trial_id
            .cmp(&b.trial_id)
            .then_with(|| a.timestamp_us.cmp(&b.timestamp_us))
    });

    let ordered_passes: Vec<bool> = ordered_trials.iter().map(|t| t.passed).collect();

    for &k in k_values {
        if k == 0 {
            continue;
        }
        let estimate = 1.0 - (1.0 - pass_rate).powi(k as i32);
        pass_at_k_estimated.insert(k.to_string(), estimate);

        let pass_all_estimate = pass_rate.powi(k as i32);
        pass_all_k_estimated.insert(k.to_string(), pass_all_estimate);

        let slice = ordered_passes.iter().take(k);
        let any_pass = slice.clone().any(|p| *p);
        let all_pass = ordered_passes.iter().take(k).all(|p| *p);

        pass_at_k_empirical.insert(k.to_string(), if any_pass { 1.0 } else { 0.0 });
        pass_all_k_empirical.insert(k.to_string(), if all_pass { 1.0 } else { 0.0 });
    }

    let costs: Vec<f64> = trials.iter().filter_map(|t| t.cost_usd).collect();
    let mean_cost_usd = if costs.is_empty() {
        None
    } else {
        Some(costs.iter().sum::<f64>() / costs.len() as f64)
    };

    let mut latencies: Vec<u64> = trials.iter().filter_map(|t| t.latency_ms).collect();
    latencies.sort_unstable();
    let p50_latency_ms = percentile_u64(&latencies, 0.50);
    let p95_latency_ms = percentile_u64(&latencies, 0.95);

    let pass_rate_ci = if total > 0 {
        Some(wilson_ci(passed as f64, total as f64, 1.96))
    } else {
        None
    };

    TaskAggregate {
        test_case_id,
        trials: total,
        passed,
        pass_rate,
        pass_at_k_estimated,
        pass_at_k_empirical,
        pass_all_k_estimated,
        pass_all_k_empirical,
        mean_cost_usd,
        p50_latency_ms,
        p95_latency_ms,
        pass_rate_ci,
    }
}

fn percentile_u64(values: &[u64], percentile: f64) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let rank = ((values.len() - 1) as f64 * percentile).round() as usize;
    values.get(rank).copied()
}

fn wilson_ci(successes: f64, total: f64, z: f64) -> PassRateCI {
    let p = if total == 0.0 { 0.0 } else { successes / total };
    let z2 = z * z;
    let denom = 1.0 + z2 / total;
    let center = (p + z2 / (2.0 * total)) / denom;
    let margin = (z * ((p * (1.0 - p) / total) + (z2 / (4.0 * total * total))).sqrt()) / denom;

    PassRateCI {
        method: "wilson".to_string(),
        lower: (center - margin).max(0.0),
        upper: (center + margin).min(1.0),
    }
}

/// Get current timestamp in microseconds since Unix epoch
fn current_timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_case() {
        let tc = TestCase::new(1, r#"{"query": "What is AI?"}"#.to_string());
        assert_eq!(tc.id, 1);
        assert!(tc.expected_output.is_none());
        assert!(tc.metadata.is_empty());
    }

    #[test]
    fn test_create_test_case_with_expected_output() {
        let tc = TestCase::with_expected_output(
            1,
            r#"{"query": "What is AI?"}"#.to_string(),
            "AI is artificial intelligence".to_string(),
        );
        assert_eq!(tc.id, 1);
        assert_eq!(
            tc.expected_output.as_ref().unwrap(),
            "AI is artificial intelligence"
        );
    }

    #[test]
    fn test_create_dataset() {
        let dataset = EvalDataset::new(
            1,
            "Test Dataset".to_string(),
            "A dataset for testing".to_string(),
            1000,
        );
        assert_eq!(dataset.id, 1);
        assert_eq!(dataset.name, "Test Dataset");
        assert_eq!(dataset.test_case_count(), 0);
    }

    #[test]
    fn test_add_test_case_to_dataset() {
        let mut dataset = EvalDataset::new(
            1,
            "Test Dataset".to_string(),
            "A dataset for testing".to_string(),
            1000,
        );

        let tc = TestCase::new(1, r#"{"query": "What is AI?"}"#.to_string());
        dataset.add_test_case(tc);

        assert_eq!(dataset.test_case_count(), 1);
        assert!(dataset.find_test_case(1).is_some());
    }

    #[test]
    fn test_create_eval_run() {
        let run = EvalRun::new(
            1,
            100,
            "Baseline Run".to_string(),
            "agent-1".to_string(),
            "gpt-4".to_string(),
            1000,
        );

        assert_eq!(run.id, 1);
        assert_eq!(run.dataset_id, 100);
        assert!(run.is_running());
        assert!(!run.is_finished());
    }

    #[test]
    fn test_run_result_success() {
        let result = RunResult::success(1, 500, 1000);
        assert!(result.passed);
        assert_eq!(result.trace_id, Some(500));
        assert!(result.error.is_none());
    }

    #[test]
    fn test_run_result_failure() {
        let result = RunResult::failure(1, "Timeout".to_string(), 1000);
        assert!(!result.passed);
        assert!(result.trace_id.is_none());
        assert_eq!(result.error.as_ref().unwrap(), "Timeout");
    }

    #[test]
    fn test_eval_run_metrics() {
        let mut run = EvalRun::new(
            1,
            100,
            "Test Run".to_string(),
            "agent-1".to_string(),
            "gpt-4".to_string(),
            1000,
        );

        // Add successful results with metrics
        let result1 = RunResult::success(1, 500, 1000).with_metric("accuracy".to_string(), 0.9);
        let result2 = RunResult::success(2, 501, 1001).with_metric("accuracy".to_string(), 0.8);

        run.add_result(result1);
        run.add_result(result2);

        assert_eq!(run.passed_count(), 2);
        assert_eq!(run.failed_count(), 0);
        assert_eq!(run.pass_rate(), 1.0);

        let aggregated = run.aggregate_metrics();
        // Use approximate comparison for floating point (average of 0.9 and 0.8)
        let accuracy = aggregated.get("accuracy").copied().unwrap_or(0.0);
        assert!(
            (accuracy - 0.85).abs() < 0.0001,
            "Expected ~0.85, got {}",
            accuracy
        );
    }

    #[test]
    fn test_run_status_transitions() {
        let mut run = EvalRun::new(
            1,
            100,
            "Test Run".to_string(),
            "agent-1".to_string(),
            "gpt-4".to_string(),
            1000,
        );

        assert_eq!(run.status, RunStatus::Running);

        run.complete(2000);
        assert_eq!(run.status, RunStatus::Completed);
        assert_eq!(run.completed_at, Some(2000));
        assert!(run.is_finished());
    }
}
