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

//! OpenTelemetry Configuration
//!
//! Supports standard OTEL environment variables for zero-config deployment.

use std::env;

/// Observability configuration with OTEL standard env vars
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    // OTEL standard env vars
    pub otel_sdk_disabled: bool,
    pub otel_service_name: String,
    pub otel_exporter_otlp_endpoint: String,
    pub otel_exporter_otlp_protocol: Protocol,

    // GenAI-specific
    pub capture_message_content: bool,
    pub sampling_rate: f64,

    // Agentreplay custom
    pub agentreplay_endpoint: Option<String>,
    pub enable_dual_export: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum Protocol {
    Grpc,
    HttpProtobuf,
    HttpJson,
}

impl ObservabilityConfig {
    pub fn from_env() -> Self {
        Self {
            otel_sdk_disabled: env::var("OTEL_SDK_DISABLED")
                .map(|v| v == "true")
                .unwrap_or(false),

            otel_service_name: env::var("OTEL_SERVICE_NAME")
                .unwrap_or_else(|_| "agentreplay".to_string()),

            otel_exporter_otlp_endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:47117".to_string()),

            otel_exporter_otlp_protocol: match env::var("OTEL_EXPORTER_OTLP_PROTOCOL")
                .unwrap_or_else(|_| "grpc".to_string())
                .as_str()
            {
                "http/protobuf" => Protocol::HttpProtobuf,
                "http/json" => Protocol::HttpJson,
                _ => Protocol::Grpc,
            },

            // GenAI-specific (document specifies this exact var name)
            capture_message_content: env::var("OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT")
                .map(|v| v == "true")
                .unwrap_or(false),

            sampling_rate: env::var("OTEL_TRACES_SAMPLER_ARG")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.1), // 10% default

            // Custom Agentreplay config
            agentreplay_endpoint: env::var("AGENTREPLAY_ENDPOINT").ok(),

            enable_dual_export: env::var("AGENTREPLAY_DUAL_EXPORT")
                .map(|v| v == "true")
                .unwrap_or(false),
        }
    }
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            otel_sdk_disabled: false,
            otel_service_name: "agentreplay".to_string(),
            otel_exporter_otlp_endpoint: "http://localhost:47117".to_string(),
            otel_exporter_otlp_protocol: Protocol::Grpc,
            capture_message_content: false,
            sampling_rate: 0.1,
            agentreplay_endpoint: None,
            enable_dual_export: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ObservabilityConfig::default();
        assert_eq!(config.otel_service_name, "agentreplay");
        assert!(!config.otel_sdk_disabled);
        assert!(!config.capture_message_content);
    }

    #[test]
    fn test_from_env() {
        env::set_var("OTEL_SERVICE_NAME", "test-service");
        let config = ObservabilityConfig::from_env();
        assert_eq!(config.otel_service_name, "test-service");
        env::remove_var("OTEL_SERVICE_NAME");
    }
}
