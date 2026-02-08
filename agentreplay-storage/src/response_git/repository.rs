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

//! Response Repository - High-Level Git-Like Interface
//!
//! Provides a unified interface for version control operations on AI responses.
//! Supports commits, branches, tags, diffs, and experiment variants.

use super::diff::{CommitDiff, DiffEngine};
use super::objects::{Author, Blob, Commit, EntryMode, ObjectId, Tree};
use super::refs::{Ref, RefError, RefStore};
use super::store::{ObjectStore, StoreError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

/// Repository errors
#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Reference not found: {0}")]
    RefNotFound(String),

    #[error("Commit not found: {0}")]
    CommitNotFound(ObjectId),

    #[error("Branch already exists: {0}")]
    BranchExists(String),

    #[error("Tag already exists: {0}")]
    TagExists(String),

    #[error("Empty repository - no commits yet")]
    EmptyRepository,

    #[error("Detached HEAD - not on a branch")]
    DetachedHead,

    #[error("Store error: {0}")]
    StoreError(#[from] StoreError),

    #[error("Reference error: {0}")]
    RefError(#[from] RefError),
}

/// A complete response snapshot for versioning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseSnapshot {
    /// Prompt/input that generated this response
    pub prompt: String,
    /// The LLM's response
    pub response: String,
    /// Model used (e.g., "gpt-4", "claude-3")
    pub model: Option<String>,
    /// Temperature setting
    pub temperature: Option<f32>,
    /// Token usage
    pub tokens: Option<TokenUsage>,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Log entry for commit history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub commit_id: ObjectId,
    pub message: String,
    pub author: String,
    pub timestamp: DateTime<Utc>,
    pub parents: Vec<ObjectId>,
}

/// Experiment for A/B testing variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub base_branch: String,
    pub variants: Vec<ExperimentVariant>,
    pub created_at: DateTime<Utc>,
}

/// A variant in an experiment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentVariant {
    pub id: String,
    pub name: String,
    pub branch_name: String,
    pub description: Option<String>,
}

/// Response version control repository
pub struct ResponseRepository {
    /// Object store
    store: Arc<ObjectStore>,
    /// Reference store
    refs: Arc<RefStore>,
    /// Diff engine
    diff_engine: DiffEngine,
    /// Default author for commits
    default_author: Author,
    /// Repository path (for persistence)
    path: Option<std::path::PathBuf>,
}

impl ResponseRepository {
    /// Create a new in-memory repository
    pub fn new(author_name: &str) -> Self {
        Self {
            store: Arc::new(ObjectStore::new()),
            refs: Arc::new(RefStore::new()),
            diff_engine: DiffEngine::new(),
            default_author: Author::new(author_name),
            path: None,
        }
    }

    /// Create repository with persistent storage
    pub fn with_path(path: impl AsRef<Path>, author_name: &str) -> Self {
        Self {
            store: Arc::new(ObjectStore::new()),
            refs: Arc::new(RefStore::new()),
            diff_engine: DiffEngine::new(),
            default_author: Author::new(author_name),
            path: Some(path.as_ref().to_path_buf()),
        }
    }

    // === Core Operations ===

    /// Commit a response snapshot
    pub fn commit(
        &self,
        snapshot: &ResponseSnapshot,
        message: &str,
    ) -> Result<ObjectId, RepositoryError> {
        self.commit_with_author(snapshot, message, self.default_author.clone())
    }

