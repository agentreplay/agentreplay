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

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

// Mock dependencies (replace with real ones)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: u128,
    // Add other fields
    pub metrics: HashMap<String, MetricValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Float(f64),
    Int(i64),
    String(String),
}

#[derive(Error, Debug)]
pub enum EvalError {
    #[error("Timeout")]
    Timeout(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineEvalConfig {
    pub sampling_rate: f64,    // 0.0 to 1.0
    pub async_mode: bool,      // Run in background without blocking
    pub max_concurrent: usize, // Limit concurrent evals
    pub timeout_secs: u64,
    pub alert_thresholds: HashMap<String, AlertThreshold>,
    pub enable_drift_detection: bool,
    pub drift_window_hours: u64, // Compare to last N hours

    // Scheduling configuration
    pub schedule: Option<EvalSchedule>,
}

/// Configuration for scheduled/automated eval runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSchedule {
    /// Cron expression for scheduling (e.g., "0 0 * * *" for daily at midnight)
    pub cron_expression: Option<String>,

    /// Interval in seconds between runs (alternative to cron)
    pub interval_secs: Option<u64>,

    /// Whether scheduled runs are enabled
    pub enabled: bool,

    /// Dataset ID to run evaluations against
    pub dataset_id: Option<u128>,

    /// Evaluator IDs to run
    pub evaluator_ids: Vec<String>,

    /// Webhook URL to call on completion
    pub webhook_url: Option<String>,

    /// Trigger conditions
    pub triggers: Vec<EvalTrigger>,

    /// Maximum runs per day (budget control)
    pub max_runs_per_day: Option<usize>,

    /// Timezone for cron interpretation (default: UTC)
    pub timezone: String,
}

impl Default for EvalSchedule {
    fn default() -> Self {
        Self {
            cron_expression: None,
            interval_secs: None,
            enabled: false,
            dataset_id: None,
            evaluator_ids: vec![],
            webhook_url: None,
            triggers: vec![],
            max_runs_per_day: None,
            timezone: "UTC".to_string(),
        }
    }
}

/// Trigger conditions for automated eval runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvalTrigger {
    /// Trigger on deployment event
    Deployment { environment: String },

    /// Trigger on webhook/API call
    Webhook { secret: String },

    /// Trigger on new data threshold
    DataThreshold { min_new_traces: usize },

    /// Trigger on metric drift detection
    MetricDrift { metric: String, threshold: f64 },

