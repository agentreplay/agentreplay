// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// agentreplay-core/src/enterprise.rs
//
// Enterprise feature data models for Agentreplay

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export types from server API modules for database storage
// These are simplified versions suitable for database storage

/// Prompt template for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: u128,
    pub name: String,
    pub description: String,
    pub template: String,
    pub variables: Vec<String>,
    pub tags: Vec<String>,
    pub version: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub created_by: String,
    #[serde(default)]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Experiment for A/B testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    pub id: u128,
    pub name: String,
    pub description: String,
    pub variants: Vec<ExperimentVariant>,
    pub status: String,
    pub traffic_split: HashMap<String, f64>,
    pub metrics: Vec<String>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentVariant {
    pub id: String,
    pub name: String,
    pub description: String,
    pub config: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResult {
    pub experiment_id: u128,
    pub variant_id: String,
    pub trace_id: u128,
    pub metrics: HashMap<String, f64>,
    pub timestamp_us: u64,
}

/// Budget alert configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAlert {
    pub id: u128,
    pub name: String,
    pub description: String,
    pub threshold_type: String,
    pub threshold_value: f64,
    pub period: String,
    pub filters: AlertFilters,
    pub actions: Vec<AlertAction>,
    pub status: String,
    pub triggered_count: u32,
    pub last_triggered: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlertFilters {
    pub project_ids: Vec<u16>,
    pub agent_ids: Vec<u64>,
    pub models: Vec<String>,
    pub environments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertAction {
    pub action_type: String,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub alert_id: u128,
    pub triggered_at: u64,
    pub actual_value: f64,
    pub threshold_value: f64,
    pub message: String,
}

/// Compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub id: u128,
    pub report_type: String,
    pub period_start: u64,
    pub period_end: u64,
    pub generated_at: u64,
    pub generated_by: String,
    pub status: String,
    pub summary: ReportSummary,
    pub findings: Vec<ComplianceFinding>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_traces: usize,
    pub total_cost: f64,
    pub total_tokens: u64,
    pub total_users: usize,
    pub pii_detected: usize,
    pub security_issues: usize,
    pub quality_score: f64,
    pub compliance_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceFinding {
    pub severity: String,
    pub category: String,
    pub description: String,
    pub affected_traces: Vec<u128>,
    pub recommendation: String,
}

/// Cost statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostStats {
    pub total_cost: f64,
    pub trace_count: usize,
    pub total_tokens: u64,
}

/// Data privacy metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataPrivacyMetrics {
    pub pii_instances: usize,
    pub pii_types: HashMap<String, usize>,
    pub data_retention_compliance: f64,
    pub deletion_requests: usize,
    pub encryption_coverage: f64,
}

/// Security metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecurityMetrics {
    pub authentication_failures: usize,
    pub rate_limit_violations: usize,
    pub suspicious_patterns: usize,
    pub token_exposures: usize,
    pub security_score: f64,
}

/// Analytics data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub timestamp: u64,
    pub value: f64,
    pub count: usize,
}