    /// Commit with custom author
    pub fn commit_with_author(
        &self,
        snapshot: &ResponseSnapshot,
        message: &str,
        author: Author,
    ) -> Result<ObjectId, RepositoryError> {
        // Create blobs for prompt and response
        let prompt_blob = Blob::text(&snapshot.prompt);
        let response_blob = Blob::text(&snapshot.response);

        let prompt_oid = self.store.put(&prompt_blob)?;
        let response_oid = self.store.put(&response_blob)?;

        // Create tree with entries
        let mut tree = Tree::new();
        tree.add_entry("prompt".to_string(), prompt_oid, EntryMode::Blob);
        tree.add_entry("response".to_string(), response_oid, EntryMode::Blob);

        // Add optional metadata as blobs
        if let Some(ref model) = snapshot.model {
            let model_blob = Blob::text(model);
            let model_oid = self.store.put(&model_blob)?;
            tree.add_entry("model".to_string(), model_oid, EntryMode::Blob);
        }

        if !snapshot.metadata.is_empty() {
            let meta_json = serde_json::to_string_pretty(&snapshot.metadata).unwrap_or_default();
            let meta_blob = Blob::text(&meta_json);
            let meta_oid = self.store.put(&meta_blob)?;
            tree.add_entry("metadata.json".to_string(), meta_oid, EntryMode::Blob);
        }

        let tree_oid = self.store.put(&tree)?;

        // Create commit
        let commit = match self.head_commit_id() {
            Some(parent) => Commit::child(parent, tree_oid, message, author),
            None => Commit::initial(tree_oid, message, author),
        };

        let commit_oid = self.store.put(&commit)?;

        // Update branch ref
        if let Some(branch_name) = self.current_branch_name() {
            self.refs.update_branch(&branch_name, commit_oid)?;
        } else {
            // Create main branch if this is first commit
            self.refs.update_branch("main", commit_oid)?;
            self.refs.set_head(Ref::symbolic("refs/heads/main"))?;
        }

        Ok(commit_oid)
    }

    /// Get commit history (newest first)
    pub fn log(&self, max_count: Option<usize>) -> Result<Vec<LogEntry>, RepositoryError> {
        let head = self
            .head_commit_id()
            .ok_or(RepositoryError::EmptyRepository)?;

        self.log_from(head, max_count)
    }

    /// Get commit history starting from a specific commit
    pub fn log_from(
        &self,
        start: ObjectId,
        max_count: Option<usize>,
    ) -> Result<Vec<LogEntry>, RepositoryError> {
        let mut entries = Vec::new();
        let mut current = Some(start);

        while let Some(oid) = current {
            if let Some(max) = max_count {
                if entries.len() >= max {
                    break;
                }
            }

            let commit: Commit = self
                .store
                .get(&oid)?
                .ok_or(RepositoryError::CommitNotFound(oid))?;

            // Convert microseconds to DateTime<Utc>
            let secs = (commit.timestamp_us / 1_000_000) as i64;
            let nsecs = ((commit.timestamp_us % 1_000_000) * 1000) as u32;
            let timestamp = DateTime::from_timestamp(secs, nsecs).unwrap_or_else(|| Utc::now());

            entries.push(LogEntry {
                commit_id: oid,
                message: commit.message.clone(),
                author: commit.author.name.clone(),
                timestamp,
                parents: commit.parents.clone(),
            });

            current = commit.parents.first().copied();
        }

        Ok(entries)
    }

    /// Get a specific commit
    pub fn show(&self, ref_or_oid: &str) -> Result<Commit, RepositoryError> {
        let oid = self.refs.resolve(ref_or_oid)?;

        self.store
            .get(&oid)?
            .ok_or(RepositoryError::CommitNotFound(oid))
    }

