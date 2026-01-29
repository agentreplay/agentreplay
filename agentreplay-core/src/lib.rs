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

//! Agentreplay Core
//!
//! Fundamental data structures and types for the AgentFlow Format.

pub mod config;
pub mod context;
pub mod edge;
pub mod enterprise;
pub mod error;
pub mod eval;
pub mod actionable_feedback;
pub mod eval_dataset;
pub mod eval_result;
pub mod eval_trace;
pub mod event;
pub mod genai;
pub mod insights;
pub mod key;
pub mod language;
pub mod memory_agent;
pub mod model_comparison;
pub mod model_pricing;
pub mod memory;
pub mod observation;
pub mod observation_types;
pub mod privacy;
pub mod project;
pub mod prompt;
pub mod quality;
pub mod resilience;
pub mod saved_view;
pub mod session;
pub mod session_summary;
pub mod tool;
pub mod tool_definition;

#[cfg(test)]
mod edge_validation_tests;

pub use config::{TimestampConfig, DEFAULT_MAX_TIMESTAMP, DEFAULT_MIN_TIMESTAMP};
pub use edge::{
    checked_timestamp_add, checked_timestamp_sub, validate_timestamp, AgentFlowEdge, Environment,
    HlcTimestamp, HybridLogicalClock, SpanType, AFF_SCHEMA_VERSION, FLAG_DELETED, HLC_LOGICAL_BITS,
    HLC_LOGICAL_MASK, HLC_MAX_DRIFT_MS, HLC_MAX_LOGICAL, SENSITIVITY_INTERNAL, SENSITIVITY_NONE,
    SENSITIVITY_NO_EMBED, SENSITIVITY_PII, SENSITIVITY_SECRET,
};
pub use actionable_feedback::*;
pub use enterprise::*;
pub use error::{AgentreplayError, Result};
pub use eval::{evaluators, metrics, EvalMetric};
pub use eval_dataset::{
    AssertionResult, EvalDataset, EvalRun, GraderPolicyV2, GraderResult, GraderSpecV2,
    GraderThresholdV2, JudgeVote, OverallResult, PassRateCI, RunResult, RunStatus,
    SideEffectExpectationV2, StateExpectationV2, SuccessCriteriaV2, TaskAggregate,
    TaskDefinitionV2, TestCase, TrialResult,
};
pub use eval_result::{EvalResultV1, MetricValueV1};
pub use eval_trace::{
    ContentPartV1, EnvironmentStateV2, EvalTraceV1, MessageV1, OutcomeV1, OutcomeV2,
    SideEffectV2, SpanSummaryV1, TraceRefV1, TranscriptEventV1, TraceStatsV1,
    EVAL_TRACE_SCHEMA_VERSION_V1,
};
pub use event::SpanEvent;
pub use genai::{
    ContentData, GenAISpanData, Message, ModelParameters as GenAIModelParameters, TokenUsage,
    ToolCall,
};
pub use insights::{Insight, InsightConfig, InsightData, InsightEngine, InsightType, Severity};
pub use key::{CausalKey, TemporalKey};
pub use model_comparison::{
    ComparisonFeedback, ComparisonStatus, ComparisonStreamChunk, ModelComparisonError,
    ModelComparisonRequest, ModelComparisonResponse, ModelComparisonResult, ModelSelection,
    ResponseRating, MAX_COMPARISON_MODELS,
};
pub use model_pricing::{
    CustomPricingOverride, ModelPricing, ModelPricingRegistry, PricingError, PricingPriority,
    PricingRegistryMetadata, SyncResult, LITELLM_PRICING_URL,
};
pub use prompt::{Completion, ModelParameters, Prompt, PromptCompletion, PromptRole};
pub use saved_view::{SavedView, SavedViewRegistry};
pub use tool::{AgentMetadata, ToolMetadata};
pub use tool_definition::{
    ExecutionConfig, ExecutionContext, HttpMethod, MCPTransport, MockResponse, RateLimit,
    RateLimitConfig, RetryPolicy, ToolDefinitionMetadata, ToolExecutionError, ToolExecutionRecord,
    ToolExecutionResult, ToolKind, ToolRegistration, ToolVersion, UnifiedToolDefinition,
};
