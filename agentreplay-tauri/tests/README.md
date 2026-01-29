# Agent Replay Desktop Integration Tests

This directory contains integration tests for the Agent Replay Desktop application.

## Test Structure

### `integration_test.rs`
Main integration test suite covering:
- **Command Tests**: Tests for Tauri commands that don't require app_handle
- **Tauri Integration Tests**: Full integration tests with mocked Tauri app context (TODO)
- **Payload Memory Monitoring Tests**: Validates memory warning system
- **Shutdown Tests**: Validates graceful shutdown and data persistence

## Running Tests

```bash
# Run all integration tests
cargo test --test integration_test

# Run specific test
cargo test --test integration_test test_health_check

# Run with output
cargo test --test integration_test -- --nocapture
```

## Test Categories

### ✅ Working Tests (No app_handle required)
- `test_health_check` - Health status endpoint
- `test_get_db_stats` - Database statistics
- `test_list_traces_empty` - Trace listing with empty DB
- `test_ingest_and_query_traces` - Basic ingestion flow
- `test_graceful_shutdown` - Database close()
- `test_memory_warnings_trigger` - Memory monitoring setup
- `test_sled_backend_no_memory_warnings` - Sled backend efficiency

### 🚧 TODO: Full Tauri Integration Tests
These tests require Tauri's test utilities and are currently placeholders:
- Backup/restore commands
- Configuration management
- Window management
- Update checking

## Implementing Full Tauri Tests

To implement full integration tests with app_handle support, use Tauri's test utilities:

```rust
use tauri::test::{mock_builder, mock_context};

#[test]
fn test_with_app_handle() {
    let app = mock_builder()
        .build(mock_context())
        .expect("Failed to build mock app");

    // Now test commands that need app_handle
}
```

## Desktop-Specific Test Focus

These tests focus on desktop-relevant concerns:
1. **Memory management** - Ensures long-running sessions don't OOM
2. **Data durability** - No data loss on shutdown/crash
3. **UX flows** - Backup/restore works seamlessly
4. **Error recovery** - Graceful handling of failures

## CI/CD Integration

These tests should run on:
- Every PR (fast unit tests)
- Pre-release (full integration tests)
- Post-deployment verification (smoke tests)

## References

- [Tauri Testing Guide](https://tauri.app/v1/guides/testing/webdriver/introduction)
- [Tauri v2 Testing](https://v2.tauri.app/develop/tests/)
