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

// Prevents additional console window on Windows in release builds
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Emitter, Manager};

#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;

#[cfg(target_os = "windows")]
use tauri::window::{Effect, EffectsBuilder};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// macOS panel support
#[cfg(target_os = "macos")]
use tauri_nspanel::tauri_panel;

// Import Agentreplay crates
use agentreplay_core::{AgentFlowEdge, SavedViewRegistry};
use agentreplay_query::Agentreplay;

mod windows;
mod commands;
mod error;
mod health;
mod menu;
mod otlp_server;
mod project_store;
mod server;
mod memory;
mod sse;
mod llm;
mod llm_service;
mod comparison_engine;
mod plugins;
mod sysinfo_state;

/// Load LLM config from persistent storage
fn load_persisted_llm_config() -> Option<llm::LLMConfig> {
    let config_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("agentreplay")
        .join("llm-config.json");
    
    if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(contents) => {
                match serde_json::from_str::<llm::LLMConfig>(&contents) {
                    Ok(config) => {
                        tracing::info!("Loaded persisted LLM config from {:?}", config_path);
                        tracing::info!("  - default_model: {}", config.default_model);
                        tracing::info!("  - {} provider(s) configured", config.providers.len());
                        for (i, p) in config.providers.iter().enumerate() {
                            let tags_str = if p.tags.is_empty() { "none".to_string() } else { p.tags.join(", ") };
                            tracing::info!("  - Provider {}: {} ({}) model={} tags=[{}]", 
                                i, p.name.as_deref().unwrap_or("unnamed"), p.provider,
                                p.model.as_deref().unwrap_or("default"), tags_str);
                        }
                        Some(config)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse persisted LLM config: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to read LLM config file: {}", e);
                None
            }
        }
    } else {
        tracing::info!("No persisted LLM config found at {:?}, using default Ollama config", config_path);
        None
    }
}

// Define custom panel class for macOS
#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(AgentreplayPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

/// Connection statistics for SDK monitoring
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionStats {
    pub total_traces_received: u64,
    pub last_trace_time: Option<u64>,
    pub server_uptime_secs: u64,
    pub ingestion_rate_per_min: f64,
}

/// Ingestion queue for async buffering with graceful shutdown
pub struct IngestionQueue {
    tx: tokio::sync::mpsc::Sender<AgentFlowEdge>, // Bounded for backpressure
    shutdown_tx: Arc<tokio::sync::Notify>,
    /// Signals when the worker has completed flushing and shutdown
    worker_done_rx: Arc<tokio::sync::RwLock<Option<tokio::sync::oneshot::Receiver<()>>>>,
}

impl IngestionQueue {
    pub fn send(&self, edge: AgentFlowEdge) -> Result<(), String> {
        self.tx.try_send(edge).map_err(|e| match e {
            tokio::sync::mpsc::error::TrySendError::Full(_) => {
                "Ingestion queue full - system under heavy load".to_string()
            }
            tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                "Ingestion queue closed - system shutting down".to_string()
            }
        })
    }

    pub fn shutdown(&self) {
        self.shutdown_tx.notify_one();
    }

    /// Wait for the worker to complete shutdown with a timeout
    /// Returns true if worker completed, false if timeout elapsed
    pub async fn wait_for_shutdown(&self, timeout: std::time::Duration) -> bool {
        let rx = {
            let mut guard = self.worker_done_rx.write().await;
            guard.take()
        };

        if let Some(rx) = rx {
            match tokio::time::timeout(timeout, rx).await {
                Ok(Ok(())) => {
                    tracing::info!("Ingestion worker completed shutdown gracefully");
                    true
                }
                Ok(Err(_)) => {
                    tracing::warn!("Ingestion worker done channel was dropped");
                    false
                }
                Err(_) => {
                    tracing::warn!("Timeout waiting for ingestion worker shutdown");
                    false
                }
            }
        } else {
            tracing::warn!("Worker done receiver already taken");
            false
        }
    }
}

impl Clone for IngestionQueue {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            shutdown_tx: Arc::clone(&self.shutdown_tx),
            worker_done_rx: Arc::clone(&self.worker_done_rx),
        }
    }
}

/// Application state shared across all Tauri commands
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Agentreplay>,
    pub db_path: PathBuf,
    pub config: Arc<RwLock<AppConfig>>,
    pub agent_registry: Arc<RwLock<Vec<String>>>, // Simplified for now
    pub saved_view_registry: Arc<tokio::sync::RwLock<SavedViewRegistry>>,
    pub connection_stats: Arc<RwLock<ConnectionStats>>,
    pub app_handle: tauri::AppHandle,
    pub eval_registry: Arc<agentreplay_evals::EvaluatorRegistry>,
    pub eval_store: Arc<agentreplay_storage::EvalStore>,
    pub ingestion_queue: Arc<IngestionQueue>,
    pub project_store: Arc<RwLock<project_store::ProjectStore>>,
    pub trace_broadcaster: tokio::sync::broadcast::Sender<AgentFlowEdge>,
    pub llm_client: Arc<tokio::sync::RwLock<llm::LLMClient>>,
    /// Online evaluator for automatic trace evaluation (Gap #10)
    pub online_evaluator: Option<Arc<agentreplay_evals::OnlineEvaluator>>,
    /// Shutdown token for graceful server shutdown coordination
    pub shutdown_token: tokio_util::sync::CancellationToken,
}

