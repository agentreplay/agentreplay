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

//! Object Store - Content-Addressable Storage
//!
//! Persistent storage for Git objects (blobs, trees, commits).
//! Uses content-addressable hashing for automatic deduplication.

use super::objects::{Blob, Commit, GitObject, ObjectId, ObjectType, Tree};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

/// Store errors
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("Object not found: {0}")]
    NotFound(ObjectId),

    #[error("Corrupted object: {0}")]
    CorruptedObject(ObjectId),

    #[error("Type mismatch: expected {expected:?}, got {actual:?}")]
    TypeMismatch {
        expected: ObjectType,
        actual: ObjectType,
    },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Stored object with type prefix
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredObject {
    /// Object type
    obj_type: ObjectType,
    /// Serialized object data
    data: Vec<u8>,
}

/// Object store statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreStats {
    pub total_objects: u64,
    pub blob_count: u64,
    pub tree_count: u64,
    pub commit_count: u64,
    pub total_size_bytes: u64,
}

/// In-memory object store (for testing and lightweight use)
pub struct ObjectStore {
    /// Object storage: ObjectId -> StoredObject
    objects: DashMap<ObjectId, StoredObject>,
    /// Statistics
    #[allow(dead_code)]
    stats: StoreStats,
    /// Atomic counters for concurrent updates
    blob_count: AtomicU64,
    tree_count: AtomicU64,
    commit_count: AtomicU64,
    total_size: AtomicU64,
}

impl ObjectStore {
    /// Create a new in-memory object store
    pub fn new() -> Self {
        Self {
            objects: DashMap::new(),
            stats: StoreStats::default(),
            blob_count: AtomicU64::new(0),
            tree_count: AtomicU64::new(0),
            commit_count: AtomicU64::new(0),
            total_size: AtomicU64::new(0),
        }
    }

    /// Store an object (idempotent - same content = same ID)
    pub fn put<T: GitObject>(&self, obj: &T) -> Result<ObjectId, StoreError> {
        let data = obj.serialize_bytes();
        let oid = ObjectId::from_content(&data);

        // Check if already exists (content-addressable dedup)
        if self.objects.contains_key(&oid) {
            return Ok(oid);
        }

        let stored = StoredObject {
            obj_type: T::TYPE,
            data: data.clone(),
        };

        // Update stats
        match T::TYPE {
            ObjectType::Blob => {
                self.blob_count.fetch_add(1, Ordering::Relaxed);
            }
            ObjectType::Tree => {
                self.tree_count.fetch_add(1, Ordering::Relaxed);
            }
            ObjectType::Commit => {
                self.commit_count.fetch_add(1, Ordering::Relaxed);
            }
        }
        self.total_size
            .fetch_add(data.len() as u64, Ordering::Relaxed);

        self.objects.insert(oid, stored);
        Ok(oid)
    }

    /// Get an object by ID
    pub fn get<T: GitObject>(&self, oid: &ObjectId) -> Result<Option<T>, StoreError> {
        match self.objects.get(oid) {
            Some(stored) => {
                if stored.obj_type != T::TYPE {
                    return Err(StoreError::TypeMismatch {
                        expected: T::TYPE,
                        actual: stored.obj_type,
                    });
                }

                let obj = T::deserialize_bytes(&stored.data)
                    .map_err(|e| StoreError::SerializationError(e.to_string()))?;

                Ok(Some(obj))
            }
            None => Ok(None),
        }
    }

    /// Get an object, returning error if not found
    pub fn get_required<T: GitObject>(&self, oid: &ObjectId) -> Result<T, StoreError> {
        self.get(oid)?.ok_or(StoreError::NotFound(*oid))
    }

    /// Check if an object exists
    pub fn exists(&self, oid: &ObjectId) -> bool {
        self.objects.contains_key(oid)
    }

