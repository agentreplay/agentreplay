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

// agentreplay-server/src/api/ingest.rs
//! Ingestion API for Agentreplay traces
//!
//! Accepts AgentreplaySpan batches via POST and converts them to AgentFlowEdge records.
//!
//! ## Architecture
//!
//! When the IngestionActor is available, traces flow through:
//! ```text
//! HTTP Request → Validation → Actor Channel → Batch → Governor → Storage
//! ```
//!
//! This provides:
//! - **Batching**: 64 traces per batch, amortizes embedding cost
//! - **Deduplication**: Semantic Governor drops similar traces (32x storage savings)
//! - **Backpressure**: Channel capacity limits memory under load

use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use agentreplay_core::{AgentFlowEdge, Environment, SpanType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

use crate::api::query::{ApiError, AppState};
use crate::auth::AuthContext;
use crate::ingestion::{IngestionResult, TracePayload};
use crate::otel_genai::GenAIPayload;
use crate::sanitization;
use crate::validation;

/// Simplified span structure for ingestion (matches agentreplay-observability)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentreplaySpan {
    pub span_id: String,
    pub trace_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    /// Timestamp in microseconds since Unix epoch
    pub start_time: u64,
    /// Timestamp in microseconds since Unix epoch
    pub end_time: Option<u64>,
    pub attributes: HashMap<String, String>,
}

/// Request body for POST /api/v1/traces
#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    pub spans: Vec<AgentreplaySpan>,
}

/// Response for POST /api/v1/traces
#[derive(Debug, Serialize)]
pub struct IngestResponse {
    pub accepted: usize,
    pub rejected: usize,
    /// Number of traces deduplicated (similar to existing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deduplicated: Option<usize>,
    pub errors: Vec<String>,
}

/// POST /api/v1/traces - Ingest a batch of spans
///
/// # Request Body
/// ```json
/// {
///   "spans": [
///     {
///       "span_id": "0x1a2b3c4d",
///       "trace_id": "0x123abc",
///       "parent_span_id": null,
///       "name": "root_span",
///       "start_time": 1234567890000000,
///       "end_time": 1234567891000000,
///       "attributes": {
///         "agent_id": "my-agent",
///         "session_id": "session-123"
///       }
///     }
///   ]
/// }
/// ```
///
/// # Validation Rules
/// - Max 1000 spans per batch
/// - span_id must be valid hex string (0x prefix optional)
/// - start_time and end_time must be microseconds since epoch
/// - start_time must be ≤ end_time
/// - Attributes total size must be ≤ 1MB
/// - Span name max length: 256 characters
///
/// # Response Codes
/// - 201: Success - Returns accepted/rejected counts
/// - 400: Validation error - Check error field for details
/// - 401: Unauthorized - Check authentication
/// - 500: Server error - Check server logs
#[tracing::instrument(skip(state, _auth, request), fields(span_count = request.spans.len()))]
pub async fn ingest_traces(
    State(state): State<AppState>,
    Extension(_auth): Extension<AuthContext>, // TODO: Use auth.tenant_id for multi-tenancy
    Json(request): Json<IngestRequest>,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    debug!("Ingesting {} spans", request.spans.len());

    // VALIDATION: Batch size (Task 5)
    validation::validate_batch_size(request.spans.len())?;

    // Try the high-performance path first (IngestionActor with deduplication)
    if let Some(ref actor) = state.ingestion_actor {
        return ingest_via_actor(&state, actor, request).await;
    }

    // Fallback: Direct ingestion (no deduplication)
    debug!("Using direct ingestion path (no actor available)");
    ingest_direct(&state, request).await
}

