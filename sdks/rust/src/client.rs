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

//! Agentreplay Client
//!
//! High-performance async client for the Agentreplay observability platform.

use crate::types::*;
use reqwest::Client as HttpClient;
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use thiserror::Error;

/// Agentreplay SDK errors.
#[derive(Error, Debug)]
pub enum AgentreplayError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("API error ({status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("Invalid feedback value: must be -1, 0, or 1")]
    InvalidFeedback,
}

/// Result type for Agentreplay operations.
pub type Result<T> = std::result::Result<T, AgentreplayError>;

/// Agentreplay client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Base URL of Agentreplay server
    pub url: String,
    /// Tenant identifier
    pub tenant_id: i64,
    /// Project identifier (default: 0)
    pub project_id: i64,
    /// Default agent identifier (default: 1)
    pub agent_id: i64,
    /// Request timeout (default: 30 seconds)
    pub timeout: Duration,
}

impl ClientConfig {
    /// Create a new client configuration.
    pub fn new(url: impl Into<String>, tenant_id: i64) -> Self {
        Self {
            url: url.into(),
            tenant_id,
            project_id: 0,
            agent_id: 1,
            timeout: Duration::from_secs(30),
        }
    }

    /// Set the project ID.
    pub fn with_project_id(mut self, project_id: i64) -> Self {
        self.project_id = project_id;
        self
    }

    /// Set the default agent ID.
    pub fn with_agent_id(mut self, agent_id: i64) -> Self {
        self.agent_id = agent_id;
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Agentreplay client for Rust applications.
///
/// # Example
///
/// ```no_run
/// use agentreplay::{AgentreplayClient, ClientConfig, SpanType, CreateTraceOptions};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = ClientConfig::new("http://localhost:8080", 1)
///         .with_project_id(0)
///         .with_agent_id(1);
///
///     let client = AgentreplayClient::new(config);
///
///     let trace = client.create_trace(CreateTraceOptions {
///         agent_id: 1,
///         session_id: Some(123),
///         span_type: SpanType::Root,
///         ..Default::default()
///     }).await?;
///
///     println!("Created trace: {}", trace.edge_id);
///     Ok(())
/// }
/// ```
pub struct AgentreplayClient {
    config: ClientConfig,
    http_client: HttpClient,
    session_counter: AtomicI64,
}

impl AgentreplayClient {
    /// Create a new Agentreplay client.
    pub fn new(config: ClientConfig) -> Self {
        let http_client = HttpClient::builder()
            .timeout(config.timeout)
            .pool_max_idle_per_host(50)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            http_client,
            session_counter: AtomicI64::new(0),
        }
    }

    /// Generate a unique edge ID.
    fn generate_edge_id() -> String {
        use rand::Rng;
        let timestamp = chrono::Utc::now().timestamp_millis();
        let random_bits: u16 = rand::thread_rng().gen();
        let edge_id = ((timestamp as u64) << 16) | (random_bits as u64);
        format!("{:x}", edge_id)
    }

    /// Get the current timestamp in microseconds.
    fn now_microseconds() -> i64 {
        chrono::Utc::now().timestamp_micros()
    }

    /// Generate the next session ID.
    fn next_session_id(&self) -> i64 {
        self.session_counter.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Make an HTTP request to the Agentreplay server.
    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        params: Option<&[(&str, String)]>,
    ) -> Result<T> {
        let url = format!("{}{}", self.config.url.trim_end_matches('/'), path);

        let mut request = self.http_client.request(method, &url);
        request = request
            .header("Content-Type", "application/json")
            .header("X-Tenant-ID", self.config.tenant_id.to_string());

        if let Some(params) = params {
            request = request.query(params);
        }

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await?;
        let status = response.status();

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(AgentreplayError::ApiError {
                status: status.as_u16(),
                message,
            });
        }

        let result = response.json().await?;
        Ok(result)
    }

