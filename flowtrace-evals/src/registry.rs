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

//! Evaluator registry and execution engine

use crate::cache::EvalCache;
use crate::{EvalConfig, EvalError, EvalResult, Evaluator, MetricValue, TraceContext};
use flowtrace_core::{
    GraderPolicyV2, GraderResult, GraderSpecV2, GraderThresholdV2, OverallResult, TaskDefinitionV2,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, error, info};

/// Registry for managing and executing evaluators
pub struct EvaluatorRegistry {
    evaluators: Arc<RwLock<HashMap<String, Arc<dyn Evaluator>>>>,
    config: EvalConfig,
    cache: Option<Arc<EvalCache>>,
}

#[derive(Debug, Clone)]
pub struct TaskEvaluationOutput {
    pub grader_results: Vec<GraderResult>,
    pub overall: OverallResult,
}

impl EvaluatorRegistry {
    /// Create a new registry with default configuration
    pub fn new() -> Self {
        Self::with_config(EvalConfig::default())
    }

    /// Create a new registry with custom configuration
    pub fn with_config(config: EvalConfig) -> Self {
        let cache = if config.enable_cache {
            Some(Arc::new(EvalCache::new(config.cache_ttl_secs)))
        } else {
            None
        };

        Self {
            evaluators: Arc::new(RwLock::new(HashMap::new())),
            config,
            cache,
        }
    }

    /// Register a new evaluator
    pub fn register(&self, evaluator: Arc<dyn Evaluator>) -> Result<(), RegistryError> {
        let id = evaluator.id().to_string();
        let mut evaluators = self.evaluators.write();

        if evaluators.contains_key(&id) {
            return Err(RegistryError::DuplicateId(id));
        }

        info!(
            "Registering evaluator: {} ({})",
            evaluator.metadata().name,
            id
        );
        evaluators.insert(id, evaluator);
        Ok(())
    }

    /// Unregister an evaluator by ID
    pub fn unregister(&self, evaluator_id: &str) -> Result<(), RegistryError> {
        let mut evaluators = self.evaluators.write();
        evaluators
            .remove(evaluator_id)
            .ok_or(RegistryError::NotFound(evaluator_id.to_string()))?;

        info!("Unregistered evaluator: {}", evaluator_id);
        Ok(())
    }

    /// Get an evaluator by ID
    pub fn get(&self, evaluator_id: &str) -> Option<Arc<dyn Evaluator>> {
        let evaluators = self.evaluators.read();
        evaluators.get(evaluator_id).cloned()
    }

    /// List all registered evaluator IDs
    pub fn list_evaluators(&self) -> Vec<String> {
        let evaluators = self.evaluators.read();
        evaluators.keys().cloned().collect()
    }

    /// Evaluate a single trace with specified evaluators
    pub async fn evaluate_trace(
        &self,
        trace: &TraceContext,
        evaluator_ids: Vec<String>,
    ) -> Result<HashMap<String, EvalResult>, RegistryError> {
        // Check cache first
        if let Some(cache) = &self.cache {
            let cache_key = cache.compute_key(trace, &evaluator_ids);
            if let Some(cached) = cache.get(&cache_key).await {
                debug!("Cache hit for trace {}", trace.trace_id);
                return Ok(cached);
            }
        }

        // Clone evaluators we need before entering async context
        let evaluators_to_run: Vec<(String, Arc<dyn Evaluator>)> = {
            let evaluators = self.evaluators.read();
            evaluator_ids
                .iter()
                .filter_map(|id| evaluators.get(id).map(|e| (id.clone(), Arc::clone(e))))
                .collect()
        }; // RwLockReadGuard is dropped here

        // Collect evaluators to run
        let mut tasks = Vec::new();
        for (id, evaluator) in evaluators_to_run {
            let trace = trace.clone();
            let timeout = Duration::from_secs(self.config.timeout_secs);

            tasks.push(tokio::spawn(async move {
                let start = Instant::now();

                let result = tokio::time::timeout(timeout, evaluator.evaluate(&trace))
                    .await
                    .map_err(|_| EvalError::Timeout)?;

                let mut eval_result = result?;
                eval_result.duration_ms = Some(start.elapsed().as_millis() as u64);

                Ok::<_, EvalError>((id, eval_result))
            }));
        }

        // Execute all evaluators in parallel
        let mut results = HashMap::new();
        for task in tasks {
            match task.await {
                Ok(Ok((id, result))) => {
                    results.insert(id, result);
                }
                Ok(Err(e)) => {
                    error!("Evaluation failed: {}", e);
                    // Continue with other evaluators
                }
                Err(e) => {
                    error!("Task panicked: {}", e);
                }
            }
        }

        // Cache successful results
        if let Some(cache) = &self.cache {
            if !results.is_empty() {
                let cache_key = cache.compute_key(trace, &evaluator_ids);
                cache.set(cache_key, results.clone()).await;
            }
        }

        Ok(results)
    }