/// High-performance ingestion via the IngestionActor
///
/// Routes traces through: Validation → Batching → Deduplication → Storage
async fn ingest_via_actor(
    state: &AppState,
    actor: &crate::ingestion::IngestionActorHandle,
    request: IngestRequest,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    let mut payloads = Vec::with_capacity(request.spans.len());
    let mut errors = Vec::new();
    let mut edges_for_storage = Vec::new();

    // Phase 1: Validate and convert spans
    for (idx, span) in request.spans.iter().enumerate() {
        // Run all validations
        if let Err(e) = validate_span(idx, span) {
            errors.push(e);
            continue;
        }

        // Convert to edge (for storage after deduplication)
        let validated_span = match sanitize_span(span) {
            Ok(s) => s,
            Err(e) => {
                errors.push(format!("Span {}: {}", idx, e));
                continue;
            }
        };

        match convert_span_to_edge(&validated_span) {
            Ok(edge) => {
                // Extract text for embedding (prompt + completion if available)
                let text = extract_embedding_text(&validated_span.attributes);
                let payload_json =
                    serde_json::to_value(&validated_span).unwrap_or(serde_json::json!({}));

                payloads.push(TracePayload {
                    trace_id: edge.edge_id,
                    text,
                    payload: payload_json,
                });
                edges_for_storage.push((edge, validated_span.attributes.clone()));
            }
            Err(e) => {
                errors.push(format!("Span {}: {}", idx, e));
            }
        }
    }

    if payloads.is_empty() {
        return Ok((
            StatusCode::CREATED,
            Json(IngestResponse {
                accepted: 0,
                rejected: errors.len(),
                deduplicated: Some(0),
                errors,
            }),
        ));
    }

    // Phase 2: Send to actor for batching and deduplication
    let results = actor.ingest_many(payloads).await;

    // Phase 3: Process results and store non-deduplicated traces
    let mut stored = 0;
    let mut deduplicated = 0;
    let mut failed = 0;

    for (result, (edge, attrs)) in results.into_iter().zip(edges_for_storage.into_iter()) {
        match result {
            Ok(IngestionResult::Stored {
                trace_id,
                embedding,
            }) => {
                // Store the edge, payload, AND embedding for semantic search
                if let Err(e) = store_edge_and_payload(state, &edge, &attrs, Some(embedding)).await
                {
                    warn!("Failed to store edge {:#x}: {}", trace_id, e);
                    failed += 1;
                    continue;
                }

                // Track cost and broadcast
                state.cost_tracker.track_edge(&edge, None).await;
                let _ = state.trace_broadcaster.send(edge);
                stored += 1;
            }
            Ok(IngestionResult::Deduplicated {
                trace_id,
                similar_to,
                similarity,
            }) => {
                debug!(
                    "Trace {:#x} deduplicated (similar to {:#x}, {:.1}% match)",
                    trace_id,
                    similar_to,
                    similarity * 100.0
                );
                deduplicated += 1;
            }
            Ok(IngestionResult::Failed { trace_id, error }) => {
                warn!("Trace {:#x} failed: {}", trace_id, error);
                errors.push(format!("Trace {:#x}: {}", trace_id, error));
                failed += 1;
            }
            Err(e) => {
                errors.push(e);
                failed += 1;
            }
        }
    }

    info!(
        "Ingestion complete: {} stored, {} deduplicated, {} failed",
        stored, deduplicated, failed
    );

    Ok((
        StatusCode::CREATED,
        Json(IngestResponse {
            accepted: stored,
            rejected: errors.len(),
            deduplicated: Some(deduplicated),
            errors,
        }),
    ))
}

/// Extract text for embedding from span attributes
fn extract_embedding_text(attrs: &HashMap<String, String>) -> String {
    // Prioritize GenAI semantic convention fields
    let mut parts = Vec::new();

    // Check for prompt content
    if let Some(prompt) = attrs.get("gen_ai.prompt.0.content") {
        parts.push(prompt.clone());
    } else if let Some(prompt) = attrs.get("llm.prompts.0.content") {
        parts.push(prompt.clone());
    }

    // Check for completion content
    if let Some(completion) = attrs.get("gen_ai.completion.0.content") {
        parts.push(completion.clone());
    } else if let Some(completion) = attrs.get("llm.completions.0.content") {
        parts.push(completion.clone());
    }

    // Fallback to span name or any input field
    if parts.is_empty() {
        if let Some(input) = attrs.get("input") {
            parts.push(input.clone());
        }
    }

    if parts.is_empty() {
        // Use all attribute values as fallback
        parts = attrs.values().take(5).cloned().collect();
    }

    parts.join(" ")
}

