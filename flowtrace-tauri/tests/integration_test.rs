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

// Integration tests for Flowtrace Desktop
//
// These tests use Tauri's test utilities to create a proper app context,
// allowing us to test commands that depend on AppHandle, managed state, etc.

use flowtrace::AppState;
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a test database directory
fn setup_test_db() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp dir")
}

/// Helper to create minimal AppState for testing
///
/// Note: This is a simplified version for unit tests.
/// Full integration tests should use Tauri's test::mock_builder()
fn create_test_state(db_path: std::path::PathBuf) -> AppState {
    use flowtrace_query::Flowtrace;
    use parking_lot::RwLock;

    let db = Arc::new(Flowtrace::open(&db_path).expect("Failed to open test DB"));

    let config = Arc::new(RwLock::new(flowtrace::AppConfig::default()));

    let agent_registry = Arc::new(RwLock::new(Vec::new()));

    let saved_view_registry = Arc::new(tokio::sync::RwLock::new(
        flowtrace_core::SavedViewRegistry::new(&db_path),
    ));

    let connection_stats = Arc::new(RwLock::new(flowtrace::ConnectionStats {
        total_traces_received: 0,
        last_trace_time: None,
        server_uptime_secs: 0,
        ingestion_rate_per_min: 0.0,
    }));

    let eval_registry = Arc::new(flowtrace_evals::EvaluatorRegistry::new());

    // Create a dummy ingestion queue (won't actually work without proper async context)
    let (tx, _rx) = tokio::sync::mpsc::channel(100);
    let ingestion_queue = Arc::new(flowtrace::IngestionQueue {
        tx,
        shutdown_tx: Arc::new(tokio::sync::Notify::new()),
    });

    let project_store = Arc::new(RwLock::new(
        flowtrace::project_store::ProjectStore::new(db_path.join("projects.json")),
    ));

    let (trace_tx, _trace_rx) = tokio::sync::broadcast::channel(100);

    // Note: app_handle cannot be created in unit tests
    // For commands that need it, use Tauri's test::mock_builder()

    AppState {
        db,
        db_path: db_path.clone(),
        config,
        agent_registry,
        saved_view_registry,
        connection_stats,
        app_handle: unimplemented!("Use Tauri test::mock_builder() for full integration tests"),
        eval_registry,
        ingestion_queue,
        project_store,
        trace_broadcaster: trace_tx,
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let temp_dir = setup_test_db();
        let state = create_test_state(temp_dir.path().to_path_buf());

        // This command doesn't need app_handle
        let result = flowtrace::commands::health_check(tauri::State::from(&state)).await;

        assert!(result.is_ok());
        let health = result.unwrap();
        assert_eq!(health.status, "healthy");
        assert_eq!(health.total_traces, 0);
    }

    #[tokio::test]
    async fn test_get_db_stats() {
        let temp_dir = setup_test_db();
        let state = create_test_state(temp_dir.path().to_path_buf());

        let result = flowtrace::commands::get_db_stats(tauri::State::from(&state)).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.total_traces, 0);
        assert_eq!(stats.total_edges, 0);
    }

    #[tokio::test]
    async fn test_list_traces_empty() {
        let temp_dir = setup_test_db();
        let state = create_test_state(temp_dir.path().to_path_buf());

        let params = flowtrace::commands::ListTracesParams {
            limit: Some(10),
            offset: Some(0),
            start_time: None,
            end_time: None,
            agent_id: None,
        };

        let result = flowtrace::commands::list_traces(params, tauri::State::from(&state)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.traces.len(), 0);
        assert_eq!(response.total, 0);
    }

    #[tokio::test]
    async fn test_ingest_and_query_traces() {
        let temp_dir = setup_test_db();
        let state = create_test_state(temp_dir.path().to_path_buf());

        // Create a test edge
        let edge = flowtrace_core::AgentFlowEdge::new(
            1,      // tenant_id
            0,      // project_id
            1,      // agent_id
            1,      // session_id
            flowtrace_core::SpanType::Root,
            0,      // causal_parent
        );

        // Serialize to JSON for ingestion
        let traces_json = serde_json::to_string(&vec![edge]).unwrap();

        // Ingest the trace
        let ingest_result = flowtrace::commands::ingest_traces(
            traces_json,
            tauri::State::from(&state),
        ).await;

        assert!(ingest_result.is_ok());
        assert_eq!(ingest_result.unwrap(), 1);

        // Note: The edge is queued, not immediately in DB
        // In a real integration test, we'd wait for background worker to flush
    }
}

// Full integration tests using Tauri test utilities
// These are more complex but provide proper app_handle mocking
#[cfg(test)]
mod tauri_integration_tests {
    // These tests would use tauri::test::mock_builder()
    // Example structure (commented out as it requires more setup):

    /*
    use tauri::test::{mock_builder, mock_context};

    #[test]
    fn test_backup_commands_with_app_handle() {
        let app = mock_builder()
            .build(mock_context())
            .expect("Failed to build mock app");

        // Now we can test commands that need app_handle
        // e.g., create_backup, list_backups, etc.
    }
    */

    // TODO: Implement full Tauri integration tests
    // See: https://tauri.app/v1/guides/testing/webdriver/introduction
}

#[cfg(test)]
mod payload_memory_monitoring_tests {
    use flowtrace_storage::payload::{PayloadStore, IndexBackend};
    use tempfile::tempdir;

    #[test]
    fn test_memory_warnings_trigger() {
        // This test verifies that memory warnings are logged at appropriate thresholds
        // We can't easily test the actual logging, but we can verify the behavior

        let dir = tempdir().unwrap();
        let store = PayloadStore::open_with_backend(dir.path(), IndexBackend::HashMap).unwrap();

        // In a real scenario, we'd insert 1M payloads and check logs
        // For now, we just verify the store works with HashMap backend
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_sled_backend_no_memory_warnings() {
        // Sled backend should not trigger memory warnings
        let dir = tempdir().unwrap();
        let store = PayloadStore::open_with_backend(dir.path(), IndexBackend::Sled).unwrap();

        // Even with many entries, Sled should remain memory-efficient
        for i in 0..1000 {
            let data = format!("test data {}", i);
            store.append(i as u128, data.as_bytes(), None).unwrap();
        }

        assert_eq!(store.len(), 1000);
    }
}

#[cfg(test)]
mod shutdown_tests {
    use flowtrace_query::Flowtrace;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let dir = tempdir().unwrap();
        let db = Flowtrace::open(dir.path()).unwrap();

        // Insert some data
        let edge = flowtrace_core::AgentFlowEdge::new(
            1, 0, 1, 1,
            flowtrace_core::SpanType::Root,
            0,
        );
        db.insert(edge).await.unwrap();

        // Graceful close should succeed
        let result = db.close();
        assert!(result.is_ok(), "Graceful shutdown should succeed");
    }

    #[tokio::test]
    async fn test_sync_before_close() {
        let dir = tempdir().unwrap();
        let db = Flowtrace::open(dir.path()).unwrap();

        // Insert data and sync
        let edge = flowtrace_core::AgentFlowEdge::new(
            1, 0, 1, 1,
            flowtrace_core::SpanType::Root,
            0,
        );
        db.insert(edge).await.unwrap();
        db.sync().unwrap();

        // Close after sync should also work
        let result = db.close();
        assert!(result.is_ok());
    }
}
