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

// flowtrace-tauri/src/otlp_server.rs
//! OTLP gRPC and HTTP server implementation
//!
//! Implements OpenTelemetry Protocol (OTLP) ingestion endpoints:
//! - gRPC on port 4317
//! - HTTP on port 4318 (protobuf and JSON)
//!
//! This allows Flowtrace to accept traces from any OTEL-compatible client
//! without requiring a custom SDK.

use crate::server::ServerState;
use crate::AppState;
use anyhow::Result;
use axum::{
    extract::State as AxumState,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use bytes::Bytes;
use flowtrace_core::{AgentFlowEdge, Environment, ModelPricingRegistry, SpanType};
use flowtrace_storage::VersionStore;
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;
use opentelemetry_proto::tonic::collector::trace::v1::{
    trace_service_server::{TraceService, TraceServiceServer},
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use opentelemetry_proto::tonic::trace::v1::{span::SpanKind, status::StatusCode as OtelStatusCode};
use prost::Message;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status as TonicStatus};
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, warn};

/// OTLP gRPC service implementation
#[derive(Clone)]
pub struct OtlpTraceService {
    pub tauri_state: AppState,
}

#[tonic::async_trait]
impl TraceService for OtlpTraceService {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, TonicStatus> {
        let req = request.into_inner();

        debug!(
            "OTLP gRPC: Received {} resource spans",
            req.resource_spans.len()
        );

        let mut total_spans = 0;
        let mut accepted = 0;
        let mut rejected = 0;

