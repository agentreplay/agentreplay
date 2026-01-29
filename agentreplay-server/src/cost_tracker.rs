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

//! Real-time cost tracking and attribution system
//!
//! Provides hierarchical cost tracking with:
//! - Per-trace, per-agent, per-project, per-tenant attribution
//! - Real-time cost calculation from token usage
//! - Cost forecasting based on usage trends
//! - Budget enforcement with alerts
//!
//! **FIXED Task #2 from task.md**: Uses rust_decimal::Decimal for zero-error billing.
//! IEEE 754 f64 arithmetic suffers from precision loss (0.1 + 0.2 != 0.3).
//! For millions of micro-transactions, these errors accumulate causing billing discrepancies.
//! Decimal uses fixed-point arithmetic with exact decimal representation.

use agentreplay_core::AgentFlowEdge;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Hierarchical cost tracker
pub struct CostTracker {
    /// Cost tracking state
    state: Arc<RwLock<CostTrackerState>>,
    /// Model pricing configuration
    pricing: ModelPricing,
}

#[derive(Debug, Default)]
struct CostTrackerState {
    /// Per-tenant costs
    tenant_costs: HashMap<u64, TenantCostData>,
    /// Per-project costs
    project_costs: HashMap<(u64, u16), ProjectCostData>,
    /// Per-agent costs
    agent_costs: HashMap<u64, AgentCostData>,
    /// Per-session costs
    session_costs: HashMap<u64, SessionCostData>,
}

#[derive(Debug, Clone, Default)]
pub struct TenantCostData {
    pub total_cost: Decimal,
    pub total_tokens: u64,
    pub trace_count: u64,
    pub hourly_costs: Vec<(u64, Decimal)>, // (timestamp_hour, cost)
    pub daily_costs: Vec<(u64, Decimal)>,  // (timestamp_day, cost)
}

#[derive(Debug, Clone, Default)]
pub struct ProjectCostData {
    pub total_cost: Decimal,
    pub total_tokens: u64,
    pub trace_count: u64,
    pub agent_breakdown: HashMap<u64, Decimal>, // agent_id -> cost
}

#[derive(Debug, Clone, Default)]
pub struct AgentCostData {
    pub total_cost: Decimal,
    pub total_tokens: u64,
    pub trace_count: u64,
    pub avg_cost_per_trace: Decimal,
    pub span_type_breakdown: HashMap<String, Decimal>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionCostData {
    pub total_cost: Decimal,
    pub total_tokens: u64,
    pub trace_count: u64,
    pub start_time: u64,
    pub last_activity: u64,
}

/// Model pricing configuration
#[derive(Debug, Clone)]
pub struct ModelPricing {
    /// Price per 1K input tokens (USD)
    pub input_price_per_1k: Decimal,
    /// Price per 1K output tokens (USD)
    pub output_price_per_1k: Decimal,
    /// Model-specific pricing overrides
    pub model_overrides: HashMap<String, (Decimal, Decimal)>,
}

impl Default for ModelPricing {
    fn default() -> Self {
        let mut model_overrides = HashMap::new();

        // GPT-4 pricing - using Decimal for exact precision
        model_overrides.insert("gpt-4".to_string(), (dec!(0.03), dec!(0.06)));
        model_overrides.insert("gpt-4-turbo".to_string(), (dec!(0.01), dec!(0.03)));
        model_overrides.insert("gpt-4o".to_string(), (dec!(0.005), dec!(0.015)));
        model_overrides.insert("gpt-4o-mini".to_string(), (dec!(0.00015), dec!(0.0006)));

        // GPT-3.5 pricing
        model_overrides.insert("gpt-3.5-turbo".to_string(), (dec!(0.0005), dec!(0.0015)));

        // Claude pricing
        model_overrides.insert("claude-3-opus".to_string(), (dec!(0.015), dec!(0.075)));
        model_overrides.insert("claude-3-sonnet".to_string(), (dec!(0.003), dec!(0.015)));
        model_overrides.insert("claude-3-haiku".to_string(), (dec!(0.00025), dec!(0.00125)));
        model_overrides.insert("claude-3-5-sonnet".to_string(), (dec!(0.003), dec!(0.015)));

        Self {
            input_price_per_1k: dec!(0.002), // Default: GPT-3.5-turbo equivalent
            output_price_per_1k: dec!(0.002),
            model_overrides,
        }
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CostTracker {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(CostTrackerState::default())),
            pricing: ModelPricing::default(),
        }
    }

