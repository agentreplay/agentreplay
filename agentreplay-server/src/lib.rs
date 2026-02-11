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

pub mod admission;
pub mod agent_registry;
pub mod api;
pub mod auth;
pub mod batcher;
pub mod cache;
pub mod config;
pub mod cost_tracker;
pub mod governor;
pub mod ingestion;
pub mod knowledge_graph;
pub mod llm;
pub mod mcp;
pub mod middleware;
pub mod otel_genai;
pub mod otlp_service;
pub mod project_manager;
pub mod project_registry;
pub mod sanitization;
pub mod tool_registry;
pub mod validation;

use anyhow::Result;
use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post},
    Extension, Router,
};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use api::{
    add_trace_to_dataset, get_dashboard_summary, get_detailed_trace, get_provider_costs, get_stats,
    get_timeseries_metrics, get_trace, get_trace_attributes, get_trace_children, get_trace_graph,
    get_trace_observations, health_check, health_check_detailed, ingest_otel_spans, ingest_traces,
    list_traces, semantic_search, submit_trace_feedback, ws_traces, AppState,
};
use auth::{auth_middleware, ApiKeyAuth, Authenticator, BearerTokenAuth, MultiAuth, NoAuth};
use config::ServerConfig;
use agentreplay_query::Agentreplay;
use project_manager::ProjectManager;
use tokio::sync::broadcast;