    /// Create a new trace span.
    pub async fn create_trace(&self, opts: CreateTraceOptions) -> Result<TraceResult> {
        let edge_id = Self::generate_edge_id();
        let session_id = opts.session_id.unwrap_or_else(|| self.next_session_id());
        let start_time_us = Self::now_microseconds();

        let mut attributes: HashMap<String, String> = HashMap::new();
        attributes.insert("tenant_id".into(), self.config.tenant_id.to_string());
        attributes.insert("project_id".into(), self.config.project_id.to_string());
        attributes.insert("agent_id".into(), opts.agent_id.to_string());
        attributes.insert("session_id".into(), session_id.to_string());
        attributes.insert("span_type".into(), (opts.span_type as u8).to_string());
        attributes.insert("token_count".into(), "0".into());
        attributes.insert("duration_us".into(), "0".into());

        let mut name = format!("span_{}", opts.agent_id);

        // Add metadata
        if let Some(metadata) = &opts.metadata {
            for (k, v) in metadata {
                if k == "name" {
                    if let Some(n) = v.as_str() {
                        name = n.to_string();
                    }
                    continue;
                }
                let value_str = match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                attributes.insert(k.clone(), value_str);
            }
        }

        let span = SpanInput {
            span_id: edge_id.clone(),
            trace_id: session_id.to_string(),
            parent_span_id: opts.parent_id.clone(),
            name,
            start_time: start_time_us,
            end_time: Some(start_time_us),
            attributes,
        };

        let _: IngestResponse = self
            .request(
                reqwest::Method::POST,
                "/api/v1/traces",
                Some(json!({ "spans": [span] })),
                None,
            )
            .await?;

        Ok(TraceResult {
            edge_id,
            tenant_id: self.config.tenant_id,
            agent_id: opts.agent_id,
            session_id,
            span_type: opts.span_type.to_string(),
        })
    }

    /// Create a GenAI trace with OpenTelemetry semantic conventions.
    pub async fn create_genai_trace(
        &self,
        opts: CreateGenAITraceOptions,
    ) -> Result<GenAITraceResult> {
        let edge_id = Self::generate_edge_id();
        let session_id = opts.session_id.unwrap_or_else(|| self.next_session_id());
        let start_time_us = Self::now_microseconds();
        let operation_name = opts.operation_name.as_deref().unwrap_or("chat");

        let mut attributes: HashMap<String, String> = HashMap::new();
        attributes.insert("tenant_id".into(), self.config.tenant_id.to_string());
        attributes.insert("project_id".into(), self.config.project_id.to_string());
        attributes.insert("agent_id".into(), opts.agent_id.to_string());
        attributes.insert("session_id".into(), session_id.to_string());
        attributes.insert("span_type".into(), "0".into());
        attributes.insert("gen_ai.operation.name".into(), operation_name.into());

        // Auto-detect system from model name
        let system = opts.system.clone().or_else(|| {
            opts.model.as_ref().map(|model| {
                let model_lower = model.to_lowercase();
                if model_lower.contains("gpt") || model_lower.contains("openai") {
                    "openai".into()
                } else if model_lower.contains("claude") || model_lower.contains("anthropic") {
                    "anthropic".into()
                } else if model_lower.contains("llama") || model_lower.contains("meta") {
                    "meta".into()
                } else if model_lower.contains("gemini") || model_lower.contains("palm") {
                    "google".into()
                } else {
                    "unknown".into()
                }
            })
        });

        if let Some(system) = system {
            attributes.insert("gen_ai.system".into(), system);
        }

        if let Some(model) = &opts.model {
            attributes.insert("gen_ai.request.model".into(), model.clone());
            attributes.insert("gen_ai.response.model".into(), model.clone());
        }

        // Model parameters
        if let Some(params) = &opts.model_parameters {
            for (k, v) in params {
                let value_str = match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                attributes.insert(format!("gen_ai.request.{}", k), value_str);
            }
        }

        // Token usage
        if let Some(input) = opts.input_usage {
            attributes.insert("gen_ai.usage.prompt_tokens".into(), input.to_string());
            attributes.insert("gen_ai.usage.input_tokens".into(), input.to_string());
        }
        if let Some(output) = opts.output_usage {
            attributes.insert("gen_ai.usage.completion_tokens".into(), output.to_string());
            attributes.insert("gen_ai.usage.output_tokens".into(), output.to_string());
        }
        if let Some(total) = opts.total_usage {
            attributes.insert("gen_ai.usage.total_tokens".into(), total.to_string());
            attributes.insert("token_count".into(), total.to_string());
        }

        if let Some(reason) = &opts.finish_reason {
            attributes.insert(
                "gen_ai.response.finish_reasons".into(),
                serde_json::to_string(&[reason])?,
            );
        }

        // Input messages
        if !opts.input_messages.is_empty() {
            attributes.insert(
                "gen_ai.prompt.messages".into(),
                serde_json::to_string(&opts.input_messages)?,
            );
        }

        // Output
        if let Some(output) = &opts.output {
            attributes.insert(
                "gen_ai.completion.message".into(),
                serde_json::to_string(output)?,
            );
        }

        // Additional metadata
        if let Some(metadata) = &opts.metadata {
            for (k, v) in metadata {
                if !attributes.contains_key(k) {
                    let value_str = match v {
                        serde_json::Value::String(s) => s.clone(),
                        _ => v.to_string(),
                    };
                    attributes.insert(format!("metadata.{}", k), value_str);
                }
            }
        }

        let model_str = opts.model.as_deref().unwrap_or("unknown");
        let span = SpanInput {
            span_id: edge_id.clone(),
            trace_id: session_id.to_string(),
            parent_span_id: opts.parent_id.clone(),
            name: format!("{}-{}", operation_name, model_str),
            start_time: start_time_us,
            end_time: Some(start_time_us),
            attributes,
        };

        let _: IngestResponse = self
            .request(
                reqwest::Method::POST,
                "/api/v1/traces",
                Some(json!({ "spans": [span] })),
                None,
            )
            .await?;

        Ok(GenAITraceResult {
            edge_id,
            tenant_id: self.config.tenant_id,
            agent_id: opts.agent_id,
            session_id,
            model: opts.model,
        })
    }