/// Desktop application configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub ui: UiConfig,
    pub server_export: ServerExportConfig,
    pub ingestion_server: IngestionServerConfig,
    /// Retention/TTL configuration for automatic data cleanup
    #[serde(default)]
    pub retention: RetentionServerConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub auto_backup_enabled: bool,
    pub auto_backup_interval_hours: u32,
    pub max_backups_to_keep: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UiConfig {
    pub theme: String, // "light", "dark", "system"
    pub default_time_range_hours: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerExportConfig {
    pub enabled: bool,
    pub server_url: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IngestionServerConfig {
    pub enabled: bool,
    pub port: u16,
    pub host: String,
    pub auth_token: Option<String>,
    pub max_connections: usize,
}

/// Retention configuration for automatic TTL cleanup
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RetentionServerConfig {
    /// Enable automatic retention cleanup
    pub enabled: bool,
    /// Retention period in days (0 = unlimited, keep forever)
    pub retention_days: Option<u32>,
    /// Cleanup interval in hours (default: 24 = once daily)
    pub cleanup_interval_hours: u32,
}

impl Default for RetentionServerConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Enable by default for 30-day retention
            retention_days: Some(30), // 30 days default (0 or None = unlimited)
            cleanup_interval_hours: 24, // Run daily
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig {
                auto_backup_enabled: true,
                auto_backup_interval_hours: 24,
                max_backups_to_keep: 7,
            },
            ui: UiConfig {
                theme: "light".to_string(),
                default_time_range_hours: 24,
            },
            server_export: ServerExportConfig {
                enabled: false,
                server_url: None,
                api_key: None,
            },
            ingestion_server: IngestionServerConfig {
                enabled: true, // Enabled by default for local ingestion
                port: 47100, // Must match UI's hardcoded API_BASE_URL port
                host: "127.0.0.1".to_string(),
                auth_token: None, // No auth by default for local-only access
                max_connections: 1000,
            },
            retention: RetentionServerConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn load(app_handle: &tauri::AppHandle) -> Result<Self> {
        // First, check if AGENTREPLAY_CONFIG_PATH env var is set (for TOML config)
        if let Ok(toml_path) = std::env::var("AGENTREPLAY_CONFIG_PATH") {
            if std::path::Path::new(&toml_path).exists() {
                tracing::info!("Loading config from TOML file: {}", toml_path);
                return Self::load_from_toml(&toml_path);
            } else {
                tracing::warn!("AGENTREPLAY_CONFIG_PATH set but file not found: {}", toml_path);
            }
        }

        // Fall back to JSON config in app data directory
        let config_path = Self::config_path(app_handle)?;

        let mut config = if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            serde_json::from_str(&contents)?
        } else {
            Self::default()
        };

        // IMPORTANT: Always force ingestion port to 47100.
        // The UI hardcodes http://127.0.0.1:47100 as API_BASE_URL.
        // Old saved configs may have port 9600 which causes "Server not responding".
        if config.ingestion_server.port != 47100 {
            tracing::warn!(
                "Migrating ingestion port from {} to 47100 (must match UI)",
                config.ingestion_server.port
            );
            config.ingestion_server.port = 47100;
        }
        config.ingestion_server.host = "127.0.0.1".to_string();
        config.ingestion_server.enabled = true;

        // Save (creates or migrates)
        config.save(app_handle)?;
        Ok(config)
    }

    fn load_from_toml(path: &str) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let toml_config: toml::Value = toml::from_str(&contents)?;

        // Parse TOML config into AppConfig
        let mut config = Self::default();

        // Parse server config
        if let Some(server) = toml_config.get("server").and_then(|s| s.as_table()) {
            if let Some(addr) = server.get("listen_addr").and_then(|a| a.as_str()) {
                // Extract host and port from address (e.g., "127.0.0.1:47100")
                let parts: Vec<&str> = addr.split(':').collect();
                if parts.len() == 2 {
                    config.ingestion_server.host = parts[0].to_string();
                    if let Ok(port) = parts[1].parse::<u16>() {
                        config.ingestion_server.port = port;
                    }
                }
            }
        }

        // Parse storage config
        if let Some(storage) = toml_config.get("storage").and_then(|s| s.as_table()) {
            if let Some(data_dir) = storage.get("data_dir").and_then(|d| d.as_str()) {
                tracing::info!("TOML config specifies data_dir: {} (Tauri will use app data dir)", data_dir);
            }
        }

        // Parse auth config
        if let Some(auth) = toml_config.get("auth").and_then(|a| a.as_table()) {
            if let Some(enabled) = auth.get("enabled").and_then(|e| e.as_bool()) {
                if enabled {
                    tracing::warn!("Auth is enabled in TOML config, but Tauri app currently doesn't support auth tokens");
                }
            }
        }

        tracing::info!("Loaded config from TOML: ingestion server on {}:{}", 
            config.ingestion_server.host, config.ingestion_server.port);

        Ok(config)
    }

    pub fn save(&self, app_handle: &tauri::AppHandle) -> Result<()> {
        let config_path = Self::config_path(app_handle)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, json)?;
        Ok(())
    }

    fn config_path(app_handle: &tauri::AppHandle) -> Result<PathBuf> {
        let config_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| anyhow::anyhow!("Failed to get app data directory: {}", e))?;

        std::fs::create_dir_all(&config_dir)?;
        Ok(config_dir.join("config.json"))
    }
}