    /// Get response content from a commit
    pub fn get_snapshot(&self, ref_or_oid: &str) -> Result<ResponseSnapshot, RepositoryError> {
        let commit = self.show(ref_or_oid)?;
        let tree: Tree = self
            .store
            .get(&commit.tree)?
            .ok_or(RepositoryError::CommitNotFound(commit.tree))?;

        let mut snapshot = ResponseSnapshot {
            prompt: String::new(),
            response: String::new(),
            model: None,
            temperature: None,
            tokens: None,
            metadata: HashMap::new(),
        };

        for entry in &tree.entries {
            if let Some(blob) = self.store.get::<Blob>(&entry.oid)? {
                match entry.name.as_str() {
                    "prompt" => {
                        snapshot.prompt = blob.as_text().unwrap_or("").to_string();
                    }
                    "response" => {
                        snapshot.response = blob.as_text().unwrap_or("").to_string();
                    }
                    "model" => {
                        snapshot.model = blob.as_text().map(|s| s.to_string());
                    }
                    "metadata.json" => {
                        if let Some(text) = blob.as_text() {
                            if let Ok(meta) = serde_json::from_str(text) {
                                snapshot.metadata = meta;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(snapshot)
    }

    // === Branch Operations ===

    /// Create a new branch at current HEAD
    pub fn create_branch(&self, name: &str) -> Result<(), RepositoryError> {
        if self.refs.get_branch(name).is_some() {
            return Err(RepositoryError::BranchExists(name.to_string()));
        }

        let head = self
            .head_commit_id()
            .ok_or(RepositoryError::EmptyRepository)?;

        self.refs.update_branch(name, head)?;
        Ok(())
    }

    /// Create a branch at a specific commit
    pub fn create_branch_at(&self, name: &str, ref_or_oid: &str) -> Result<(), RepositoryError> {
        if self.refs.get_branch(name).is_some() {
            return Err(RepositoryError::BranchExists(name.to_string()));
        }

        let oid = self.refs.resolve(ref_or_oid)?;

        self.refs.update_branch(name, oid)?;
        Ok(())
    }

    /// Switch to a branch or commit
    pub fn checkout(&self, ref_or_oid: &str) -> Result<ObjectId, RepositoryError> {
        // Try as branch first
        if let Some(branch) = self.refs.get_branch(ref_or_oid) {
            self.refs
                .set_head(Ref::symbolic(format!("refs/heads/{}", ref_or_oid)))?;
            return Ok(branch.commit);
        }

        // Try as tag or commit
        let oid = self.refs.resolve(ref_or_oid)?;

        // Detached HEAD
        self.refs.set_head(Ref::direct(oid))?;
        Ok(oid)
    }

    /// List all branches
    pub fn list_branches(&self) -> Vec<(String, ObjectId)> {
        self.refs
            .list_branches()
            .into_iter()
            .map(|b| (b.name.clone(), b.commit))
            .collect()
    }

    /// Get current branch name (None if detached HEAD)
    pub fn current_branch_name(&self) -> Option<String> {
        self.refs.current_branch()
    }

    /// Delete a branch
    pub fn delete_branch(&self, name: &str) -> Result<(), RepositoryError> {
        // Don't delete current branch
        if Some(name.to_string()) == self.current_branch_name() {
            return Err(RepositoryError::RefNotFound(format!(
                "Cannot delete current branch: {}",
                name
            )));
        }

        self.refs.delete_branch(name)?;
        Ok(())
    }

    // === Tag Operations ===

    /// Create a tag at current HEAD
    pub fn tag(&self, name: &str, message: Option<&str>) -> Result<(), RepositoryError> {
        if self.refs.get_tag(name).is_some() {
            return Err(RepositoryError::TagExists(name.to_string()));
        }

        let head = self
            .head_commit_id()
            .ok_or(RepositoryError::EmptyRepository)?;

        let annotation = message.map(|m| (self.default_author.clone(), m.to_string()));
        self.refs.create_tag(name, head, annotation)?;

        Ok(())
    }

    /// Create a tag at a specific commit
    pub fn tag_at(
        &self,
        name: &str,
        ref_or_oid: &str,
        message: Option<&str>,
    ) -> Result<(), RepositoryError> {
        if self.refs.get_tag(name).is_some() {
            return Err(RepositoryError::TagExists(name.to_string()));
        }

        let oid = self.refs.resolve(ref_or_oid)?;

        let annotation = message.map(|m| (self.default_author.clone(), m.to_string()));
        self.refs.create_tag(name, oid, annotation)?;

        Ok(())
    }

    /// List all tags
    pub fn list_tags(&self) -> Vec<(String, ObjectId)> {
        self.refs
            .list_tags()
            .into_iter()
            .map(|t| (t.name.clone(), t.target))
            .collect()
    }

    // === Diff Operations ===

    /// Diff between two refs or commits
    pub fn diff(&self, old_ref: &str, new_ref: &str) -> Result<CommitDiff, RepositoryError> {
        let old_oid = self.refs.resolve(old_ref)?;
        let new_oid = self.refs.resolve(new_ref)?;

        let old_commit: Commit = self
            .store
            .get(&old_oid)?
            .ok_or(RepositoryError::CommitNotFound(old_oid))?;
        let new_commit: Commit = self
            .store
            .get(&new_oid)?
            .ok_or(RepositoryError::CommitNotFound(new_oid))?;

        let old_tree: Tree = self
            .store
            .get(&old_commit.tree)?
            .ok_or(RepositoryError::CommitNotFound(old_commit.tree))?;
        let new_tree: Tree = self
            .store
            .get(&new_commit.tree)?
            .ok_or(RepositoryError::CommitNotFound(new_commit.tree))?;

        let mut tree_diff = self.diff_engine.diff_trees(&old_tree, &new_tree);

        // Add blob diffs for modified files
        for modified in &mut tree_diff.modified {
            if let (Some(old_blob), Some(new_blob)) = (
                self.store.get::<Blob>(&modified.old_oid)?,
                self.store.get::<Blob>(&modified.new_oid)?,
            ) {
                modified.blob_diff = Some(self.diff_engine.diff_blobs(&old_blob, &new_blob));
            }
        }

        let stats = self.diff_engine.calculate_stats(&tree_diff);

        Ok(CommitDiff {
            old_commit: old_oid,
            new_commit: new_oid,
            tree_diff,
            stats,
        })
    }

    /// Diff current HEAD against its parent
    pub fn diff_head(&self) -> Result<Option<CommitDiff>, RepositoryError> {
        let head = self
            .head_commit_id()
            .ok_or(RepositoryError::EmptyRepository)?;

        let commit: Commit = self
            .store
            .get(&head)?
            .ok_or(RepositoryError::CommitNotFound(head))?;

        match commit.parents.first() {
            Some(parent) => {
                let diff = self.diff(&parent.short(), &head.short())?;
                Ok(Some(diff))
            }
            None => Ok(None), // Initial commit has no parent
        }
    }

    // === Experiment Operations ===

    /// Start an experiment with variants branching from current HEAD
    pub fn start_experiment(
        &self,
        name: &str,
        variant_names: &[&str],
    ) -> Result<Experiment, RepositoryError> {
        let base_branch = self
            .current_branch_name()
            .ok_or(RepositoryError::DetachedHead)?;

        let id = uuid::Uuid::new_v4().to_string();
        let mut variants = Vec::new();

        for variant_name in variant_names {
            let branch_name = format!("experiment/{}/{}", name, variant_name);
            self.create_branch(&branch_name)?;

            variants.push(ExperimentVariant {
                id: uuid::Uuid::new_v4().to_string(),
                name: variant_name.to_string(),
                branch_name,
                description: None,
            });
        }

        Ok(Experiment {
            id,
            name: name.to_string(),
            description: None,
            base_branch,
            variants,
            created_at: Utc::now(),
        })
    }

    /// Compare all variants in an experiment
    pub fn compare_variants(
        &self,
        experiment: &Experiment,
    ) -> Result<Vec<(String, CommitDiff)>, RepositoryError> {
        let mut results = Vec::new();

        for variant in &experiment.variants {
            let diff = self.diff(&experiment.base_branch, &variant.branch_name)?;
            results.push((variant.name.clone(), diff));
        }

        Ok(results)
    }

    // === Helper Methods ===

    /// Get current HEAD commit ID
    pub fn head_commit_id(&self) -> Option<ObjectId> {
        self.refs.head()
    }

    /// Get object store
    pub fn store(&self) -> &ObjectStore {
        &self.store
    }

    /// Get ref store
    pub fn refs(&self) -> &RefStore {
        &self.refs
    }

    /// Check if repository is empty
    pub fn is_empty(&self) -> bool {
        self.head_commit_id().is_none()
    }

    // === Persistence ===

    /// Save repository to disk
    pub fn save(&self) -> Result<(), RepositoryError> {
        if let Some(ref path) = self.path {
            std::fs::create_dir_all(path).map_err(StoreError::IoError)?;

            self.store.save_to_file(&path.join("objects.bin"))?;
            self.refs.save_to_file(&path.join("refs.bin"))?;
        }
        Ok(())
    }

    /// Load repository from disk
    pub fn load(path: impl AsRef<Path>, author_name: &str) -> Result<Self, RepositoryError> {
        let path = path.as_ref();

        let store = if path.join("objects.bin").exists() {
            Arc::new(ObjectStore::load_from_file(&path.join("objects.bin"))?)
        } else {
            Arc::new(ObjectStore::new())
        };

        let refs = if path.join("refs.bin").exists() {
            Arc::new(RefStore::load_from_file(&path.join("refs.bin"))?)
        } else {
            Arc::new(RefStore::new())
        };

        Ok(Self {
            store,
            refs,
            diff_engine: DiffEngine::new(),
            default_author: Author::new(author_name),
            path: Some(path.to_path_buf()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_snapshot() -> ResponseSnapshot {
        ResponseSnapshot {
            prompt: "What is Rust?".to_string(),
            response: "Rust is a systems programming language.".to_string(),
            model: Some("gpt-4".to_string()),
            temperature: Some(0.7),
            tokens: Some(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            }),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_commit_and_log() {
        let repo = ResponseRepository::new("test");

        let snapshot = sample_snapshot();
        let oid = repo.commit(&snapshot, "Initial response").unwrap();

        let log = repo.log(None).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].commit_id, oid);
        assert_eq!(log[0].message, "Initial response");
    }

    #[test]
    fn test_multiple_commits() {
        let repo = ResponseRepository::new("test");

        let mut snapshot1 = sample_snapshot();
        snapshot1.response = "Version 1".to_string();
        let oid1 = repo.commit(&snapshot1, "First version").unwrap();

        let mut snapshot2 = sample_snapshot();
        snapshot2.response = "Version 2".to_string();
        let oid2 = repo.commit(&snapshot2, "Second version").unwrap();

        let log = repo.log(None).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].commit_id, oid2); // Newest first
        assert_eq!(log[1].commit_id, oid1);
    }

    #[test]
    fn test_branches() {
        let repo = ResponseRepository::new("test");

        repo.commit(&sample_snapshot(), "Initial").unwrap();

        repo.create_branch("feature").unwrap();

        let branches = repo.list_branches();
        assert_eq!(branches.len(), 2);

        repo.checkout("feature").unwrap();
        assert_eq!(repo.current_branch_name(), Some("feature".to_string()));
    }

    #[test]
    fn test_tags() {
        let repo = ResponseRepository::new("test");

        repo.commit(&sample_snapshot(), "Initial").unwrap();

        repo.tag("v1.0", Some("First release")).unwrap();

        let tags = repo.list_tags();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].0, "v1.0");
    }

    #[test]
    fn test_diff() {
        let repo = ResponseRepository::new("test");

        let mut snapshot1 = sample_snapshot();
        snapshot1.response = "Version 1".to_string();
        repo.commit(&snapshot1, "First").unwrap();

        repo.create_branch("feature").unwrap();
        repo.checkout("feature").unwrap();

        let mut snapshot2 = sample_snapshot();
        snapshot2.response = "Version 2".to_string();
        repo.commit(&snapshot2, "Modified").unwrap();

        let diff = repo.diff("main", "feature").unwrap();
        assert!(!diff.tree_diff.modified.is_empty());
    }

    #[test]
    fn test_get_snapshot() {
        let repo = ResponseRepository::new("test");

        let snapshot = sample_snapshot();
        let oid = repo.commit(&snapshot, "Test").unwrap();

        let retrieved = repo.get_snapshot(&oid.to_hex()).unwrap();
        assert_eq!(retrieved.prompt, snapshot.prompt);
        assert_eq!(retrieved.response, snapshot.response);
        assert_eq!(retrieved.model, snapshot.model);
    }

    #[test]
    fn test_experiment() {
        let repo = ResponseRepository::new("test");

        repo.commit(&sample_snapshot(), "Initial").unwrap();

        let experiment = repo
            .start_experiment("prompt-test", &["variant-a", "variant-b"])
            .unwrap();

        assert_eq!(experiment.variants.len(), 2);

        let branches = repo.list_branches();
        assert_eq!(branches.len(), 3); // main + 2 variants
    }
}
