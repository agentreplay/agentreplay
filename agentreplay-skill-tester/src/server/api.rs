// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! REST API handlers for the Skill Tester server
//!
//! These handlers are used by the main AgentReplay server to expose
//! skill tester functionality through the existing API surface.

use serde::{Deserialize, Serialize};

/// Request to load and validate a skill
#[derive(Debug, Serialize, Deserialize)]
pub struct LoadSkillRequest {
    /// Path to SKILL.md or skill directory
    pub path: Option<String>,
    /// Raw SKILL.md content (alternative to path)
    pub content: Option<String>,
    /// ClawHub URL to fetch from
    pub url: Option<String>,
}

/// Response after loading a skill
#[derive(Debug, Serialize, Deserialize)]
pub struct LoadSkillResponse {
    pub success: bool,
    pub skill_name: Option<String>,
    pub version: Option<String>,
    pub version_hash: Option<String>,
    pub validation_passed: bool,
    pub validation_findings: Vec<ValidationFindingResponse>,
    pub error: Option<String>,
}

/// Individual validation finding for API response
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationFindingResponse {
    pub check: String,
    pub severity: String,
    pub message: String,
    pub detail: Option<String>,
}

/// Request to run skill tests
#[derive(Debug, Serialize, Deserialize)]
pub struct RunTestsRequest {
    /// Skill path or name
    pub skill: String,
    /// Test directory path (optional)
    pub tests_dir: Option<String>,
    /// Tags to filter
    pub tags: Option<Vec<String>>,
    /// Risk tier filter
    pub risk_tier: Option<String>,
}

/// Test run response
#[derive(Debug, Serialize, Deserialize)]
pub struct RunTestsResponse {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration_ms: u64,
    pub safety_gate_passed: bool,
    pub results: Vec<TestResultResponse>,
}

/// Individual test result for API response
#[derive(Debug, Serialize, Deserialize)]
pub struct TestResultResponse {
    pub test_id: String,
    pub skill: String,
    pub status: String,
    pub duration_ms: u64,
    pub assertions_passed: usize,
    pub assertions_failed: usize,
    pub error: Option<String>,
}

/// Request to run OWASP security scan
#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityScanRequest {
    pub skill: String,
    pub scan_level: Option<String>, // "basic", "full"
}

/// Security scan response
#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityScanResponse {
    pub skill_name: String,
    pub safe_for_production: bool,
    pub high_risk_count: usize,
    pub medium_risk_count: usize,
    pub findings: Vec<OwaspFindingResponse>,
    pub verdict: String,
}

/// OWASP finding for API response
#[derive(Debug, Serialize, Deserialize)]
pub struct OwaspFindingResponse {
    pub id: String,
    pub name: String,
    pub risk: String,
    pub description: String,
    pub detail: Option<String>,
    pub recommendation: Option<String>,
}

/// Skill diff request
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillDiffRequest {
    pub v1_path: String,
    pub v2_path: String,
}

/// Skill diff response
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillDiffResponse {
    pub v1_hash: String,
    pub v2_hash: String,
    pub changes: Vec<SkillDiffChange>,
}

/// A change between skill versions
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillDiffChange {
    pub field: String,
    pub change_type: String, // "added", "removed", "modified"
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}
