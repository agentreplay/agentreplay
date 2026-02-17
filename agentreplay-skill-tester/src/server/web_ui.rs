// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Embedded web UI server for the Skill Tester
//!
//! Runs a local HTTP server (like MCP Inspector on port 6274)
//! that serves the React frontend and REST API.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use axum::{Router, routing::get};
use tower_http::cors::{CorsLayer, Any};

use crate::manifest::SkillManifest;
use crate::runner::scenario::{TestResult, TestSuite};
use crate::security::owasp_scanner::OwaspScanResult;
use crate::manifest::validator::ValidationReport;

/// Shared state for the skill tester server
pub struct SkillTesterState {
    /// Currently loaded skill manifest
    pub manifest: Option<SkillManifest>,
    /// Validation report
    pub validation: Option<ValidationReport>,
    /// OWASP scan result
    pub owasp_scan: Option<OwaspScanResult>,
    /// Test suites
    pub test_suites: Vec<TestSuite>,
    /// Test results
    pub test_results: Vec<TestResult>,
    /// Server port
    pub port: u16,
}

impl SkillTesterState {
    pub fn new(port: u16) -> Self {
        Self {
            manifest: None,
            validation: None,
            owasp_scan: None,
            test_suites: Vec::new(),
            test_results: Vec::new(),
            port,
        }
    }
}

/// Skill Tester web UI server
pub struct SkillTesterServer {
    state: Arc<RwLock<SkillTesterState>>,
    port: u16,
}

impl SkillTesterServer {
    pub fn new(port: u16) -> Self {
        Self {
            state: Arc::new(RwLock::new(SkillTesterState::new(port))),
            port,
        }
    }

    /// Get shared state handle
    pub fn state(&self) -> Arc<RwLock<SkillTesterState>> {
        self.state.clone()
    }

    /// Build the Axum router
    pub fn router(&self) -> Router {
        let state = self.state.clone();

        Router::new()
            // API routes
            .route("/api/health", get(|| async { "OK" }))
            .route("/api/manifest", get({
                let state = state.clone();
                move || async move {
                    let s = state.read().await;
                    match &s.manifest {
                        Some(m) => axum::Json(serde_json::to_value(m).unwrap_or_default()).into_response(),
                        None => axum::http::StatusCode::NOT_FOUND.into_response(),
                    }
                }
            }))
            .route("/api/validation", get({
                let state = state.clone();
                move || async move {
                    let s = state.read().await;
                    match &s.validation {
                        Some(v) => axum::Json(serde_json::to_value(v).unwrap_or_default()).into_response(),
                        None => axum::http::StatusCode::NOT_FOUND.into_response(),
                    }
                }
            }))
            .route("/api/owasp", get({
                let state = state.clone();
                move || async move {
                    let s = state.read().await;
                    match &s.owasp_scan {
                        Some(o) => axum::Json(serde_json::to_value(o).unwrap_or_default()).into_response(),
                        None => axum::http::StatusCode::NOT_FOUND.into_response(),
                    }
                }
            }))
            .route("/api/tests", get({
                let state = state.clone();
                move || async move {
                    let s = state.read().await;
                    axum::Json(serde_json::to_value(&s.test_suites).unwrap_or_default())
                }
            }))
            .route("/api/results", get({
                let state = state.clone();
                move || async move {
                    let s = state.read().await;
                    axum::Json(serde_json::to_value(&s.test_results).unwrap_or_default())
                }
            }))
            .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
    }

    /// Start the server
    pub async fn start(&self) -> anyhow::Result<()> {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        let app = self.router();

        tracing::info!("Skill Tester server starting on http://{}", addr);
        println!("🧪 AgentReplay Skill Tester running at http://localhost:{}", self.port);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

/// Helper trait for response types
use axum::response::IntoResponse;