/// Get database path using platform-specific app data directory
fn get_db_path(app_handle: &tauri::AppHandle) -> Result<PathBuf> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| anyhow::anyhow!("Failed to get app data directory: {}", e))?;

    let db_path = app_data_dir.join("database");
    std::fs::create_dir_all(&db_path)?;

    tracing::info!("Database path: {:?}", db_path);
    Ok(db_path)
}

/// Initialize application state
fn initialize_app_state(app_handle: &tauri::AppHandle) -> Result<AppState> {
    // Get database path
    let db_path = get_db_path(app_handle)?;

    // Open Agentreplay database with high-performance WAL mode
    tracing::info!("Opening Agentreplay database at: {:?}", db_path);
    tracing::info!("Using high-performance WAL mode (Group Commit)");
    let db = Arc::new(Agentreplay::open_high_performance(&db_path)?);

    // Load configuration
    let config = Arc::new(RwLock::new(AppConfig::load(app_handle)?));

    // Initialize agent registry (simplified - will be expanded later)
    let agent_registry = Arc::new(RwLock::new(Vec::new()));

    // Initialize saved view registry
    let saved_view_registry = Arc::new(tokio::sync::RwLock::new(SavedViewRegistry::new(&db_path)));

    // Initialize connection stats
    let connection_stats = Arc::new(RwLock::new(ConnectionStats {
        total_traces_received: 0,
        last_trace_time: None,
        server_uptime_secs: 0,
        ingestion_rate_per_min: 0.0,
    }));

    // Initialize evaluation registry
    let eval_config = agentreplay_evals::EvalConfig {
        max_concurrent: 10,
        timeout_secs: 30,
        retry_on_failure: true,
        max_retries: 2,
        enable_cache: true,
        cache_ttl_secs: 3600,
    };
    let eval_registry = Arc::new(agentreplay_evals::EvaluatorRegistry::with_config(eval_config));

    // Register built-in local evaluators (no LLM required)
    {
        use agentreplay_evals::evaluators::{LatencyBenchmark, CostAnalyzer, TrajectoryEfficiencyEvaluator};
        
        // Latency evaluator - analyzes timing and performance
        if let Err(e) = eval_registry.register(Arc::new(LatencyBenchmark::new())) {
            tracing::warn!("Failed to register latency evaluator: {}", e);
        }
        
        // Cost evaluator - analyzes token usage and costs
        if let Err(e) = eval_registry.register(Arc::new(CostAnalyzer::new())) {
            tracing::warn!("Failed to register cost evaluator: {}", e);
        }
        
        // Trajectory efficiency evaluator - analyzes agent paths
        if let Err(e) = eval_registry.register(Arc::new(TrajectoryEfficiencyEvaluator::new())) {
            tracing::warn!("Failed to register trajectory evaluator: {}", e);
        }
        
        let count = eval_registry.list_evaluators().len();
        tracing::info!("Evaluation registry initialized ({} local evaluators registered)", count);
    }

    // Create ingestion queue with bounded channel
    // PERFORMANCE: Increased from 1000 to 10000 to handle burst traffic
    // At 10K spans/sec target, this provides ~1 second of buffering
    let (tx, mut rx) = tokio::sync::mpsc::channel::<AgentFlowEdge>(10_000);
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let (worker_done_tx, worker_done_rx) = tokio::sync::oneshot::channel::<()>();
    let ingestion_queue = Arc::new(IngestionQueue {
        tx,
        shutdown_tx: Arc::clone(&shutdown_notify),
        worker_done_rx: Arc::new(tokio::sync::RwLock::new(Some(worker_done_rx))),
    });

    // Create broadcast channel for real-time trace streaming (SSE) - must come BEFORE worker spawn
    // PERFORMANCE: Increased from 1024 to 4096 to handle higher throughput
    let (trace_tx, _trace_rx) = tokio::sync::broadcast::channel::<AgentFlowEdge>(4096);

    // Spawn background worker for batched writes using Tauri's async runtime
    let db_for_worker = Arc::clone(&db);
    let app_handle_for_worker = app_handle.clone();
    let stats_for_worker = Arc::clone(&connection_stats);
    let shutdown_notify_worker = Arc::clone(&shutdown_notify);
    let broadcaster_for_worker = trace_tx.clone();

    tauri::async_runtime::spawn(async move {
        let mut batch = Vec::new();
        // PERFORMANCE: Increased batch_size from 50 to 500 for better I/O amortization
        // With group commit WAL, larger batches reduce fsync frequency significantly
        // Expected improvement: ~10x throughput (500 spans/sec → 5000+ spans/sec)
        let batch_size = 500;
        // PERFORMANCE: Increased flush_interval from 100ms to 200ms
        // This allows more writes to accumulate, reducing fsync overhead
        // Trade-off: slightly higher latency (acceptable for observability workloads)
        let flush_interval = std::time::Duration::from_millis(200);

        tracing::info!("Background ingestion worker started");

        loop {
            tokio::select! {
                // Receive new edges
                Some(edge) = rx.recv() => {
                    batch.push(edge);

                    // Flush if batch is full
                    if batch.len() >= batch_size {
                        flush_batch(&db_for_worker, &mut batch, &app_handle_for_worker, &stats_for_worker, broadcaster_for_worker.clone()).await;
                    }
                }
                // Periodic flush
                _ = tokio::time::sleep(flush_interval) => {
                    if !batch.is_empty() {
                        flush_batch(&db_for_worker, &mut batch, &app_handle_for_worker, &stats_for_worker, broadcaster_for_worker.clone()).await;
                    }
                }
                // Shutdown signal
                _ = shutdown_notify_worker.notified() => {
                    let batch_size = batch.len();
                    tracing::info!(
                        batch_size = batch_size,
                        "SHUTDOWN: Ingestion worker received shutdown signal"
                    );

                    if !batch.is_empty() {
                        tracing::info!("SHUTDOWN: Flushing final batch of {} edges...", batch_size);
                        let start = std::time::Instant::now();
                        flush_batch(&db_for_worker, &mut batch, &app_handle_for_worker, &stats_for_worker, broadcaster_for_worker.clone()).await;
                        tracing::info!(
                            batch_size = batch_size,
                            flush_duration_ms = start.elapsed().as_millis(),
                            "SHUTDOWN: Final batch flushed successfully"
                        );
                    } else {
                        tracing::info!("SHUTDOWN: No pending edges to flush");
                    }
                    tracing::info!("Background ingestion worker stopped gracefully");
                    
                    // Signal that we're done - ignore error if receiver dropped
                    let _ = worker_done_tx.send(());
                    break;
                }
            }
        }
    });

    // Spawn background worker for metrics persistence
    let db_for_metrics = Arc::clone(&db);
    let shutdown_notify_metrics = Arc::clone(&shutdown_notify);
    
    tauri::async_runtime::spawn(async move {
        // flush metrics every 10 seconds
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        
        tracing::info!("Background metrics persistence worker started");
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = db_for_metrics.flush_metrics() {
                        tracing::error!("Failed to flush metrics: {}", e);
                    }
                }
                _ = shutdown_notify_metrics.notified() => {
                    tracing::info!("SHUTDOWN: Flushing final metrics...");
                    if let Err(e) = db_for_metrics.flush_metrics() {
                         tracing::error!("Failed to flush final metrics: {}", e);
                    }
                    tracing::info!("Background metrics persistence worker stopped");
                    break;
                }
            }
        }
    });

    // Initialize project store
    let project_store_path = get_db_path(app_handle)?.join("projects.json");
    let project_store = Arc::new(RwLock::new(
        project_store::ProjectStore::load(project_store_path.clone())
            .unwrap_or_else(|e| {
                tracing::error!("Failed to load project store: {}", e);
                project_store::ProjectStore::new(project_store_path)
            }),
    ));

    // Initialize LLM client - try to load persisted config first
    let llm_config = load_persisted_llm_config()
        .unwrap_or_else(llm::LLMConfig::default_with_ollama);
    tracing::info!("LLM client initialized with {} provider(s)", llm_config.providers.len());
    let llm_client = Arc::new(tokio::sync::RwLock::new(
        llm::LLMClient::new(llm_config)
    ));

    // Create shutdown token for graceful server shutdown coordination
    let shutdown_token = tokio_util::sync::CancellationToken::new();

    // Initialize persistent eval store
    let eval_store_path = get_db_path(app_handle)?.join("eval_metrics");
    let eval_store = Arc::new(
        agentreplay_storage::EvalStore::open(&eval_store_path)
            .map_err(|e| anyhow::anyhow!("Failed to open eval store: {}", e))?
    );
    tracing::info!("Eval store initialized at {:?}", eval_store_path);

    Ok(AppState {
        db,
        db_path,
        config,
        agent_registry,
        saved_view_registry,
        connection_stats,
        app_handle: app_handle.clone(),
        eval_registry,
        eval_store,
        ingestion_queue,
        project_store,
        trace_broadcaster: trace_tx,
        llm_client,
        // Online evaluator is None by default (can be enabled via settings)
        // This avoids automatic evaluation overhead unless explicitly enabled
        online_evaluator: None,
        shutdown_token,
    })
}

