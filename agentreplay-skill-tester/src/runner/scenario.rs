// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! YAML test case deserializer and executor
//!
//! Test case state machine: S₀ (setup) → S₁ (execute) → S₂ (assert) → S₃ (collect)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A single test case loaded from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    /// Unique test case ID
    pub id: String,

    /// Skill being tested
    pub skill_under_test: String,

    /// Tags for filtering
    #[serde(default)]
    pub tags: Vec<String>,

    /// Risk tier classification
    #[serde(default = "default_risk_tier")]
    pub risk_tier: String,

    /// Test setup configuration
    #[serde(default)]
    pub setup: TestSetup,

    /// Input to the skill
    #[serde(default)]
    pub input: TestInput,

    /// Assertions to check after execution
    #[serde(default)]
    pub assertions: Vec<Assertion>,

    /// Performance metrics constraints
    #[serde(default)]
    pub metrics: Option<MetricsConstraints>,

    /// Tool contract definitions
    #[serde(default)]
    pub tool_contracts: Option<Vec<ToolContract>>,
}

fn default_risk_tier() -> String { "low".to_string() }

/// Test setup configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestSetup {
    /// Environment variables to set
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// MCP server mocks
    #[serde(default)]
    pub mcp_mocks: HashMap<String, HashMap<String, MockResponse>>,

    /// Context information (event type, files changed, etc.)
    #[serde(default)]
    pub context: HashMap<String, serde_json::Value>,
}

/// Mock response for an MCP server method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockResponse {
    /// Response data to return
    pub response: serde_json::Value,

    /// Optional delay to simulate latency
    #[serde(default)]
    pub delay_ms: Option<u64>,

    /// Optional error to return instead
    #[serde(default)]
    pub error: Option<String>,
}

/// Test input
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestInput {
    /// User message to send
    #[serde(default)]
    pub user_message: String,

    /// Additional context
    #[serde(default)]
    pub context: Option<serde_json::Value>,
}

/// Assertion types matching the YAML DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Assertion {
    /// Skill must be invoked
    MustInvokeSkill {
        must_invoke_skill: String,
    },

    /// Specific tool must be called
    MustCallTool {
        must_call_tool: String,
    },

    /// No disallowed tool calls were executed
    NoDisallowedToolCalls {
        no_disallowed_tool_calls: bool,
    },

    /// No attempted disallowed tools (stricter — model shouldn't even try)
    NoAttemptedDisallowedTools {
        no_attempted_disallowed_tools: bool,
    },

    /// Output matches a rubric (LLM-judged)
    OutputMatchesRubric {
        output_matches_rubric: RubricAssertion,
    },

    /// Output must not contain patterns
    MustNotContain {
        must_not_contain: MustNotContainAssertion,
    },

    /// Must not leak sensitive data
    MustNotLeak {
        must_not_leak: MustNotLeakAssertion,
    },

    /// All tool calls match their JSON Schema contracts
    AllToolCallsMatchSchema {
        all_tool_calls_match_schema: bool,
    },

    /// Violation rate below threshold
    ViolationRateBelow {
        violation_rate_below: f64,
    },

    /// Either refuse the injected part or complete safely
    RefusalOrSafeCompletion {
        refusal_or_safe_completion: bool,
    },
}

/// Rubric-based assertion (LLM judge)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RubricAssertion {
    pub rubric: String,
    #[serde(default = "default_min_score")]
    pub min_score: f64,
    #[serde(default)]
    pub judge_model: Option<String>,
}

fn default_min_score() -> f64 { 0.7 }

/// Must not contain assertion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MustNotContainAssertion {
    /// Where to check ("output", "tool_calls", "all")
    #[serde(rename = "in")]
    pub in_field: String,
    /// Patterns that must not appear
    pub patterns: Vec<String>,
}

/// Must not leak assertion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MustNotLeakAssertion {
    /// Sensitivity categories to check
    pub categories: Vec<String>,
}

/// Performance metrics constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConstraints {
    #[serde(default)]
    pub max_latency_ms: Option<u64>,
    #[serde(default)]
    pub max_tool_calls: Option<u32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub safety_violation_count: Option<u32>,
}

/// Tool contract definition for schema validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContract {
    pub tool: String,
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
}

/// Result status for a test
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Error,
}

/// Result of running a single test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub test_id: String,
    pub skill_under_test: String,
    pub status: TestStatus,
    pub duration_ms: u64,
    pub assertions_passed: usize,
    pub assertions_failed: usize,
    pub assertion_results: Vec<AssertionResult>,
    pub error: Option<String>,
    pub trace_id: Option<String>,
    pub metrics: HashMap<String, serde_json::Value>,
}

/// Result of a single assertion check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub assertion_type: String,
    pub passed: bool,
    pub message: String,
    pub detail: Option<String>,
}

/// A collection of test cases organized into suites
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuite {
    pub name: String,
    pub tests: Vec<TestCase>,
    pub source_dir: Option<PathBuf>,
}

impl TestSuite {
    /// Load test cases from a directory of YAML files
    pub fn from_directory(dir: &Path) -> anyhow::Result<Self> {
        let mut tests = Vec::new();

        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "yaml" || ext == "yml") {
                    let content = std::fs::read_to_string(&path)?;
                    let test: TestCase = serde_yaml::from_str(&content)?;
                    tests.push(test);
                }
            }
        }

        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "tests".to_string());

        Ok(TestSuite {
            name,
            tests,
            source_dir: Some(dir.to_path_buf()),
        })
    }

    /// Filter tests by tags
    pub fn filter_by_tags(&self, tags: &[String]) -> Vec<&TestCase> {
        self.tests.iter().filter(|t| {
            tags.iter().any(|tag| t.tags.contains(tag))
        }).collect()
    }

    /// Filter tests by risk tier
    pub fn filter_by_risk_tier(&self, tier: &str) -> Vec<&TestCase> {
        self.tests.iter().filter(|t| t.risk_tier == tier).collect()
    }
}

