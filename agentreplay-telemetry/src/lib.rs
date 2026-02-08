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

//! Agentreplay telemetry (metrics + tracing + logging).

pub mod economics;

use opentelemetry::{
    metrics::{Counter, Histogram, Meter, MeterProvider},
    KeyValue,
};
use opentelemetry::trace::TracerProvider;
use opentelemetry::Context;
use opentelemetry_otlp::WithExportConfig;
use serde::Serialize;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub use economics::{CostRecord, TokenUsage, WorkflowEconomics};

/// Metrics registry.
pub struct Metrics {
    pub observations_created: Counter<u64>,
    pub observations_queried: Counter<u64>,
    pub query_latency_ms: Histogram<f64>,
    pub tool_events_processed: Counter<u64>,
    pub tool_event_batch_size: Histogram<u64>,
    pub mcp_requests: Counter<u64>,
    pub mcp_request_latency_ms: Histogram<f64>,
    pub active_sessions: opentelemetry::metrics::UpDownCounter<i64>,
}

impl Metrics {
    pub fn new(meter: &Meter) -> Self {
        Self {
            observations_created: meter
                .u64_counter("agentreplay.observations.created")
                .with_description("Total observations created")
                .init(),
            observations_queried: meter
                .u64_counter("agentreplay.observations.queried")
                .with_description("Total observation queries")
                .init(),
            query_latency_ms: meter
                .f64_histogram("agentreplay.query.latency_ms")
                .with_description("Query latency in milliseconds")
                .init(),
            tool_events_processed: meter
                .u64_counter("agentreplay.tool_events.processed")
                .with_description("Total tool events processed")
                .init(),
            tool_event_batch_size: meter
                .u64_histogram("agentreplay.tool_events.batch_size")
                .with_description("Tool event batch sizes")
                .init(),
            mcp_requests: meter
                .u64_counter("agentreplay.mcp.requests")
                .with_description("Total MCP requests")
                .init(),
            mcp_request_latency_ms: meter
                .f64_histogram("agentreplay.mcp.latency_ms")
                .with_description("MCP request latency")
                .init(),
            active_sessions: meter
                .i64_up_down_counter("agentreplay.sessions.active")
                .with_description("Currently active sessions")
                .init(),
        }
    }

    pub fn record_query(&self, project_id: &str, latency_ms: f64, _result_count: usize) {
        self.observations_queried
            .add(1, &[KeyValue::new("project", project_id.to_string())]);
        self.query_latency_ms
            .record(latency_ms, &[KeyValue::new("project", project_id.to_string())]);
    }

    pub fn record_query_with_exemplar(
        &self,
        project_id: &str,
        latency_ms: f64,
        context: Option<&Context>,
    ) {
        let attrs = [KeyValue::new("project", project_id.to_string())];
        self.observations_queried.add(1, &attrs);
        let _ = context;
        self.query_latency_ms.record(latency_ms, &attrs);
    }
}

/// Initialize telemetry (tracing + metrics).
pub fn init_telemetry(service_name: &str, otlp_endpoint: Option<&str>) -> anyhow::Result<Metrics> {
    let tracer_provider = if let Some(endpoint) = otlp_endpoint {
        opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(opentelemetry_otlp::new_exporter().tonic().with_endpoint(endpoint))
            .with_trace_config(
                opentelemetry_sdk::trace::Config::default().with_resource(
                    opentelemetry_sdk::Resource::new(vec![
                        KeyValue::new("service.name", service_name.to_string()),
                    ]),
                ),
            )
            .install_batch(opentelemetry_sdk::runtime::Tokio)?
    } else {
        opentelemetry_sdk::trace::TracerProvider::builder().build()
    };

    let tracer = tracer_provider.tracer(service_name.to_string());

    let meter_provider = if let Some(endpoint) = otlp_endpoint {
        opentelemetry_otlp::new_pipeline()
            .metrics(opentelemetry_sdk::runtime::Tokio)
            .with_exporter(opentelemetry_otlp::new_exporter().tonic().with_endpoint(endpoint))
            .build()?
    } else {
        SdkMeterProvider::default()
    };

    let meter = meter_provider.meter(service_name.to_string());
    let metrics = Metrics::new(&meter);

    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE);

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(fmt_layer)
        .with(otel_layer)
        .init();

    Ok(metrics)
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthStatus {
    pub status: HealthState,
    pub checks: std::collections::HashMap<String, ComponentHealth>,
    pub version: String,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentHealth {
    pub healthy: bool,
    pub message: Option<String>,
    pub latency_ms: Option<u64>,
}

impl ComponentHealth {
    pub fn healthy(latency_ms: u64) -> Self {
        Self {
            healthy: true,
            message: None,
            latency_ms: Some(latency_ms),
        }
    }

    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            healthy: false,
            message: Some(message.into()),
            latency_ms: None,
        }
    }
}

#[macro_export]
macro_rules! instrument_async {
    ($name:expr, $($field:tt)*) => {
        tracing::info_span!($name, $($field)*)
    };
}
