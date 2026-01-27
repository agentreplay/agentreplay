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

// flowtrace-core/src/eval.rs
//
// Evaluation metrics storage for LLM traces.
// Provides flexible key-value storage for eval results from various frameworks
// (RAGAS, DeepEval, custom evaluators, etc.)

use std::fmt;

/// Fixed-size evaluation metric entry (96 bytes, cache-friendly)
///
/// Stores a single evaluation metric for a trace/edge, allowing multiple
/// metrics and multiple evaluators per trace.
///
/// # Design Decisions
/// - Fixed-size struct for efficient storage in SSTables
/// - edge_id links to AgentFlowEdge
/// - metric_name: up to 31 chars (e.g., "accuracy", "hallucination")
/// - evaluator: up to 31 chars (e.g., "ragas", "deepeval", "custom")
/// - Supports async evaluation (can be added after trace creation)
///
/// # Example
/// ```
/// use flowtrace_core::eval::EvalMetric;
///
/// let metric = EvalMetric::new(
///     0x1234567890abcdef,
///     "accuracy",
///     0.95,
///     "ragas",
///     1234567890000000,
/// ).unwrap();
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvalMetric {
    /// The edge/trace this metric belongs to
    pub edge_id: u128,

    /// Metric name (e.g., "accuracy", "hallucination", "relevance")
    /// Max 31 chars + null terminator
    pub metric_name: [u8; 32],

    /// Metric value (typically 0.0-1.0, but can be any f64)
    pub metric_value: f64,

    /// Evaluator that produced this metric (e.g., "ragas", "deepeval")
    /// Max 31 chars + null terminator
    pub evaluator: [u8; 32],

    /// When this metric was computed (microseconds since Unix epoch)
    pub timestamp_us: u64,
}

// Compile-time size check
const _: () = assert!(std::mem::size_of::<EvalMetric>() == 96);

impl EvalMetric {
    /// Create a new evaluation metric
    ///
    /// # Arguments
    /// - `edge_id`: The trace/edge this metric belongs to
    /// - `metric_name`: Name of the metric (max 31 chars)
    /// - `metric_value`: Numeric value of the metric
    /// - `evaluator`: Name of the evaluator (max 31 chars)
    /// - `timestamp_us`: When the metric was computed
    ///
    /// # Errors
    /// Returns `None` if metric_name or evaluator exceed 31 characters
    pub fn new(
        edge_id: u128,
        metric_name: &str,
        metric_value: f64,
        evaluator: &str,
        timestamp_us: u64,
    ) -> Option<Self> {
        if metric_name.len() > 31 || evaluator.len() > 31 {
            return None;
        }

        let mut metric_name_bytes = [0u8; 32];
        metric_name_bytes[..metric_name.len()].copy_from_slice(metric_name.as_bytes());

        let mut evaluator_bytes = [0u8; 32];
        evaluator_bytes[..evaluator.len()].copy_from_slice(evaluator.as_bytes());

        Some(Self {
            edge_id,
            metric_name: metric_name_bytes,
            metric_value,
            evaluator: evaluator_bytes,
            timestamp_us,
        })
    }

    /// Get the metric name as a string
    pub fn get_metric_name(&self) -> &str {
        let len = self.metric_name.iter().position(|&c| c == 0).unwrap_or(32);
        std::str::from_utf8(&self.metric_name[..len]).unwrap_or("")
    }

    /// Get the evaluator name as a string
    pub fn get_evaluator(&self) -> &str {
        let len = self.evaluator.iter().position(|&c| c == 0).unwrap_or(32);
        std::str::from_utf8(&self.evaluator[..len]).unwrap_or("")
    }

    /// Convert to bytes for storage
    pub fn to_bytes(&self) -> [u8; 96] {
        unsafe { std::mem::transmute(*self) }
    }

    /// Create from bytes (storage format)
    pub fn from_bytes(bytes: &[u8; 96]) -> Self {
        unsafe { std::mem::transmute(*bytes) }
    }
}

impl fmt::Display for EvalMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EvalMetric(edge={:x}, {}={:.4}, evaluator={})",
            self.edge_id,
            self.get_metric_name(),
            self.metric_value,
            self.get_evaluator()
        )
    }
}

/// Common evaluation metric names (for convenience)
pub mod metrics {
    pub const ACCURACY: &str = "accuracy";
    pub const HALLUCINATION: &str = "hallucination";
    pub const RELEVANCE: &str = "relevance";
    pub const TOXICITY: &str = "toxicity";
    pub const GROUNDEDNESS: &str = "groundedness";
    pub const CONTEXT_PRECISION: &str = "context_precision";
    pub const CONTEXT_RECALL: &str = "context_recall";
    pub const FAITHFULNESS: &str = "faithfulness";
    pub const ANSWER_SIMILARITY: &str = "answer_similarity";
    pub const BIAS: &str = "bias";
    pub const COHERENCE: &str = "coherence";
    pub const CORRECTNESS: &str = "correctness";
}

/// Common evaluator names (for convenience)
pub mod evaluators {
    pub const RAGAS: &str = "ragas";
    pub const DEEPEVAL: &str = "deepeval";
    pub const LANGSMITH: &str = "langsmith";
    pub const CUSTOM: &str = "custom";
    pub const MANUAL: &str = "manual";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_metric_creation() {
        let metric = EvalMetric::new(
            0x1234567890abcdef,
            "accuracy",
            0.95,
            "ragas",
            1234567890000000,
        )
        .unwrap();

        assert_eq!(metric.edge_id, 0x1234567890abcdef);
        assert_eq!(metric.get_metric_name(), "accuracy");
        assert_eq!(metric.metric_value, 0.95);
        assert_eq!(metric.get_evaluator(), "ragas");
        assert_eq!(metric.timestamp_us, 1234567890000000);
    }

    #[test]
    fn test_eval_metric_long_names() {
        // Should reject names > 31 chars
        let result = EvalMetric::new(
            0x123,
            "this_is_a_very_long_metric_name_that_exceeds_limit",
            0.5,
            "ragas",
            0,
        );
        assert!(result.is_none());

        let result = EvalMetric::new(
            0x123,
            "accuracy",
            0.5,
            "this_is_a_very_long_evaluator_name_that_exceeds_limit",
            0,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_eval_metric_serialization() {
        let metric = EvalMetric::new(
            0x1234567890abcdef,
            "hallucination",
            0.02,
            "deepeval",
            1234567890000000,
        )
        .unwrap();

        let bytes = metric.to_bytes();
        let deserialized = EvalMetric::from_bytes(&bytes);

        assert_eq!(metric, deserialized);
        assert_eq!(deserialized.get_metric_name(), "hallucination");
        assert_eq!(deserialized.metric_value, 0.02);
    }

    #[test]
    fn test_eval_metric_size() {
        assert_eq!(std::mem::size_of::<EvalMetric>(), 96);
    }

    #[test]
    fn test_common_metrics() {
        let metric = EvalMetric::new(0x123, metrics::ACCURACY, 0.95, evaluators::RAGAS, 0).unwrap();

        assert_eq!(metric.get_metric_name(), "accuracy");
        assert_eq!(metric.get_evaluator(), "ragas");
    }
}