    pub fn with_pricing(mut self, pricing: ModelPricing) -> Self {
        self.pricing = pricing;
        self
    }

    /// Track cost for a new edge
    pub async fn track_edge(&self, edge: &AgentFlowEdge, model_name: Option<&str>) {
        if edge.token_count == 0 {
            return; // No tokens, no cost
        }

        // Calculate cost for this edge
        let cost = self.calculate_edge_cost(edge, model_name);

        let mut state = self.state.write().await;

        // Update tenant costs
        let tenant_cost = state.tenant_costs.entry(edge.tenant_id).or_default();
        tenant_cost.total_cost += cost;
        tenant_cost.total_tokens += edge.token_count as u64;
        tenant_cost.trace_count += 1;

        // Track hourly costs
        let hour_timestamp = (edge.timestamp_us / 3_600_000_000) * 3_600_000_000;
        if let Some(last_hour) = tenant_cost.hourly_costs.last_mut() {
            if last_hour.0 == hour_timestamp {
                last_hour.1 += cost;
            } else {
                tenant_cost.hourly_costs.push((hour_timestamp, cost));
            }
        } else {
            tenant_cost.hourly_costs.push((hour_timestamp, cost));
        }

        // Update project costs
        let project_key = (edge.tenant_id, edge.project_id);
        let project_cost = state.project_costs.entry(project_key).or_default();
        project_cost.total_cost += cost;
        project_cost.total_tokens += edge.token_count as u64;
        project_cost.trace_count += 1;
        *project_cost
            .agent_breakdown
            .entry(edge.agent_id)
            .or_default() += cost;

        // Update agent costs
        let agent_cost = state.agent_costs.entry(edge.agent_id).or_default();
        agent_cost.total_cost += cost;
        agent_cost.total_tokens += edge.token_count as u64;
        agent_cost.trace_count += 1;
        agent_cost.avg_cost_per_trace =
            agent_cost.total_cost / Decimal::from(agent_cost.trace_count);

        let span_type = format!("{:?}", edge.get_span_type());
        *agent_cost.span_type_breakdown.entry(span_type).or_default() += cost;

        // Update session costs
        let session_cost = state.session_costs.entry(edge.session_id).or_default();
        if session_cost.start_time == 0 {
            session_cost.start_time = edge.timestamp_us;
        }
        session_cost.total_cost += cost;
        session_cost.total_tokens += edge.token_count as u64;
        session_cost.trace_count += 1;
        session_cost.last_activity = edge.timestamp_us;
    }

    /// Calculate cost for an edge using exact Decimal arithmetic
    fn calculate_edge_cost(&self, edge: &AgentFlowEdge, model_name: Option<&str>) -> Decimal {
        let (input_price, output_price) = if let Some(model) = model_name {
            self.pricing.model_overrides.get(model).copied().unwrap_or((
                self.pricing.input_price_per_1k,
                self.pricing.output_price_per_1k,
            ))
        } else {
            (
                self.pricing.input_price_per_1k,
                self.pricing.output_price_per_1k,
            )
        };

        // For now, assume equal split between input/output
        // In production, track input_tokens and output_tokens separately
        let tokens = Decimal::from(edge.token_count);
        let two = dec!(2);
        let thousand = dec!(1000);
        ((tokens / two) * input_price / thousand) + ((tokens / two) * output_price / thousand)
    }

    /// Get tenant cost data
    pub async fn get_tenant_costs(&self, tenant_id: u64) -> Option<TenantCostData> {
        let state = self.state.read().await;
        state.tenant_costs.get(&tenant_id).cloned()
    }

