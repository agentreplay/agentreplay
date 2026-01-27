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

//! Pending Message Queue
//!
//! Atomic claim-and-delete queue for tool events and edges pending
//! processing by the memory agent.
//!
//! # Key Encoding
//!
//! ```text
//! pending/{session_id:032x}/{hlc_timestamp:020}/{edge_id:032x}
//! ```
//!
//! # Workflow
//!
//! 1. Hook dispatcher enqueues messages
//! 2. Memory agent claims batch with atomic CAS
//! 3. On successful processing, messages are deleted
//! 4. On failure, messages remain for retry

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use parking_lot::RwLock;

/// A pending message in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingMessage {
    /// Unique message ID.
    pub id: u128,
    /// Session this message belongs to.
    pub session_id: u128,
    /// Project ID.
    pub project_id: u128,
    /// HLC timestamp when enqueued.
    pub timestamp: u64,
    /// Message type (e.g., "tool_event", "user_prompt", "session_end").
    pub message_type: String,
    /// Serialized message payload.
    pub payload: Vec<u8>,
    /// Number of claim attempts.
    pub claim_count: u32,
    /// Last claim timestamp (system time).
    pub last_claimed_at: Option<u64>,
    /// Claimed by worker ID.
    pub claimed_by: Option<String>,
}

impl PendingMessage {
    /// Create a new pending message.
    pub fn new(
        id: u128,
        session_id: u128,
        project_id: u128,
        timestamp: u64,
        message_type: impl Into<String>,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            id,
            session_id,
            project_id,
            timestamp,
            message_type: message_type.into(),
            payload,
            claim_count: 0,
            last_claimed_at: None,
            claimed_by: None,
        }
    }

    /// Create storage key for this message.
    pub fn storage_key(&self) -> String {
        format!(
            "pending/{:032x}/{:020}/{:032x}",
            self.session_id, self.timestamp, self.id
        )
    }

    /// Check if this message can be claimed.
    pub fn can_claim(&self, claim_timeout_secs: u64) -> bool {
        match self.last_claimed_at {
            None => true,
            Some(claimed_at) => {
                let now = current_timestamp_secs();
                now - claimed_at > claim_timeout_secs
            }
        }
    }
}

/// Queue configuration.
#[derive(Debug, Clone)]
pub struct PendingQueueConfig {
    /// Claim timeout in seconds (default: 60).
    pub claim_timeout_secs: u64,
    /// Maximum claim attempts before message is dead-lettered.
    pub max_claim_attempts: u32,
    /// Batch size for claiming messages.
    pub batch_size: usize,
}

impl Default for PendingQueueConfig {
    fn default() -> Self {
        Self {
            claim_timeout_secs: 60,
            max_claim_attempts: 5,
            batch_size: 10,
        }
    }
}

/// Claim result.
#[derive(Debug)]
pub struct ClaimResult {
    /// Claimed messages.
    pub messages: Vec<PendingMessage>,
    /// Worker ID that claimed the messages.
    pub worker_id: String,
    /// Claim timestamp.
    pub claimed_at: u64,
}

/// In-memory pending message queue.
///
/// Production should use SochDB for persistence.
pub struct PendingMessageQueue {
    messages: RwLock<BTreeMap<String, PendingMessage>>,
    dead_letters: RwLock<Vec<PendingMessage>>,
    config: PendingQueueConfig,
}

impl Default for PendingMessageQueue {
    fn default() -> Self {
        Self::new(PendingQueueConfig::default())
    }
}

impl PendingMessageQueue {
    /// Create a new queue.
    pub fn new(config: PendingQueueConfig) -> Self {
        Self {
            messages: RwLock::new(BTreeMap::new()),
            dead_letters: RwLock::new(Vec::new()),
            config,
        }
    }

    /// Enqueue a message.
    pub fn enqueue(&self, message: PendingMessage) -> Result<(), QueueError> {
        let key = message.storage_key();
        self.messages.write().insert(key, message);
        Ok(())
    }

    /// Enqueue multiple messages atomically.
    pub fn enqueue_batch(&self, messages: Vec<PendingMessage>) -> Result<(), QueueError> {
        let mut queue = self.messages.write();
        for message in messages {
            let key = message.storage_key();
            queue.insert(key, message);
        }
        Ok(())
    }

    /// Claim a batch of messages for processing.
    ///
    /// Returns messages that are unclaimed or whose claim has expired.
    pub fn claim(&self, worker_id: impl Into<String>, session_id: Option<u128>) -> ClaimResult {
        let worker_id = worker_id.into();
        let now = current_timestamp_secs();
        let mut messages_out = Vec::new();

        let mut queue = self.messages.write();

        // Find claimable messages
        let keys_to_claim: Vec<_> = queue
            .iter()
            .filter(|(_, m)| {
                if let Some(sid) = session_id {
                    if m.session_id != sid {
                        return false;
                    }
                }
                m.can_claim(self.config.claim_timeout_secs)
            })
            .take(self.config.batch_size)
            .map(|(k, _)| k.clone())
            .collect();

        // Claim them
        for key in keys_to_claim {
            if let Some(msg) = queue.get_mut(&key) {
                msg.claim_count += 1;
                msg.last_claimed_at = Some(now);
                msg.claimed_by = Some(worker_id.clone());
                messages_out.push(msg.clone());
            }
        }

        ClaimResult {
            messages: messages_out,
            worker_id,
            claimed_at: now,
        }
    }

