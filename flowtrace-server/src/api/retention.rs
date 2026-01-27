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

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use flowtrace_query::retention::{RetentionConfig, RetentionPolicy, RetentionStats};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{ApiError, AppState};

/// Request to manually trigger retention cleanup
#[derive(Debug, Deserialize)]
pub struct TriggerCleanupRequest {
    /// Optional: Specific environment to clean up
    pub environment: Option<String>,
    /// Optional: Override retention days (for manual cleanup)
    pub retention_days: Option<u32>,
}

/// Response with retention statistics
#[derive(Debug, Serialize)]
pub struct RetentionResponse {
    pub success: bool,
    pub message: String,
    pub stats: Option<RetentionStats>,
}

/// Database statistics response
#[derive(Debug, Serialize)]
pub struct DatabaseStatsResponse {
    pub total_traces: usize,
    pub oldest_trace_us: Option<u64>,
    pub newest_trace_us: Option<u64>,
    pub oldest_trace_age_days: Option<f64>,
}

/// Retention configuration response
#[derive(Debug, Serialize)]
pub struct RetentionConfigResponse {
    pub policies: Vec<RetentionPolicy>,
    pub global_retention_days: Option<u32>,
}

/// Update retention configuration request
#[derive(Debug, Deserialize)]
pub struct UpdateRetentionConfigRequest {
    pub policies: Vec<RetentionPolicyRequest>,
    pub global_retention_days: Option<u32>,
}

/// Retention policy in request format
#[derive(Debug, Deserialize)]
pub struct RetentionPolicyRequest {
    pub environment: String,
    pub retention_days: Option<u32>,
    pub enabled: bool,
}

/// GET /api/v1/retention/config - Get current retention configuration
pub async fn get_retention_config(
    State(_state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    // Load from config file
    let config_path = get_retention_config_path();
    let config = RetentionConfig::load(&config_path);

    Ok(Json(RetentionConfigResponse {
        policies: config.policies,
        global_retention_days: config.global_retention_days,
    }))
}

/// POST /api/v1/retention/config - Update retention configuration
pub async fn update_retention_config(
    State(_state): State<AppState>,
    Json(req): Json<UpdateRetentionConfigRequest>,
) -> Result<Json<RetentionConfigResponse>, (StatusCode, String)> {
    // Convert request to policies
    let policies: Vec<RetentionPolicy> = req
        .policies
        .into_iter()
        .map(|p| RetentionPolicy {
            environment: p.environment,
            retention_days: p.retention_days,
            enabled: p.enabled,
        })
        .collect();

    // Create and save config
    let config = RetentionConfig {
        version: 1,
        policies: policies.clone(),
        global_retention_days: req.global_retention_days,
    };

    let config_path = get_retention_config_path();
    if let Err(e) = config.save(&config_path) {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save retention config: {}", e),
        ));
    }

    Ok(Json(RetentionConfigResponse {
        policies,
        global_retention_days: req.global_retention_days,
    }))
}

/// POST /api/v1/retention/cleanup - Manually trigger retention cleanup
pub async fn trigger_cleanup(
    State(state): State<AppState>,
    Json(req): Json<TriggerCleanupRequest>,
) -> Result<Json<RetentionResponse>, (StatusCode, String)> {
    // Build config for cleanup
    let config = if let Some(ref env) = req.environment {
        RetentionConfig {
            version: 1,
            policies: vec![RetentionPolicy {
                environment: env.clone(),
                retention_days: req.retention_days,
                enabled: true,
            }],
            global_retention_days: req.retention_days,
        }
    } else {
        // Load existing config
        let config_path = get_retention_config_path();
        let mut config = RetentionConfig::load(&config_path);
        // Override with request retention_days if provided
        if let Some(days) = req.retention_days {
            config.global_retention_days = Some(days);
        }
        config
    };

    match state.db.apply_retention(&config).await {
        Ok(stats) => Ok(Json(RetentionResponse {
            success: true,
            message: format!(
                "Cleanup completed: deleted {} traces, freed {} bytes",
                stats.traces_deleted, stats.disk_freed_bytes
            ),
            stats: Some(stats),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Cleanup failed: {}", e),
        )),
    }
}

/// GET /api/v1/retention/stats - Get database storage statistics
pub async fn get_database_stats(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let total_traces = state.db.trace_count().await;
    let oldest_trace_us = state.db.oldest_trace_timestamp().await;
    let newest_trace_us = state.db.newest_trace_timestamp().await;

    // Calculate age in days
    let oldest_trace_age_days =
        if let (Some(oldest), Some(newest)) = (oldest_trace_us, newest_trace_us) {
            let age_us = newest - oldest;
            Some(age_us as f64 / (24.0 * 60.0 * 60.0 * 1_000_000.0))
        } else {
            None
        };

    Ok(Json(DatabaseStatsResponse {
        total_traces,
        oldest_trace_us,
        newest_trace_us,
        oldest_trace_age_days,
    }))
}

/// Background task that runs retention cleanup periodically
pub async fn retention_worker(db: std::sync::Arc<flowtrace_query::Flowtrace>, interval_hours: u64) {
    let mut interval =
        tokio::time::interval(tokio::time::Duration::from_secs(interval_hours * 3600));

    loop {
        interval.tick().await;

        println!("Running scheduled retention cleanup...");

        // Load current config
        let config_path = get_retention_config_path();
        let config = RetentionConfig::load(&config_path);

        match db.apply_retention(&config).await {
            Ok(stats) => {
                println!(
                    "Retention cleanup completed: deleted {} traces, freed {} bytes",
                    stats.traces_deleted, stats.disk_freed_bytes
                );
            }
            Err(e) => {
                eprintln!("Retention cleanup failed: {}", e);
            }
        }
    }
}

/// Get the retention config path
fn get_retention_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".flowtrace")
        .join("retention-config.json")
}