    /// Evaluate multiple traces in batch
    pub async fn evaluate_batch(
        &self,
        traces: Vec<TraceContext>,
        evaluator_ids: Vec<String>,
    ) -> Vec<Result<HashMap<String, EvalResult>, RegistryError>> {
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent));
        let mut tasks = Vec::new();

        for trace in traces {
            let evaluator_ids = evaluator_ids.clone();
            let semaphore = Arc::clone(&semaphore);
            let registry = self.clone_ref();

            tasks.push(tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                registry.evaluate_trace(&trace, evaluator_ids).await
            }));
        }

        let mut batch_results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(result) => batch_results.push(result),
                Err(e) => batch_results.push(Err(RegistryError::TaskPanic(e.to_string()))),
            }
        }

        batch_results
    }

    /// Evaluate a trace using TaskDefinitionV2 graders and policy.
    pub async fn evaluate_task_definition(
        &self,
        trace: &TraceContext,
        task_definition: &TaskDefinitionV2,
    ) -> Result<TaskEvaluationOutput, RegistryError> {
        if task_definition.graders.is_empty() {
            return Err(RegistryError::MissingGraders);
        }

        let grader_ids: Vec<String> = task_definition
            .graders
            .iter()
            .map(|g| g.grader_id.clone())
            .collect();

        let eval_results = self.evaluate_trace(trace, grader_ids).await?;
        let grader_results: Vec<GraderResult> = task_definition
            .graders
            .iter()
            .filter_map(|spec| eval_results.get(&spec.grader_id).map(|r| (spec, r)))
            .map(|(spec, eval_result)| self.to_grader_result(spec, eval_result))
            .collect();

        let policy = task_definition
            .graders
            .iter()
            .filter_map(|g| g.policy.clone())
            .next()
            .unwrap_or_else(default_grader_policy);

        let overall = self.apply_grader_policy(&policy, &grader_results);

        Ok(TaskEvaluationOutput {
            grader_results,
            overall,
        })
    }

    /// Helper to clone Arc reference for async tasks
    fn clone_ref(&self) -> RegistryRef {
        RegistryRef {
            evaluators: Arc::clone(&self.evaluators),
            config: self.config.clone(),
            cache: self.cache.clone(),
        }
    }

    fn to_grader_result(&self, spec: &GraderSpecV2, eval_result: &EvalResult) -> GraderResult {
        let score = extract_primary_score(&eval_result.metrics);
        let passed = if spec.thresholds.is_empty() {
            eval_result.passed
        } else {
            apply_thresholds(&spec.thresholds, &eval_result.metrics)
        };

        GraderResult {
            grader_id: spec.grader_id.clone(),
            grader_type: spec
                .grader_type
                .clone()
                .or_else(|| eval_result.evaluator_type.clone())
                .unwrap_or_else(|| "grader".to_string()),
            weight: spec.weight,
            score,
            passed,
            assertions: eval_result.assertions.clone(),
            judge_votes: eval_result.judge_votes.clone(),
            rationale: eval_result.explanation.clone(),
            evidence_refs: eval_result.evidence_refs.clone(),
        }
    }

    fn apply_grader_policy(&self, policy: &GraderPolicyV2, results: &[GraderResult]) -> OverallResult {
        let pass_count = results.iter().filter(|r| r.passed).count();
        let total = results.len().max(1);
        let quorum = policy
            .quorum
            .map(|q| q as usize)
            .unwrap_or_else(|| (total + 1) / 2);

        let mut passed = match policy.mode.as_str() {
            "all" => pass_count == total,
            "any" => pass_count > 0,
            "quorum" => pass_count >= quorum,
            "weighted" | "score" => false,
            _ => pass_count >= quorum,
        };

        let composite_score = aggregate_scores(results, policy.aggregation.as_deref());
        if matches!(policy.mode.as_str(), "weighted" | "score") {
            let threshold = policy.threshold.unwrap_or(0.5);
            passed = composite_score.map(|s| s >= threshold).unwrap_or(false);
        }

        if policy.mode == "quorum" && pass_count * 2 == total {
            if let Some(tie) = policy.tie_breaker.as_deref() {
                passed = match tie {
                    "pass" => true,
                    "fail" => false,
                    _ => passed,
                };
            }
        }

        OverallResult {
            policy: policy.mode.clone(),
            passed,
            composite_score,
        }
    }

    /// Get statistics about the registry
    pub fn stats(&self) -> RegistryStats {
        let evaluators = self.evaluators.read();
        let cache_stats = self.cache.as_ref().map(|c| c.stats());

        RegistryStats {
            num_evaluators: evaluators.len(),
            evaluator_ids: evaluators.keys().cloned().collect(),
            cache_enabled: self.cache.is_some(),
            cache_hit_rate: cache_stats.map(|s| s.hit_rate),
        }
    }
}

