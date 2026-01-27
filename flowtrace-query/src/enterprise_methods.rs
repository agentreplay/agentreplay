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

//! Enterprise feature database methods
//!
//! Additional database methods for prompt templates, experiments, budget alerts,
//! and compliance reports.

use crate::engine::Flowtrace;
use flowtrace_core::{
    AgentFlowEdge, AlertEvent, BudgetAlert, ComplianceReport, CostStats, DataPoint,
    DataPrivacyMetrics, EvalMetric, Experiment, ExperimentResult, FlowtraceError, PromptTemplate,
    Result, SecurityMetrics,
};
use std::collections::HashMap;

impl Flowtrace {
    // ============================================================================
    // Prompt Template Methods
    // ============================================================================

    /// Store a new prompt template or update an existing one
    pub fn store_prompt_template(&self, template: PromptTemplate) -> Result<()> {
        let mut templates = self
            .prompt_templates
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        templates.insert(template.id, template);
        
        // **PERSISTENCE FIX**: Persist to disk after every store
        self.persist_prompt_templates(&templates)?;
        Ok(())
    }

    /// Get a prompt template by ID
    pub fn get_prompt_template(&self, id: u128) -> Result<Option<PromptTemplate>> {
        let templates = self
            .prompt_templates
            .read()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        Ok(templates.get(&id).cloned())
    }

    /// List all prompt templates
    pub fn list_prompt_templates(&self) -> Result<Vec<PromptTemplate>> {
        let templates = self
            .prompt_templates
            .read()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        let mut result: Vec<PromptTemplate> = templates.values().cloned().collect();
        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(result)
    }

    /// Update a prompt template
    pub fn update_prompt_template<F>(&self, id: u128, update_fn: F) -> Result<()>
    where
        F: FnOnce(&mut PromptTemplate),
    {
        let mut templates = self
            .prompt_templates
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        let template = templates
            .get_mut(&id)
            .ok_or_else(|| FlowtraceError::NotFound(format!("Template {} not found", id)))?;

        update_fn(template);
        
        // **PERSISTENCE FIX**: Persist after update
        self.persist_prompt_templates(&templates)?;
        Ok(())
    }

    /// Delete a prompt template
    pub fn delete_prompt_template(&self, id: u128) -> Result<bool> {
        let mut templates = self
            .prompt_templates
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        let removed = templates.remove(&id).is_some();
        if removed {
            // **PERSISTENCE FIX**: Persist after delete
            self.persist_prompt_templates(&templates)?;
        }
        Ok(removed)
    }

    // ============================================================================
    // Experiment Methods
    // ============================================================================

    /// Store a new experiment or update an existing one
    pub fn store_experiment(&self, experiment: Experiment) -> Result<()> {
        let mut experiments = self
            .experiments
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        experiments.insert(experiment.id, experiment);
        Ok(())
    }

    /// Get an experiment by ID
    pub fn get_experiment(&self, id: u128) -> Result<Option<Experiment>> {
        let experiments = self
            .experiments
            .read()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        Ok(experiments.get(&id).cloned())
    }

    /// List all experiments
    pub fn list_experiments(&self) -> Result<Vec<Experiment>> {
        let experiments = self
            .experiments
            .read()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        let mut result: Vec<Experiment> = experiments.values().cloned().collect();
        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(result)
    }

    /// Update an experiment
    pub fn update_experiment<F>(&self, id: u128, update_fn: F) -> Result<()>
    where
        F: FnOnce(&mut Experiment),
    {
        let mut experiments = self
            .experiments
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        let experiment = experiments
            .get_mut(&id)
            .ok_or_else(|| FlowtraceError::NotFound(format!("Experiment {} not found", id)))?;

        update_fn(experiment);
        Ok(())
    }

    /// Delete an experiment
    pub fn delete_experiment(&self, id: u128) -> Result<bool> {
        let mut experiments = self
            .experiments
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        // Also delete associated results
        let mut results = self
            .experiment_results
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;
        results.remove(&id);

        Ok(experiments.remove(&id).is_some())
    }

    /// Store an experiment result
    pub fn store_experiment_result(&self, result: ExperimentResult) -> Result<()> {
        let mut results = self
            .experiment_results
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        results
            .entry(result.experiment_id)
            .or_insert_with(Vec::new)
            .push(result);
        Ok(())
    }

    /// Get all results for an experiment
    pub fn get_experiment_results(&self, experiment_id: u128) -> Result<Vec<ExperimentResult>> {
        let results = self
            .experiment_results
            .read()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        Ok(results.get(&experiment_id).cloned().unwrap_or_default())
    }

    // ============================================================================
    // Budget Alert Methods
    // ============================================================================