/// Validate a span and return error message if invalid
fn validate_span(idx: usize, span: &AgentreplaySpan) -> Result<(), String> {
    if let Err(e) = validation::validate_span_id(&span.span_id) {
        return Err(format!("Span {}: {}", idx, e));
    }
    if let Err(e) = validation::validate_timestamp_range(span.start_time, span.end_time) {
        return Err(format!("Span {}: {}", idx, e));
    }
    if let Err(e) = validation::validate_span_name(&span.name) {
        return Err(format!("Span {}: {}", idx, e));
    }
    if let Err(e) = validation::validate_attributes_size(&span.attributes) {
        return Err(format!("Span {}: {}", idx, e));
    }
    Ok(())
}

/// Sanitize a span (validate and clean name/attributes)
fn sanitize_span(span: &AgentreplaySpan) -> Result<AgentreplaySpan, String> {
    let validated_name =
        sanitization::validate_name(&span.name).map_err(|e| format!("invalid name - {}", e))?;
    let validated_attrs = sanitization::validate_attributes(&span.attributes)
        .map_err(|e| format!("invalid attributes - {}", e))?;

    Ok(AgentreplaySpan {
        span_id: span.span_id.clone(),
        trace_id: span.trace_id.clone(),
        parent_span_id: span.parent_span_id.clone(),
        name: validated_name,
        start_time: span.start_time,
        end_time: span.end_time,
        attributes: validated_attrs,
    })
}

/// Store edge, payload, and embedding to the database
///
/// When an embedding is provided, it's stored in the vector index to enable
/// semantic search across all ingested traces.
async fn store_edge_and_payload(
    state: &AppState,
    edge: &AgentFlowEdge,
    attrs: &HashMap<String, String>,
    embedding: Option<Vec<f32>>,
) -> Result<(), String> {
    // Store payload first
    if !attrs.is_empty() {
        let mut genai_payload = GenAIPayload::from_attributes(attrs);
        genai_payload.calculate_total_tokens();

        let json_bytes = serde_json::to_vec(&genai_payload)
            .map_err(|e| format!("Failed to serialize payload: {}", e))?;

        let store_result = if let Some(ref pm) = state.project_manager {
            pm.get_or_open_project(edge.project_id)
                .and_then(|db| db.put_payload(edge.edge_id, &json_bytes))
        } else {
            state.db.put_payload(edge.edge_id, &json_bytes)
        };

        store_result.map_err(|e| format!("Failed to store payload: {}", e))?;
    }

    // Insert edge (with embedding if available for semantic search)
    if let Some(ref pm) = state.project_manager {
        if let Some(emb) = embedding {
            // Store with embedding for semantic search
            pm.insert_with_embedding(*edge, emb)
                .await
                .map_err(|e| format!("Failed to insert edge with embedding: {}", e))?;
        } else {
            pm.insert(*edge)
                .await
                .map_err(|e| format!("Failed to insert edge: {}", e))?;
        }
    } else {
        // Fallback to batch insert without embedding
        if let Some(emb) = embedding {
            use agentreplay_index::Embedding;
            let embedding_array = Embedding::from_vec(emb);
            state
                .db
                .insert_with_vector(*edge, embedding_array)
                .await
                .map_err(|e| format!("Failed to insert edge with embedding: {}", e))?;
        } else {
            state
                .db
                .insert_batch(&[*edge])
                .await
                .map_err(|e| format!("Failed to insert edge: {}", e))?;
        }
    }

    Ok(())
}

