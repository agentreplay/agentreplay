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

pub mod admin;
pub mod agents;
pub mod analytics;
pub mod backup;
pub mod budget_alerts;
pub mod chat;
pub mod compliance;
pub mod converters;
pub mod cost;
pub mod debug;
pub mod detailed_trace;
pub mod eval_datasets;
pub mod eval_trace;
pub mod eval_pipeline;
pub mod eval_runs;
pub mod evals;
pub mod evaluate;
pub mod experiments;
pub mod feedback;
pub mod flywheel;
pub mod git_versioning;
pub mod graph;
pub mod health;
pub mod ingest;
pub mod insights;
pub mod memory;
pub mod metrics;
pub mod payload_extractors;
pub mod projects;
pub mod prompts;
pub mod query;
pub mod realtime;
pub mod retention;
pub mod search;
pub mod sessions;
pub mod storage_debug;
pub mod views;

pub use agents::*;
pub use analytics::{
    ComparativeAnalysisQuery,
    ComparativeAnalysisResponse,
    CorrelationQuery,
    CorrelationResponse,
    GroupMetrics,
    LatencyBreakdown,
    LatencyBreakdownQuery,
    LatencyStats,
    TimeSeriesQuery,
    TimeSeriesResponse,
    TimeSeriesSummary,
    TrendAnalysisQuery,
    TrendAnalysisResponse,
    // Note: CostBreakdown, CostBreakdownQuery, ModelCost, TokenUsageSummary also exist here
    // but we use the ones from cost module
};
pub use backup::*;
pub use budget_alerts::{
    create_alert,
    delete_alert,
    get_alert,
    get_alert_events,
    get_budget_status,
    list_alerts,
    update_alert,
    AlertAction,
    AlertEvent,
    AlertEventResponse,
    AlertEventsResponse,
    AlertFilters,
    AlertListResponse,
    AlertResponse,
    AlertStatus,
    BudgetAlert,
    BudgetStatusResponse,
    CreateAlertRequest,
    ListAlertsQuery,
    Period,
    ThresholdType,
    UpdateAlertRequest,
    // Note: DeleteResponse also exists here but we use the one from prompts module
};
pub use chat::{chat_completion, list_models, stream_completion};
pub use compliance::{
    delete_report,
    generate_report,
    get_privacy_metrics,
    get_report,
    get_security_metrics,
    list_reports,
    ComplianceFinding,
    ComplianceReport,
    FindingResponse,
    GenerateReportRequest,
    ListReportsQuery,
    ReportDetailResponse,
    ReportFilters,
    ReportListResponse,
    ReportResponse,
    ReportStatus,
    ReportSummary,
    ReportType,
    Severity,
    // Note: DeleteResponse also exists here but we use the one from prompts module
};
pub use converters::{
    convert_otel_span_to_edge, ConversionError, IngestResponse, OtelSpan, OtelSpanBatch,
};
pub use cost::*;
pub use detailed_trace::*;
pub use eval_datasets::{
    create_dataset,
    delete_dataset,
    get_dataset,
    list_datasets,
    CreateDatasetRequest,
    DatasetDetailResponse,
    DatasetListResponse,
    DatasetResponse,
    TestCaseInput,
    TestCaseOutput,
    // Note: DeleteResponse also exists here but we use the one from prompts module
};
pub use eval_trace::build_eval_trace_v1;
pub use eval_runs::{
    add_run_result,
    create_run,
    delete_run,
    get_run,
    list_runs,
    update_run_status,
    AddRunResultRequest,
    CreateRunRequest,
    ListRunsQuery,
    RunDetailResponse,
    RunListResponse,
    RunResponse,
    RunResultOutput,
    UpdateRunStatusRequest,
    // Note: DeleteResponse also exists here but we use the one from prompts module
};
pub use evals::*;
pub use evaluate::{get_evaluation_history, run_geval, run_ragas};
pub use experiments::{
    create_experiment,
    delete_experiment,
    get_experiment,
    get_experiment_stats,
    list_experiments,
    record_result,
    start_experiment,
    stop_experiment,
    update_experiment,
    CreateExperimentRequest,
    Experiment,
    ExperimentListResponse,
    ExperimentResponse,
    ExperimentStatsResponse,
    ExperimentStatus,
    ListExperimentsQuery,
    MetricStats,
    RecordResultRequest,
    StartExperimentRequest,
    UpdateExperimentRequest,
    Variant,
    VariantInput,
    VariantStats,
    // Note: DeleteResponse also exists here but we use the one from prompts module
};
pub use feedback::{add_trace_to_dataset, submit_trace_feedback};
pub use git_versioning::{git_versioning_router, GitVersioningState};
pub use graph::*;
pub use health::health_check_detailed;
pub use ingest::*;
pub use metrics::*;
pub use projects::*;
pub use prompts::*;
pub use query::AppState;
pub use query::*;
pub use realtime::{sse_traces, ws_traces};
pub use retention::*;
pub use search::*;
pub use sessions::*;
pub use storage_debug::*;
pub use views::*;