/// Flush batch of edges to database with proper error handling
async fn flush_batch(
    db: &Arc<Agentreplay>,
    batch: &mut Vec<AgentFlowEdge>,
    app_handle: &tauri::AppHandle,
    stats: &Arc<RwLock<ConnectionStats>>,
    broadcaster: tokio::sync::broadcast::Sender<AgentFlowEdge>,
) {
    if batch.is_empty() {
        return;
    }

    let count = batch.len();
    tracing::debug!("Flushing batch of {} edges", count);

    // Perform batch insert (blocking operation in thread pool)
    let db_clone = Arc::clone(db);
    let batch_to_insert = std::mem::take(batch);
    let app_handle_clone = app_handle.clone();
    let stats_clone = Arc::clone(stats);

    let result = tokio::spawn(async move {
        // Attempt database write
        if let Err(e) = db_clone.insert_batch(&batch_to_insert).await {
            tracing::error!(
                "Failed to insert batch of {} edges: {}",
                batch_to_insert.len(),
                e
            );
            return Err(e);
        }

        // **DURABILITY FIX**: Sync to disk after batch write
        // This ensures the payload index is persisted, preventing data loss on crash
        if let Err(e) = db_clone.sync() {
            tracing::warn!("Failed to sync after batch write: {}", e);
            // Non-fatal: data is in WAL, can be recovered
        }

        // Update stats AFTER successful write (prevents inflated stats on crash)
        {
            let mut stats = stats_clone.write();
            stats.total_traces_received += batch_to_insert.len() as u64;
            stats.last_trace_time = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_micros() as u64,
            );
        }

        // Emit events for UI updates AFTER successful write
        for edge in &batch_to_insert {
            let event_payload = serde_json::json!({
                "edge_id": format!("{:#x}", edge.edge_id),
                "timestamp_us": edge.timestamp_us,
                "agent_id": edge.agent_id,
                "span_type": edge.get_span_type() as u32,
            });

            if let Err(e) = app_handle_clone.emit("trace_ingested", event_payload) {
                tracing::warn!("Failed to emit trace_ingested event: {}", e);
            }
        }

        // Broadcast traces for SSE clients
        for edge in &batch_to_insert {
            // Ignore send errors (no subscribers is fine)
            let _ = broadcaster.send(*edge);
        }

        Ok(batch_to_insert.len())
    })
    .await;

    match result {
        Ok(Ok(flushed_count)) => {
            tracing::debug!("Successfully flushed {} edges", flushed_count);
        }
        Ok(Err(e)) => {
            tracing::error!("Database write failed: {}", e);
            // TODO: Could implement retry logic here
        }
        Err(e) => {
            tracing::error!("Flush task panicked: {}", e);
        }
    }
}