    /// Acknowledge successful processing of messages.
    ///
    /// Removes the messages from the queue.
    pub fn ack(&self, message_ids: &[u128]) -> Result<usize, QueueError> {
        let mut queue = self.messages.write();
        let mut removed = 0;

        // Find keys for the given IDs
        let keys_to_remove: Vec<_> = queue
            .iter()
            .filter(|(_, m)| message_ids.contains(&m.id))
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_remove {
            queue.remove(&key);
            removed += 1;
        }

        Ok(removed)
    }

    /// Negative acknowledgment - release claim without deleting.
    pub fn nack(&self, message_ids: &[u128]) -> Result<usize, QueueError> {
        let released = {
            let mut queue = self.messages.write();
            let mut released = 0;

            for (_, msg) in queue.iter_mut() {
                if message_ids.contains(&msg.id) {
                    // Check for max attempts
                    if msg.claim_count >= self.config.max_claim_attempts {
                        // Will be moved to dead letter queue
                        continue;
                    }
                    msg.claimed_by = None;
                    msg.last_claimed_at = None;
                    released += 1;
                }
            }

            released
        };

        // Move exceeded messages to dead letter queue
        self.move_to_dead_letter();

        Ok(released)
    }

    /// Move messages that exceeded max claim attempts to dead letter queue.
    fn move_to_dead_letter(&self) {
        let mut queue = self.messages.write();
        let mut dead_letters = self.dead_letters.write();

        let keys_to_move: Vec<_> = queue
            .iter()
            .filter(|(_, m)| m.claim_count >= self.config.max_claim_attempts)
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_move {
            if let Some(msg) = queue.remove(&key) {
                dead_letters.push(msg);
            }
        }
    }

    /// Get pending messages for a session.
    pub fn get_session_messages(&self, session_id: u128) -> Vec<PendingMessage> {
        let prefix = format!("pending/{:032x}/", session_id);
        self.messages
            .read()
            .range(prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&prefix))
            .map(|(_, m)| m.clone())
            .collect()
    }

    /// Get queue depth.
    pub fn depth(&self) -> usize {
        self.messages.read().len()
    }

    /// Get session queue depth.
    pub fn session_depth(&self, session_id: u128) -> usize {
        let prefix = format!("pending/{:032x}/", session_id);
        self.messages
            .read()
            .range(prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&prefix))
            .count()
    }

    /// Get dead letter count.
    pub fn dead_letter_count(&self) -> usize {
        self.dead_letters.read().len()
    }

    /// Get dead letter messages.
    pub fn get_dead_letters(&self) -> Vec<PendingMessage> {
        self.dead_letters.read().clone()
    }

    /// Clear dead letter queue.
    pub fn clear_dead_letters(&self) {
        self.dead_letters.write().clear();
    }

    /// Clear all messages (use with caution).
    pub fn clear(&self) {
        self.messages.write().clear();
        self.dead_letters.write().clear();
    }
}

/// Queue errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum QueueError {
    #[error("Queue full")]
    QueueFull,

    #[error("Message not found: {0}")]
    NotFound(u128),

    #[error("Storage error: {0}")]
    Storage(String),
}

fn current_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_message(id: u128, session_id: u128, timestamp: u64) -> PendingMessage {
        PendingMessage::new(
            id,
            session_id,
            100, // project_id
            timestamp,
            "tool_event",
            vec![1, 2, 3],
        )
    }

    #[test]
    fn test_enqueue_and_claim() {
        let queue = PendingMessageQueue::default();

        queue.enqueue(create_test_message(1, 100, 1000)).unwrap();
        queue.enqueue(create_test_message(2, 100, 2000)).unwrap();

        let claim = queue.claim("worker-1", None);
        assert_eq!(claim.messages.len(), 2);
        assert_eq!(claim.worker_id, "worker-1");
    }

    #[test]
    fn test_claim_respects_session() {
        let queue = PendingMessageQueue::default();

        queue.enqueue(create_test_message(1, 100, 1000)).unwrap();
        queue.enqueue(create_test_message(2, 200, 2000)).unwrap();

        let claim = queue.claim("worker-1", Some(100));
        assert_eq!(claim.messages.len(), 1);
        assert_eq!(claim.messages[0].session_id, 100);
    }

    #[test]
    fn test_ack_removes_messages() {
        let queue = PendingMessageQueue::default();

        queue.enqueue(create_test_message(1, 100, 1000)).unwrap();
        queue.enqueue(create_test_message(2, 100, 2000)).unwrap();

        let claim = queue.claim("worker-1", None);
        assert_eq!(queue.depth(), 2);

        let removed = queue.ack(&[1]).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(queue.depth(), 1);
    }

    #[test]
    fn test_session_depth() {
        let queue = PendingMessageQueue::default();

        for i in 0..10 {
            let session_id = if i < 5 { 100 } else { 200 };
            queue
                .enqueue(create_test_message(i, session_id, i as u64 * 1000))
                .unwrap();
        }

        assert_eq!(queue.session_depth(100), 5);
        assert_eq!(queue.session_depth(200), 5);
    }

    #[test]
    fn test_storage_key_ordering() {
        let msg1 = create_test_message(1, 100, 1000);
        let msg2 = create_test_message(2, 100, 2000);
        let msg3 = create_test_message(3, 200, 1000);

        // Same session, earlier timestamp should come first
        assert!(msg1.storage_key() < msg2.storage_key());
        // Different sessions should be ordered by session ID
        assert!(msg1.storage_key() < msg3.storage_key());
    }
}