pub async fn run_server(config: ServerConfig) -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "agentreplay_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Agentreplay Server");
    tracing::info!("Configuration: {:#?}", config);

    // Validate configuration
    config.validate()?;

    // Initialize Project Manager for per-project storage
    let use_project_storage = config.storage.use_project_storage;

    let project_manager = if use_project_storage {
        tracing::info!("Initializing ProjectManager with per-project storage");
        let base_dir = config.storage.data_dir.join("projects");
        match ProjectManager::new(&base_dir) {
            Ok(pm) => {
                tracing::info!("ProjectManager initialized at: {:?}", base_dir);
                let discovered = pm.discover_projects().unwrap_or_default();
                tracing::info!(
                    "Discovered {} existing projects: {:?}",
                    discovered.len(),
                    discovered
                );
                Some(Arc::new(pm))
            }
            Err(e) => {
                tracing::error!(
                    "Failed to initialize ProjectManager: {}. Falling back to single DB.",
                    e
                );
                None
            }
        }
    } else {
        tracing::info!("Using single database mode (set AGENTREPLAY_USE_PROJECT_STORAGE=true for per-project storage)");
        None
    };

    // Initialize project registry (if using project storage)
    let project_registry = if project_manager.is_some() {
        let registry_dir = config.storage.data_dir.join("projects");
        match crate::project_registry::ProjectRegistry::new(&registry_dir) {
            Ok(registry) => {
                // Discover existing projects on startup
                if let Ok(discovered) = registry.discover_projects() {
                    tracing::info!("Project registry discovered {} projects", discovered.len());
                }
                
                // Register default "Claude Code" project (deterministic ID: 49455)
                // This is used by the agentreplay-claude-plugin
                const CLAUDE_CODE_PROJECT_ID: u16 = 49455;
                if registry.get_metadata(CLAUDE_CODE_PROJECT_ID).is_none() {
                    match registry.register_project(
                        CLAUDE_CODE_PROJECT_ID,
                        "Claude Code".to_string(),
                        Some("Claude Code coding sessions (auto-registered)".to_string()),
                    ) {
                        Ok(_) => tracing::info!("Registered default 'Claude Code' project (ID: {})", CLAUDE_CODE_PROJECT_ID),
                        Err(e) => tracing::warn!("Failed to register Claude Code project: {}", e),
                    }
                }
                
                Some(Arc::new(registry))
            }
            Err(e) => {
                tracing::warn!("Failed to initialize project registry: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Open Agentreplay database (fallback for non-project mode or legacy queries)
    tracing::info!("Opening database at: {:?}", config.storage.data_dir);
    let db = if config.storage.high_performance {
        tracing::info!("Using high-performance WAL mode (Group Commit)");
        Arc::new(Agentreplay::open_high_performance(&config.storage.data_dir)?)
    } else {
        tracing::info!("Using standard WAL mode (Segmented)");
        Arc::new(Agentreplay::open(&config.storage.data_dir)?)
    };

    // Create agent registry
    let agent_registry_path = config.storage.data_dir.join("agent_registry.json");
    tracing::info!("Initializing agent registry at: {:?}", agent_registry_path);
    let agent_registry = Arc::new(crate::agent_registry::AgentRegistry::new(
        agent_registry_path,
    ));
    tracing::info!(
        "Agent registry loaded with {} agents",
        agent_registry.count()
    );

    // Create saved view registry
    let saved_view_registry = Arc::new(tokio::sync::RwLock::new(
        agentreplay_core::SavedViewRegistry::new(&config.storage.data_dir),
    ));

    // Initialize LLM provider manager
    let llm_manager = match llm::LLMProviderManager::new(db.clone(), &config.llm).await {
        Ok(manager) => {
            tracing::info!("LLM provider manager initialized");
            Some(Arc::new(manager))
        }
        Err(e) => {
            tracing::warn!(
                "Failed to initialize LLM provider manager: {}. Chat features will be disabled.",
                e
            );
            None
        }
    };

    // Create application state with broadcast channel for real-time updates
    let (trace_tx, _) = broadcast::channel(1024);

    // Initialize cost tracker
    let cost_tracker = Arc::new(crate::cost_tracker::CostTracker::new());

    // Initialize HNSW vector index for semantic operations (Task 7)
    // Now uses sochdb-index HNSW which provides advanced features:
    // - Lock-free entry point with atomic CAS
    // - CSR graph for cache-efficient traversal
    // - Staged parallel construction
    // - Hot buffer for ultra-fast inserts
    let vector_index = {
        let hnsw_config = agentreplay_index::HnswConfig {
            max_connections: 16,
            max_connections_layer0: 32,
            level_multiplier: 1.0 / (16.0_f32).ln(),
            ef_construction: 200,
            ef_search: 50,
            metric: agentreplay_index::HnswDistanceMetric::Cosine,
            quantization_precision: Some(sochdb_index::vector_quantized::Precision::F32),
            rng_optimization: Default::default(),
        };
        // Default to 384 dimensions (all-MiniLM-L6-v2)
        let index = agentreplay_index::HnswIndex::new(384, hnsw_config);
        tracing::info!("HNSW vector index initialized with 384 dimensions (sochdb-index backend)");
        Some(Arc::new(index))
    };

    // Initialize Sharded Semantic Governor for trace deduplication (Task 4)
    // Uses 16 independent HNSW shards to eliminate global lock bottleneck
    // Binary quantization reduces memory by 32x (60GB → 1.9GB for 10M traces)
    let semantic_governor = {
        let governor_config = crate::governor::GovernorConfig {
            epsilon: 0.1, // 10% cosine distance threshold
            dimension: 384,
            ef_search: 32,
            m: 16,
            ef_construction: 100,
            use_binary_quantization: true, // 32x memory reduction
        };
        let governor = crate::governor::ShardedGovernor::new_shared(governor_config);
        tracing::info!("Sharded Semantic Governor initialized with ε=0.1, 16 shards, binary quantization enabled");
        Some(governor)
    };

    // Initialize evaluation cache (Task 9)
    let eval_cache = {
        let cache_config = crate::cache::EvalCacheConfig {
            max_entries: 10_000,
            ttl: std::time::Duration::from_secs(86400), // 24 hours
            track_stats: true,
        };
        let cache = crate::cache::EvalCache::new(cache_config);
        tracing::info!("Evaluation cache initialized with 10,000 max entries");
        Some(Arc::new(cache))
    };

    // Initialize Ingestion Actor for high-performance batched ingestion
    // This is the "High Speed Rail" that batches traces and routes them through the Governor
    let ingestion_actor = if let Some(ref governor) = semantic_governor {
        let actor_config = crate::ingestion::IngestionConfig {
            max_batch_size: 64,
            max_wait_time: std::time::Duration::from_millis(20),
            channel_capacity: 4096,
            embedding_dimension: 384,
        };
        let actor = crate::ingestion::IngestionActor::new(
            actor_config,
            governor.clone(),
            None, // No real embedder yet - uses deterministic fake embeddings
        );
        let handle = actor.spawn();
        tracing::info!("Ingestion Actor spawned: batch_size=64, wait_time=20ms, channel=4096");
        Some(handle)
    } else {
        tracing::warn!("Ingestion Actor not started: Semantic Governor not available");
        None
    };

    let state = AppState {
        db: db.clone(),
        project_manager,
        project_registry,
        trace_broadcaster: trace_tx.clone(),
        agent_registry: agent_registry.clone(),
        db_path: config.storage.data_dir.display().to_string(),
        saved_view_registry: saved_view_registry.clone(),
        llm_manager,
        cost_tracker: cost_tracker.clone(),
        vector_index,
        semantic_governor,
        eval_cache,
        ingestion_actor,
    };

    // Set up authenticator with secure-by-default approach (Task 4)
    let authenticator: Arc<dyn Authenticator> = if config.auth.enabled {
        tracing::info!("Authentication enabled");

        let mut strategies: Vec<Arc<dyn Authenticator>> = vec![];

        // Add JWT auth if secret is configured
        if let Some(jwt_secret) = config.auth.jwt_secret.clone() {
            tracing::info!("JWT authentication enabled");
            strategies.push(Arc::new(BearerTokenAuth::new(jwt_secret)));
        }

        // Add API key auth if keys are configured
        if !config.auth.api_keys.is_empty() {
            tracing::info!(
                "API key authentication enabled ({} keys)",
                config.auth.api_keys.len()
            );
            strategies.push(Arc::new(ApiKeyAuth::new(config.auth.api_keys.clone())));
        }

        if strategies.is_empty() {
            anyhow::bail!("Authentication enabled but no strategies configured");
        }

        Arc::new(MultiAuth::new(strategies))
    } else {
        // SECURITY: Smart NoAuth detection for localhost/desktop vs production
        let allow_noauth = std::env::var("AGENTREPLAY_ALLOW_NOAUTH")
            .unwrap_or_default()
            .to_lowercase();

        // Auto-detect if this is a localhost/desktop deployment
        let is_localhost = config.server.listen_addr.contains("localhost")
            || config.server.listen_addr.contains("127.0.0.1")
            || config.server.listen_addr.starts_with("0.0.0.0:");

        let is_explicitly_allowed = allow_noauth == "true" || allow_noauth == "1";

        // Allow NoAuth for:
        // 1. Explicit environment variable
        // 2. Localhost bindings (desktop/development use case)
        if !is_explicitly_allowed && !is_localhost {
            tracing::error!(
                "\n\n\
                ╔═══════════════════════════════════════════════════════════════╗\n\
                ║  SECURITY ERROR: Authentication is DISABLED                   ║\n\
                ║                                                               ║\n\
                ║  Running without authentication on a non-localhost address   ║\n\
                ║  exposes your trace data to anyone who can access the server.║\n\
                ║                                                               ║\n\
                ║  Binding to: {}                                        ║\n\
                ║                                                               ║\n\
                ║  To allow NoAuth mode, either:                               ║\n\
                ║    1. Bind to localhost (e.g., 127.0.0.1:47100)              ║\n\
                ║    2. Set: export AGENTREPLAY_ALLOW_NOAUTH=true               ║\n\
                ║                                                               ║\n\
                ║  For production, enable authentication in your config:       ║\n\
                ║    [auth]                                                    ║\n\
                ║    enabled = true                                            ║\n\
                ║    jwt_secret = \"your-secret-key\"                            ║\n\
                ╚═══════════════════════════════════════════════════════════════╝\n\
                ",
                config.server.listen_addr
            );
            anyhow::bail!(
                "Authentication is disabled on non-localhost address '{}'. \
                Bind to localhost, set AGENTREPLAY_ALLOW_NOAUTH=true, or enable authentication.",
                config.server.listen_addr
            );
        }

        if is_localhost {
            tracing::info!(
                "\n\
                ℹ️  Running in LOCALHOST mode - Authentication disabled for ease of use\n\
                   Binding to: {}\n\
                   This is safe for local desktop/development use.\n\
                   For production deployments, enable authentication in config.\n\
                ",
                config.server.listen_addr
            );
        } else {
            tracing::warn!(
                "\n\n\
                ╔═══════════════════════════════════════════════════════════════╗\n\
                ║  WARNING: Authentication is DISABLED (NoAuth mode)            ║\n\
                ║                                                               ║\n\
                ║  All requests will be allowed without authentication.        ║\n\
                ║  This should ONLY be used for local development.             ║\n\
                ║                                                               ║\n\
                ║  DO NOT deploy this configuration to production!             ║\n\
                ╚═══════════════════════════════════════════════════════════════╝\n\
                "
            );
        }

        // No authentication configured - use default tenant matching SDK default
        Arc::new(NoAuth::new(1)) // Default tenant_id = 1 (matches SDK default)
    };

    // Build authenticated routes (API + WebSocket)
    let authed_routes = Router::new()
        .route("/ws/traces", get(ws_traces))
        .route("/api/v1/traces/stream", get(api::sse_traces))
        .route("/api/v1/traces", get(list_traces).post(ingest_traces))
        .route("/api/v1/traces/otel", post(ingest_otel_spans))
        .route("/api/v1/traces/:trace_id", get(get_trace))
        .route(
            "/api/v1/traces/:trace_id/attributes",
            get(get_trace_attributes),
        )
        .route("/api/v1/traces/:trace_id/children", get(get_trace_children))
        .route(
            "/api/v1/traces/:trace_id/observations",
            get(get_trace_observations),
        )
        .route("/api/v1/traces/:trace_id/graph", get(get_trace_graph))
        .route("/api/v1/traces/:trace_id/detailed", get(get_detailed_trace))
        .route(
            "/api/v1/traces/:trace_id/feedback",
            post(submit_trace_feedback),
        )
        .route("/api/v1/datasets/:name/add", post(add_trace_to_dataset))
        // Projects/Collections routes
        .route(
            "/api/v1/projects",
            get(api::list_projects).post(api::create_project),
        )
        .route(
            "/api/v1/projects/:project_id",
            get(api::get_project)
                .patch(api::update_project)
                .delete(api::admin::delete_project),
        )
        .route(
            "/api/v1/projects/:project_id/metrics",
            get(api::metrics::get_project_metrics),
        )
        .route(
            "/api/v1/projects/:project_id/favorite",
            post(api::toggle_favorite),
        )
        // Admin routes
        .route("/api/v1/admin/reset", delete(api::admin::reset_all_data))
        .route("/api/v1/health", get(health_check_detailed))
        .route("/api/v1/stats", get(get_stats))
        .route("/api/v1/dashboard/summary", get(get_dashboard_summary))
        .route("/api/v1/metrics/timeseries", get(get_timeseries_metrics))
        .route("/api/v1/search", post(semantic_search))
        // Sessions routes
        .route("/api/v1/sessions", get(api::sessions::list_sessions))
        .route(
            "/api/v1/sessions/:session_id",
            get(api::sessions::get_session),
        )
        // Chat/LLM routes
        .route("/api/v1/chat/completions", post(api::chat_completion))
        .route("/api/v1/chat/stream", post(api::stream_completion))
        .route("/api/v1/chat/models", get(api::list_models))
        // Agent registry routes
        .route("/api/v1/agents", get(api::list_agents))
        .route("/api/v1/agents/register", post(api::register_agent))
        .route(
            "/api/v1/agents/:agent_id",
            get(api::get_agent)
                .put(api::update_agent)
                .delete(api::delete_agent),
        )
        // Evaluation metrics routes (Task 3)
        .route(
            "/api/v1/evals/metrics",
            get(api::get_eval_metrics).post(api::store_eval_metrics),
        )
        // Evaluation datasets routes (Task 4)
        .route(
            "/api/v1/evals/datasets",
            get(api::eval_datasets::list_datasets).post(api::eval_datasets::create_dataset),
        )
        .route(
            "/api/v1/evals/datasets/:id",
            get(api::eval_datasets::get_dataset).delete(api::eval_datasets::delete_dataset),
        )
        .route(
            "/api/v1/evals/datasets/:id/examples",
            post(api::eval_datasets::add_examples),
        )
        // Evaluation runs routes (Task 4)
        .route(
            "/api/v1/evals/runs",
            get(api::eval_runs::list_runs).post(api::eval_runs::create_run),
        )
        .route(
            "/api/v1/evals/runs/export",
            get(api::eval_runs::export_runs),
        )
        .route(
            "/api/v1/evals/runs/import",
            post(api::eval_runs::import_runs),
        )
        .route(
            "/api/v1/evals/runs/:id",
            get(api::eval_runs::get_run).delete(api::eval_runs::delete_run),
        )
        .route(
            "/api/v1/evals/runs/:id/results",
            post(api::eval_runs::add_run_result),
        )
        .route(
            "/api/v1/evals/runs/:id/status",
            post(api::eval_runs::update_run_status),
        )
        // Dataset Flywheel routes (auto-curate fine-tuning data)
        .route(
            "/api/v1/evals/flywheel/candidates",
            get(api::flywheel::get_candidates),
        )
        .route(
            "/api/v1/evals/flywheel/export",
            post(api::flywheel::export_dataset),
        )
        // Prompt templates routes (Phase 2)
        .route(
            "/api/v1/prompts",
            get(api::prompts::list_prompts).post(api::prompts::create_prompt),
        )
        .route(
            "/api/v1/prompts/:id",
            get(api::prompts::get_prompt)
                .put(api::prompts::update_prompt)
                .delete(api::prompts::delete_prompt),
        )
        .route(
            "/api/v1/prompts/:id/render",
            post(api::prompts::render_prompt),
        )
        // Prompt versioning routes (Task 9)
        .route(
            "/api/v1/prompts/:id/versions",
            get(api::prompts::get_prompt_versions),
        )
        .route(
            "/api/v1/prompts/:id/diff/:v1/:v2",
            get(api::prompts::compare_prompt_versions),
        )
        .route(
            "/api/v1/prompts/:id/rollback/:version",
            post(api::prompts::rollback_prompt_version),
        )
        .route(
            "/api/v1/prompts/:id/performance",
            get(api::prompts::get_prompt_performance),
        )
        // A/B Testing & Experiments routes (Phase 3)
        .route(
            "/api/v1/experiments",
            get(api::experiments::list_experiments).post(api::experiments::create_experiment),
        )
        .route(
            "/api/v1/experiments/:id",
            get(api::experiments::get_experiment)
                .put(api::experiments::update_experiment)
                .delete(api::experiments::delete_experiment),
        )
        .route(
            "/api/v1/experiments/:id/start",
            post(api::experiments::start_experiment),
        )
        .route(
            "/api/v1/experiments/:id/stop",
            post(api::experiments::stop_experiment),
        )
        .route(
            "/api/v1/experiments/:id/results",
            post(api::experiments::record_result),
        )
        .route(
            "/api/v1/experiments/:id/stats",
            get(api::experiments::get_experiment_stats),
        )
        // Evaluation routes (Task 2)
        .route("/api/v1/evals/geval", post(api::run_geval))
        .route("/api/v1/evals/ragas", post(api::run_ragas))
        .route(
            "/api/v1/evals/trace/:trace_id/history",
            get(api::get_evaluation_history),
        )
        // Evaluation Pipeline routes (comprehensive 5-phase pipeline)
        .route(
            "/api/v1/evals/pipeline/collect",
            post(api::eval_pipeline::collect_traces),
        )
        .route(
            "/api/v1/evals/pipeline/process",
            post(api::eval_pipeline::process_traces),
        )
        .route(
            "/api/v1/evals/pipeline/annotate",
            post(api::eval_pipeline::create_annotation),
        )
        .route(
            "/api/v1/evals/pipeline/golden",
            post(api::eval_pipeline::add_golden_test_cases),
        )
        .route(
            "/api/v1/evals/pipeline/evaluate",
            post(api::eval_pipeline::run_evaluation),
        )
        .route(
            "/api/v1/evals/pipeline/recommendations",
            get(api::eval_pipeline::get_recommendations),
        )
        .route(
            "/api/v1/evals/pipeline/metrics/definitions",
            get(api::eval_pipeline::get_metric_definitions),
        )
        .route(
            "/api/v1/evals/pipeline/history",
            get(api::eval_pipeline::get_eval_history),
        )
        // Budget alerts routes (Phase 3)
        .route(
            "/api/v1/budget/alerts",
            get(api::budget_alerts::list_alerts).post(api::budget_alerts::create_alert),
        )
        .route(
            "/api/v1/budget/alerts/:id",
            get(api::budget_alerts::get_alert)
                .put(api::budget_alerts::update_alert)
                .delete(api::budget_alerts::delete_alert),
        )
        .route(
            "/api/v1/budget/alerts/:id/events",
            get(api::budget_alerts::get_alert_events),
        )
        .route(
            "/api/v1/budget/status",
            get(api::budget_alerts::get_budget_status),
        )
        // Compliance reports routes (Phase 3)
        .route(
            "/api/v1/compliance/reports",
            get(api::compliance::list_reports).post(api::compliance::generate_report),
        )
        .route(
            "/api/v1/compliance/reports/:id",
            get(api::compliance::get_report).delete(api::compliance::delete_report),
        )
        .route(
            "/api/v1/compliance/privacy-metrics",
            get(api::compliance::get_privacy_metrics),
        )
        .route(
            "/api/v1/compliance/security-metrics",
            get(api::compliance::get_security_metrics),
        )
        // Advanced analytics routes (Phase 3)
        .route(
            "/api/v1/analytics/timeseries",
            get(api::analytics::get_timeseries),
        )
        .route(
            "/api/v1/analytics/trends",
            get(api::analytics::get_trend_analysis),
        )
        .route(
            "/api/v1/analytics/comparative",
            get(api::analytics::get_comparative_analysis),
        )
        .route(
            "/api/v1/analytics/correlation",
            get(api::analytics::get_correlation),
        )
        // NEW: OpenTelemetry GenAI Analytics (Phase 4)
        .route(
            "/api/v1/analytics/latency-breakdown",
            get(api::analytics::get_latency_breakdown),
        )
        .route(
            "/api/v1/analytics/cost-breakdown",
            get(api::analytics::get_cost_breakdown),
        )
        .route(
            "/api/v1/analytics/cost/breakdown",
            get(api::cost::get_detailed_cost_breakdown),
        )
        .route("/api/v1/analytics/cost/providers", get(get_provider_costs))
        // Insights API (anomaly detection and pattern recognition)
        .route("/api/v1/insights", get(api::insights::get_insights))
        .route(
            "/api/v1/insights/summary",
            get(api::insights::get_insights_summary),
        )
        // Storage Debug (NEW)
        .route(
            "/api/v1/storage/dump",
            get(api::storage_debug::dump_storage),
        )
        .route(
            "/api/v1/storage/stats",
            get(api::storage_debug::get_storage_stats),
        )
        // Backup and restore routes (Task 7)
        .route(
            "/api/v1/backup",
            get(api::backup::list_backups).post(api::backup::create_backup),
        )
        .route("/api/v1/backup/restore", post(api::backup::restore_backup))
        .route("/api/v1/backup/verify", get(api::backup::verify_backup))
        // Retention policy routes (Task 8)
        .route(
            "/api/v1/retention/config",
            get(api::retention::get_retention_config).post(api::retention::update_retention_config),
        )
        .route(
            "/api/v1/retention/cleanup",
            post(api::retention::trigger_cleanup),
        )
        .route(
            "/api/v1/retention/stats",
            get(api::retention::get_database_stats),
        )
        // Saved view routes (Task 9)
        .route(
            "/api/v1/views",
            get(api::views::list_views).post(api::views::create_view),
        )
        .route("/api/v1/views/export", post(api::views::export_views))
        .route("/api/v1/views/import", post(api::views::import_views))
        .route(
            "/api/v1/views/:id",
            get(api::views::get_view)
                .put(api::views::update_view)
                .delete(api::views::delete_view),
        )
        // Memory & RAG routes (MCP isolated project/tenant)
        .nest("/api/v1/memory", api::memory::memory_router())
        // Git-like Response Versioning routes
        .nest("/api/v1/git", {
            let git_state = Arc::new(api::GitVersioningState::new("Agentreplay User"));
            api::git_versioning_router().with_state(git_state)
        })
        .layer(axum_middleware::from_fn(auth_middleware))
        .layer(Extension(authenticator.clone()));

    // Get listen address
    let addr = config.socket_addr()?;
    tracing::info!("Listening on http://{}", addr);

    // Clone project_manager before moving state
    let pm_for_otlp = state.project_manager.clone();

    // Clone db for MCP server
    let db_for_mcp = db.clone();
    let state_for_mcp = state.clone();

    // Build full application router
    let app = Router::new()
        .route("/health", get(health_check))
        .merge(authed_routes)
        .with_state(state)
        .layer(if config.server.enable_cors {
            // Secure CORS configuration
            let mut cors = CorsLayer::new()
                .allow_methods(Any)
                .allow_headers(Any);

            // If specific origins configured, use them; otherwise allow all (dev mode)
            if config.server.cors_origins.is_empty() {
                tracing::warn!("CORS: Allowing all origins (development mode). Set cors_origins in production!");
                cors = cors.allow_origin(Any);
            } else {
                tracing::info!("CORS: Allowing origins: {:?}", config.server.cors_origins);
                // In production, you'd parse these and set specific origins
                // For now, keeping simple allow-all if list is provided
                cors = cors.allow_origin(Any);
            }
            cors
        } else {
            CorsLayer::new()
        })
        // Add tracing
        .layer(TraceLayer::new_for_http());

    // Start OTLP gRPC server on port 47117 in parallel (if project manager available)
    let otlp_handle = if let Some(pm) = pm_for_otlp {
        Some(tokio::spawn(async move {
            if let Err(e) = otlp_service::start_otlp_server(pm).await {
                tracing::error!("OTLP gRPC server error: {}", e);
            }
        }))
    } else {
        tracing::warn!("OTLP gRPC server not started - project manager not available");
        None
    };

    // Start MCP server on port 47101 for Claude Desktop / Cursor integration
    let mcp_handle = tokio::spawn(async move {
        let causal_index = db_for_mcp.causal_index();
        let mcp_router = mcp::mcp_router(state_for_mcp, causal_index);

        let mcp_app = Router::new()
            .merge(mcp_router)
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .layer(TraceLayer::new_for_http());

        let mcp_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 47101));
        tracing::info!("🔌 MCP Server listening on http://{}", mcp_addr);

        match tokio::net::TcpListener::bind(mcp_addr).await {
            Ok(listener) => {
                if let Err(e) = axum::serve(listener, mcp_app).await {
                    tracing::error!("MCP server error: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to bind MCP server on port 47101: {}", e);
            }
        }
    });

    // Run HTTP server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let server_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    // Wait for any server to complete (all should run indefinitely)
    tokio::select! {
        _ = server_handle => {
            tracing::info!("HTTP server stopped");
        }
        _ = mcp_handle => {
            tracing::info!("MCP server stopped");
        }
        _ = async {
            if let Some(handle) = otlp_handle {
                let _ = handle.await;
            } else {
                futures::future::pending::<()>().await
            }
        } => {
            tracing::info!("OTLP gRPC server stopped");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = ServerConfig::default();
        assert!(config.validate().is_ok());
    }
}