    /// Trigger on PR merge (CI/CD integration)
    PullRequestMerge { branch: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThreshold {
    pub metric: String,
    pub operator: ComparisonOperator, // GT, LT, GTE, LTE
    pub value: f64,
    pub channels: Vec<AlertChannel>, // Slack, Email, PagerDuty
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    GT,
    LT,
    GTE,
    LTE,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertChannel {
    Slack { webhook_url: String },
    Email { recipients: Vec<String> },
    PagerDuty { integration_key: String },
    Webhook { url: String },
}

#[derive(Debug, Clone)]
pub struct EvalTask {
    pub trace_id: u128,
    pub trace: TraceContext,
    pub priority: TaskPriority,
    pub submitted_at: u64,
}

#[derive(Debug, Clone)]
pub enum TaskPriority {
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone)]
pub struct EvalResult {
    pub metrics: HashMap<String, MetricValue>,
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub trace_id: u128,
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub severity: Severity,
    pub channels: Vec<AlertChannel>,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

// Traits
#[async_trait::async_trait]
pub trait Evaluator: Send + Sync {
    fn id(&self) -> String;
    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult>;
}

pub struct OnlineEvaluator {
    evaluators: Vec<Arc<dyn Evaluator>>,
    config: OnlineEvalConfig,
    alert_manager: Arc<AlertManager>,
    metrics_collector: Arc<MetricsCollector>,
    eval_queue: Arc<Mutex<VecDeque<EvalTask>>>,
    scheduler: Arc<EvalScheduler>,
}

/// Scheduler for automated evaluation runs
pub struct EvalScheduler {
    schedule: Option<EvalSchedule>,
    last_run: RwLock<Option<u64>>,
    runs_today: RwLock<usize>,
    last_reset_day: RwLock<u64>,
    shutdown: RwLock<bool>,
}

impl EvalScheduler {
    pub fn new(schedule: Option<EvalSchedule>) -> Self {
        Self {
            schedule,
            last_run: RwLock::new(None),
            runs_today: RwLock::new(0),
            last_reset_day: RwLock::new(current_day()),
            shutdown: RwLock::new(false),
        }
    }

    /// Check if it's time to run a scheduled evaluation
    pub fn should_run_now(&self) -> bool {
        let schedule = match &self.schedule {
            Some(s) if s.enabled => s,
            _ => return false,
        };

        // Reset daily counter if needed
        let today = current_day();
        {
            let last_reset = *self.last_reset_day.read();
            if today > last_reset {
                *self.runs_today.write() = 0;
                *self.last_reset_day.write() = today;
            }
        }

        // Check daily limit
        if let Some(max_runs) = schedule.max_runs_per_day {
            if *self.runs_today.read() >= max_runs {
                return false;
            }
        }

        let now = current_timestamp();
        let last_run = *self.last_run.read();

        // Check interval-based scheduling
        if let Some(interval_secs) = schedule.interval_secs {
            let interval_us = interval_secs * 1_000_000;
            if let Some(last) = last_run {
                if now < last + interval_us {
                    return false;
                }
            }
            return true;
        }

        // Check cron-based scheduling
        if let Some(ref cron_expr) = schedule.cron_expression {
            return self.cron_matches_now(cron_expr, now);
        }

        false
    }

    /// Parse and check if cron expression matches current time
    fn cron_matches_now(&self, cron_expr: &str, now: u64) -> bool {
        // Simplified cron parsing (minute hour day month weekday)
        // Full implementation would use a cron parsing library
        let parts: Vec<&str> = cron_expr.split_whitespace().collect();
        if parts.len() < 5 {
            return false;
        }

        // Convert timestamp to components
        let secs = now / 1_000_000;
        let mins = (secs / 60) % 60;
        let hours = (secs / 3600) % 24;
        let day_of_month = ((secs / 86400) % 31) + 1;
        let month = ((secs / 2629800) % 12) + 1; // Approximate
        let day_of_week = (secs / 86400 + 4) % 7; // Unix epoch was Thursday

        let matches_field = |field: &str, value: u64| -> bool {
            if field == "*" {
                return true;
            }
            if let Ok(v) = field.parse::<u64>() {
                return v == value;
            }
            // Handle ranges like "1-5"
            if field.contains('-') {
                let parts: Vec<&str> = field.split('-').collect();
                if parts.len() == 2 {
                    if let (Ok(start), Ok(end)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>())
                    {
                        return value >= start && value <= end;
                    }
                }
            }
            // Handle lists like "1,3,5"
            if field.contains(',') {
                return field
                    .split(',')
                    .any(|v| v.parse::<u64>().ok() == Some(value));
            }
            // Handle step values like "*/5"
            if let Some(step_str) = field.strip_prefix("*/") {
                if let Ok(step) = step_str.parse::<u64>() {
                    return value.is_multiple_of(step);
                }
            }
            false
        };

        matches_field(parts[0], mins)
            && matches_field(parts[1], hours)
            && matches_field(parts[2], day_of_month)
            && matches_field(parts[3], month)
            && matches_field(parts[4], day_of_week)
    }

    /// Record that a scheduled run has started
    pub fn record_run(&self) {
        *self.last_run.write() = Some(current_timestamp());
        *self.runs_today.write() += 1;
    }

    /// Check if scheduler should shutdown
    pub fn is_shutdown(&self) -> bool {
        *self.shutdown.read()
    }

    /// Shutdown the scheduler
    pub fn shutdown(&self) {
        *self.shutdown.write() = true;
    }

    /// Check if a trigger condition is met
    pub fn check_trigger(&self, trigger: &EvalTrigger, context: &TriggerContext) -> bool {
        match trigger {
            EvalTrigger::Deployment { environment } => {
                context.deployment_environment.as_ref() == Some(environment)
            }
            EvalTrigger::Webhook { secret } => context.webhook_secret.as_ref() == Some(secret),
            EvalTrigger::DataThreshold { min_new_traces } => {
                context.new_trace_count >= *min_new_traces
            }
            EvalTrigger::MetricDrift { metric, threshold } => context
                .metric_drifts
                .get(metric)
                .is_some_and(|v| *v >= *threshold),
            EvalTrigger::PullRequestMerge { branch } => {
                context.merged_branch.as_ref() == Some(branch)
            }
        }
    }
}

/// Context for trigger evaluation
#[derive(Debug, Default)]
pub struct TriggerContext {
    pub deployment_environment: Option<String>,
    pub webhook_secret: Option<String>,
    pub new_trace_count: usize,
    pub metric_drifts: HashMap<String, f64>,
    pub merged_branch: Option<String>,
}

fn current_day() -> u64 {
    current_timestamp() / (86400 * 1_000_000) // Days since epoch
}

impl OnlineEvaluator {
    /// Initialize with background worker threads
    pub fn new(config: OnlineEvalConfig, evaluators: Vec<Arc<dyn Evaluator>>) -> Self {
        let eval_queue = Arc::new(Mutex::new(VecDeque::new()));
        let scheduler = Arc::new(EvalScheduler::new(config.schedule.clone()));

        Self {
            evaluators,
            config,
            alert_manager: Arc::new(AlertManager::new()),
            metrics_collector: Arc::new(MetricsCollector::new()),
            eval_queue,
            scheduler,
        }
    }

    /// Start the scheduler background task
    pub fn start_scheduler(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let scheduler = self.scheduler.clone();
        let evaluator = self.clone();

        tokio::spawn(async move {
            loop {
                if scheduler.is_shutdown() {
                    break;
                }

                if scheduler.should_run_now() {
                    scheduler.record_run();

                    // Get schedule configuration
                    if let Some(ref schedule) = evaluator.config.schedule {
                        if let Some(dataset_id) = schedule.dataset_id {
                            // Trigger scheduled evaluation run
                            println!("Starting scheduled eval run for dataset {}", dataset_id);

                            // TODO: Integrate with actual eval run creation
                            // This would call into the eval_runs API to create and execute a run

                            // Call webhook if configured
                            if let Some(ref webhook_url) = schedule.webhook_url {
                                let _ = evaluator
                                    .call_webhook(webhook_url, "scheduled_run_started")
                                    .await;
                            }
                        }
                    }
                }

                // Check every minute
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
        })
    }

    /// Stop the scheduler
    pub fn stop_scheduler(&self) {
        self.scheduler.shutdown();
    }

    /// Trigger evaluation based on external event
    pub async fn trigger_evaluation(&self, context: TriggerContext) -> Result<bool> {
        if let Some(ref schedule) = self.config.schedule {
            for trigger in &schedule.triggers {
                if self.scheduler.check_trigger(trigger, &context) {
                    self.scheduler.record_run();

                    // Execute triggered evaluation
                    if let Some(dataset_id) = schedule.dataset_id {
                        println!(
                            "Triggered eval run for dataset {} due to {:?}",
                            dataset_id, trigger
                        );
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    /// Call webhook with event data
    async fn call_webhook(&self, url: &str, event: &str) -> Result<()> {
        // Simple webhook call implementation
        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "event": event,
            "timestamp": current_timestamp(),
        });

        let _ = client.post(url).json(&payload).send().await;

        Ok(())
    }

    /// Called automatically after each trace is ingested
    pub async fn evaluate_trace_async(&self, trace_id: u128, trace: TraceContext) -> Result<()> {
        // Sampling decision
        if !self.should_sample() {
            return Ok(());
        }

        let task = EvalTask {
            trace_id,
            trace,
            priority: TaskPriority::Normal,
            submitted_at: current_timestamp(),
        };

        if self.config.async_mode {
            // Add to queue, return immediately
            // In a real worker pool, we'd notify workers.
            // For now, let's spawn a task immediately to simulate async worker picking it up.
            let self_clone = self.clone_for_task(); // Need to clone Arc internals?
                                                    // Since OnlineEvaluator holds Arcs, cloning it is cheap.
            let evaluator = self_clone;
            tokio::spawn(async move {
                if let Err(e) = evaluator.evaluate_task(task).await {
                    eprintln!("Eval task failed: {}", e);
                }
            });
            Ok(())
        } else {
            // Blocking evaluation (for critical paths)
            self.evaluate_task(task).await
        }
    }

    // Helper to clone Arcs for async task
    fn clone_for_task(&self) -> Self {
        Self {
            evaluators: self.evaluators.clone(),
            config: self.config.clone(),
            alert_manager: self.alert_manager.clone(),
            metrics_collector: self.metrics_collector.clone(),
            eval_queue: self.eval_queue.clone(),
            scheduler: self.scheduler.clone(),
        }
    }

    /// Background worker function
    async fn evaluate_task(&self, task: EvalTask) -> Result<()> {
        let start = std::time::Instant::now();

        // Run all evaluators in parallel
        let mut eval_futures = Vec::new();
        for evaluator in &self.evaluators {
            let eval = Arc::clone(evaluator);
            let trace = task.trace.clone();
            eval_futures.push(tokio::spawn(async move { eval.evaluate(&trace).await }));
        }

        // Wait for all with timeout
        let timeout = std::time::Duration::from_secs(self.config.timeout_secs);
        // Using tokio timeout
        let results = tokio::time::timeout(timeout, futures::future::join_all(eval_futures))
            .await
            .map_err(|_| EvalError::Timeout("Evaluation timeout".into()))?;

        // Process results
        let mut eval_results = Vec::new();
        for result in results {
            match result {
                Ok(Ok(eval_result)) => {
                    eval_results.push(eval_result);
                }
                Ok(Err(e)) => {
                    // Evaluator failed
                    eprintln!("Evaluator failed for trace {}: {}", task.trace_id, e);
                }
                Err(e) => {
                    // Panic
                    eprintln!("Evaluator panicked for trace {}: {}", task.trace_id, e);
                }
            }
        }

        // Check alert thresholds
        for eval_result in &eval_results {
            self.check_alerts(task.trace_id, eval_result).await?;
        }

        // Collect metrics for drift detection
        if self.config.enable_drift_detection {
            self.metrics_collector
                .record(task.trace_id, &eval_results)
                .await?;
            self.check_drift().await?;
        }

        let duration = start.elapsed();
        // info!("Evaluated trace {} in {:?}", task.trace_id, duration);
        println!("Evaluated trace {} in {:?}", task.trace_id, duration);

        Ok(())
    }

    /// Check if any alert thresholds are violated
    async fn check_alerts(&self, trace_id: u128, eval_result: &EvalResult) -> Result<()> {
        for (metric_name, metric_value) in &eval_result.metrics {
            if let Some(threshold) = self.config.alert_thresholds.get(metric_name) {
                let value = match metric_value {
                    MetricValue::Float(v) => *v,
                    MetricValue::Int(v) => *v as f64,
                    _ => continue,
                };

                let violated = match threshold.operator {
                    ComparisonOperator::GT => value > threshold.value,
                    ComparisonOperator::LT => value < threshold.value,
                    ComparisonOperator::GTE => value >= threshold.value,
                    ComparisonOperator::LTE => value <= threshold.value,
                };

                if violated {
                    self.alert_manager
                        .send_alert(Alert {
                            trace_id,
                            metric: metric_name.clone(),
                            value,
                            threshold: threshold.value,
                            severity: Severity::Warning,
                            channels: threshold.channels.clone(),
                            timestamp: current_timestamp(),
                        })
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// Detect quality drift over time
    async fn check_drift(&self) -> Result<()> {
        let window_start =
            current_timestamp() - (self.config.drift_window_hours * 3600 * 1_000_000);

        for evaluator in &self.evaluators {
            let recent_metrics = self
                .metrics_collector
                .get_metrics_window(&evaluator.id(), window_start, current_timestamp())
                .await?;

            let baseline_metrics = self.metrics_collector.get_baseline(&evaluator.id()).await?;

            // Statistical test: compare recent vs baseline
            if let Some(drift) = self.detect_distribution_shift(&recent_metrics, &baseline_metrics)
            {
                self.alert_manager
                    .send_alert(Alert {
                        trace_id: 0, // Not specific to one trace
                        metric: format!("{}_drift", evaluator.id()),
                        value: drift.magnitude,
                        threshold: 0.0,
                        severity: Severity::Critical,
                        channels: vec![AlertChannel::Slack {
                            webhook_url: "...".into(),
                        }],
                        timestamp: current_timestamp(),
                    })
                    .await?;
            }
        }

        Ok(())
    }

    fn should_sample(&self) -> bool {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen::<f64>() < self.config.sampling_rate
    }

    fn detect_distribution_shift(&self, recent: &[f64], baseline: &[f64]) -> Option<DriftResult> {
        // Simplified drift detection (e.g., mean shift > 10%)
        if recent.is_empty() || baseline.is_empty() {
            return None;
        }

        let recent_mean: f64 = recent.iter().sum::<f64>() / recent.len() as f64;
        let baseline_mean: f64 = baseline.iter().sum::<f64>() / baseline.len() as f64;

        let diff_pct = (recent_mean - baseline_mean).abs() / baseline_mean;
        if diff_pct > 0.1 {
            Some(DriftResult {
                magnitude: diff_pct,
            })
        } else {
            None
        }
    }
}

struct DriftResult {
    magnitude: f64,
}

pub struct AlertManager;
impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AlertManager {
    pub fn new() -> Self {
        Self
    }
    pub async fn send_alert(&self, alert: Alert) -> Result<()> {
        println!("ALERT: {:?}", alert);
        Ok(())
    }
}

pub struct MetricsCollector {
    // In-memory store for now
    data: RwLock<HashMap<String, Vec<(u64, f64)>>>, // metric -> (timestamp, value)
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }

    pub async fn record(&self, _trace_id: u128, results: &[EvalResult]) -> Result<()> {
        let mut data = self.data.write();
        let now = current_timestamp();
        for res in results {
            for (name, val) in &res.metrics {
                if let MetricValue::Float(v) = val {
                    data.entry(name.clone()).or_default().push((now, *v));
                }
            }
        }
        Ok(())
    }

    pub async fn get_metrics_window(&self, metric: &str, start: u64, end: u64) -> Result<Vec<f64>> {
        let data = self.data.read();
        Ok(data
            .get(metric)
            .map(|v| {
                v.iter()
                    .filter(|(t, _)| *t >= start && *t <= end)
                    .map(|(_, val)| *val)
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn get_baseline(&self, metric: &str) -> Result<Vec<f64>> {
        // Just return all data for baseline mock
        let data = self.data.read();
        Ok(data
            .get(metric)
            .map(|v| v.iter().map(|(_, val)| *val).collect())
            .unwrap_or_default())
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}