/// Direct ingestion path (fallback when actor is not available)
async fn ingest_direct(
    state: &AppState,
    request: IngestRequest,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    let mut edges = Vec::new();
    let mut errors = Vec::new();

    // Build list of (edge, attributes) pairs
    let mut edge_attributes: Vec<(AgentFlowEdge, std::collections::HashMap<String, String>)> =
        Vec::new();

    for (idx, span) in request.spans.iter().enumerate() {
        // VALIDATION: Span ID format (Task 5)
        if let Err(e) = validation::validate_span_id(&span.span_id) {
            warn!("Invalid span ID at index {}: {}", idx, e);
            errors.push(format!("Span {}: {}", idx, e));
            continue;
        }

        // VALIDATION: Timestamp range (Task 5)
        if let Err(e) = validation::validate_timestamp_range(span.start_time, span.end_time) {
            warn!("Invalid timestamp at index {}: {}", idx, e);
            errors.push(format!("Span {}: {}", idx, e));
            continue;
        }

        // VALIDATION: Span name (Task 5)
        if let Err(e) = validation::validate_span_name(&span.name) {
            warn!("Invalid span name at index {}: {}", idx, e);
            errors.push(format!("Span {}: {}", idx, e));
            continue;
        }

        // VALIDATION: Attributes size (Task 5)
        if let Err(e) = validation::validate_attributes_size(&span.attributes) {
            warn!("Invalid attributes at index {}: {}", idx, e);
            errors.push(format!("Span {}: {}", idx, e));
            continue;
        }

        // SECURITY: Validate and sanitize span name
        let validated_name = match sanitization::validate_name(&span.name) {
            Ok(name) => name,
            Err(e) => {
                warn!("Invalid span name at index {}: {}", idx, e);
                errors.push(format!("Span {}: invalid name - {}", idx, e));
                continue;
            }
        };

        // SECURITY: Validate and sanitize attributes
        let validated_attrs = match sanitization::validate_attributes(&span.attributes) {
            Ok(attrs) => attrs,
            Err(e) => {
                warn!("Invalid attributes at index {}: {}", idx, e);
                errors.push(format!("Span {}: invalid attributes - {}", idx, e));
                continue;
            }
        };

        // Create a validated span with cleaned data
        let validated_span = AgentreplaySpan {
            span_id: span.span_id.clone(),
            trace_id: span.trace_id.clone(),
            parent_span_id: span.parent_span_id.clone(),
            name: validated_name,
            start_time: span.start_time,
            end_time: span.end_time,
            attributes: validated_attrs.clone(),
        };

        match convert_span_to_edge(&validated_span) {
            Ok(edge) => {
                // Store edge and its validated attributes together
                edge_attributes.push((edge, validated_attrs));
                edges.push(edge);
            }
            Err(e) => {
                warn!("Failed to convert span {}: {}", idx, e);
                errors.push(format!("Span {}: {}", idx, e));
            }
        }
    }

    let accepted = edges.len();
    let rejected = errors.len();

    // CRITICAL FIX: Reorder operations to prevent read-your-writes race condition
    // 1. Store payloads FIRST
    // 2. Insert edges to LSM
    // 3. Broadcast to UI (only after data is fully consistent)
    if !edges.is_empty() {
        // Step 1: Store all payloads FIRST (before inserting edges)
        for (edge, attributes) in &edge_attributes {
            if !attributes.is_empty() {
                eprintln!(
                    "[INGEST] Storing payload for edge {:#x}, project {}, tenant {}",
                    edge.edge_id, edge.project_id, edge.tenant_id
                );
                eprintln!("[INGEST] Raw attributes ({} keys):", attributes.len());
                for (key, value) in attributes.iter() {
                    eprintln!("[INGEST]   {} = {}", key, value);
                }

                // Convert to GenAI-compliant payload
                let mut genai_payload = GenAIPayload::from_attributes(attributes);
                genai_payload.calculate_total_tokens();

                match serde_json::to_vec(&genai_payload) {
                    Ok(json_bytes) => {
                        eprintln!("[INGEST] Serialized payload: {} bytes", json_bytes.len());
                        if let Ok(json_str) = String::from_utf8(json_bytes.clone()) {
                            eprintln!("[INGEST] Payload JSON: {}", json_str);
                        }

                        // Store payload in the correct database (project-specific or main)
                        let store_result = if let Some(ref pm) = state.project_manager {
                            eprintln!(
                                "[INGEST] Storing payload in project database {}",
                                edge.project_id
                            );
                            pm.get_or_open_project(edge.project_id)
                                .and_then(|db| db.put_payload(edge.edge_id, &json_bytes))
                        } else {
                            eprintln!("[INGEST] Storing payload in main database");
                            state.db.put_payload(edge.edge_id, &json_bytes)
                        };

                        match store_result {
                            Ok(_) => {
                                eprintln!(
                                    "[INGEST] ✓ Payload stored successfully for {:#x}",
                                    edge.edge_id
                                );
                            }
                            Err(e) => {
                                eprintln!("[INGEST] ✗ Failed to store payload: {}", e);
                                warn!(
                                    "Failed to store attributes for edge {:#x}: {}",
                                    edge.edge_id, e
                                );
                                // Don't fail entire batch, but payload won't be available
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[INGEST] ✗ Failed to serialize payload: {}", e);
                        warn!(
                            "Failed to serialize GenAI payload for edge {:#x}: {}",
                            edge.edge_id, e
                        );
                    }
                }
            } else {
                eprintln!(
                    "[INGEST] No attributes to store for edge {:#x}",
                    edge.edge_id
                );
            }
        }

        // Step 2: Insert edges to LSM Tree (Atomic Batch)
        if let Some(ref pm) = state.project_manager {
            // Use ProjectManager - routes to per-project storage
            for edge in &edges {
                if let Err(e) = pm.insert(*edge).await {
                    error!("Failed to ingest edge via ProjectManager: {}", e);
                    return Err(ApiError::Internal(format!("Ingestion failed: {}", e)));
                }
            }
        } else {
            // Fallback to single database with batch insert
            if let Err(e) = state.db.insert_batch(&edges).await {
                error!("Failed to ingest edges: {}", e);
                return Err(ApiError::Internal(format!("Ingestion failed: {}", e)));
            }
        }

        // Step 3: Broadcast to UI (Only after data is fully consistent)
        for edge in &edges {
            // Track cost after successful write
            state.cost_tracker.track_edge(edge, None).await;

            // Broadcast to UI
            if let Err(e) = state.trace_broadcaster.send(*edge) {
                warn!(
                    "Failed to broadcast edge {:#x}: {} (edge persisted but not broadcast)",
                    edge.edge_id, e
                );
                // Don't fail the request - edge is persisted, broadcast channel may be full
            }
        }
    }

    debug!(
        "Ingestion complete: {} accepted, {} rejected",
        accepted, rejected
    );

    Ok((
        StatusCode::CREATED,
        Json(IngestResponse {
            accepted,
            rejected,
            deduplicated: None, // Direct path doesn't deduplicate
            errors,
        }),
    ))
}

/// Convert AgentreplaySpan to AgentFlowEdge
fn convert_span_to_edge(span: &AgentreplaySpan) -> Result<AgentFlowEdge, String> {
    // Parse span_id as u64 (will be cast to u128 for edge_id)
    let edge_id = parse_id_to_u64(&span.span_id)
        .ok_or_else(|| format!("Invalid span_id format: {}", span.span_id))?
        as u128;

    // Parse parent_span_id as u64 (0 if None)
    let causal_parent = span
        .parent_span_id
        .as_ref()
        .and_then(|id| parse_id_to_u64(id))
        .unwrap_or(0) as u128;

    // Extract session_id from attributes if provided, otherwise hash trace_id
    let session_id = span
        .attributes
        .get("session_id")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| hash_string_to_u64(&span.trace_id));

    // Validate timestamps (must be in microseconds)
    // Use production config (2020-2099) for server validation
    let config = agentreplay_core::TimestampConfig::production();
    agentreplay_core::validate_timestamp(span.start_time, &config)
        .map_err(|e| format!("Invalid start_time: {}", e))?;

    if let Some(end_time) = span.end_time {
        agentreplay_core::validate_timestamp(end_time, &config)
            .map_err(|e| format!("Invalid end_time: {}", e))?;

        if end_time < span.start_time {
            return Err("end_time cannot be before start_time".to_string());
        }
    }

    // Calculate duration
    let duration_us = span
        .end_time
        .map(|end| (end - span.start_time) as u32)
        .unwrap_or(0);

    // Parse span name to SpanType
    let span_type = parse_span_name_to_type(&span.name);

    // Extract attributes
    // Try to extract token split first (OpenTelemetry GenAI standard)
    let input_tokens = span
        .attributes
        .get("gen_ai.usage.input_tokens")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    let output_tokens = span
        .attributes
        .get("gen_ai.usage.output_tokens")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    // Calculate total tokens
    let token_count = if input_tokens > 0 || output_tokens > 0 {
        input_tokens + output_tokens
    } else {
        // Fallback to legacy attributes
        span.attributes
            .get("gen_ai.usage.total_tokens")
            .or_else(|| span.attributes.get("tokens"))
            .or_else(|| span.attributes.get("token_count"))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    };

    // Extract environment (default to Development)
    let environment = span
        .attributes
        .get("environment")
        .or_else(|| span.attributes.get("deployment.environment"))
        .map(|s| Environment::parse(s) as u8)
        .unwrap_or(Environment::Development as u8);

    // Extract service name for tenant_id (hash for now, proper mapping in Phase 2)
    // Check for direct tenant_id attribute first
    let tenant_id = span
        .attributes
        .get("tenant_id")
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| {
            span.attributes
                .get("service.name")
                .or_else(|| span.attributes.get("service"))
                .map(|s| hash_string_to_u64(s))
        })
        .unwrap_or(0); // Default tenant

    // Extract project (check for direct project_id attribute first, then hash)
    let project_id = span
        .attributes
        .get("project_id")
        .and_then(|s| s.parse::<u16>().ok())
        .or_else(|| {
            span.attributes
                .get("project")
                .or_else(|| span.attributes.get("service.namespace"))
                .map(|s| hash_string_to_u16(s))
        })
        .unwrap_or(0);

    // Extract agent_id (parse or hash)
    let agent_id = span
        .attributes
        .get("agent_id")
        .or_else(|| span.attributes.get("agent"))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| hash_string_to_u64(&span.name));

    // Create edge
    let mut edge = AgentFlowEdge::new(
        tenant_id,
        project_id,
        agent_id,
        session_id,
        span_type,
        causal_parent,
    );

    // Override timestamp, duration, and other fields
    edge.timestamp_us = span.start_time;
    edge.duration_us = duration_us;
    edge.token_count = token_count;
    edge.environment = environment;
    edge.edge_id = edge_id; // Use provided span_id instead of generated

    // Recompute checksum after modifications
    edge.checksum = edge.compute_checksum();

    // Validate final edge
    edge.validate()
        .map_err(|e| format!("Edge validation failed: {}", e))?;

    Ok(edge)
}

