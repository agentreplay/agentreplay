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

//! Integration tests for storage components

use flowtrace_storage::{
    observation_store::{ObservationStore, ObservationQuery, ObservationKey, StoredObservation},
    pending_queue::{PendingMessageQueue, PendingMessage, PendingQueueConfig},
};

/// Test observation storage and retrieval
#[test]
fn test_observation_storage() {
    let store = ObservationStore::new();
    
    let obs = create_test_observation(1, 100, 200, 1000);
    
    // Store observation
    store.put(obs.clone()).unwrap();
    
    // Retrieve by ID
    let retrieved = store.get_by_id(1).unwrap().unwrap();
    assert_eq!(retrieved.id, obs.id);
    assert_eq!(retrieved.title, obs.title);
    
    // Query by project
    let query = ObservationQuery::new()
        .project(100)
        .limit(10);
    
    let results = store.query(&query).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 1);
}

/// Test observation key encoding/decoding
#[test]
fn test_observation_key_encoding() {
    let key = ObservationKey::new(1, 2, 12345, 100);
    
    // Encode to bytes
    let encoded = key.encode();
    
    // Decode back
    let decoded = ObservationKey::decode(&encoded).unwrap();
    
    // Verify all fields match
    assert_eq!(key.project_id, decoded.project_id);
    assert_eq!(key.session_id, decoded.session_id);
    assert_eq!(key.timestamp, decoded.timestamp);
    assert_eq!(key.observation_id, decoded.observation_id);
}

/// Test observation queries with filters
#[test]
fn test_observation_queries() {
    let store = ObservationStore::new();
    
    // Insert multiple observations
    for i in 1..=10 {
        let obs = create_test_observation(i, 100, 200 + i, (1000 + i * 100) as u64);
        store.put(obs).unwrap();
    }
    
    // Query with time range
    let query = ObservationQuery::new()
        .project(100)
        .time_range(1200, 1600);
    
    let results = store.query(&query).unwrap();
    assert!(results.len() >= 4); // Should get obs 2-5
    
    // Query with session filter
    let query = ObservationQuery::new()
        .project(100)
        .session(205)
        .limit(1);
    
    let results = store.query(&query).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].session_id, 205);
}

/// Test observation type filtering
#[test]
fn test_observation_type_filter() {
    let store = ObservationStore::new();
    
    // Insert observations with different types
    let mut obs1 = create_test_observation(1, 100, 200, 1000);
    obs1.observation_type = "implementation".to_string();
    store.put(obs1).unwrap();
    
    let mut obs2 = create_test_observation(2, 100, 200, 1100);
    obs2.observation_type = "testing".to_string();
    store.put(obs2).unwrap();
    
    let mut obs3 = create_test_observation(3, 100, 200, 1200);
    obs3.observation_type = "documentation".to_string();
    store.put(obs3).unwrap();
    
    // Query for all
    let query = ObservationQuery::new().project(100);
    
    let results = store.query(&query).unwrap();
    assert_eq!(results.len(), 3);
}

/// Test batch operations
#[test]
fn test_batch_operations() {
    let store = ObservationStore::new();
    
    // Create batch of observations
    let mut batch = Vec::new();
    for i in 1..=20 {
        batch.push(create_test_observation(i, 100, 200, (1000 + i * 10) as u64));
    }
    
    // Store batch
    for obs in batch {
        store.put(obs).unwrap();
    }
    
    // Query all
    let query = ObservationQuery::new()
        .project(100)
        .limit(100);
    
    let results = store.query(&query).unwrap();
    assert_eq!(results.len(), 20);
}

/// Test pending message queue
#[test]
fn test_pending_queue() {
    let queue = PendingMessageQueue::new(PendingQueueConfig::default());

    let msg = PendingMessage::new(
        1,
        200,
        100,
        1000,
        "tool_event",
        b"Test message".to_vec(),
    );
    
    // Enqueue
    queue.enqueue(msg.clone()).unwrap();
    
    // Claim
    let claimed = queue.claim("worker-1", None);
    assert_eq!(claimed.messages.len(), 1);
    let claimed_msg = &claimed.messages[0];
    assert_eq!(claimed_msg.id, 1);
    assert_eq!(claimed_msg.claimed_by, Some("worker-1".to_string()));
    
    // Acknowledge
    queue.ack(&[claimed_msg.id]).unwrap();
    
    // Try to claim again - should be empty
    let empty = queue.claim("worker-2", None);
    assert!(empty.messages.is_empty());
}

