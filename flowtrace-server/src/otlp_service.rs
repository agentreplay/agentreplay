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

//! OTLP gRPC Service for receiving OpenTelemetry traces
//!
//! Implements the standard OTLP/gRPC protocol to accept traces from any
//! OpenTelemetry-instrumented application.

use anyhow::Result;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, warn};

use opentelemetry_proto::tonic::collector::trace::v1::{
    trace_service_server::TraceService, ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use opentelemetry_proto::tonic::common::v1::any_value;

use crate::project_manager::ProjectManager;

/// OTLP trace service implementation
pub struct OtlpTraceService {
    project_manager: Arc<ProjectManager>,
}

impl OtlpTraceService {
    pub fn new(project_manager: Arc<ProjectManager>) -> Self {
        Self { project_manager }
    }

    /// Extract tenant_id and project_id from resource attributes
    fn extract_metadata(&self, request: &ExportTraceServiceRequest) -> (u64, u64) {
        let mut tenant_id = 1u64;
        let mut project_id = 0u64;

        for resource_spans in &request.resource_spans {
            if let Some(resource) = &resource_spans.resource {
                for attr in &resource.attributes {
                    if let Some(value) = &attr.value {
                        match attr.key.as_str() {
                            "tenant.id" | "tenant_id" => match &value.value {
                                Some(any_value::Value::IntValue(v)) => {
                                    tenant_id = *v as u64;
                                }
                                Some(any_value::Value::StringValue(s)) => {
                                    if let Ok(v) = s.parse::<u64>() {
                                        tenant_id = v;
                                    }
                                }
                                _ => {}
                            },
                            "project.id" | "project_id" => match &value.value {
                                Some(any_value::Value::IntValue(v)) => {
                                    project_id = *v as u64;
                                }
                                Some(any_value::Value::StringValue(s)) => {
                                    if let Ok(v) = s.parse::<u64>() {
                                        project_id = v;
                                    }
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                }
            }
        }

        (tenant_id, project_id)
    }

    /// Convert OTLP spans to Flowtrace JSON format
    fn convert_to_flowtrace(
        &self,
        request: &ExportTraceServiceRequest,
    ) -> Result<Vec<serde_json::Value>> {
        let mut spans = Vec::new();

        for resource_spans in &request.resource_spans {
            // Extract resource attributes for context
            let mut resource_attrs = std::collections::HashMap::new();
            if let Some(resource) = &resource_spans.resource {
                for attr in &resource.attributes {
                    if let Some(value) = &attr.value {
                        let val = match &value.value {
                            Some(any_value::Value::StringValue(s)) => serde_json::json!(s),
                            Some(any_value::Value::IntValue(i)) => serde_json::json!(i),
                            Some(any_value::Value::DoubleValue(d)) => serde_json::json!(d),
                            Some(any_value::Value::BoolValue(b)) => serde_json::json!(b),
                            _ => serde_json::json!(null),
                        };
                        resource_attrs.insert(attr.key.clone(), val);
                    }
                }
            }

            // Process each scope (instrumentation library)
            for scope_spans in &resource_spans.scope_spans {
                for span in &scope_spans.spans {
                    // Build attributes map
                    let mut attributes = serde_json::Map::new();

                    // Add resource attributes
                    for (key, value) in &resource_attrs {
                        attributes.insert(key.clone(), value.clone());
                    }

                    // Add span attributes
                    for attr in &span.attributes {
                        if let Some(value) = &attr.value {
                            let val = match &value.value {
                                Some(any_value::Value::StringValue(s)) => serde_json::json!(s),
                                Some(any_value::Value::IntValue(i)) => serde_json::json!(i),
                                Some(any_value::Value::DoubleValue(d)) => serde_json::json!(d),
                                Some(any_value::Value::BoolValue(b)) => serde_json::json!(b),
                                Some(any_value::Value::ArrayValue(arr)) => {
                                    let values: Vec<_> = arr
                                        .values
                                        .iter()
                                        .filter_map(|v| match &v.value {
                                            Some(any_value::Value::StringValue(s)) => {
                                                Some(serde_json::json!(s))
                                            }
                                            Some(any_value::Value::IntValue(i)) => {
                                                Some(serde_json::json!(i))
                                            }
                                            _ => None,
                                        })
                                        .collect();
                                    serde_json::json!(values)
                                }
                                _ => serde_json::json!(null),
                            };
                            attributes.insert(attr.key.clone(), val);
                        }
                    }

                    // Convert span to Flowtrace format
                    let flowtrace_span = serde_json::json!({
                        "span_id": hex::encode(&span.span_id),
                        "trace_id": hex::encode(&span.trace_id),
                        "parent_span_id": if span.parent_span_id.is_empty() {
                            serde_json::Value::Null
                        } else {
                            serde_json::json!(hex::encode(&span.parent_span_id))
                        },
                        "name": span.name,
                        "start_time": span.start_time_unix_nano / 1000, // Convert nanoseconds → microseconds
                        "end_time": span.end_time_unix_nano / 1000,     // Convert nanoseconds → microseconds
                        "attributes": attributes,
                        "events": span.events.iter().map(|e| {
                            let mut event_attrs = serde_json::Map::new();
                            for attr in &e.attributes {
                                if let Some(value) = &attr.value {
                                    if let Some(any_value::Value::StringValue(s)) = &value.value {
                                        event_attrs.insert(attr.key.clone(), serde_json::json!(s));
                                    }
                                }
                            }
                            serde_json::json!({
                                "name": e.name,
                                "timestamp": e.time_unix_nano,
                                "attributes": event_attrs,
                            })
                        }).collect::<Vec<_>>(),
                    });

                    spans.push(flowtrace_span);
                }
            }
        }

        Ok(spans)
    }
}

#[tonic::async_trait]
impl TraceService for OtlpTraceService {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        let otlp_request = request.into_inner();

        // Count spans
        let span_count: usize = otlp_request
            .resource_spans
            .iter()
            .map(|rs| {
                rs.scope_spans
                    .iter()
                    .map(|ss| ss.spans.len())
                    .sum::<usize>()
            })
            .sum();

        debug!("OTLP: Received {} spans", span_count);

        // Debug: Log span relationships
        for rs in &otlp_request.resource_spans {
            for ss in &rs.scope_spans {
                for span in &ss.spans {
                    debug!(
                        "OTLP Span: name='{}' span_id={} parent_id={} trace_id={}",
                        span.name,
                        hex::encode(&span.span_id),
                        hex::encode(&span.parent_span_id),
                        hex::encode(&span.trace_id)
                    );
                }
            }
        }

        // Extract metadata
        let (tenant_id, project_id) = self.extract_metadata(&otlp_request);
        info!(
            "OTLP: Processing {} spans for tenant={}, project={}",
            span_count, tenant_id, project_id
        );

        // Convert to Flowtrace format
        let spans = match self.convert_to_flowtrace(&otlp_request) {
            Ok(spans) => spans,
            Err(e) => {
                error!("OTLP: Failed to convert spans: {}", e);
                return Err(Status::invalid_argument(format!(
                    "Failed to convert spans: {}",
                    e
                )));
            }
        };

        // Get Flowtrace instance for this project
        let flowtrace = match self.project_manager.get_or_open_project(project_id as u16) {
            Ok(ft) => ft,
            Err(e) => {
                error!(
                    "OTLP: Failed to get Flowtrace for tenant={}, project={}: {}",
                    tenant_id, project_id, e
                );
                return Err(Status::internal("Failed to access storage"));
            }
        };

        // Ingest spans using existing converters
        use crate::api::converters::convert_otel_span_to_edge;

        let mut entries = Vec::new();
        for span_json in spans {
            // Parse as OtelSpan
            match serde_json::from_value::<crate::api::converters::OtelSpan>(span_json.clone()) {
                Ok(otel_span) => {
                    match convert_otel_span_to_edge(&otel_span, tenant_id, project_id as u16) {
                        Ok(edge) => {
                            // Construct payload from attributes and events
                            let mut payload_map = serde_json::Map::new();

                            // Add attributes
                            for (k, v) in &otel_span.attributes {
                                payload_map.insert(k.clone(), v.clone());
                            }

                            // Add events if present
                            if !otel_span.events.is_empty() {
                                payload_map.insert(
                                    "events".to_string(),
                                    serde_json::json!(otel_span.events),
                                );
                            }

                            let payload = serde_json::to_vec(&payload_map).unwrap_or_default();
                            entries.push((edge, payload));
                        }
                        Err(e) => {
                            warn!("Failed to convert span: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to parse span: {}", e);
                }
            }
        }

        // Store edges and payloads
        if !entries.is_empty() {
            for (edge, payload) in entries {
                // Insert edge (metadata)
                if let Err(e) = flowtrace.insert(edge).await {
                    warn!("Failed to insert edge: {}", e);
                    continue;
                }

                // Insert payload (attributes + events)
                // Only insert if we have data, though has_payload=1 implies we should always have something
                // even if empty (to distinguish from missing). But empty payload is fine.
                if let Err(e) = flowtrace.put_payload(edge.edge_id, &payload) {
                    warn!("Failed to insert payload for edge {}: {}", edge.edge_id, e);
                }
            }
            info!("OTLP: Successfully stored {} spans", span_count);
        }

        // Return success response
        Ok(Response::new(ExportTraceServiceResponse {
            partial_success: None,
        }))
    }
}

/// Start OTLP gRPC server on port 4317
pub async fn start_otlp_server(project_manager: Arc<ProjectManager>) -> Result<()> {
    use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::TraceServiceServer;

    let addr = "0.0.0.0:4317".parse()?;
    let service = OtlpTraceService::new(project_manager);

    info!("🚀 OTLP gRPC server starting on {}", addr);

    tonic::transport::Server::builder()
        .add_service(TraceServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