    /// Get object type without deserializing
    pub fn get_type(&self, oid: &ObjectId) -> Option<ObjectType> {
        self.objects.get(oid).map(|s| s.obj_type)
    }

    /// Get object size without deserializing
    pub fn get_size(&self, oid: &ObjectId) -> Option<usize> {
        self.objects.get(oid).map(|s| s.data.len())
    }

    /// Get store statistics
    pub fn stats(&self) -> StoreStats {
        StoreStats {
            total_objects: self.objects.len() as u64,
            blob_count: self.blob_count.load(Ordering::Relaxed),
            tree_count: self.tree_count.load(Ordering::Relaxed),
            commit_count: self.commit_count.load(Ordering::Relaxed),
            total_size_bytes: self.total_size.load(Ordering::Relaxed),
        }
    }

    /// Iterate over all objects of a specific type
    pub fn iter_type<T: GitObject>(&self) -> impl Iterator<Item = (ObjectId, T)> + '_ {
        self.objects
            .iter()
            .filter(|r| r.value().obj_type == T::TYPE)
            .filter_map(|r| {
                let oid = *r.key();
                T::deserialize_bytes(&r.value().data)
                    .ok()
                    .map(|obj| (oid, obj))
            })
    }

    /// Get all commit IDs
    pub fn all_commits(&self) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|r| r.value().obj_type == ObjectType::Commit)
            .map(|r| *r.key())
            .collect()
    }

    // === Convenience methods ===

    /// Store a blob
    pub fn put_blob(&self, blob: &Blob) -> Result<ObjectId, StoreError> {
        self.put(blob)
    }

    /// Store a tree
    pub fn put_tree(&self, tree: &Tree) -> Result<ObjectId, StoreError> {
        self.put(tree)
    }

    /// Store a commit
    pub fn put_commit(&self, commit: &Commit) -> Result<ObjectId, StoreError> {
        self.put(commit)
    }

    /// Get a blob
    pub fn get_blob(&self, oid: &ObjectId) -> Result<Option<Blob>, StoreError> {
        self.get(oid)
    }

    /// Get a tree
    pub fn get_tree(&self, oid: &ObjectId) -> Result<Option<Tree>, StoreError> {
        self.get(oid)
    }

    /// Get a commit
    pub fn get_commit(&self, oid: &ObjectId) -> Result<Option<Commit>, StoreError> {
        self.get(oid)
    }

    // === Persistence ===

    /// Save store to file
    pub fn save_to_file(&self, path: &Path) -> Result<(), StoreError> {
        let objects: Vec<(ObjectId, StoredObject)> = self
            .objects
            .iter()
            .map(|r| (*r.key(), r.value().clone()))
            .collect();

        let data = bincode::serialize(&objects)
            .map_err(|e| StoreError::SerializationError(e.to_string()))?;

        std::fs::write(path, data)?;
        Ok(())
    }

    /// Load store from file
    pub fn load_from_file(path: &Path) -> Result<Self, StoreError> {
        let data = std::fs::read(path)?;
        let objects: Vec<(ObjectId, StoredObject)> = bincode::deserialize(&data)
            .map_err(|e| StoreError::SerializationError(e.to_string()))?;

        let store = Self::new();

        for (oid, stored) in objects {
            match stored.obj_type {
                ObjectType::Blob => {
                    store.blob_count.fetch_add(1, Ordering::Relaxed);
                }
                ObjectType::Tree => {
                    store.tree_count.fetch_add(1, Ordering::Relaxed);
                }
                ObjectType::Commit => {
                    store.commit_count.fetch_add(1, Ordering::Relaxed);
                }
            }
            store
                .total_size
                .fetch_add(stored.data.len() as u64, Ordering::Relaxed);
            store.objects.insert(oid, stored);
        }

        Ok(store)
    }
}

