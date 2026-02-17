// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Test sandbox with skill snapshot isolation
//!
//! Extends the concept of TrialSandbox from agentreplay-evals with:
//! - Skill-level isolation (each test gets a fresh skill snapshot)
//! - MCP mock injection
//! - Environment variable sandboxing
//! - Filesystem scoping

use std::collections::HashMap;
use std::path::PathBuf;
use crate::manifest::SkillManifest;
use crate::runner::scenario::MockResponse;

/// Isolated sandbox for running a skill test
pub struct SkillSandbox {
    /// Skill manifest being tested
    pub skill: SkillManifest,

    /// Sandboxed environment variables
    env_vars: HashMap<String, String>,

    /// MCP mock configurations
    mcp_mocks: HashMap<String, HashMap<String, MockResponse>>,

    /// Working directory for the test
    work_dir: PathBuf,

    /// Tool allowlist (if set, only these tools can be called)
    tool_allowlist: Option<Vec<String>>,

    /// Tool denylist (these tools are always blocked)
    tool_denylist: Vec<String>,

    /// Recorded tool calls during execution
    recorded_tool_calls: Vec<RecordedToolCall>,

    /// Recorded policy violations
    policy_violations: Vec<PolicyViolation>,
}

/// A tool call recorded during sandbox execution
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordedToolCall {
    pub tool_name: String,
    pub parameters: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub timestamp_ms: u64,
    pub blocked: bool,
    pub block_reason: Option<String>,
}

/// A policy violation recorded during execution
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PolicyViolation {
    pub tool_name: String,
    pub violation_type: String,
    pub message: String,
    pub timestamp_ms: u64,
    pub was_blocked: bool,
}

impl SkillSandbox {
    pub fn new(skill: SkillManifest, work_dir: PathBuf) -> Self {
        Self {
            skill,
            env_vars: HashMap::new(),
            mcp_mocks: HashMap::new(),
            work_dir,
            tool_allowlist: None,
            tool_denylist: Vec::new(),
            recorded_tool_calls: Vec::new(),
            policy_violations: Vec::new(),
        }
    }

    /// Set environment variables for the sandbox
    pub fn set_env(&mut self, env: HashMap<String, String>) {
        self.env_vars = env;
    }

    /// Configure MCP mocks
    pub fn set_mcp_mocks(&mut self, mocks: HashMap<String, HashMap<String, MockResponse>>) {
        self.mcp_mocks = mocks;
    }

    /// Set tool allowlist
    pub fn set_tool_allowlist(&mut self, tools: Vec<String>) {
        self.tool_allowlist = Some(tools);
    }

    /// Set tool denylist
    pub fn set_tool_denylist(&mut self, tools: Vec<String>) {
        self.tool_denylist = tools;
    }

    /// Check if a tool call is allowed by policy
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        // Check denylist first
        if self.tool_denylist.iter().any(|t| {
            if t.ends_with('*') {
                tool_name.starts_with(&t[..t.len() - 1])
            } else {
                tool_name == t
            }
        }) {
            return false;
        }

        // Check allowlist if set
        if let Some(allowlist) = &self.tool_allowlist {
            return allowlist.iter().any(|t| {
                if t.ends_with('*') {
                    tool_name.starts_with(&t[..t.len() - 1])
                } else {
                    tool_name == t
                }
            });
        }

        true
    }

    /// Record a tool call attempt
    pub fn record_tool_call(&mut self, tool_name: &str, params: Option<serde_json::Value>, timestamp_ms: u64) {
        let allowed = self.is_tool_allowed(tool_name);

        let recorded = RecordedToolCall {
            tool_name: tool_name.to_string(),
            parameters: params,
            result: None,
            timestamp_ms,
            blocked: !allowed,
            block_reason: if !allowed {
                Some("Tool not in allowlist or in denylist".to_string())
            } else {
                None
            },
        };

        if !allowed {
            self.policy_violations.push(PolicyViolation {
                tool_name: tool_name.to_string(),
                violation_type: "disallowed_tool_call".to_string(),
                message: format!("Tool '{}' is not allowed by policy", tool_name),
                timestamp_ms,
                was_blocked: true,
            });
        }

        self.recorded_tool_calls.push(recorded);
    }

    /// Get all recorded tool calls
    pub fn tool_calls(&self) -> &[RecordedToolCall] {
        &self.recorded_tool_calls
    }

    /// Get all policy violations
    pub fn policy_violations(&self) -> &[PolicyViolation] {
        &self.policy_violations
    }

    /// Resolve an MCP mock response
    pub fn resolve_mcp_mock(&self, server: &str, method: &str) -> Option<&MockResponse> {
        self.mcp_mocks
            .get(server)
            .and_then(|methods| methods.get(method))
    }

    /// Get sandbox working directory
    pub fn work_dir(&self) -> &PathBuf {
        &self.work_dir
    }

    /// Reset recorded state (for re-running)
    pub fn reset(&mut self) {
        self.recorded_tool_calls.clear();
        self.policy_violations.clear();
    }
}