/// Test pending queue with multiple messages
#[test]
fn test_pending_queue_multiple() {
    let queue = PendingMessageQueue::new(PendingQueueConfig {
        batch_size: 1,
        ..PendingQueueConfig::default()
    });
    
    // Enqueue multiple messages
    for i in 1..=5 {
        let msg = PendingMessage::new(
            i as u128,
            200,
            100,
            (1000 + i) as u64,
            "tool_event",
            format!("Message {}", i).into_bytes(),
        );
        queue.enqueue(msg).unwrap();
    }
    
    // Claim messages with different workers
    let msg1 = queue.claim("worker-1", None).messages[0].clone();
    let msg2 = queue.claim("worker-2", None).messages[0].clone();
    
    assert_ne!(msg1.id, msg2.id);
    assert_eq!(msg1.claimed_by, Some("worker-1".to_string()));
    assert_eq!(msg2.claimed_by, Some("worker-2".to_string()));
}

/// Test pending queue nack
#[test]
fn test_pending_queue_nack() {
    let queue = PendingMessageQueue::new(PendingQueueConfig::default());

    let msg = PendingMessage::new(
        1,
        200,
        100,
        1000,
        "tool_event",
        b"Test message".to_vec(),
    );
    
    queue.enqueue(msg).unwrap();
    
    // Claim message
    let claimed = queue.claim("worker-1", None).messages[0].clone();
    
    // Nack (reject) message
    queue.nack(&[claimed.id]).unwrap();
    
    // Should be available for claim again
    let reclaimed = queue.claim("worker-2", None);
    assert_eq!(reclaimed.messages.len(), 1);
    let reclaimed_msg = &reclaimed.messages[0];
    assert_eq!(reclaimed_msg.id, 1);
    assert_eq!(reclaimed_msg.claim_count, 2);
}

/// Test prefix generation
#[test]
fn test_key_prefixes() {
    let project_id = 100;
    let session_id = 200;
    
    // Project prefix
    let project_prefix = ObservationKey::project_prefix(project_id);
    assert!(project_prefix.starts_with(b"obs/"));
    
    // Session prefix
    let session_prefix = ObservationKey::session_prefix(project_id, session_id);
    assert!(session_prefix.starts_with(b"obs/"));
    assert!(session_prefix.len() > project_prefix.len());
}

/// Helper function to create test observation
fn create_test_observation(id: u128, project_id: u128, session_id: u128, timestamp: u64) -> StoredObservation {
    StoredObservation {
        id,
        project_id,
        session_id,
        observation_type: "implementation".to_string(),
        title: format!("Test observation {}", id),
        subtitle: Some(format!("Subtitle {}", id)),
        facts: vec![format!("Fact {}", id)],
        narrative: format!("Narrative {}", id),
        concepts: vec![format!("concept-{}", id)],
        files_read: vec![],
        files_modified: vec![],
        source_edge_id: None,
        created_at: timestamp,
        updated_at: timestamp,
    }
}

/// Test concurrent access
#[test]
fn test_concurrent_store_access() {
    use std::sync::Arc;
    use std::thread;
    
    let store = Arc::new(ObservationStore::new());
    let mut handles = vec![];
    
    // Spawn multiple threads writing observations
    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = thread::spawn(move || {
            let obs = create_test_observation(i, 100, 200, (1000 + i) as u64);
            store_clone.put(obs).unwrap();
        });
        handles.push(handle);
    }
    
    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Verify all observations were stored
    let query = ObservationQuery::new()
        .project(100)
        .limit(20);
    
    let results = store.query(&query).unwrap();
    assert_eq!(results.len(), 10);
}

/// Test deletion
#[test]
fn test_observation_deletion() {
    let store = ObservationStore::new();
    
    let obs = create_test_observation(1, 100, 200, 1000);
    store.put(obs.clone()).unwrap();
    
    // Verify it exists
    let retrieved = store.get_by_id(1).unwrap();
    assert!(retrieved.is_some());
    
    // Delete
    let key = obs.storage_key();
    store.delete(&key).unwrap();
    
    // Verify it's gone
    let retrieved = store.get_by_id(1).unwrap();
    assert!(retrieved.is_none());
}

/// Test update
#[test]
fn test_observation_update() {
    let store = ObservationStore::new();
    
    let mut obs = create_test_observation(1, 100, 200, 1000);
    store.put(obs.clone()).unwrap();
    
    // Update
    obs.title = "Updated title".to_string();
    obs.updated_at = 2000;
    store.put(obs.clone()).unwrap();
    
    // Verify update
    let retrieved = store.get_by_id(1).unwrap().unwrap();
    assert_eq!(retrieved.title, "Updated title");
    assert_eq!(retrieved.updated_at, 2000);
}