    /// Store a new budget alert or update an existing one
    pub fn store_budget_alert(&self, alert: BudgetAlert) -> Result<()> {
        let mut alerts = self
            .budget_alerts
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        alerts.insert(alert.id, alert);
        Ok(())
    }

    /// Get a budget alert by ID
    pub fn get_budget_alert(&self, id: u128) -> Result<Option<BudgetAlert>> {
        let alerts = self
            .budget_alerts
            .read()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        Ok(alerts.get(&id).cloned())
    }

    /// List all budget alerts
    pub fn list_budget_alerts(&self) -> Result<Vec<BudgetAlert>> {
        let alerts = self
            .budget_alerts
            .read()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        let mut result: Vec<BudgetAlert> = alerts.values().cloned().collect();
        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(result)
    }

    /// Update a budget alert
    pub fn update_budget_alert<F>(&self, id: u128, update_fn: F) -> Result<()>
    where
        F: FnOnce(&mut BudgetAlert),
    {
        let mut alerts = self
            .budget_alerts
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        let alert = alerts
            .get_mut(&id)
            .ok_or_else(|| FlowtraceError::NotFound(format!("Alert {} not found", id)))?;

        update_fn(alert);
        Ok(())
    }

    /// Delete a budget alert
    pub fn delete_budget_alert(&self, id: u128) -> Result<bool> {
        let mut alerts = self
            .budget_alerts
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        // Also delete associated events
        let mut events = self
            .alert_events
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;
        events.remove(&id);

        Ok(alerts.remove(&id).is_some())
    }

    /// Store an alert event
    pub fn store_alert_event(&self, event: AlertEvent) -> Result<()> {
        let mut events = self
            .alert_events
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        events
            .entry(event.alert_id)
            .or_insert_with(Vec::new)
            .push(event);
        Ok(())
    }

    /// Get all events for an alert
    pub fn get_alert_events(&self, alert_id: u128) -> Result<Vec<AlertEvent>> {
        let events = self
            .alert_events
            .read()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        Ok(events.get(&alert_id).cloned().unwrap_or_default())
    }

    /// Get cost statistics for a time range
    pub fn get_cost_stats(&self, start_time: u64, end_time: u64) -> Result<CostStats> {
        let edges = self.storage.range_scan(start_time, end_time)?;

        let mut total_cost = 0.0;
        let mut total_tokens = 0u64;

        for edge in &edges {
            // Simple cost estimation: $0.01 per 1000 tokens
            let tokens = edge.token_count as u64;
            total_tokens += tokens;
            total_cost += (tokens as f64 / 1000.0) * 0.01;
        }

        Ok(CostStats {
            total_cost,
            trace_count: edges.len(),
            total_tokens,
        })
    }

    // ============================================================================
    // Compliance Report Methods
    // ============================================================================

    /// Store a new compliance report or update an existing one
    pub fn store_compliance_report(&self, report: ComplianceReport) -> Result<()> {
        let mut reports = self
            .compliance_reports
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        reports.insert(report.id, report);
        Ok(())
    }

    /// Get a compliance report by ID
    pub fn get_compliance_report(&self, id: u128) -> Result<Option<ComplianceReport>> {
        let reports = self
            .compliance_reports
            .read()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        Ok(reports.get(&id).cloned())
    }

    /// List all compliance reports
    pub fn list_compliance_reports(&self) -> Result<Vec<ComplianceReport>> {
        let reports = self
            .compliance_reports
            .read()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        let mut result: Vec<ComplianceReport> = reports.values().cloned().collect();
        result.sort_by(|a, b| b.generated_at.cmp(&a.generated_at));
        Ok(result)
    }

    /// Delete a compliance report
    pub fn delete_compliance_report(&self, id: u128) -> Result<bool> {
        let mut reports = self
            .compliance_reports
            .write()
            .map_err(|e| FlowtraceError::Internal(format!("Lock poisoned: {}", e)))?;

        Ok(reports.remove(&id).is_some())
    }

    /// Scan traces for PII (placeholder implementation)
    pub fn scan_for_pii(&self, _traces: &[AgentFlowEdge]) -> Result<usize> {
        // TODO: Implement actual PII detection
        // For now, return a placeholder value
        Ok(0)
    }

    /// Get privacy metrics for a time range (placeholder)
    pub fn get_privacy_metrics(&self, _start: u64, _end: u64) -> Result<DataPrivacyMetrics> {
        // TODO: Implement actual privacy metrics collection
        Ok(DataPrivacyMetrics::default())
    }

    /// Get security metrics for a time range (placeholder)
    pub fn get_security_metrics(&self, _start: u64, _end: u64) -> Result<SecurityMetrics> {
        // TODO: Implement actual security metrics collection
        Ok(SecurityMetrics::default())
    }