    /// Create a tool call trace.
    pub async fn create_tool_trace(&self, opts: CreateToolTraceOptions) -> Result<ToolTraceResult> {
        let edge_id = Self::generate_edge_id();
        let session_id = opts.session_id.unwrap_or_else(|| self.next_session_id());
        let start_time_us = Self::now_microseconds();

        let mut attributes: HashMap<String, String> = HashMap::new();
        attributes.insert("tenant_id".into(), self.config.tenant_id.to_string());
        attributes.insert("project_id".into(), self.config.project_id.to_string());
        attributes.insert("agent_id".into(), opts.agent_id.to_string());
        attributes.insert("session_id".into(), session_id.to_string());
        attributes.insert("span_type".into(), "3".into()); // TOOL_CALL
        attributes.insert("gen_ai.tool.name".into(), opts.tool_name.clone());

        if let Some(desc) = &opts.tool_description {
            attributes.insert("gen_ai.tool.description".into(), desc.clone());
        }
        if let Some(input) = &opts.tool_input {
            attributes.insert(
                "gen_ai.tool.call.input".into(),
                serde_json::to_string(input)?,
            );
        }
        if let Some(output) = &opts.tool_output {
            attributes.insert(
                "gen_ai.tool.call.output".into(),
                serde_json::to_string(output)?,
            );
        }

        // Additional metadata
        if let Some(metadata) = &opts.metadata {
            for (k, v) in metadata {
                if !attributes.contains_key(k) {
                    let value_str = match v {
                        serde_json::Value::String(s) => s.clone(),
                        _ => v.to_string(),
                    };
                    attributes.insert(format!("metadata.{}", k), value_str);
                }
            }
        }

        let span = SpanInput {
            span_id: edge_id.clone(),
            trace_id: session_id.to_string(),
            parent_span_id: opts.parent_id.clone(),
            name: format!("tool-{}", opts.tool_name),
            start_time: start_time_us,
            end_time: Some(start_time_us),
            attributes,
        };

        let _: IngestResponse = self
            .request(
                reqwest::Method::POST,
                "/api/v1/traces",
                Some(json!({ "spans": [span] })),
                None,
            )
            .await?;

        Ok(ToolTraceResult {
            edge_id,
            tenant_id: self.config.tenant_id,
            agent_id: opts.agent_id,
            session_id,
            tool_name: opts.tool_name,
        })
    }

    /// Update a trace with completion information.
    pub async fn update_trace(&self, opts: UpdateTraceOptions) -> Result<()> {
        let end_time_us = Self::now_microseconds();
        let duration_us = opts
            .duration_us
            .or_else(|| opts.duration_ms.map(|ms| ms * 1000))
            .unwrap_or(1000);

        let start_time_us = end_time_us - duration_us;
        let token_count = opts.token_count.unwrap_or(0);

        let mut attributes: HashMap<String, String> = HashMap::new();
        attributes.insert("tenant_id".into(), self.config.tenant_id.to_string());
        attributes.insert("project_id".into(), self.config.project_id.to_string());
        attributes.insert("agent_id".into(), self.config.agent_id.to_string());
        attributes.insert("session_id".into(), opts.session_id.to_string());
        attributes.insert("span_type".into(), "6".into()); // RESPONSE
        attributes.insert("token_count".into(), token_count.to_string());
        attributes.insert("duration_us".into(), duration_us.to_string());

        if let Some(payload) = &opts.payload {
            for (k, v) in payload {
                let value_str = match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                attributes.insert(format!("payload.{}", k), value_str);
            }
        }

        let span = SpanInput {
            span_id: format!("{}_complete", opts.edge_id),
            trace_id: opts.session_id.to_string(),
            parent_span_id: Some(opts.edge_id),
            name: "RESPONSE".into(),
            start_time: start_time_us,
            end_time: Some(end_time_us),
            attributes,
        };

        let _: IngestResponse = self
            .request(
                reqwest::Method::POST,
                "/api/v1/traces",
                Some(json!({ "spans": [span] })),
                None,
            )
            .await?;

        Ok(())
    }