impl Default for EvaluatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn default_grader_policy() -> GraderPolicyV2 {
    GraderPolicyV2 {
        mode: "quorum".to_string(),
        quorum: None,
        aggregation: None,
        threshold: None,
        tie_breaker: None,
    }
}

fn extract_primary_score(metrics: &HashMap<String, MetricValue>) -> Option<f64> {
    for key in ["score", "overall_score", "accuracy", "completion_score"] {
        if let Some(value) = metrics.get(key) {
            if let Some(v) = metric_value_to_f64(value) {
                return Some(v);
            }
        }
    }
    None
}

fn apply_thresholds(
    thresholds: &[GraderThresholdV2],
    metrics: &HashMap<String, MetricValue>,
) -> bool {
    thresholds.iter().all(|threshold| {
        let metric_value = metrics.get(&threshold.metric).and_then(metric_value_to_f64);
        match metric_value {
            Some(value) => compare_threshold(value, threshold),
            None => false,
        }
    })
}

fn compare_threshold(value: f64, threshold: &GraderThresholdV2) -> bool {
    match threshold.operator.as_str() {
        "gt" => value > threshold.value,
        "gte" => value >= threshold.value,
        "lt" => value < threshold.value,
        "lte" => value <= threshold.value,
        "eq" => (value - threshold.value).abs() < f64::EPSILON,
        "ne" => (value - threshold.value).abs() > f64::EPSILON,
        _ => false,
    }
}

