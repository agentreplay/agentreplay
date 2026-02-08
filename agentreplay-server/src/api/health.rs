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

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use tracing::debug;

use crate::api::{ApiError, AppState};

/// Health check response structure
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub storage: StorageHealth,
    pub api: ApiHealth,
}

#[derive(Debug, Serialize)]
pub struct StorageHealth {
    pub reachable: bool,
    pub total_edges: u64,
}

#[derive(Debug, Serialize)]
pub struct ApiHealth {
    pub requests_total: u64,
    pub avg_latency_ms: f64,
}

/// GET /api/v1/health - Comprehensive health check endpoint
pub async fn health_check_detailed(
    State(_state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    debug!("Health check requested");

    // For now, just return healthy status
    // TODO: Add actual storage health checks when db stats API is available
    let storage_reachable = true;
    let total_edges = 0u64;

    // Get uptime (simplified - using current timestamp)
    let uptime = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let health = HealthResponse {
        status: if storage_reachable {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        storage: StorageHealth {
            reachable: storage_reachable,
            total_edges,
        },
        api: ApiHealth {
            requests_total: 0, // TODO: Implement metrics tracking
            avg_latency_ms: 0.0,
        },
    };

    let status_code = if storage_reachable {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    Ok((status_code, Json(health)))
}