    /// Ingest multiple spans in a batch.
    pub async fn ingest_batch(&self, spans: Vec<SpanInput>) -> Result<IngestResponse> {
        self.request(
            reqwest::Method::POST,
            "/api/v1/traces",
            Some(json!({ "spans": spans })),
            None,
        )
        .await
    }

    /// Query traces with optional filters.
    pub async fn query_traces(&self, filter: Option<&QueryFilter>) -> Result<QueryResponse> {
        let mut params: Vec<(&str, String)> = Vec::new();

        if let Some(f) = filter {
            if let Some(project_id) = f.project_id {
                params.push(("project_id", project_id.to_string()));
            }
            if let Some(agent_id) = f.agent_id {
                params.push(("agent_id", agent_id.to_string()));
            }
            if let Some(session_id) = f.session_id {
                params.push(("session_id", session_id.to_string()));
            }
            if let Some(env) = &f.environment {
                params.push(("environment", env.as_str().to_string()));
            }
            if f.exclude_pii {
                params.push(("exclude_pii", "true".into()));
            }
            if f.exclude_secrets {
                params.push(("exclude_secrets", "true".into()));
            }
            if let Some(limit) = f.limit {
                params.push(("limit", limit.to_string()));
            }
            if let Some(offset) = f.offset {
                params.push(("offset", offset.to_string()));
            }
        }

        self.request(
            reqwest::Method::GET,
            "/api/v1/traces",
            None,
            if params.is_empty() {
                None
            } else {
                Some(&params)
            },
        )
        .await
    }

    /// Query traces within a time range.
    pub async fn query_temporal_range(
        &self,
        start_us: i64,
        end_us: i64,
        filter: Option<&QueryFilter>,
    ) -> Result<QueryResponse> {
        let mut params: Vec<(&str, String)> = vec![
            ("start_ts", start_us.to_string()),
            ("end_ts", end_us.to_string()),
        ];

        if let Some(f) = filter {
            if let Some(session_id) = f.session_id {
                params.push(("session_id", session_id.to_string()));
            }
            if let Some(agent_id) = f.agent_id {
                params.push(("agent_id", agent_id.to_string()));
            }
            if let Some(env) = &f.environment {
                params.push(("environment", env.as_str().to_string()));
            }
            if f.exclude_pii {
                params.push(("exclude_pii", "true".into()));
            }
            if let Some(limit) = f.limit {
                params.push(("limit", limit.to_string()));
            }
            if let Some(offset) = f.offset {
                params.push(("offset", offset.to_string()));
            }
        }

        self.request(reqwest::Method::GET, "/api/v1/traces", None, Some(&params))
            .await
    }

    /// Get a specific trace by ID.
    pub async fn get_trace(&self, trace_id: &str) -> Result<TraceView> {
        self.request(
            reqwest::Method::GET,
            &format!("/api/v1/traces/{}", trace_id),
            None,
            None,
        )
        .await
    }

    /// Get the hierarchical trace tree.
    pub async fn get_trace_tree(&self, trace_id: &str) -> Result<TraceTreeResponse> {
        self.request(
            reqwest::Method::GET,
            &format!("/api/v1/traces/{}/tree", trace_id),
            None,
            None,
        )
        .await
    }

    /// Get all traces in a session.
    pub async fn filter_by_session(&self, session_id: i64) -> Result<Vec<TraceView>> {
        let filter = QueryFilter {
            session_id: Some(session_id),
            ..Default::default()
        };
        let response = self.query_traces(Some(&filter)).await?;
        Ok(response.traces)
    }

    /// Submit user feedback for a trace.
    pub async fn submit_feedback(&self, trace_id: &str, feedback: i8) -> Result<FeedbackResponse> {
        if !(-1..=1).contains(&feedback) {
            return Err(AgentreplayError::InvalidFeedback);
        }

        self.request(
            reqwest::Method::POST,
            &format!("/api/v1/traces/{}/feedback", trace_id),
            Some(json!({ "feedback": feedback })),
            None,
        )
        .await
    }

    /// Add a trace to an evaluation dataset.
    pub async fn add_to_dataset(
        &self,
        trace_id: &str,
        dataset_name: &str,
        input_data: Option<&HashMap<String, serde_json::Value>>,
        output_data: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<DatasetResponse> {
        let mut payload = json!({ "trace_id": trace_id });
        if let Some(input) = input_data {
            payload["input"] = json!(input);
        }
        if let Some(output) = output_data {
            payload["output"] = json!(output);
        }

        self.request(
            reqwest::Method::POST,
            &format!("/api/v1/datasets/{}/add", dataset_name),
            Some(payload),
            None,
        )
        .await
    }

    /// Check server health.
    pub async fn health(&self) -> Result<HealthResponse> {
        self.request(reqwest::Method::GET, "/api/v1/health", None, None)
            .await
    }
}