        for resource_span in &req.resource_spans {
            // Extract resource attributes
            let resource_attrs = extract_resource_attributes(resource_span);

            for scope_span in &resource_span.scope_spans {
                total_spans += scope_span.spans.len();

                for span in &scope_span.spans {
                    match convert_otel_span_to_edge(span, &resource_attrs) {
                        Ok(edge) => {
                            // Queue edge for ingestion
                            if let Err(e) = self.tauri_state.ingestion_queue.send(edge) {
                                error!("Failed to queue edge: {}", e);
                                rejected += 1;
                            } else {
                                accepted += 1;

                                // Store span attributes as payload
                                let attributes = extract_span_attributes(span, &resource_attrs);
                                if !attributes.is_empty() {
                                    let db = Arc::clone(&self.tauri_state.db);
                                    let edge_id = edge.edge_id;
                                    tokio::spawn(async move {
                                        if let Ok(json_bytes) = serde_json::to_vec(&attributes) {
                                            if let Err(e) = db.put_payload(edge_id, &json_bytes) {
                                                warn!(
                                                    "Failed to store payload for {:#x}: {}",
                                                    edge_id, e
                                                );
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to convert OTEL span: {}", e);
                            rejected += 1;
                        }
                    }
                }
            }
        }

        debug!(
            "OTLP gRPC: Processed {} spans ({} accepted, {} rejected)",
            total_spans, accepted, rejected
        );

        Ok(Response::new(ExportTraceServiceResponse {
            partial_success: None,
        }))
    }
}

/// Start OTLP gRPC server on port 4317
pub async fn start_otlp_grpc_server(tauri_state: AppState) -> Result<()> {
    let addr = "127.0.0.1:4317".parse()?;
    let shutdown_token = tauri_state.shutdown_token.clone();
    let service = OtlpTraceService { tauri_state };

    info!("Starting OTLP gRPC server on {}", addr);

    Server::builder()
        .add_service(TraceServiceServer::new(service))
        .serve_with_shutdown(addr, async move {
            shutdown_token.cancelled().await;
            info!("OTLP gRPC server received shutdown signal");
        })
        .await?;

    info!("OTLP gRPC server shutdown complete");
    Ok(())
}

/// Start OTLP HTTP server on port 4318
pub async fn start_otlp_http_server(tauri_state: AppState) -> Result<()> {
    // Get data directory for pricing cache
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("flowtrace");
    
    let shutdown_token = tauri_state.shutdown_token.clone();
    
    // Create rate limiter for OTLP ingestion (10000 spans/min with burst of 5000)
    let rate_limiter = Arc::new(RateLimiter::direct(Quota::per_minute(
        NonZeroU32::new(10000).unwrap()
    ).allow_burst(NonZeroU32::new(5000).unwrap())));
    
    // Initialize version store for response versioning
    let version_store_path = data_dir.join("version-store");
    let version_store = Arc::new(
        VersionStore::new("flowtrace")
    );
    
    let server_state = ServerState {
        tauri_state,
        start_time: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        pricing_registry: Arc::new(ModelPricingRegistry::new(data_dir)),
        rate_limiter,
        version_store,
    };

    let app = Router::new()
        .route("/v1/traces", post(handle_otlp_http))
        .with_state(server_state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    let addr = "127.0.0.1:4318";
    info!("Starting OTLP HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_token.cancelled().await;
            info!("OTLP HTTP server received shutdown signal");
        })
        .await?;

    info!("OTLP HTTP server shutdown complete");
    Ok(())
}

/// Handle OTLP HTTP endpoint (supports both protobuf and JSON)
async fn handle_otlp_http(
    AxumState(state): AxumState<ServerState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/x-protobuf");

    debug!(
        "OTLP HTTP: Received request with content-type: {}",
        content_type
    );

    // Parse based on content type
    let request = if content_type.contains("json") {
        // Parse OTLP JSON format
        match parse_otlp_json(&body) {
            Ok(req) => req,
            Err(e) => {
                return Err((StatusCode::BAD_REQUEST, format!("Invalid OTLP JSON: {}", e)));
            }
        }
    } else {
        // Parse protobuf
        match ExportTraceServiceRequest::decode(body) {
            Ok(req) => req,
            Err(e) => {
                return Err((StatusCode::BAD_REQUEST, format!("Invalid protobuf: {}", e)));
            }
        }
    };

    // Process spans (same logic as gRPC)
    let mut total_spans = 0;
    let mut accepted = 0;
    let mut rejected = 0;

    // Extract project_id from headers if present
    let header_project_id = headers
        .get("x-flowtrace-project-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    for resource_span in &request.resource_spans {
        let mut resource_attrs = extract_resource_attributes(resource_span);
        
        // Inject project ID from header if available and not already in resource attributes
        if let Some(pid) = &header_project_id {
            if !resource_attrs.contains_key("project_id") && !resource_attrs.contains_key("flowtrace.project_id") {
                resource_attrs.insert("flowtrace.project_id".to_string(), pid.clone());
            }
        }

        for scope_span in &resource_span.scope_spans {
            total_spans += scope_span.spans.len();

            for span in &scope_span.spans {
                match convert_otel_span_to_edge(span, &resource_attrs) {
                    Ok(edge) => {
                        if let Err(e) = state.tauri_state.ingestion_queue.send(edge) {
                            error!("Failed to queue edge: {}", e);
                            rejected += 1;
                        } else {
                            accepted += 1;

                            // Store attributes as payload
                            let attributes = extract_span_attributes(span, &resource_attrs);
                            if !attributes.is_empty() {
                                let db = state.tauri_state.db.clone();
                                let edge_id = edge.edge_id;
                                tokio::spawn(async move {
                                    if let Ok(json_bytes) = serde_json::to_vec(&attributes) {
                                        if let Err(e) = db.put_payload(edge_id, &json_bytes) {
                                            warn!(
                                                "Failed to store payload for {:#x}: {}",
                                                edge_id, e
                                            );
                                        }
                                    }
                                });
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to convert OTEL span: {}", e);
                        rejected += 1;
                    }
                }
            }
        }
    }

    debug!(
        "OTLP HTTP: Processed {} spans ({} accepted, {} rejected)",
        total_spans, accepted, rejected
    );

    Ok((StatusCode::OK, Json(serde_json::json!({}))))
}

/// Extract resource attributes and map to tenant/project
fn extract_resource_attributes(
    resource_span: &opentelemetry_proto::tonic::trace::v1::ResourceSpans,
) -> HashMap<String, String> {
    let mut attrs = HashMap::new();

    if let Some(resource) = &resource_span.resource {
        for attr in &resource.attributes {
            if let Some(value) = &attr.value {
                let val_str = format_attribute_value(value);
                attrs.insert(attr.key.clone(), val_str);
            }
        }
    }

    attrs
}

/// Extract span attributes including GenAI semantic conventions
fn extract_span_attributes(
    span: &opentelemetry_proto::tonic::trace::v1::Span,
    resource_attrs: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut attrs = HashMap::new();

    // Add resource attributes
    for (k, v) in resource_attrs {
        attrs.insert(k.clone(), v.clone());
    }

    // Add span attributes
    for attr in &span.attributes {
        if let Some(value) = &attr.value {
            let val_str = format_attribute_value(value);
            attrs.insert(attr.key.clone(), val_str);
        }
    }

    // Add span name
    attrs.insert("span.name".to_string(), span.name.clone());

    // Add span kind
    let kind_str = match SpanKind::try_from(span.kind).ok() {
        Some(SpanKind::Internal) => "internal",
        Some(SpanKind::Server) => "server",
        Some(SpanKind::Client) => "client",
        Some(SpanKind::Producer) => "producer",
        Some(SpanKind::Consumer) => "consumer",
        _ => "unspecified",
    };
    attrs.insert("span.kind".to_string(), kind_str.to_string());

    // Add status
    if let Some(status) = &span.status {
        let status_str = match OtelStatusCode::try_from(status.code).ok() {
            Some(OtelStatusCode::Ok) => "ok",
            Some(OtelStatusCode::Error) => "error",
            _ => "unset",
        };
        attrs.insert("span.status".to_string(), status_str.to_string());
        if !status.message.is_empty() {
            attrs.insert("span.status.message".to_string(), status.message.clone());
        }
    }

    attrs
}

/// Convert OTEL attribute value to string
fn format_attribute_value(value: &opentelemetry_proto::tonic::common::v1::AnyValue) -> String {
    use opentelemetry_proto::tonic::common::v1::any_value::Value;

    match &value.value {
        Some(Value::StringValue(s)) => s.clone(),
        Some(Value::BoolValue(b)) => b.to_string(),
        Some(Value::IntValue(i)) => i.to_string(),
        Some(Value::DoubleValue(d)) => d.to_string(),
        Some(Value::ArrayValue(arr)) => {
            format!("[{}]", arr.values.len())
        }
        Some(Value::KvlistValue(_)) => "[object]".to_string(),
        Some(Value::BytesValue(b)) => format!("[{} bytes]", b.len()),
        None => "".to_string(),
    }
}

/// Convert OTEL span to AgentFlowEdge
fn convert_otel_span_to_edge(
    span: &opentelemetry_proto::tonic::trace::v1::Span,
    resource_attrs: &HashMap<String, String>,
) -> Result<AgentFlowEdge, String> {
    // Parse span_id (8 bytes) to u64, then extend to u128
    let span_id = u64::from_be_bytes(
        span.span_id
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid span_id length")?,
    ) as u128;

    // Parse parent_span_id
    let parent_span_id = if span.parent_span_id.is_empty() {
        0u128
    } else {
        u64::from_be_bytes(
            span.parent_span_id
                .as_slice()
                .try_into()
                .map_err(|_| "Invalid parent_span_id length")?,
        ) as u128
    };

    // Parse trace_id (16 bytes) - use as session_id
    let trace_id_bytes: [u8; 16] = span
        .trace_id
        .as_slice()
        .try_into()
        .map_err(|_| "Invalid trace_id length")?;
    let session_id = u64::from_be_bytes([
        trace_id_bytes[0],
        trace_id_bytes[1],
        trace_id_bytes[2],
        trace_id_bytes[3],
        trace_id_bytes[4],
        trace_id_bytes[5],
        trace_id_bytes[6],
        trace_id_bytes[7],
    ]);

    // Extract tenant_id from resource attributes
    // Priority: tenant_id > flowtrace.tenant_id > service.namespace > default(1)
    let tenant_id = resource_attrs
        .get("tenant_id")
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| {
            resource_attrs
                .get("flowtrace.tenant_id")
                .and_then(|s| s.parse::<u64>().ok())
        })
        .or_else(|| {
            resource_attrs
                .get("service.namespace")
                .and_then(|s| s.parse::<u64>().ok())
        })
        .unwrap_or(1);

    // Extract project_id from resource attributes
    // Priority: project_id > flowtrace.project_id > hash of service.name > default(0)
    let project_id = resource_attrs
        .get("project_id")
        .and_then(|s| s.parse::<u16>().ok())
        .or_else(|| {
            resource_attrs
                .get("flowtrace.project_id")
                .and_then(|s| s.parse::<u16>().ok())
        })
        .or_else(|| {
            // Hash service.name to project_id as fallback
            resource_attrs
                .get("service.name")
                .map(|s| (hash_string(s) % 65535) as u16)
        })
        .unwrap_or(0);

    // Extract agent_id
    // Priority: service.instance.id > process.pid > service.name hash
    let agent_id = resource_attrs
        .get("service.instance.id")
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| {
            resource_attrs
                .get("process.pid")
                .and_then(|s| s.parse::<u64>().ok())
        })
        .or_else(|| resource_attrs.get("service.name").map(|s| hash_string(s)))
        .unwrap_or_else(|| hash_string(&span.name));

    // Determine span type from OTEL span kind and attributes
    let span_type = determine_span_type(span, resource_attrs);

    // Convert timestamps (OTEL uses nanoseconds, we use microseconds)
    let timestamp_us = span.start_time_unix_nano / 1000;
    let end_time_us = span.end_time_unix_nano / 1000;
    let duration_us = if end_time_us > timestamp_us {
        (end_time_us - timestamp_us) as u32
    } else {
        0
    };

    // Extract token count from GenAI attributes
    let token_count = extract_token_count(span);

    // Extract environment
    let environment = resource_attrs
        .get("deployment.environment")
        .map(|s| Environment::parse(s) as u8)
        .unwrap_or(Environment::Development as u8);

    // Create edge
    let mut edge = AgentFlowEdge::new(
        tenant_id,
        project_id,
        agent_id,
        session_id,
        span_type,
        parent_span_id,
    );

    // Override with OTEL values
    edge.edge_id = span_id;
    edge.timestamp_us = timestamp_us;
    edge.duration_us = duration_us;
    edge.token_count = token_count;
    edge.environment = environment;

    // Set error flag if span status is error
    if let Some(status) = &span.status {
        if matches!(
            OtelStatusCode::try_from(status.code),
            Ok(OtelStatusCode::Error)
        ) {
            edge.span_type = SpanType::Error as u32;
        }
    }

    // Recompute checksum
    edge.checksum = edge.compute_checksum();

    // Validate
    edge.validate()
        .map_err(|e| format!("Edge validation failed: {}", e))?;

    Ok(edge)
}

/// Determine SpanType from OTEL span
fn determine_span_type(
    span: &opentelemetry_proto::tonic::trace::v1::Span,
    _resource_attrs: &HashMap<String, String>,
) -> flowtrace_core::SpanType {
    // Check for GenAI attributes first
    for attr in &span.attributes {
        match attr.key.as_str() {
            "gen_ai.request.model" | "gen_ai.system" => {
                // LLM call - determine specific type
                let name_lower = span.name.to_lowercase();
                if name_lower.contains("plan") {
                    return SpanType::Planning;
                } else if name_lower.contains("reason") {
                    return SpanType::Reasoning;
                } else if name_lower.contains("synthes") {
                    return SpanType::Synthesis;
                }
                return SpanType::Response;
            }
            "db.system" => return SpanType::Database,
            "http.method" | "http.request.method" => return SpanType::HttpCall,
            key if key.starts_with("db.") => return SpanType::Database,
            key if key.starts_with("retrieval.") => return SpanType::Retrieval,
            key if key.starts_with("embedding.") => return SpanType::Embedding,
            _ => {}
        }
    }

    // Check span name
    let name_lower = span.name.to_lowercase();
    if name_lower.contains("retriev") || name_lower.contains("search") {
        return SpanType::Retrieval;
    } else if name_lower.contains("embed") {
        return SpanType::Embedding;
    } else if name_lower.contains("http") || name_lower.contains("request") {
        return SpanType::HttpCall;
    } else if name_lower.contains("database") || name_lower.contains("query") {
        return SpanType::Database;
    } else if name_lower.contains("function") || name_lower.contains("call") {
        return SpanType::Function;
    } else if name_lower.contains("tool") {
        return SpanType::ToolCall;
    } else if name_lower.contains("root") || name_lower.contains("trace") {
        return SpanType::Root;
    }

    // Check OTEL span kind
    match SpanKind::try_from(span.kind).ok() {
        Some(SpanKind::Client) => SpanType::HttpCall,
        Some(SpanKind::Server) => SpanType::Function,
        Some(SpanKind::Internal) => SpanType::Function,
        _ => SpanType::Custom,
    }
}

/// Extract token count from GenAI attributes
fn extract_token_count(span: &opentelemetry_proto::tonic::trace::v1::Span) -> u32 {
    let mut input_tokens = 0u32;
    let mut output_tokens = 0u32;

    for attr in &span.attributes {
        match attr.key.as_str() {
            "gen_ai.usage.input_tokens" | "gen_ai.usage.prompt_tokens" => {
                if let Some(value) = &attr.value {
                    if let Some(
                        opentelemetry_proto::tonic::common::v1::any_value::Value::IntValue(i),
                    ) = &value.value
                    {
                        input_tokens = *i as u32;
                    }
                }
            }
            "gen_ai.usage.output_tokens" | "gen_ai.usage.completion_tokens" => {
                if let Some(value) = &attr.value {
                    if let Some(
                        opentelemetry_proto::tonic::common::v1::any_value::Value::IntValue(i),
                    ) = &value.value
                    {
                        output_tokens = *i as u32;
                    }
                }
            }
            "gen_ai.usage.total_tokens" => {
                if let Some(value) = &attr.value {
                    if let Some(
                        opentelemetry_proto::tonic::common::v1::any_value::Value::IntValue(i),
                    ) = &value.value
                    {
                        return *i as u32;
                    }
                }
            }
            _ => {}
        }
    }

    input_tokens + output_tokens
}

/// Hash string to u64
fn hash_string(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

// ============================================================================
// OTLP JSON Parsing (Gap #5 fix)
// ============================================================================

/// OTLP JSON format structures matching the OpenTelemetry protobuf definitions
mod otlp_json {
    use serde::Deserialize;
    
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ExportTraceServiceRequest {
        #[serde(default)]
        pub resource_spans: Vec<ResourceSpans>,
    }
    
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ResourceSpans {
        #[serde(default)]
        pub resource: Option<Resource>,
        #[serde(default)]
        pub scope_spans: Vec<ScopeSpans>,
    }
    
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Resource {
        #[serde(default)]
        pub attributes: Vec<KeyValue>,
    }
    
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ScopeSpans {
        #[serde(default)]
        pub scope: Option<InstrumentationScope>,
        #[serde(default)]
        pub spans: Vec<Span>,
    }
    
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct InstrumentationScope {
        #[serde(default)]
        pub name: String,
        #[serde(default)]
        pub version: String,
    }
    
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Span {
        /// Base64-encoded trace_id
        #[serde(default)]
        pub trace_id: String,
        /// Base64-encoded span_id
        #[serde(default)]
        pub span_id: String,
        #[serde(default)]
        pub parent_span_id: String,
        #[serde(default)]
        pub name: String,
        #[serde(default)]
        pub kind: u32,
        #[serde(default)]
        pub start_time_unix_nano: u64,
        #[serde(default)]
        pub end_time_unix_nano: u64,
        #[serde(default)]
        pub attributes: Vec<KeyValue>,
        #[serde(default)]
        pub status: Option<Status>,
    }
    
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Status {
        #[serde(default)]
        pub code: u32,
        #[serde(default)]
        pub message: String,
    }
    
    #[derive(Debug, Deserialize, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct KeyValue {
        pub key: String,
        #[serde(default)]
        pub value: Option<AnyValue>,
    }
    
    #[derive(Debug, Deserialize, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct AnyValue {
        #[serde(default)]
        pub string_value: Option<String>,
        #[serde(default)]
        pub bool_value: Option<bool>,
        #[serde(default)]
        pub int_value: Option<i64>,
        #[serde(default)]
        pub double_value: Option<f64>,
        #[serde(default)]
        pub bytes_value: Option<String>, // Base64 encoded
    }
}

/// Parse OTLP JSON and convert to protobuf format
fn parse_otlp_json(body: &[u8]) -> Result<ExportTraceServiceRequest, String> {
    use opentelemetry_proto::tonic::resource::v1::Resource;
    use opentelemetry_proto::tonic::trace::v1::{
        ResourceSpans, ScopeSpans, Span, Status,
    };
    
    let json_req: otlp_json::ExportTraceServiceRequest = serde_json::from_slice(body)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    
    let mut resource_spans = Vec::new();
    
    for json_rs in json_req.resource_spans {
        let resource = json_rs.resource.map(|r| {
            Resource {
                attributes: r.attributes.into_iter()
                    .map(|kv| convert_json_keyvalue(&kv))
                    .collect(),
                dropped_attributes_count: 0,
            }
        });
        
        let mut scope_spans = Vec::new();
        for json_ss in json_rs.scope_spans {
            let mut spans = Vec::new();
            for json_span in json_ss.spans {
                // Decode base64 trace_id and span_id
                let trace_id = decode_base64_or_hex(&json_span.trace_id)?;
                let span_id = decode_base64_or_hex(&json_span.span_id)?;
                let parent_span_id = if json_span.parent_span_id.is_empty() {
                    vec![]
                } else {
                    decode_base64_or_hex(&json_span.parent_span_id)?
                };
                
                let status = json_span.status.map(|s| Status {
                    code: s.code as i32,
                    message: s.message,
                });
                
                spans.push(Span {
                    trace_id,
                    span_id,
                    parent_span_id,
                    name: json_span.name,
                    kind: json_span.kind as i32,
                    start_time_unix_nano: json_span.start_time_unix_nano,
                    end_time_unix_nano: json_span.end_time_unix_nano,
                    attributes: json_span.attributes.into_iter()
                        .map(|kv| convert_json_keyvalue(&kv))
                        .collect(),
                    status,
                    ..Default::default()
                });
            }
            
            scope_spans.push(ScopeSpans {
                scope: json_ss.scope.map(|s| {
                    opentelemetry_proto::tonic::common::v1::InstrumentationScope {
                        name: s.name,
                        version: s.version,
                        ..Default::default()
                    }
                }),
                spans,
                ..Default::default()
            });
        }
        
        resource_spans.push(ResourceSpans {
            resource,
            scope_spans,
            ..Default::default()
        });
    }
    
    Ok(ExportTraceServiceRequest { resource_spans })
}

/// Convert JSON KeyValue to protobuf KeyValue
fn convert_json_keyvalue(kv: &otlp_json::KeyValue) -> opentelemetry_proto::tonic::common::v1::KeyValue {
    use opentelemetry_proto::tonic::common::v1::{any_value, AnyValue, KeyValue};
    
    let value = kv.value.as_ref().map(|v| {
        let val = if let Some(s) = &v.string_value {
            any_value::Value::StringValue(s.clone())
        } else if let Some(b) = v.bool_value {
            any_value::Value::BoolValue(b)
        } else if let Some(i) = v.int_value {
            any_value::Value::IntValue(i)
        } else if let Some(d) = v.double_value {
            any_value::Value::DoubleValue(d)
        } else {
            any_value::Value::StringValue(String::new())
        };
        AnyValue { value: Some(val) }
    });
    
    KeyValue {
        key: kv.key.clone(),
        value,
    }
}

/// Decode base64 or hex string to bytes
fn decode_base64_or_hex(s: &str) -> Result<Vec<u8>, String> {
    if s.is_empty() {
        return Ok(vec![]);
    }
    
    // Try hex first (common in OTEL JSON)
    if s.len() == 32 || s.len() == 16 { // trace_id or span_id in hex
        if let Ok(bytes) = hex::decode(s) {
            return Ok(bytes);
        }
    }
    
    // Try base64
    use base64::{Engine as _, engine::general_purpose};
    general_purpose::STANDARD.decode(s)
        .or_else(|_| general_purpose::URL_SAFE.decode(s))
        .map_err(|e| format!("Failed to decode '{}': {}", s, e))
}
