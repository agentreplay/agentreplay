// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Tool policy enforcement evaluator
//!
//! Validates that tool calls conform to allow/deny policies.
//! Tracks both executed violations and attempted-but-blocked violations.

use serde::{Deserialize, Serialize};

/// Tool policy definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicy {
    /// Tools that are explicitly allowed (supports glob patterns like "sentry.*")
    #[serde(default)]
    pub allowlist: Vec<String>,

    /// Tools that are explicitly denied (supports glob patterns like "shell.*")
    #[serde(default)]
    pub denylist: Vec<String>,
}

/// Tool policy evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicyResult {
    /// Number of executed violations (worst case — policy didn't catch)
    pub executed_violations: usize,
    /// Number of attempted violations (caught by policy)
    pub attempted_violations: usize,
    /// Total tool calls checked
    pub total_calls: usize,
    /// Per-tool violation details
    pub violations: Vec<ToolPolicyViolation>,
    /// Whether policy enforcement passed
    pub passed: bool,
}

/// A single tool policy violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicyViolation {
    pub tool_name: String,
    pub violation_type: ToolPolicyViolationType,
    pub was_blocked: bool,
    pub message: String,
}

/// Type of tool policy violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolPolicyViolationType {
    /// Tool was in denylist
    Denylisted,
    /// Tool was not in allowlist
    NotAllowlisted,
    /// Tool exceeded rate limit
    RateLimitExceeded,
}

/// Tool policy evaluator
pub struct ToolPolicyEvaluator {
    policy: ToolPolicy,
}

impl ToolPolicyEvaluator {
    pub fn new(policy: ToolPolicy) -> Self {
        Self { policy }
    }

    /// Check if a tool name matches a pattern (supports trailing *)
    fn matches_pattern(name: &str, pattern: &str) -> bool {
        if pattern.ends_with('*') {
            name.starts_with(&pattern[..pattern.len() - 1])
        } else {
            name == pattern
        }
    }

    /// Check if a tool is allowed by the policy
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        // Denylist takes precedence
        for pattern in &self.policy.denylist {
            if Self::matches_pattern(tool_name, pattern) {
                return false;
            }
        }

        // If allowlist is empty, everything not denied is allowed
        if self.policy.allowlist.is_empty() {
            return true;
        }

        // Check allowlist
        for pattern in &self.policy.allowlist {
            if Self::matches_pattern(tool_name, pattern) {
                return true;
            }
        }

        false
    }

    /// Evaluate tool calls against the policy
    pub fn evaluate(
        &self,
        tool_calls: &[(String, bool)], // (tool_name, was_executed)
    ) -> ToolPolicyResult {
        let mut violations = Vec::new();
        let mut executed_violations = 0;
        let mut attempted_violations = 0;

        for (tool_name, was_executed) in tool_calls {
            if !self.is_allowed(tool_name) {
                let was_blocked = !was_executed;

                if *was_executed {
                    executed_violations += 1;
                } else {
                    attempted_violations += 1;
                }

                // Determine violation type
                let violation_type = if self.policy.denylist.iter().any(|p| Self::matches_pattern(tool_name, p)) {
                    ToolPolicyViolationType::Denylisted
                } else {
                    ToolPolicyViolationType::NotAllowlisted
                };

                violations.push(ToolPolicyViolation {
                    tool_name: tool_name.clone(),
                    violation_type,
                    was_blocked,
                    message: if was_blocked {
                        format!("🛡️ POLICY BLOCK: {} not in allowlist", tool_name)
                    } else {
                        format!("🔴 VIOLATION: {} was executed despite policy", tool_name)
                    },
                });
            }
        }

        ToolPolicyResult {
            executed_violations,
            attempted_violations,
            total_calls: tool_calls.len(),
            violations,
            passed: executed_violations == 0,
        }
    }
}
