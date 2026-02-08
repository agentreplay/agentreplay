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

// agentreplay-server/src/api/compliance.rs
//
// Compliance reports and audit API endpoints

use super::query::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

// ============================================================================
// Data Models - Use core types with API enums
// ============================================================================

use agentreplay_core::enterprise::{
    ComplianceFinding as CoreComplianceFinding, ComplianceReport as CoreComplianceReport,
    DataPrivacyMetrics, ReportSummary as CoreReportSummary, SecurityMetrics,
};

// API representation with typed enums
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub id: u128,
    pub report_type: ReportType,
    pub period_start: u64,
    pub period_end: u64,
    pub generated_at: u64,
    pub generated_by: String,
    pub status: ReportStatus,
    pub summary: ReportSummary,
    pub findings: Vec<ComplianceFinding>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReportType {
    DataPrivacy, // GDPR, CCPA compliance
    Security,    // Security audit
    Usage,       // Usage and cost audit
    Quality,     // Quality metrics
    Full,        // Comprehensive audit
}

impl ReportType {
    pub fn as_str(&self) -> &str {
        match self {
            ReportType::DataPrivacy => "data_privacy",
            ReportType::Security => "security",
            ReportType::Usage => "usage",
            ReportType::Quality => "quality",
            ReportType::Full => "full",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "data_privacy" => ReportType::DataPrivacy,
            "security" => ReportType::Security,
            "usage" => ReportType::Usage,
            "quality" => ReportType::Quality,
            "full" => ReportType::Full,
            _ => ReportType::Full,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReportStatus {
    Generating,
    Completed,
    Failed,
}

impl ReportStatus {
    pub fn as_str(&self) -> &str {
        match self {
            ReportStatus::Generating => "generating",
            ReportStatus::Completed => "completed",
            ReportStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "generating" => ReportStatus::Generating,
            "completed" => ReportStatus::Completed,
            "failed" => ReportStatus::Failed,
            _ => ReportStatus::Completed,
        }
    }
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
    pub severity: Severity,
    pub category: String,
    pub description: String,
    pub affected_traces: Vec<u128>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &str {
        match self {
            Severity::Critical => "critical",
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
            Severity::Info => "info",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "critical" => Severity::Critical,
            "high" => Severity::High,
            "medium" => Severity::Medium,
            "low" => Severity::Low,
            "info" => Severity::Info,
            _ => Severity::Info,
        }
    }
}

// Conversion from API type to Core type
impl From<ComplianceReport> for CoreComplianceReport {
    fn from(report: ComplianceReport) -> Self {
        CoreComplianceReport {
            id: report.id,
            report_type: report.report_type.as_str().to_string(),
            period_start: report.period_start,
            period_end: report.period_end,
            generated_at: report.generated_at,
            generated_by: report.generated_by,
            status: report.status.as_str().to_string(),
            summary: CoreReportSummary {
                total_traces: report.summary.total_traces,
                total_cost: report.summary.total_cost,
                total_tokens: report.summary.total_tokens,
                total_users: report.summary.total_users,
                pii_detected: report.summary.pii_detected,
                security_issues: report.summary.security_issues,
                quality_score: report.summary.quality_score,
                compliance_score: report.summary.compliance_score,
            },
            findings: report
                .findings
                .into_iter()
                .map(|f| CoreComplianceFinding {
                    severity: f.severity.as_str().to_string(),
                    category: f.category,
                    description: f.description,
                    affected_traces: f.affected_traces,
                    recommendation: f.recommendation,
                })
                .collect(),
            recommendations: report.recommendations,
        }
    }
}

// Conversion from Core type to API type
impl From<CoreComplianceReport> for ComplianceReport {
    fn from(core: CoreComplianceReport) -> Self {
        ComplianceReport {
            id: core.id,
            report_type: ReportType::parse(&core.report_type),
            period_start: core.period_start,
            period_end: core.period_end,
            generated_at: core.generated_at,
            generated_by: core.generated_by,
            status: ReportStatus::parse(&core.status),
            summary: ReportSummary {
                total_traces: core.summary.total_traces,
                total_cost: core.summary.total_cost,
                total_tokens: core.summary.total_tokens,
                total_users: core.summary.total_users,
                pii_detected: core.summary.pii_detected,
                security_issues: core.summary.security_issues,
                quality_score: core.summary.quality_score,
                compliance_score: core.summary.compliance_score,
            },
            findings: core
                .findings
                .into_iter()
                .map(|f| ComplianceFinding {
                    severity: Severity::parse(&f.severity),
                    category: f.category,
                    description: f.description,
                    affected_traces: f.affected_traces,
                    recommendation: f.recommendation,
                })
                .collect(),
            recommendations: core.recommendations,
        }
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct GenerateReportRequest {
    pub report_type: ReportType,
    pub period_start: u64,
    pub period_end: u64,
    #[serde(default)]
    pub filters: ReportFilters,
}

#[derive(Debug, Deserialize, Default)]
pub struct ReportFilters {
    #[serde(default)]
    pub project_ids: Vec<u16>,
    #[serde(default)]
    pub agent_ids: Vec<u64>,
    #[serde(default)]
    pub include_pii_scan: bool,
    #[serde(default)]
    pub include_cost_analysis: bool,
}

#[derive(Debug, Deserialize)]
pub struct ListReportsQuery {
    pub report_type: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReportResponse {
    pub id: String,
    pub report_type: String,
    pub period_start: u64,
    pub period_end: u64,
    pub generated_at: u64,
    pub generated_by: String,
    pub status: String,
    pub summary: ReportSummary,
}

#[derive(Debug, Serialize)]
pub struct ReportDetailResponse {
    pub id: String,
    pub report_type: String,
    pub period_start: u64,
    pub period_end: u64,
    pub generated_at: u64,
    pub generated_by: String,
    pub status: String,
    pub summary: ReportSummary,
    pub findings: Vec<FindingResponse>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FindingResponse {
    pub severity: String,
    pub category: String,
    pub description: String,
    pub affected_count: usize,
    pub recommendation: String,
}

#[derive(Debug, Serialize)]
pub struct ReportListResponse {
    pub reports: Vec<ReportResponse>,
    pub total: usize,
}

// DataPrivacyMetrics and SecurityMetrics are imported from agentreplay_core::enterprise

#[derive(Debug, Serialize)]
pub struct DeleteResponse {
    pub success: bool,
    pub message: String,
}

// ============================================================================
// Helper Functions
// ============================================================================

fn generate_id() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros();

    let random = (rand::random::<u64>() as u128) << 64;
    timestamp ^ random
}

fn current_timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

fn parse_id(id_str: &str) -> Result<u128, String> {
    let id_str = id_str.trim_start_matches("0x");
    u128::from_str_radix(id_str, 16).map_err(|e| format!("Invalid ID: {}", e))
}

fn report_to_response(report: &ComplianceReport) -> ReportResponse {
    ReportResponse {
        id: format!("0x{:x}", report.id),
        report_type: report.report_type.as_str().to_string(),
        period_start: report.period_start,
        period_end: report.period_end,
        generated_at: report.generated_at,
        generated_by: report.generated_by.clone(),
        status: report.status.as_str().to_string(),
        summary: report.summary.clone(),
    }
}

fn report_to_detail_response(report: &ComplianceReport) -> ReportDetailResponse {
    ReportDetailResponse {
        id: format!("0x{:x}", report.id),
        report_type: report.report_type.as_str().to_string(),
        period_start: report.period_start,
        period_end: report.period_end,
        generated_at: report.generated_at,
        generated_by: report.generated_by.clone(),
        status: report.status.as_str().to_string(),
        summary: report.summary.clone(),
        findings: report
            .findings
            .iter()
            .map(|f| FindingResponse {
                severity: f.severity.as_str().to_string(),
                category: f.category.clone(),
                description: f.description.clone(),
                affected_count: f.affected_traces.len(),
                recommendation: f.recommendation.clone(),
            })
            .collect(),
        recommendations: report.recommendations.clone(),
    }
}

// ============================================================================
// API Handlers
// ============================================================================

/// POST /api/v1/compliance/reports
/// Generate a new compliance report
pub async fn generate_report(
    State(state): State<AppState>,
    Json(req): Json<GenerateReportRequest>,
) -> Result<(StatusCode, Json<ReportResponse>), (StatusCode, String)> {
    let report_id = generate_id();
    let timestamp = current_timestamp_us();

    // Initialize report with generating status
    let mut report = ComplianceReport {
        id: report_id,
        report_type: req.report_type.clone(),
        period_start: req.period_start,
        period_end: req.period_end,
        generated_at: timestamp,
        generated_by: "api-user".to_string(), // TODO: Get from auth context
        status: ReportStatus::Generating,
        summary: ReportSummary {
            total_traces: 0,
            total_cost: 0.0,
            total_tokens: 0,
            total_users: 0,
            pii_detected: 0,
            security_issues: 0,
            quality_score: 0.0,
            compliance_score: 0.0,
        },
        findings: Vec::new(),
        recommendations: Vec::new(),
    };

    // Generate report based on type
    let traces = state
        .db
        .list_traces_in_range(req.period_start, req.period_end)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    report.summary.total_traces = traces.len();

    // Calculate basic metrics
    let mut total_tokens = 0u64;
    let mut total_cost = 0.0;

    for trace in &traces {
        total_tokens += trace.token_count as u64;
        // Rough cost estimate: $0.01 per 1000 tokens
        total_cost += (trace.token_count as f64 / 1000.0) * 0.01;
    }

    report.summary.total_tokens = total_tokens;
    report.summary.total_cost = total_cost;

    // Perform compliance checks based on report type
    match req.report_type {
        ReportType::DataPrivacy => {
            // Check for PII in traces
            if req.filters.include_pii_scan {
                let pii_count = state
                    .db
                    .scan_for_pii(&traces)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

                report.summary.pii_detected = pii_count;

                if pii_count > 0 {
                    report.findings.push(ComplianceFinding {
                        severity: Severity::High,
                        category: "Data Privacy".to_string(),
                        description: format!("Found {} traces with potential PII", pii_count),
                        affected_traces: vec![],
                        recommendation: "Review and implement PII redaction or encryption"
                            .to_string(),
                    });
                }
            }
        }
        ReportType::Security => {
            // Security audit checks
            report
                .recommendations
                .push("Enable authentication for production deployments".to_string());
            report
                .recommendations
                .push("Implement rate limiting on all API endpoints".to_string());
        }
        ReportType::Usage => {
            // Usage analysis
            if total_cost > 1000.0 {
                report.findings.push(ComplianceFinding {
                    severity: Severity::Medium,
                    category: "Cost Management".to_string(),
                    description: format!("High cost detected: ${:.2}", total_cost),
                    affected_traces: vec![],
                    recommendation: "Consider implementing budget alerts and cost optimization"
                        .to_string(),
                });
            }
        }
        ReportType::Quality => {
            // Quality metrics analysis
            let eval_metrics = state
                .db
                .get_eval_metrics_for_period(req.period_start, req.period_end)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            let quality_score = if !eval_metrics.is_empty() {
                eval_metrics.iter().map(|m| m.metric_value).sum::<f64>() / eval_metrics.len() as f64
            } else {
                0.0
            };

            report.summary.quality_score = quality_score;
        }
        ReportType::Full => {
            // Comprehensive audit
            report
                .recommendations
                .push("Implement comprehensive monitoring and alerting".to_string());
            report
                .recommendations
                .push("Regular security audits and compliance reviews recommended".to_string());
        }
    }

    // Calculate compliance score
    let base_score = 100.0;
    let deductions = report
        .findings
        .iter()
        .map(|f| match f.severity {
            Severity::Critical => 20.0,
            Severity::High => 10.0,
            Severity::Medium => 5.0,
            Severity::Low => 2.0,
            Severity::Info => 0.0,
        })
        .sum::<f64>();

    report.summary.compliance_score = (base_score - deductions).max(0.0);
    report.status = ReportStatus::Completed;

    // Store report - convert API type to Core type
    let core_report: CoreComplianceReport = report.clone().into();
    state
        .db
        .store_compliance_report(core_report)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(report_to_response(&report))))
}

/// GET /api/v1/compliance/reports
/// List all compliance reports
pub async fn list_reports(
    State(state): State<AppState>,
    Query(params): Query<ListReportsQuery>,
) -> Result<Json<ReportListResponse>, (StatusCode, String)> {
    let core_reports = state
        .db
        .list_compliance_reports()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Convert from Core to API types
    let mut reports: Vec<ComplianceReport> = core_reports.into_iter().map(|r| r.into()).collect();

    // Filter by report type if provided
    if let Some(report_type_str) = params.report_type {
        reports.retain(|r| r.report_type.as_str() == report_type_str.to_lowercase());
    }

    // Filter by status if provided
    if let Some(status_str) = params.status {
        reports.retain(|r| r.status.as_str() == status_str.to_lowercase());
    }

    let total = reports.len();
    let report_responses: Vec<ReportResponse> = reports.iter().map(report_to_response).collect();

    Ok(Json(ReportListResponse {
        reports: report_responses,
        total,
    }))
}

/// GET /api/v1/compliance/reports/:id
/// Get a specific compliance report
pub async fn get_report(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ReportDetailResponse>, (StatusCode, String)> {
    let report_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let core_report = state
        .db
        .get_compliance_report(report_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Report not found".to_string()))?;

    let report: ComplianceReport = core_report.into();
    Ok(Json(report_to_detail_response(&report)))
}

/// GET /api/v1/compliance/privacy-metrics
/// Get data privacy metrics
pub async fn get_privacy_metrics(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<DataPrivacyMetrics>, (StatusCode, String)> {
    let period_start = params
        .get("start")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let period_end = params
        .get("end")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(current_timestamp_us());

    let metrics = state
        .db
        .get_privacy_metrics(period_start, period_end)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(metrics))
}

/// GET /api/v1/compliance/security-metrics
/// Get security metrics
pub async fn get_security_metrics(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<SecurityMetrics>, (StatusCode, String)> {
    let period_start = params
        .get("start")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let period_end = params
        .get("end")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(current_timestamp_us());

    let metrics = state
        .db
        .get_security_metrics(period_start, period_end)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(metrics))
}

/// DELETE /api/v1/compliance/reports/:id
/// Delete a compliance report
pub async fn delete_report(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, String)> {
    let report_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let deleted = state
        .db
        .delete_compliance_report(report_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(DeleteResponse {
            success: true,
            message: "Report deleted successfully".to_string(),
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Report not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_score_calculation() {
        let mut report = ComplianceReport {
            id: 1,
            report_type: ReportType::Full,
            period_start: 0,
            period_end: 1000,
            generated_at: 1000,
            generated_by: "test".to_string(),
            status: ReportStatus::Completed,
            summary: ReportSummary {
                total_traces: 100,
                total_cost: 10.0,
                total_tokens: 1000,
                total_users: 10,
                pii_detected: 0,
                security_issues: 0,
                quality_score: 0.95,
                compliance_score: 100.0,
            },
            findings: vec![],
            recommendations: vec![],
        };

        // Add critical finding
        report.findings.push(ComplianceFinding {
            severity: Severity::Critical,
            category: "Security".to_string(),
            description: "Critical issue".to_string(),
            affected_traces: vec![],
            recommendation: "Fix immediately".to_string(),
        });

        let base_score = 100.0;
        let deductions: f64 = report
            .findings
            .iter()
            .map(|f| match f.severity {
                Severity::Critical => 20.0,
                _ => 0.0,
            })
            .sum();

        assert_eq!(deductions, 20.0);
        assert_eq!((base_score - deductions).max(0.0), 80.0);
    }
}
