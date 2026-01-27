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

//! GenAI-Specific Instrumentation Helpers
//!
//! Provides ergonomic builders and helpers for instrumenting LLM operations
//! according to OpenTelemetry GenAI semantic conventions.

use crate::genai_conventions::{keys, span_name, Operation, Provider};
use opentelemetry::{
    trace::{Span, SpanKind, Status, Tracer},
    KeyValue,
};
use opentelemetry_sdk::trace::Tracer as SdkTracer;
use std::time::SystemTime;

/// Builder for GenAI operation spans
pub struct GenAISpanBuilder<'a> {
    tracer: &'a SdkTracer,
    operation: Operation,
    model: String,
    provider: Provider,
    attributes: Vec<KeyValue>,
    capture_content: bool,
}

impl<'a> GenAISpanBuilder<'a> {
    pub fn new(tracer: &'a SdkTracer, operation: Operation, model: impl Into<String>) -> Self {
        Self {
            tracer,
            operation,
            model: model.into(),
            provider: Provider::OpenAI, // Default, should be set explicitly
            attributes: Vec::new(),
            capture_content: std::env::var("OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT")
                .map(|v| v == "true")
                .unwrap_or(false),
        }
    }

    /// Set provider (OpenAI, Anthropic, etc.)
    pub fn provider(mut self, provider: Provider) -> Self {
        self.provider = provider;
        self
    }

    /// Set request parameters
    pub fn temperature(mut self, temp: f64) -> Self {
        self.attributes.push(KeyValue::new(keys::TEMPERATURE, temp));
        self
    }

    pub fn top_p(mut self, top_p: f64) -> Self {
        self.attributes.push(KeyValue::new(keys::TOP_P, top_p));
        self
    }

    pub fn max_tokens(mut self, max_tokens: i64) -> Self {
        self.attributes
            .push(KeyValue::new(keys::MAX_TOKENS, max_tokens));
        self
    }

    pub fn seed(mut self, seed: i64) -> Self {
        self.attributes.push(KeyValue::new(keys::SEED, seed));
        self
    }

    /// Set custom attribute
    pub fn attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<opentelemetry::Value>,
    ) -> Self {
        self.attributes
            .push(KeyValue::new(key.into(), value.into()));
        self
    }

    /// Start the span
    pub fn start(self) -> GenAISpan {
        let mut builder = self
            .tracer
            .span_builder(span_name(self.operation, &self.model))
            .with_kind(SpanKind::Client)
            .with_start_time(SystemTime::now());

        // Add required attributes
        let mut attrs = vec![
            KeyValue::new(keys::OPERATION_NAME, self.operation.as_str()),
            KeyValue::new(keys::REQUEST_MODEL, self.model.clone()),
            KeyValue::new(keys::PROVIDER_NAME, self.provider.as_str()),
        ];
        attrs.extend(self.attributes);

        builder = builder.with_attributes(attrs);
        let span = builder.start(self.tracer);

        GenAISpan {
            span,
            capture_content: self.capture_content,
        }
    }
}

/// Active GenAI span with helper methods
pub struct GenAISpan {
    span: opentelemetry_sdk::trace::Span,
    capture_content: bool,
}

impl GenAISpan {
    /// Record token usage
    pub fn record_usage(&mut self, input_tokens: u32, output_tokens: u32) {
        self.span
            .set_attribute(KeyValue::new(keys::INPUT_TOKENS, input_tokens as i64));
        self.span
            .set_attribute(KeyValue::new(keys::OUTPUT_TOKENS, output_tokens as i64));
        self.span.set_attribute(KeyValue::new(
            keys::TOTAL_TOKENS,
            (input_tokens + output_tokens) as i64,
        ));
    }

    /// Record cost (requires model pricing config)
    pub fn record_cost(&mut self, cost_usd: f64) {
        self.span
            .set_attribute(KeyValue::new(keys::COST_USD, cost_usd));
    }

    /// Record response metadata
    pub fn record_response(
        &mut self,
        response_id: impl Into<String>,
        finish_reason: impl Into<String>,
    ) {
        self.span
            .set_attribute(KeyValue::new(keys::RESPONSE_ID, response_id.into()));
        self.span
            .set_attribute(KeyValue::new(keys::FINISH_REASONS, finish_reason.into()));
    }

    /// Record input prompt (only if capture_content=true)
    pub fn record_input(&mut self, messages: &str) {
        if self.capture_content {
            // Add as event to avoid span bloat
            self.span.add_event(
                "gen_ai.content.prompt",
                vec![KeyValue::new("input", messages.to_string())],
            );
        }
    }

    /// Record output completion (only if capture_content=true)
    pub fn record_output(&mut self, completion: &str) {
        if self.capture_content {
            self.span.add_event(
                "gen_ai.content.completion",
                vec![KeyValue::new("output", completion.to_string())],
            );
        }
    }

    /// Record error
    pub fn record_error(&mut self, error: &dyn std::error::Error) {
        self.span.record_error(error);
        self.span.set_status(Status::error(error.to_string()));
    }

    /// Set attribute
    pub fn set_attribute(&mut self, key: &'static str, value: impl Into<opentelemetry::Value>) {
        self.span.set_attribute(KeyValue::new(key, value.into()));
    }

    /// End span
    pub fn end(mut self) {
        self.span.end();
    }
}

/// Convenience function: Instrument an async LLM call
pub async fn instrument_llm_call<F, T, E>(
    tracer: &SdkTracer,
    operation: Operation,
    model: &str,
    provider: Provider,
    f: F,
) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
    E: std::error::Error,
{
    let mut span = GenAISpanBuilder::new(tracer, operation, model)
        .provider(provider)
        .start();

    match f.await {
        Ok(result) => {
            span.end();
            Ok(result)
        }
        Err(err) => {
            span.record_error(&err);
            span.end();
            Err(err)
        }
    }
}