impl Default for ObjectStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response_git::objects::{Author, ContentType, EntryMode};

    #[test]
    fn test_blob_storage() {
        let store = ObjectStore::new();

        let blob = Blob::new(b"Hello, world!".to_vec(), ContentType::Text);
        let oid = store.put(&blob).unwrap();

        // Retrieve
        let retrieved: Blob = store.get_required(&oid).unwrap();
        assert_eq!(retrieved.as_text(), Some("Hello, world!"));
    }

    #[test]
    fn test_content_addressable_dedup() {
        let store = ObjectStore::new();

        let blob1 = Blob::text("same content");
        let blob2 = Blob::text("same content");
        let blob3 = Blob::text("different");

        let oid1 = store.put(&blob1).unwrap();
        let oid2 = store.put(&blob2).unwrap();
        let oid3 = store.put(&blob3).unwrap();

        assert_eq!(oid1, oid2); // Same content = same ID
        assert_ne!(oid1, oid3); // Different content = different ID

        // Should only have 2 objects
        assert_eq!(store.stats().blob_count, 2);
    }

    #[test]
    fn test_tree_storage() {
        let store = ObjectStore::new();

        let blob1 = Blob::text("input data");
        let blob2 = Blob::text("output data");

        let oid1 = store.put(&blob1).unwrap();
        let oid2 = store.put(&blob2).unwrap();

        let mut tree = Tree::new();
        tree.add_entry("input".to_string(), oid1, EntryMode::Blob);
        tree.add_entry("output".to_string(), oid2, EntryMode::Blob);

        let tree_oid = store.put(&tree).unwrap();

        let retrieved: Tree = store.get_required(&tree_oid).unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved.get("input").unwrap().oid, oid1);
    }

    #[test]
    fn test_commit_storage() {
        let store = ObjectStore::new();

        let blob = Blob::text("content");
        let blob_oid = store.put(&blob).unwrap();

        let mut tree = Tree::new();
        tree.add_entry("file".to_string(), blob_oid, EntryMode::Blob);
        let tree_oid = store.put(&tree).unwrap();

        let commit = Commit::initial(tree_oid, "Initial commit", Author::new("test"));
        let commit_oid = store.put(&commit).unwrap();

        let retrieved: Commit = store.get_required(&commit_oid).unwrap();
        assert_eq!(retrieved.message, "Initial commit");
        assert_eq!(retrieved.tree, tree_oid);
    }

    #[test]
    fn test_commit_chain() {
        let store = ObjectStore::new();

        let tree_oid = store.put(&Tree::new()).unwrap();

        // Initial commit
        let commit1 = Commit::initial(tree_oid, "First", Author::new("test"));
        let oid1 = store.put(&commit1).unwrap();

        // Child commit
        let commit2 = Commit::child(oid1, tree_oid, "Second", Author::new("test"));
        let oid2 = store.put(&commit2).unwrap();

        // Verify parent chain
        let retrieved: Commit = store.get_required(&oid2).unwrap();
        assert_eq!(retrieved.parents, vec![oid1]);
    }

    #[test]
    fn test_type_mismatch() {
        let store = ObjectStore::new();

        let blob = Blob::text("content");
        let oid = store.put(&blob).unwrap();

        // Try to retrieve as wrong type
        let result: Result<Option<Tree>, _> = store.get(&oid);
        assert!(matches!(result, Err(StoreError::TypeMismatch { .. })));
    }

    #[test]
    fn test_stats() {
        let store = ObjectStore::new();

        store.put(&Blob::text("blob1")).unwrap();
        store.put(&Blob::text("blob2")).unwrap();
        store.put(&Tree::new()).unwrap();
        store
            .put(&Commit::initial(
                ObjectId::default(),
                "test",
                Author::new("test"),
            ))
            .unwrap();

        let stats = store.stats();
        assert_eq!(stats.blob_count, 2);
        assert_eq!(stats.tree_count, 1);
        assert_eq!(stats.commit_count, 1);
        assert_eq!(stats.total_objects, 4);
    }
}