/// Parse string ID to u64
/// Supports:
/// - Hex strings with 0x prefix
/// - Decimal strings
/// - Falls back to hashing for unparseable strings
fn parse_id_to_u64(id: &str) -> Option<u64> {
    // Try hex (0x prefix only - explicit hex marker)
    if let Some(hex) = id.strip_prefix("0x") {
        return u64::from_str_radix(hex, 16).ok();
    }

    // Try decimal (most common case)
    if let Ok(n) = id.parse::<u64>() {
        return Some(n);
    }

    // Fallback: hash the string (handles arbitrary IDs)
    Some(hash_string_to_u64(id))
}

/// Hash string to u64
pub fn hash_string_to_u64(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Hash string to u16
fn hash_string_to_u16(s: &str) -> u16 {
    (hash_string_to_u64(s) & 0xFFFF) as u16
}

/// Parse span name to SpanType
fn parse_span_name_to_type(name: &str) -> SpanType {
    let lower = name.to_lowercase();

    // Check for common OTel semantic conventions
    if lower.contains("llm") || lower.contains("chat") || lower.contains("planning") {
        return SpanType::Planning;
    }

    if lower.contains("tool") || lower.contains("function_call") {
        return SpanType::ToolCall;
    }

    if lower.contains("root") || lower.contains("trace") {
        return SpanType::Root;
    }

    if lower.contains("reasoning") {
        return SpanType::Reasoning;
    }

    if lower.contains("synthesis") {
        return SpanType::Synthesis;
    }

    if lower.contains("response") {
        return SpanType::Response;
    }

    if lower.contains("error") {
        return SpanType::Error;
    }

    // Default to Custom
    SpanType::Custom
}

/// POST /api/v1/traces/otel - Ingest OpenTelemetry format spans
/// This endpoint accepts industry-standard OpenTelemetry span format
/// and converts it to Agentreplay's internal AgentFlowEdge format
pub async fn ingest_otel_spans(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(batch): Json<crate::api::OtelSpanBatch>,
) -> Result<impl IntoResponse, ApiError> {
    tracing::info!("🔵 [OTEL INGEST] Received {} spans", batch.spans.len());

    if batch.spans.is_empty() {
        return Err(ApiError::BadRequest("Empty span batch".to_string()));
    }

    if batch.spans.len() > 10_000 {
        return Err(ApiError::BadRequest(
            "Batch size exceeds limit of 10,000 spans".to_string(),
        ));
    }

    let mut edges = Vec::new();
    let mut errors = Vec::new();
    let mut edge_payloads: Vec<(u128, serde_json::Value)> = Vec::new();

    // Extract project_id from first span's attributes if not in auth
    let mut project_id = auth.project_id.unwrap_or(0);

    // Try to extract project_id from span attributes
    if project_id == 0 && !batch.spans.is_empty() {
        if let Some(first_span) = batch.spans.first() {
            if let Some(proj_id_val) = first_span.attributes.get("project_id") {
                // Try as string or number
                let pid = if let Some(s) = proj_id_val.as_str() {
                    s.parse::<u16>().ok()
                } else {
                    proj_id_val.as_u64().map(|n| n as u16)
                };

                if let Some(pid) = pid {
                    project_id = pid;
                    tracing::info!(
                        "🔵 [OTEL INGEST] Extracted project_id={} from span attributes",
                        project_id
                    );
                }
            }
        }
    }

    tracing::info!(
        "🔵 [OTEL INGEST] Using tenant_id={}, project_id={}",
        auth.tenant_id,
        project_id
    );

    for (idx, span) in batch.spans.iter().enumerate() {
        match crate::api::convert_otel_span_to_edge(span, auth.tenant_id, project_id) {
            Ok(edge) => {
                tracing::debug!(
                    "🔵 [OTEL INGEST] Converted span {} -> edge {:#x} (project={})",
                    idx,
                    edge.edge_id,
                    project_id
                );

                // Store span payload for later storage
                if !span.attributes.is_empty() {
                    let mut payload = serde_json::Map::new();
                    payload.insert("name".to_string(), serde_json::json!(span.name));

                    // Include all attributes
                    for (key, value) in &span.attributes {
                        payload.insert(key.clone(), value.clone());
                    }

                    // Add timing info
                    payload.insert("start_time".to_string(), serde_json::json!(span.start_time));
                    if let Some(end_time) = span.end_time {
                        payload.insert("end_time".to_string(), serde_json::json!(end_time));
                    }

                    edge_payloads.push((edge.edge_id, serde_json::Value::Object(payload)));
                }

                edges.push(edge);
            }
            Err(e) => {
                warn!("Failed to convert OTel span {}: {}", idx, e);
                errors.push(format!("Span {}: {}", idx, e));
            }
        }
    }

    let accepted = edges.len();
    let rejected = errors.len();

    tracing::info!(
        "🔵 [OTEL INGEST] Processing {} edges for project {}",
        accepted,
        project_id
    );

    // Write edges to storage - use project manager if available
    if !edges.is_empty() {
        // Try project-specific storage first
        if let Some(ref pm) = state.project_manager {
            match pm.get_or_open_project(project_id) {
                Ok(agentreplay) => {
                    tracing::info!(
                        "🔵 [OTEL INGEST] Using project-specific storage for project {}",
                        project_id
                    );
                    for edge in &edges {
                        if let Err(e) = agentreplay.insert(*edge).await {
                            error!("Failed to insert edge {:#x}: {}", edge.edge_id, e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to open project {}: {}", project_id, e);
                    return Err(ApiError::Internal(format!("Failed to open project: {}", e)));
                }
            }
        } else {
            // Fallback to default database
            tracing::warn!("🔵 [OTEL INGEST] No project manager, using default DB");
            if let Err(e) = state.db.insert_batch(&edges).await {
                error!("Failed to ingest edges: {}", e);
                return Err(ApiError::Internal(format!("Ingestion failed: {}", e)));
            }
        }

        // Immediately broadcast after successful write to maintain ordering
        for edge in &edges {
            if let Err(e) = state.trace_broadcaster.send(*edge) {
                warn!(
                    "Failed to broadcast edge {:#x}: {} (edge persisted but not broadcast)",
                    edge.edge_id, e
                );
            }
        }

        // Store payloads (can be async, not critical path)
        for (edge_id, payload) in edge_payloads {
            match serde_json::to_vec(&payload) {
                Ok(json_bytes) => {
                    // Store payload in the correct database (project-specific or main)
                    let store_result = if let Some(ref pm) = state.project_manager {
                        if let Ok(db) = pm.get_or_open_project(project_id) {
                            db.put_payload(edge_id, &json_bytes)
                        } else {
                            // Fallback or error
                            state.db.put_payload(edge_id, &json_bytes)
                        }
                    } else {
                        state.db.put_payload(edge_id, &json_bytes)
                    };

                    if let Err(e) = store_result {
                        warn!("Failed to store payload for edge {:#x}: {}", edge_id, e);
                    }
                }
                Err(e) => {
                    warn!("Failed to serialize payload for edge {:#x}: {}", edge_id, e);
                }
            }
        }
    }

    tracing::info!(
        "🔵 [OTEL INGEST] Complete: {} accepted, {} rejected for project {}",
        accepted,
        rejected,
        project_id
    );

    Ok((
        StatusCode::CREATED,
        Json(crate::api::IngestResponse {
            accepted,
            rejected,
            errors,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_id_to_u64() {
        // Hex with 0x prefix
        assert_eq!(parse_id_to_u64("0x123"), Some(0x123));

        // Hex with 0x prefix (larger)
        assert_eq!(
            parse_id_to_u64("0x0123456789abcdef"),
            Some(0x0123456789abcdef)
        );

        // Decimal (without 0x, treated as decimal)
        assert_eq!(parse_id_to_u64("12345"), Some(12345));

        // Decimal (even if looks like hex, without 0x it's decimal)
        assert_eq!(parse_id_to_u64("123"), Some(123));

        // Fallback to hash for non-numeric strings
        assert!(parse_id_to_u64("arbitrary-string").is_some());

        // UUID or other string IDs get hashed
        assert!(parse_id_to_u64("550e8400-e29b-41d4-a716-446655440000").is_some());
    }

    #[test]
    fn test_parse_span_name_to_type() {
        assert_eq!(parse_span_name_to_type("llm.request"), SpanType::Planning);
        assert_eq!(
            parse_span_name_to_type("chat.completion"),
            SpanType::Planning
        );
        assert_eq!(parse_span_name_to_type("tool.call"), SpanType::ToolCall);
        assert_eq!(parse_span_name_to_type("function_call"), SpanType::ToolCall);
        assert_eq!(parse_span_name_to_type("root"), SpanType::Root);
        assert_eq!(parse_span_name_to_type("reasoning"), SpanType::Reasoning);
        assert_eq!(parse_span_name_to_type("synthesis"), SpanType::Synthesis);
        assert_eq!(parse_span_name_to_type("response"), SpanType::Response);
        assert_eq!(parse_span_name_to_type("error"), SpanType::Error);
        assert_eq!(
            parse_span_name_to_type("custom_operation"),
            SpanType::Custom
        );
    }

    #[test]
    fn test_convert_span_to_edge() {
        let mut attributes = HashMap::new();
        attributes.insert("tokens".to_string(), "150".to_string());
        attributes.insert("environment".to_string(), "production".to_string());
        attributes.insert("service.name".to_string(), "my-agent".to_string());

        let span = AgentreplaySpan {
            span_id: "0x123".to_string(),
            trace_id: "trace-001".to_string(),
            parent_span_id: None,
            name: "llm.request".to_string(),
            start_time: 1_700_000_000_000_000,
            end_time: Some(1_700_000_001_000_000),
            attributes,
        };

        let edge = convert_span_to_edge(&span).unwrap();

        assert_eq!(edge.edge_id, 0x123);
        assert_eq!(edge.causal_parent, 0);
        assert_eq!(edge.timestamp_us, 1_700_000_000_000_000);
        assert_eq!(edge.duration_us, 1_000_000);
        assert_eq!(edge.token_count, 150);
        assert_eq!(edge.environment, Environment::Production as u8);
        assert_eq!(edge.get_span_type(), SpanType::Planning);
        assert!(edge.verify_checksum());
    }

    #[test]
    fn test_convert_span_invalid_timestamp() {
        let span = AgentreplaySpan {
            span_id: "1".to_string(),
            trace_id: "trace-001".to_string(),
            parent_span_id: None,
            name: "test".to_string(),
            start_time: 100, // Too old (before 2020)
            end_time: None,
            attributes: HashMap::new(),
        };

        assert!(convert_span_to_edge(&span).is_err());
    }
}