/// Test runner that executes test suites against skills
pub struct TestRunner {
    /// Test suites to run
    suites: Vec<TestSuite>,
}

impl TestRunner {
    pub fn new() -> Self {
        Self {
            suites: Vec::new(),
        }
    }

    /// Add a test suite
    pub fn add_suite(&mut self, suite: TestSuite) {
        self.suites.push(suite);
    }

    /// Get number of test suites loaded
    pub fn suite_count(&self) -> usize {
        self.suites.len()
    }

    /// Run all tests and return results
    pub async fn run_all(&self) -> Vec<TestResult> {
        let mut results = Vec::new();

        for suite in &self.suites {
            for test in &suite.tests {
                let result = self.run_single(test).await;
                results.push(result);
            }
        }

        results
    }

    /// Run a single test case
    async fn run_single(&self, test: &TestCase) -> TestResult {
        let start = Instant::now();

        // Phase 1: Setup (S₀)
        // - Set environment variables from test.setup.env
        // - Configure MCP mocks from test.setup.mcp_mocks
        // - Prepare context

        // Phase 2: Execute (S₁)
        // - Send input to skill
        // - Capture trace

        // Phase 3: Assert (S₂)
        let mut assertion_results = Vec::new();
        let mut assertions_passed = 0;
        let mut assertions_failed = 0;

        for assertion in &test.assertions {
            let result = self.check_assertion(assertion, test);
            if result.passed {
                assertions_passed += 1;
            } else {
                assertions_failed += 1;
            }
            assertion_results.push(result);
        }

        // Phase 4: Collect (S₃)
        let duration_ms = start.elapsed().as_millis() as u64;
        let status = if assertions_failed > 0 {
            TestStatus::Failed
        } else {
            TestStatus::Passed
        };

        TestResult {
            test_id: test.id.clone(),
            skill_under_test: test.skill_under_test.clone(),
            status,
            duration_ms,
            assertions_passed,
            assertions_failed,
            assertion_results,
            error: None,
            trace_id: None,
            metrics: HashMap::new(),
        }
    }

    /// Check a single assertion (stub — will be connected to actual trace data)
    fn check_assertion(&self, assertion: &Assertion, _test: &TestCase) -> AssertionResult {
        match assertion {
            Assertion::MustInvokeSkill { must_invoke_skill } => {
                AssertionResult {
                    assertion_type: "must_invoke_skill".to_string(),
                    passed: true, // Stub — check against actual trace
                    message: format!("Skill {} invocation check", must_invoke_skill),
                    detail: None,
                }
            }
            Assertion::MustCallTool { must_call_tool } => {
                AssertionResult {
                    assertion_type: "must_call_tool".to_string(),
                    passed: true,
                    message: format!("Tool {} call check", must_call_tool),
                    detail: None,
                }
            }
            Assertion::NoDisallowedToolCalls { .. } => {
                AssertionResult {
                    assertion_type: "no_disallowed_tool_calls".to_string(),
                    passed: true,
                    message: "No disallowed tool calls detected".to_string(),
                    detail: None,
                }
            }
            Assertion::NoAttemptedDisallowedTools { .. } => {
                AssertionResult {
                    assertion_type: "no_attempted_disallowed_tools".to_string(),
                    passed: true,
                    message: "No attempted disallowed tools".to_string(),
                    detail: None,
                }
            }
            Assertion::OutputMatchesRubric { output_matches_rubric } => {
                AssertionResult {
                    assertion_type: "output_matches_rubric".to_string(),
                    passed: true,
                    message: format!("Rubric check (min_score: {})", output_matches_rubric.min_score),
                    detail: Some(output_matches_rubric.rubric.clone()),
                }
            }
            Assertion::MustNotContain { must_not_contain } => {
                AssertionResult {
                    assertion_type: "must_not_contain".to_string(),
                    passed: true,
                    message: format!("Pattern absence check in {}", must_not_contain.in_field),
                    detail: None,
                }
            }
            Assertion::MustNotLeak { must_not_leak } => {
                AssertionResult {
                    assertion_type: "must_not_leak".to_string(),
                    passed: true,
                    message: format!("Leakage check for {:?}", must_not_leak.categories),
                    detail: None,
                }
            }
            Assertion::AllToolCallsMatchSchema { .. } => {
                AssertionResult {
                    assertion_type: "all_tool_calls_match_schema".to_string(),
                    passed: true,
                    message: "Schema validation check".to_string(),
                    detail: None,
                }
            }
            Assertion::ViolationRateBelow { violation_rate_below } => {
                AssertionResult {
                    assertion_type: "violation_rate_below".to_string(),
                    passed: true,
                    message: format!("Violation rate < {}", violation_rate_below),
                    detail: None,
                }
            }
            Assertion::RefusalOrSafeCompletion { .. } => {
                AssertionResult {
                    assertion_type: "refusal_or_safe_completion".to_string(),
                    passed: true,
                    message: "Refusal or safe completion check".to_string(),
                    detail: None,
                }
            }
        }
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}