fn metric_value_to_f64(value: &MetricValue) -> Option<f64> {
    match value {
        MetricValue::Float(f) => Some(*f),
        MetricValue::Int(i) => Some(*i as f64),
        MetricValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        MetricValue::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

fn aggregate_scores(results: &[GraderResult], aggregation: Option<&str>) -> Option<f64> {
    let mut scores: Vec<f64> = results.iter().filter_map(|r| r.score).collect();
    if scores.is_empty() {
        return None;
    }

    match aggregation.unwrap_or("median") {
        "weighted_mean" => {
            let mut total_weight = 0.0;
            let mut weighted_sum = 0.0;
            for result in results.iter().filter(|r| r.score.is_some()) {
                let weight = result.weight.unwrap_or(1.0);
                let score = result.score.unwrap_or(0.0);
                total_weight += weight;
                weighted_sum += weight * score;
            }
            if total_weight > 0.0 {
                Some(weighted_sum / total_weight)
            } else {
                None
            }
        }
        "trimmed_mean" => {
            scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let trim = (scores.len() as f64 * 0.1).floor() as usize;
            let slice = scores
                .iter()
                .skip(trim)
                .take(scores.len().saturating_sub(trim * 2));
            let count = slice.len().max(1) as f64;
            let sum: f64 = slice.cloned().sum();
            Some(sum / count)
        }
        _ => {
            scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mid = scores.len() / 2;
            let median = if scores.len() % 2 == 0 {
                (scores[mid - 1] + scores[mid]) / 2.0
            } else {
                scores[mid]
            };
            Some(median)
        }
    }
}

/// Clone-able reference to registry for async tasks
#[derive(Clone)]
struct RegistryRef {
    evaluators: Arc<RwLock<HashMap<String, Arc<dyn Evaluator>>>>,
    #[allow(dead_code)]
    config: EvalConfig,
    cache: Option<Arc<EvalCache>>,
}

impl RegistryRef {
    async fn evaluate_trace(
        &self,
        trace: &TraceContext,
        evaluator_ids: Vec<String>,
    ) -> Result<HashMap<String, EvalResult>, RegistryError> {
        // Check cache
        if let Some(cache) = &self.cache {
            let cache_key = cache.compute_key(trace, &evaluator_ids);
            if let Some(cached) = cache.get(&cache_key).await {
                return Ok(cached);
            }
        }

        // Clone evaluators to avoid holding lock across await
        let evaluators_to_run: Vec<(String, Arc<dyn Evaluator>)> = {
            let evaluators = self.evaluators.read();
            evaluator_ids
                .iter()
                .filter_map(|id| evaluators.get(id).map(|e| (id.clone(), Arc::clone(e))))
                .collect()
        };

        let mut results = HashMap::new();

        for (id, evaluator) in evaluators_to_run {
            match evaluator.evaluate(trace).await {
                Ok(result) => {
                    results.insert(id.clone(), result);
                }
                Err(e) => {
                    error!("Evaluator {} failed: {}", id, e);
                }
            }
        }

        // Cache results
        if let Some(cache) = &self.cache {
            if !results.is_empty() {
                let cache_key = cache.compute_key(trace, &evaluator_ids);
                cache.set(cache_key, results.clone()).await;
            }
        }

        Ok(results)
    }
}

/// Statistics about the registry
#[derive(Debug, Clone)]
pub struct RegistryStats {
    pub num_evaluators: usize,
    pub evaluator_ids: Vec<String>,
    pub cache_enabled: bool,
    pub cache_hit_rate: Option<f64>,
}

/// Errors that can occur in the registry
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Duplicate evaluator ID: {0}")]
    DuplicateId(String),

    #[error("Task definition has no graders")]
    MissingGraders,

    #[error("Evaluator not found: {0}")]
    NotFound(String),

    #[error("Evaluation error: {0}")]
    EvalError(#[from] EvalError),

    #[error("Task panicked: {0}")]
    TaskPanic(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EvaluatorMetadata, MetricValue};

    struct TestEvaluator;

    #[async_trait::async_trait]
    impl Evaluator for TestEvaluator {
        fn id(&self) -> &str {
            "test_v1"
        }

        async fn evaluate(&self, _trace: &TraceContext) -> Result<EvalResult, EvalError> {
            Ok(EvalResult {
                evaluator_id: self.id().to_string(),
                evaluator_type: Some("test".to_string()),
                metrics: HashMap::from([("score".to_string(), MetricValue::Float(0.95))]),
                passed: true,
                explanation: Some("Test passed".to_string()),
                assertions: Vec::new(),
                judge_votes: Vec::new(),
                evidence_refs: Vec::new(),
                confidence: 1.0,
                cost: Some(0.0001),
                duration_ms: None,
                actionable_feedback: None,
            })
        }

        fn metadata(&self) -> EvaluatorMetadata {
            EvaluatorMetadata {
                name: "Test Evaluator".to_string(),
                version: "1.0.0".to_string(),
                description: "For testing".to_string(),
                cost_per_eval: Some(0.0001),
                avg_latency_ms: Some(100),
                tags: vec!["test".to_string()],
                author: None,
            }
        }
    }

    #[tokio::test]
    async fn test_registry_register() {
        let registry = EvaluatorRegistry::new();
        let evaluator = Arc::new(TestEvaluator);

        assert!(registry.register(evaluator).is_ok());
        assert!(registry.get("test_v1").is_some());
    }

    #[tokio::test]
    async fn test_registry_duplicate() {
        let registry = EvaluatorRegistry::new();
        let evaluator = Arc::new(TestEvaluator);

        assert!(registry.register(evaluator.clone()).is_ok());
        assert!(registry.register(evaluator).is_err());
    }

    #[tokio::test]
    async fn test_evaluate_trace() {
        let registry = EvaluatorRegistry::new();
        registry.register(Arc::new(TestEvaluator)).unwrap();

        let trace = TraceContext {
            trace_id: 1,
            edges: vec![],
            input: Some("test input".to_string()),
            output: Some("test output".to_string()),
            context: None,
            metadata: HashMap::new(),
            eval_trace: None,
            timestamp_us: 0,
        };

        let results = registry
            .evaluate_trace(&trace, vec!["test_v1".to_string()])
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results.contains_key("test_v1"));
        assert!(results["test_v1"].passed);
    }
}