    /// Get traces in a time range
    pub fn list_traces_in_range(
        &self,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<AgentFlowEdge>> {
        self.storage.range_scan(start_time, end_time)
    }

    /// Get eval metrics for a time period
    ///
    /// CRITICAL FIX: After migrating to moka::Cache for memory safety, this method
    /// cannot efficiently iterate over all entries (Cache doesn't expose iterator).
    ///
    /// Alternative approaches:
    /// 1. Query by specific edge IDs (use get_eval_metrics_batch)
    /// 2. Store metrics in persistent storage for time-based queries
    /// 3. Maintain a separate index of edge_ids by timestamp
    ///
    /// For now, returns empty vec. Callers should use edge-based queries instead.
    pub fn get_eval_metrics_for_period(&self, _start: u64, _end: u64) -> Result<Vec<EvalMetric>> {
        // NOTE: This method is deprecated after Cache migration
        // Use get_eval_metrics_batch with specific edge IDs instead
        Ok(Vec::new())
    }

    // ============================================================================
    // Analytics Methods (Actual Implementations)
    // ============================================================================

    /// Get time-series data for a metric with actual aggregation
    ///
    /// Aggregates metric values into time buckets for visualization.
    /// Supports filtering by project, agent, and model.
    #[allow(clippy::too_many_arguments)]
    pub fn get_timeseries_data(
        &self,
        metric: &str,
        start_time: u64,
        end_time: u64,
        interval: u64,
        project_id: Option<u16>,
        agent_id: Option<u64>,
        _model: Option<&str>, // Model filtering requires payload lookup, not implemented yet
    ) -> Result<Vec<DataPoint>> {
        // Get all traces in the time range
        let edges = self.storage.range_scan(start_time, end_time)?;

        // Filter edges based on criteria
        let filtered_edges: Vec<_> = edges
            .iter()
            .filter(|e| {
                // Filter by project
                if let Some(pid) = project_id {
                    if e.project_id != pid {
                        return false;
                    }
                }
                // Filter by agent
                if let Some(aid) = agent_id {
                    if e.agent_id != aid {
                        return false;
                    }
                }
                true
            })
            .collect();

        // Create time buckets
        let num_buckets = ((end_time - start_time) / interval.max(1)) as usize + 1;
        let mut buckets: Vec<Vec<f64>> = vec![Vec::new(); num_buckets];

        // Get eval metrics for filtered edges and aggregate
        for edge in &filtered_edges {
            let metrics = self.get_eval_metrics(edge.edge_id)?;
            for m in metrics {
                if m.get_metric_name() == metric {
                    // Determine which bucket this belongs to
                    let bucket_idx = ((m.timestamp_us - start_time) / interval.max(1)) as usize;
                    if bucket_idx < num_buckets {
                        buckets[bucket_idx].push(m.metric_value);
                    }
                }
            }
        }

        // Also aggregate from trace data if metric is latency/duration/tokens
        match metric {
            "latency" | "duration" | "duration_ms" => {
                for edge in &filtered_edges {
                    let bucket_idx = ((edge.timestamp_us - start_time) / interval.max(1)) as usize;
                    if bucket_idx < num_buckets {
                        buckets[bucket_idx].push(edge.duration_us as f64 / 1000.0);
                        // Convert to ms
                    }
                }
            }
            "tokens" | "token_count" => {
                for edge in &filtered_edges {
                    let bucket_idx = ((edge.timestamp_us - start_time) / interval.max(1)) as usize;
                    if bucket_idx < num_buckets {
                        buckets[bucket_idx].push(edge.token_count as f64);
                    }
                }
            }
            "trace_count" | "count" => {
                for edge in &filtered_edges {
                    let bucket_idx = ((edge.timestamp_us - start_time) / interval.max(1)) as usize;
                    if bucket_idx < num_buckets {
                        buckets[bucket_idx].push(1.0); // Count each trace
                    }
                }
            }
            _ => {}
        }

        // Convert buckets to DataPoints
        let data_points: Vec<DataPoint> = buckets
            .into_iter()
            .enumerate()
            .map(|(i, values)| {
                let timestamp = start_time + (i as u64 * interval);
                let value = if values.is_empty() {
                    0.0
                } else {
                    values.iter().sum::<f64>() / values.len() as f64
                };
                DataPoint {
                    timestamp,
                    value,
                    count: values.len(),
                }
            })
            .collect();

        Ok(data_points)
    }

    /// Get a single aggregated metric value for a time range
    pub fn get_metric_value(&self, metric: &str, start_time: u64, end_time: u64) -> Result<f64> {
        let edges = self.storage.range_scan(start_time, end_time)?;

        if edges.is_empty() {
            return Ok(0.0);
        }

        let value = match metric {
            "avg_latency" | "latency" => {
                let total: f64 = edges.iter().map(|e| e.duration_us as f64).sum();
                total / edges.len() as f64 / 1000.0 // Convert to ms
            }
            "p50_latency" => {
                let mut latencies: Vec<f64> = edges
                    .iter()
                    .map(|e| e.duration_us as f64 / 1000.0)
                    .collect();
                latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                latencies[latencies.len() / 2]
            }
            "p95_latency" => {
                let mut latencies: Vec<f64> = edges
                    .iter()
                    .map(|e| e.duration_us as f64 / 1000.0)
                    .collect();
                latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let idx = (latencies.len() as f64 * 0.95) as usize;
                latencies
                    .get(idx.min(latencies.len() - 1))
                    .copied()
                    .unwrap_or(0.0)
            }
            "p99_latency" => {
                let mut latencies: Vec<f64> = edges
                    .iter()
                    .map(|e| e.duration_us as f64 / 1000.0)
                    .collect();
                latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let idx = (latencies.len() as f64 * 0.99) as usize;
                latencies
                    .get(idx.min(latencies.len() - 1))
                    .copied()
                    .unwrap_or(0.0)
            }
            "total_tokens" => edges.iter().map(|e| e.token_count as f64).sum(),
            "avg_tokens" => {
                let total: f64 = edges.iter().map(|e| e.token_count as f64).sum();
                total / edges.len() as f64
            }
            "trace_count" | "count" => edges.len() as f64,
            "error_rate" => {
                // Check flags for error status (bit 0 = error)
                let errors = edges.iter().filter(|e| e.flags & 1 != 0).count();
                errors as f64 / edges.len() as f64
            }
            _ => {
                // Try to get from eval metrics
                let mut total = 0.0;
                let mut count = 0;
                for edge in &edges {
                    let metrics = self.get_eval_metrics(edge.edge_id)?;
                    for m in metrics {
                        if m.get_metric_name() == metric {
                            total += m.metric_value;
                            count += 1;
                        }
                    }
                }
                if count > 0 {
                    total / count as f64
                } else {
                    0.0
                }
            }
        };

        Ok(value)
    }

    /// Get metrics grouped by a dimension (model, agent, project)
    pub fn get_grouped_metrics(
        &self,
        metric: &str,
        start_time: u64,
        end_time: u64,
        group_by: &str,
    ) -> Result<HashMap<String, (f64, usize)>> {
        let edges = self.storage.range_scan(start_time, end_time)?;

        // Group edges by the specified dimension
        let mut groups: HashMap<String, Vec<&AgentFlowEdge>> = HashMap::new();

        for edge in &edges {
            let key = match group_by {
                "model" => format!("model_{}", edge.span_type), // Simplified - would need model lookup
                "agent" | "agent_id" => format!("{}", edge.agent_id),
                "project" | "project_id" => format!("{}", edge.project_id),
                "environment" => format!("{}", edge.environment),
                "session" | "session_id" => format!("{}", edge.session_id),
                _ => "unknown".to_string(),
            };
            groups.entry(key).or_default().push(edge);
        }

        // Calculate metric for each group
        let mut result: HashMap<String, (f64, usize)> = HashMap::new();

        for (key, group_edges) in groups {
            let value = match metric {
                "avg_latency" | "latency" => {
                    let total: f64 = group_edges.iter().map(|e| e.duration_us as f64).sum();
                    total / group_edges.len() as f64 / 1000.0
                }
                "total_tokens" => group_edges.iter().map(|e| e.token_count as f64).sum(),
                "avg_tokens" => {
                    let total: f64 = group_edges.iter().map(|e| e.token_count as f64).sum();
                    total / group_edges.len() as f64
                }
                "count" | "trace_count" => group_edges.len() as f64,
                "error_rate" => {
                    let errors = group_edges.iter().filter(|e| e.flags & 1 != 0).count();
                    errors as f64 / group_edges.len() as f64
                }
                _ => 0.0,
            };

            result.insert(key, (value, group_edges.len()));
        }

        Ok(result)
    }

    /// Get time-series values for a metric (raw values, not aggregated)
    pub fn get_timeseries_values(
        &self,
        metric: &str,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<f64>> {
        let edges = self.storage.range_scan(start_time, end_time)?;

        let values: Vec<f64> = match metric {
            "latency" | "duration" => edges
                .iter()
                .map(|e| e.duration_us as f64 / 1000.0)
                .collect(),
            "tokens" | "token_count" => edges.iter().map(|e| e.token_count as f64).collect(),
            _ => {
                // Get from eval metrics
                let mut vals = Vec::new();
                for edge in &edges {
                    let metrics = self.get_eval_metrics(edge.edge_id)?;
                    for m in metrics {
                        if m.get_metric_name() == metric {
                            vals.push(m.metric_value);
                        }
                    }
                }
                vals
            }
        };

        Ok(values)
    }
}