    /// Get project cost data
    pub async fn get_project_costs(
        &self,
        tenant_id: u64,
        project_id: u16,
    ) -> Option<ProjectCostData> {
        let state = self.state.read().await;
        state.project_costs.get(&(tenant_id, project_id)).cloned()
    }

    /// Get agent cost data
    pub async fn get_agent_costs(&self, agent_id: u64) -> Option<AgentCostData> {
        let state = self.state.read().await;
        state.agent_costs.get(&agent_id).cloned()
    }

    /// Forecast cost for next period
    pub async fn forecast_cost(&self, tenant_id: u64, hours: u64) -> Option<Decimal> {
        let state = self.state.read().await;
        let tenant_cost = state.tenant_costs.get(&tenant_id)?;

        if tenant_cost.hourly_costs.len() < 3 {
            return None; // Need at least 3 hours of data
        }

        // Calculate average cost per hour from recent data
        let recent_hours: Decimal = tenant_cost
            .hourly_costs
            .iter()
            .rev()
            .take(24) // Use last 24 hours
            .map(|(_, cost)| *cost)
            .sum();

        let hours_counted = Decimal::from(tenant_cost.hourly_costs.len().min(24));
        let avg_per_hour = recent_hours / hours_counted;

        Some(avg_per_hour * Decimal::from(hours))
    }

    /// Check if budget threshold is exceeded
    pub async fn check_budget(
        &self,
        tenant_id: u64,
        threshold: Decimal,
    ) -> Option<(Decimal, bool)> {
        let state = self.state.read().await;
        let tenant_cost = state.tenant_costs.get(&tenant_id)?;
        let exceeded = tenant_cost.total_cost >= threshold;
        Some((tenant_cost.total_cost, exceeded))
    }

    /// Get summary of all tracked costs
    pub async fn get_summary(&self) -> CostSummary {
        let state = self.state.read().await;

        let total_cost: Decimal = state.tenant_costs.values().map(|t| t.total_cost).sum();

        let total_tokens = state
            .tenant_costs
            .values()
            .map(|t| t.total_tokens)
            .sum::<u64>();

        let trace_count = state
            .tenant_costs
            .values()
            .map(|t| t.trace_count)
            .sum::<u64>();

        CostSummary {
            total_cost,
            total_tokens,
            trace_count,
        }
    }
}

/// Summary of all tracked costs
#[derive(Debug, Clone)]
pub struct CostSummary {
    pub total_cost: Decimal,
    pub total_tokens: u64,
    pub trace_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentreplay_core::{AgentFlowEdge, SpanType};

    #[tokio::test]
    async fn test_cost_tracking() {
        let tracker = CostTracker::new();

        let mut edge = AgentFlowEdge::new(1, 1, 100, 200, SpanType::Planning, 0);
        edge.token_count = 1000;
        edge.timestamp_us = 1_700_000_000_000_000;

        tracker.track_edge(&edge, Some("gpt-4o-mini")).await;

        let tenant_costs = tracker.get_tenant_costs(1).await.unwrap();
        assert!(tenant_costs.total_cost > Decimal::ZERO);
        assert_eq!(tenant_costs.total_tokens, 1000);
        assert_eq!(tenant_costs.trace_count, 1);
    }

    #[tokio::test]
    async fn test_cost_forecasting() {
        let tracker = CostTracker::new();

        // Simulate 5 hours of usage
        for i in 0..5 {
            let mut edge = AgentFlowEdge::new(1, 1, 100, 200, SpanType::Planning, 0);
            edge.token_count = 1000;
            edge.timestamp_us = 1_700_000_000_000_000 + (i * 3_600_000_000);
            tracker.track_edge(&edge, Some("gpt-3.5-turbo")).await;
        }

        let forecast = tracker.forecast_cost(1, 24).await;
        assert!(forecast.is_some());
        assert!(forecast.unwrap() > Decimal::ZERO);
    }
}