fn main() {
    // Initialize tracing with file output for production troubleshooting
    let log_dir = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".agentreplay"))
        .unwrap_or_else(|_| PathBuf::from(".agentreplay"));

    std::fs::create_dir_all(&log_dir).ok();
    let log_file = log_dir.join("agentreplay-desktop.log");

    let file_appender = tracing_appender::rolling::never(&log_dir, "agentreplay-desktop.log");

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "agentreplay=info,agentreplay_query=info,agentreplay_core=info".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::fmt::layer().with_writer(file_appender))
        .init();

    tracing::info!("Starting Agentreplay Desktop Application");
    tracing::info!("Logs are being written to: {:?}", log_file);

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build());

    // Add nspanel plugin for macOS
    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .setup(|app| {
            // Set activation policy to Regular mode (shows dock icon)
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Regular);
            }

            // Menu bar disabled for cleaner UI - navigation is in-app
            // But we enable it for specific tools like Memory
            let menu = menu::create_app_menu(app.handle())
                .expect("Failed to create app menu");
            app.set_menu(menu).expect("Failed to set menu");

            // Handle menu events
            let app_handle = app.handle().clone();
            app.on_menu_event(move |app, event| {
                match event.id().as_ref() {
                    "open_memory" => {
                        windows::memory_window::open_memory_window(app);
                    }
                    "quit" => {
                        app.exit(0);
                    }
                     // Add other menu actions here if needed
                    _ => {}
                }
            });

            // Initialize application state
            let state =
                initialize_app_state(app.handle()).expect("Failed to initialize application state");

            // Manage state in Tauri
            app.manage(state.clone());
            
            // Initialize system info state for real-time metrics
            let sysinfo_state = sysinfo_state::SysInfoState::new();
            app.manage(sysinfo_state);
            tracing::info!("System info state initialized");

            // Initialize plugin state
            let plugin_state = plugins::PluginState::new();
            let plugin_data_dir = state.db_path.clone();
            let plugin_state_clone = plugin_state.manager.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = plugins::PluginState::init_with_arc(&plugin_state_clone, plugin_data_dir).await {
                    tracing::error!("Failed to initialize plugin manager: {}", e);
                } else {
                    tracing::info!("Plugin manager initialized");
                }
            });
            app.manage(plugin_state);

            // Start embedded HTTP server if enabled
            let ingestion_enabled = state.config.read().ingestion_server.enabled;
            if ingestion_enabled {
                let host = state.config.read().ingestion_server.host.clone();
                let port = state.config.read().ingestion_server.port;
                let server_state = state.clone();

                tracing::info!("Starting embedded ingestion server on {}:{}", host, port);

                tauri::async_runtime::spawn(async move {
                    match server::start_embedded_server(host.clone(), port, server_state).await {
                        Ok(_) => {
                            tracing::info!("Embedded server on {}:{} stopped", host, port);
                        }
                        Err(e) => {
                            let error_msg = format!(
                                "CRITICAL: Embedded server failed to start on {}:{}: {}",
                                host, port, e
                            );
                            tracing::error!("{}", error_msg);
                            eprintln!("{}", error_msg);
                        }
                    }
                });
            } else {
                tracing::info!("Ingestion server disabled in configuration");
            }

            // Start dedicated MCP server on port 47101 (isolated from ingestion)
            let mcp_state = state.clone();
            // Re-use host from ingestion config, but force port 47101
            let mcp_host = state.config.read().ingestion_server.host.clone(); 
            let mcp_port = 47101; 
            
            tracing::info!("Starting dedicated MCP server on {}:{}", mcp_host, mcp_port);
            tauri::async_runtime::spawn(async move {
                match server::start_mcp_server(mcp_host.clone(), mcp_port, mcp_state).await {
                    Ok(_) => {
                         tracing::info!("MCP server on {}:{} stopped", mcp_host, mcp_port);
                    },
                    Err(e) => {
                         tracing::error!("CRITICAL: MCP server failed to start on {}:{}: {}", mcp_host, mcp_port, e);
                    }
                }
            });

            // Start OTLP gRPC server on port 47117
            let otlp_grpc_state = state.clone();
            tracing::info!("Starting OTLP gRPC server on 127.0.0.1:47117");
            tauri::async_runtime::spawn(async move {
                match otlp_server::start_otlp_grpc_server(otlp_grpc_state).await {
                    Ok(_) => {
                        tracing::info!("OTLP gRPC server stopped");
                    }
                    Err(e) => {
                        tracing::error!("OTLP gRPC server failed: {}", e);
                    }
                }
            });

            // Start OTLP HTTP server on port 4318
            let otlp_http_state = state.clone();
            tracing::info!("Starting OTLP HTTP server on 127.0.0.1:4318");
            tauri::async_runtime::spawn(async move {
                match otlp_server::start_otlp_http_server(otlp_http_state).await {
                    Ok(_) => {
                        tracing::info!("OTLP HTTP server stopped");
                    }
                    Err(e) => {
                        tracing::error!("OTLP HTTP server failed: {}", e);
                    }
                }
            });

            // Start background retention worker for automatic TTL cleanup
            let retention_config = state.config.read().retention.clone();
            if retention_config.enabled {
                let retention_db = Arc::clone(&state.db);
                let retention_days = retention_config.retention_days;
                let interval_hours = retention_config.cleanup_interval_hours;

                tracing::info!(
                    retention_days = ?retention_days,
                    interval_hours = interval_hours,
                    "Starting background retention worker"
                );

                tauri::async_runtime::spawn(async move {
                    // Wait a bit before first cleanup to let the app start
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

                    let interval = tokio::time::Duration::from_secs(interval_hours as u64 * 3600);
                    let mut ticker = tokio::time::interval(interval);

                    loop {
                        ticker.tick().await;

                        // Check if retention is unlimited (None or 0)
                        if retention_days.is_none() || retention_days == Some(0) {
                            tracing::debug!("Retention is unlimited, skipping cleanup");
                            continue;
                        }

                        let days = retention_days.unwrap_or(30);
                        let cutoff_us = {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_micros() as u64;
                            let retention_period_us = days as u64 * 24 * 60 * 60 * 1_000_000;
                            now.saturating_sub(retention_period_us)
                        };

                        tracing::info!(
                            cutoff_us = cutoff_us,
                            retention_days = days,
                            "Running scheduled retention cleanup"
                        );

                        match retention_db.delete_traces_before(cutoff_us).await {
                            Ok(stats) => {
                                if stats.traces_deleted > 0 {
                                    tracing::info!(
                                        deleted = stats.traces_deleted,
                                        duration_ms = stats.cleanup_duration_ms,
                                        "Retention cleanup completed"
                                    );
                                } else {
                                    tracing::debug!("No expired traces to delete");
                                }
                            }
                            Err(e) => {
                                tracing::error!("Retention cleanup failed: {}", e);
                            }
                        }
                    }
                });
            } else {
                tracing::info!("Retention worker disabled in configuration");
            }
            // Setup tray icon if supported
            #[cfg(desktop)]
            {
                use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder};

                let _tray = TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .on_tray_icon_event(|tray, event| {
                        if let tauri::tray::TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();

                            // Show and focus the main window
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                                let _ = window.unminimize();
                            }
                        }
                    })
                    .build(app)
                    .expect("Failed to build tray icon");
            }

            // CRITICAL FIX: Register cleanup handler for graceful shutdown
            // In Tauri v2, we use window close event handlers for cleanup
            let cleanup_state = state.clone();

            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "macos")]
                {
                    if let Err(err) = window.set_title_bar_style(TitleBarStyle::Overlay) {
                        tracing::warn!("Failed to apply macOS overlay titlebar: {}", err);
                    }
                }

                #[cfg(target_os = "windows")]
                {
                    if let Err(err) =
                        window.set_effects(Some(EffectsBuilder::new().effect(Effect::Mica).build()))
                    {
                        tracing::warn!("Failed to enable Windows Mica: {}", err);
                    }
                }
                // Apply enhanced window vibrancy effects on macOS
                #[cfg(target_os = "macos")]
                {
                    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};

                    // Try different materials in priority order
                    let materials = [
                        NSVisualEffectMaterial::UnderWindowBackground,
                        NSVisualEffectMaterial::Popover,
                        NSVisualEffectMaterial::Sidebar,
                        NSVisualEffectMaterial::WindowBackground,
                    ];

                    let mut vibrancy_applied = false;
                    for material in materials.iter() {
                        // Try with 12.0px corner radius for polished appearance
                        if apply_vibrancy(&window, *material, None, Some(12.0)).is_ok() {
                            tracing::info!(
                                "Applied vibrancy material: {:?} with rounded corners",
                                material
                            );
                            vibrancy_applied = true;
                            break;
                        }
                    }

                    if !vibrancy_applied {
                        // Fallback without corner radius
                        if let Err(e) = apply_vibrancy(
                            &window,
                            NSVisualEffectMaterial::WindowBackground,
                            None,
                            None,
                        ) {
                            tracing::warn!("Failed to apply window vibrancy: {}", e);
                        } else {
                            tracing::info!("Applied fallback vibrancy without rounded corners");
                        }
                    }
                }

                // DISABLED: Floating panel breaks dock activation
                // Using regular window instead for better macOS integration
                // #[cfg(target_os = "macos")]
                // {
                //     match window.to_panel::<AgentreplayPanel>() {
                //         Ok(panel) => {
                //             tracing::info!("Successfully converted window to macOS panel");
                //             panel.set_level(25);
                //             panel.show();
                //         }
                //         Err(e) => {
                //             tracing::error!("Failed to convert to panel: {}", e);
                //         }
                //     }
                // }

                let window_label = window.label().to_string();
                let app_handle_for_cleanup = app.handle().clone();

                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        tracing::info!("Window close requested, starting graceful shutdown...");

                        // Prevent default close behavior temporarily
                        api.prevent_close();

                        // Clone state for async task
                        let cleanup_state_clone = cleanup_state.clone();
                        let window_label_clone = window_label.clone();
                        let app_handle_clone = app_handle_for_cleanup.clone();

                        // Spawn async cleanup task
                        tauri::async_runtime::spawn(async move {
                            tracing::info!("Starting async cleanup...");

                            // 1. Signal all servers to shutdown gracefully
                            cleanup_state_clone.shutdown_token.cancel();
                            tracing::info!("Server shutdown signal sent (HTTP + OTLP)");

                            // 2. Signal ingestion worker to shutdown
                            cleanup_state_clone.ingestion_queue.shutdown();
                            tracing::info!("Ingestion queue shutdown signal sent");

                            // 3. Wait for servers to finish in-flight requests (up to 2 seconds)
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            tracing::info!("Server graceful shutdown period complete");

                            // 4. CRITICAL FIX: Actually wait for worker to complete, not just sleep
                            // This ensures all pending edges are flushed before we close the database
                            // Timeout of 5 seconds should be sufficient for even large batches
                            let worker_completed = cleanup_state_clone
                                .ingestion_queue
                                .wait_for_shutdown(tokio::time::Duration::from_secs(5))
                                .await;
                            
                            if worker_completed {
                                tracing::info!("Ingestion worker shutdown completed, proceeding to database close");
                            } else {
                                tracing::warn!("Ingestion worker did not complete in time, proceeding anyway");
                            }

                            // 3. Close database (in a spawn_blocking to avoid blocking runtime)
                            let db_clone = Arc::clone(&cleanup_state_clone.db);
                            let close_result =
                                tokio::task::spawn_blocking(move || db_clone.close()).await;

                            match close_result {
                                Ok(Ok(())) => {
                                    tracing::info!("Database closed successfully");
                                }
                                Ok(Err(e)) => {
                                    tracing::error!("Failed to close database: {}", e);
                                }
                                Err(e) => {
                                    tracing::error!("Database close task panicked: {}", e);
                                }
                            }

                            tracing::info!("Cleanup complete, closing window");

                            // Force process exit after graceful cleanup
                            // This ensures all background tasks (servers, retention worker) terminate
                            tracing::info!("All cleanup complete, forcing process exit");
                            
                            // Use libc::_exit for immediate termination without running atexit handlers
                            // This bypasses any blocking cleanup that might hang the process
                            #[cfg(unix)]
                            unsafe {
                                libc::_exit(0);
                            }
                            #[cfg(windows)]
                            std::process::exit(0);
                        });
                    }
                });
            }

            tracing::info!("Agentreplay Desktop initialized successfully");

            #[allow(unreachable_code)]
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Configuration commands
            commands::get_config,
            commands::update_config,
            // Database commands
            commands::get_db_stats,
            commands::get_write_stall_stats,
            commands::health_check,
            // Trace commands (core functionality)
            commands::list_traces,
            commands::get_trace,
            commands::search_traces,
            commands::ingest_traces,
            commands::get_trace_stats,
            commands::delete_session,
            commands::delete_trace,
            // Analytics commands
            commands::get_timeseries,
            commands::get_costs,
            // Connection monitoring
            commands::get_connection_stats,
            commands::get_connection_health,
            // Projects & Agents
            commands::list_projects,
            commands::list_agents,
            commands::register_agent,
            // Backup commands
            commands::create_backup,
            commands::list_backups,
            commands::export_backup_with_dialog,
            commands::import_backup_with_dialog,
            commands::restore_backup,
            // Window management
            commands::open_trace_window,
            // Server export commands (hybrid mode)
            commands::export_traces_to_server,
            // Evaluation commands
            commands::list_evaluators,
            commands::run_evaluation,
            commands::get_evaluation_summary,
            // CIP commands
            commands::run_cip_evaluation,
            commands::get_cip_info,
            // Settings commands (User/Project/Local scopes)
            commands::get_agentreplay_settings,
            commands::save_agentreplay_settings,
            commands::sync_llm_settings,
            commands::get_current_project_path,
            // System commands
            commands::os_type,
            sysinfo_state::get_all_system_info,
            sysinfo_state::get_memory_info,
            sysinfo_state::get_cpu_info,
            sysinfo_state::get_static_system_info,
            // Update commands
            commands::check_for_updates,
            // Reset/Delete commands
            commands::reset_all_data,
            // Model comparison commands
            commands::compare_models,
            commands::list_comparison_models,
            commands::get_model_pricing,
            commands::sync_model_pricing,
            commands::calculate_model_cost,
            // Insights commands
            commands::get_insights,
            commands::get_insights_summary,
            // Plugin commands
            plugins::plugin_list,
            plugins::plugin_get,
            plugins::plugin_install,
            plugins::plugin_uninstall,
            plugins::plugin_enable,
            plugins::plugin_disable,
            plugins::plugin_get_settings,
            plugins::plugin_update_settings,
            plugins::plugin_search,
            plugins::plugin_reload,
            plugins::plugin_scan,
            plugins::plugin_get_dir,
            plugins::plugin_stats,
            // Bundle plugin commands (Schema v2+) - EXPERIMENTAL - Commented out
            // TODO: Uncomment when PluginManager methods are implemented
            plugins::plugin_bundle_info,
            plugins::plugin_bundle_targets,
            plugins::plugin_bundle_detect,
            plugins::plugin_bundle_install_md,
            plugins::plugin_bundle_variables,
            plugins::plugin_bundle_plan,
            plugins::plugin_bundle_execute,
            // Storage health commands (Gap #1, #2, #3, #10)
            commands::get_mvcc_stats,
            commands::get_tombstone_gc_stats,
            commands::get_bloom_filter_stats,
            commands::get_write_amplification_stats,
            commands::get_storage_health,
            // I/O performance commands (Gap #8)
            commands::get_io_performance_mode,
            commands::list_io_performance_modes,
            // Evaluator preset commands (Gap #7)
            commands::list_eval_presets,
            commands::list_eval_categories,
            // Sharded metrics & sketches commands (Phase 1-3)
            commands::get_sharded_metrics_stats,
            commands::query_sharded_timeseries,
            commands::get_sketch_capabilities,
            commands::get_sketch_memory_usage,
            // Annotation commands (Gap #8)
            commands::create_annotation,
            commands::get_annotations,
            commands::get_annotation_stats,
            commands::get_annotation_campaign,
            commands::create_annotation_campaign,
            commands::delete_annotation,
            // Online evaluator commands (Gap #10)
            commands::get_online_eval_settings,
            commands::update_online_eval_settings,
            // Dataset Flywheel commands (Fine-tuning export)
            commands::export_finetuning_dataset,
            commands::get_finetuning_candidates,
            // Time-Travel Debugging commands (Trace forking)
            commands::fork_trace_state,
            commands::get_trace_conversation,
            commands::preview_fork,
            commands::get_bridge_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
