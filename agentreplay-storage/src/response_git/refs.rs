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

//! Reference System (Branches & Tags)
//!
//! Mutable references to immutable objects.

use super::objects::{Author, ObjectId};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

/// Reference errors
#[derive(Debug, Error)]
pub enum RefError {
    #[error("Reference not found: {0}")]
    NotFound(String),

    #[error("Tag already exists: {0}")]
    TagExists(String),

    #[error("Branch already exists: {0}")]
    BranchExists(String),

    #[error("Invalid reference name: {0}")]
    InvalidName(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Reference types (like Git refs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Ref {
    /// Direct reference to a commit
    Direct(ObjectId),
    /// Symbolic reference to another ref (like HEAD -> refs/heads/main)
    Symbolic(String),
}

impl Ref {
    /// Create a direct reference
    pub fn direct(oid: ObjectId) -> Self {
        Self::Direct(oid)
    }

    /// Create a symbolic reference
    pub fn symbolic(target: impl Into<String>) -> Self {
        Self::Symbolic(target.into())
    }

    /// Check if this is a symbolic reference
    pub fn is_symbolic(&self) -> bool {
        matches!(self, Ref::Symbolic(_))
    }
}

/// Branch - mutable reference to latest commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    /// Branch name (without refs/heads/ prefix)
    pub name: String,
    /// Current commit
    pub commit: ObjectId,
    /// Creation timestamp (microseconds)
    pub created_at: u64,
    /// Last updated timestamp (microseconds)
    pub updated_at: u64,
    /// Optional upstream branch (for tracking)
    pub upstream: Option<String>,
    /// Optional description
    pub description: Option<String>,
}

impl Branch {
    /// Create a new branch
    pub fn new(name: impl Into<String>, commit: ObjectId) -> Self {
        let now = current_timestamp_us();
        Self {
            name: name.into(),
            commit,
            created_at: now,
            updated_at: now,
            upstream: None,
            description: None,
        }
    }

    /// Full reference path
    pub fn ref_path(&self) -> String {
        format!("refs/heads/{}", self.name)
    }
}

/// Tag - immutable reference with optional annotation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    /// Tag name (without refs/tags/ prefix)
    pub name: String,
    /// Target commit
    pub target: ObjectId,
    /// Tagger (for annotated tags)
    pub tagger: Option<Author>,
    /// Message (for annotated tags)
    pub message: Option<String>,
    /// Creation timestamp (microseconds)
    pub created_at: u64,
}

impl Tag {
    /// Create a lightweight tag
    pub fn lightweight(name: impl Into<String>, target: ObjectId) -> Self {
        Self {
            name: name.into(),
            target,
            tagger: None,
            message: None,
            created_at: current_timestamp_us(),
        }
    }

    /// Create an annotated tag
    pub fn annotated(
        name: impl Into<String>,
        target: ObjectId,
        tagger: Author,
        message: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            target,
            tagger: Some(tagger),
            message: Some(message.into()),
            created_at: current_timestamp_us(),
        }
    }

    /// Check if this is an annotated tag
    pub fn is_annotated(&self) -> bool {
        self.message.is_some()
    }

    /// Full reference path
    pub fn ref_path(&self) -> String {
        format!("refs/tags/{}", self.name)
    }
}

/// Reference store - manages branches, tags, and HEAD
pub struct RefStore {
    /// refs/heads/* -> Branch
    branches: DashMap<String, Branch>,
    /// refs/tags/* -> Tag
    tags: DashMap<String, Tag>,
    /// HEAD - current branch or detached commit
    head: RwLock<Ref>,
    /// Persistence path (optional)
    refs_dir: Option<PathBuf>,
}

impl RefStore {
    /// Create a new reference store (in-memory only)
    pub fn new() -> Self {
        Self {
            branches: DashMap::new(),
            tags: DashMap::new(),
            head: RwLock::new(Ref::Symbolic("refs/heads/main".to_string())),
            refs_dir: None,
        }
    }

    /// Create with persistence
    pub fn with_persistence(refs_dir: PathBuf) -> Result<Self, RefError> {
        fs::create_dir_all(&refs_dir)?;
        fs::create_dir_all(refs_dir.join("heads"))?;
        fs::create_dir_all(refs_dir.join("tags"))?;

        let store = Self {
            branches: DashMap::new(),
            tags: DashMap::new(),
            head: RwLock::new(Ref::Symbolic("refs/heads/main".to_string())),
            refs_dir: Some(refs_dir.clone()),
        };

        // Load existing refs
        store.load_refs()?;

        Ok(store)
    }

    /// Load refs from disk
    fn load_refs(&self) -> Result<(), RefError> {
        let refs_dir = match &self.refs_dir {
            Some(dir) => dir,
            None => return Ok(()),
        };

        // Load HEAD
        let head_path = refs_dir.join("HEAD");
        if head_path.exists() {
            let content = fs::read_to_string(&head_path)?;
            let content = content.trim();
            if let Some(target) = content.strip_prefix("ref: ") {
                *self.head.write() = Ref::Symbolic(target.to_string());
            } else if let Ok(oid) = ObjectId::from_hex(content) {
                *self.head.write() = Ref::Direct(oid);
            }
        }

        // Load branches
        let heads_dir = refs_dir.join("heads");
        if heads_dir.exists() {
            for entry in fs::read_dir(&heads_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let content = fs::read_to_string(entry.path())?;
                    if let Ok(oid) = ObjectId::from_hex(content.trim()) {
                        self.branches.insert(name.clone(), Branch::new(name, oid));
                    }
                }
            }
        }

        // Load tags
        let tags_dir = refs_dir.join("tags");
        if tags_dir.exists() {
            for entry in fs::read_dir(&tags_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let content = fs::read_to_string(entry.path())?;
                    if let Ok(oid) = ObjectId::from_hex(content.trim()) {
                        self.tags.insert(name.clone(), Tag::lightweight(name, oid));
                    }
                }
            }
        }

        Ok(())
    }

    /// Persist a reference to disk
    fn persist_ref(&self, ref_path: &str, oid: &ObjectId) -> Result<(), RefError> {
        let refs_dir = match &self.refs_dir {
            Some(dir) => dir,
            None => return Ok(()),
        };

        let full_path = refs_dir.join(ref_path.strip_prefix("refs/").unwrap_or(ref_path));
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full_path, format!("{}\n", oid.to_hex()))?;
        Ok(())
    }

    /// Create or update a branch
    pub fn update_branch(&self, name: &str, commit: ObjectId) -> Result<(), RefError> {
        validate_ref_name(name)?;

        let now = current_timestamp_us();

        self.branches
            .entry(name.to_string())
            .and_modify(|b| {
                b.commit = commit;
                b.updated_at = now;
            })
            .or_insert_with(|| Branch::new(name, commit));

        self.persist_ref(&format!("refs/heads/{}", name), &commit)?;
        Ok(())
    }

    /// Delete a branch
    pub fn delete_branch(&self, name: &str) -> Result<Branch, RefError> {
        let (_, branch) = self
            .branches
            .remove(name)
            .ok_or_else(|| RefError::NotFound(format!("refs/heads/{}", name)))?;

        // Remove from disk
        if let Some(ref refs_dir) = self.refs_dir {
            let path = refs_dir.join("heads").join(name);
            if path.exists() {
                fs::remove_file(path)?;
            }
        }

        Ok(branch)
    }

    /// Create an immutable tag
    pub fn create_tag(
        &self,
        name: &str,
        target: ObjectId,
        annotation: Option<(Author, String)>,
    ) -> Result<(), RefError> {
        validate_ref_name(name)?;

        if self.tags.contains_key(name) {
            return Err(RefError::TagExists(name.to_string()));
        }

        let tag = match annotation {
            Some((tagger, message)) => Tag::annotated(name, target, tagger, message),
            None => Tag::lightweight(name, target),
        };

        self.tags.insert(name.to_string(), tag);
        self.persist_ref(&format!("refs/tags/{}", name), &target)?;
        Ok(())
    }

    /// Delete a tag
    pub fn delete_tag(&self, name: &str) -> Result<Tag, RefError> {
        let (_, tag) = self
            .tags
            .remove(name)
            .ok_or_else(|| RefError::NotFound(format!("refs/tags/{}", name)))?;

        // Remove from disk
        if let Some(ref refs_dir) = self.refs_dir {
            let path = refs_dir.join("tags").join(name);
            if path.exists() {
                fs::remove_file(path)?;
            }
        }

        Ok(tag)
    }

    /// Resolve a ref to a commit ID
    ///
    /// Handles:
    /// - HEAD
    /// - refs/heads/<branch>
    /// - refs/tags/<tag>
    /// - <branch> (shorthand)
    /// - <tag> (shorthand)
    /// - <commit-id> (full or prefix)
    pub fn resolve(&self, ref_name: &str) -> Result<ObjectId, RefError> {
        // Handle special refs
        if ref_name == "HEAD" {
            let head = self.head.read();
            return match &*head {
                Ref::Direct(oid) => Ok(*oid),
                Ref::Symbolic(target) => self.resolve(target),
            };
        }

        // Try full ref path first
        if let Some(rest) = ref_name.strip_prefix("refs/heads/") {
            if let Some(branch) = self.branches.get(rest) {
                return Ok(branch.commit);
            }
        }

        if let Some(rest) = ref_name.strip_prefix("refs/tags/") {
            if let Some(tag) = self.tags.get(rest) {
                return Ok(tag.target);
            }
        }

        // Try as branch name
        if let Some(branch) = self.branches.get(ref_name) {
            return Ok(branch.commit);
        }

        // Try as tag name
        if let Some(tag) = self.tags.get(ref_name) {
            return Ok(tag.target);
        }

        // Try parsing as object ID (full or prefix)
        if ref_name.len() >= 7 && ref_name.chars().all(|c| c.is_ascii_hexdigit()) {
            // Full ID
            if ref_name.len() == 64 {
                return ObjectId::from_hex(ref_name)
                    .map_err(|_| RefError::NotFound(ref_name.to_string()));
            }
            // For prefix matching, we'd need to scan the object store
            // For now, return not found
        }

        Err(RefError::NotFound(ref_name.to_string()))
    }

    /// Update HEAD
    pub fn set_head(&self, target: Ref) -> Result<(), RefError> {
        *self.head.write() = target.clone();

        // Persist HEAD
        if let Some(ref refs_dir) = self.refs_dir {
            let head_path = refs_dir.join("HEAD");
            let content = match target {
                Ref::Direct(oid) => oid.to_hex(),
                Ref::Symbolic(ref_name) => format!("ref: {}", ref_name),
            };
            fs::write(head_path, format!("{}\n", content))?;
        }

        Ok(())
    }

    /// Get current HEAD
    pub fn get_head(&self) -> Ref {
        self.head.read().clone()
    }

    /// Get HEAD commit ID (resolving symbolic refs)
    pub fn head(&self) -> Option<ObjectId> {
        self.resolve("HEAD").ok()
    }

    /// Get current branch name (if HEAD points to a branch)
    pub fn current_branch(&self) -> Option<String> {
        let head = self.head.read();
        match &*head {
            Ref::Symbolic(target) => target.strip_prefix("refs/heads/").map(|s| s.to_string()),
            Ref::Direct(_) => None, // Detached HEAD
        }
    }

    /// List all branches
    pub fn list_branches(&self) -> Vec<Branch> {
        self.branches.iter().map(|r| r.value().clone()).collect()
    }

    /// List all tags
    pub fn list_tags(&self) -> Vec<Tag> {
        self.tags.iter().map(|r| r.value().clone()).collect()
    }

    /// Get a branch by name
    pub fn get_branch(&self, name: &str) -> Option<Branch> {
        self.branches.get(name).map(|r| r.clone())
    }

    /// Get a tag by name
    pub fn get_tag(&self, name: &str) -> Option<Tag> {
        self.tags.get(name).map(|r| r.clone())
    }

    /// Check if a branch exists
    pub fn branch_exists(&self, name: &str) -> bool {
        self.branches.contains_key(name)
    }

    /// Check if a tag exists
    pub fn tag_exists(&self, name: &str) -> bool {
        self.tags.contains_key(name)
    }

    /// Save refs to a file (simple binary serialization)
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), RefError> {
        use std::io::Write;

        let head = self.head.read().clone();
        let branches: Vec<Branch> = self.branches.iter().map(|r| r.value().clone()).collect();
        let tags: Vec<Tag> = self.tags.iter().map(|r| r.value().clone()).collect();

        let data = bincode::serialize(&(head, branches, tags)).map_err(|e| {
            RefError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        let mut file = std::fs::File::create(path)?;
        file.write_all(&data)?;
        Ok(())
    }

    /// Load refs from a file
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, RefError> {
        let data = std::fs::read(path)?;

        let (head, branches, tags): (Ref, Vec<Branch>, Vec<Tag>) = bincode::deserialize(&data)
            .map_err(|e| {
                RefError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;

        let store = Self::new();
        *store.head.write() = head;

        for branch in branches {
            store.branches.insert(branch.name.clone(), branch);
        }

        for tag in tags {
            store.tags.insert(tag.name.clone(), tag);
        }

        Ok(store)
    }
}

impl Default for RefStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate reference name (similar to Git's rules)
fn validate_ref_name(name: &str) -> Result<(), RefError> {
    if name.is_empty() {
        return Err(RefError::InvalidName("empty name".to_string()));
    }

    if name.starts_with('.') || name.ends_with('.') {
        return Err(RefError::InvalidName(
            "cannot start or end with '.'".to_string(),
        ));
    }

    if name.contains("..") {
        return Err(RefError::InvalidName("cannot contain '..'".to_string()));
    }

    if name.contains("//") {
        return Err(RefError::InvalidName("cannot contain '//'".to_string()));
    }

    let invalid_chars = ['~', '^', ':', '\\', '?', '*', '[', ' ', '\t', '\n'];
    for c in invalid_chars {
        if name.contains(c) {
            return Err(RefError::InvalidName(format!(
                "cannot contain '{}'",
                c.escape_default()
            )));
        }
    }

    Ok(())
}

/// Get current timestamp in microseconds
fn current_timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_operations() {
        let store = RefStore::new();

        let oid1 = ObjectId::from_content(b"commit1");
        let oid2 = ObjectId::from_content(b"commit2");

        // Create branch
        store.update_branch("main", oid1).unwrap();
        assert!(store.branch_exists("main"));

        // Update branch
        store.update_branch("main", oid2).unwrap();
        assert_eq!(store.resolve("main").unwrap(), oid2);

        // List branches
        let branches = store.list_branches();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "main");
    }

    #[test]
    fn test_tag_operations() {
        let store = RefStore::new();

        let oid = ObjectId::from_content(b"commit");

        // Create lightweight tag
        store.create_tag("v1.0", oid, None).unwrap();
        assert!(store.tag_exists("v1.0"));

        // Cannot create duplicate tag
        assert!(store.create_tag("v1.0", oid, None).is_err());

        // Create annotated tag
        store
            .create_tag(
                "v2.0",
                oid,
                Some((Author::new("tester"), "Release 2.0".to_string())),
            )
            .unwrap();

        let tag = store.get_tag("v2.0").unwrap();
        assert!(tag.is_annotated());
        assert_eq!(tag.message.as_deref(), Some("Release 2.0"));
    }

    #[test]
    fn test_resolve() {
        let store = RefStore::new();

        let oid = ObjectId::from_content(b"commit");
        store.update_branch("main", oid).unwrap();
        store.update_branch("feature", oid).unwrap();
        store.create_tag("v1.0", oid, None).unwrap();

        // Resolve various formats
        assert_eq!(store.resolve("main").unwrap(), oid);
        assert_eq!(store.resolve("refs/heads/main").unwrap(), oid);
        assert_eq!(store.resolve("v1.0").unwrap(), oid);
        assert_eq!(store.resolve("refs/tags/v1.0").unwrap(), oid);

        // HEAD should resolve to main (default)
        store
            .set_head(Ref::Symbolic("refs/heads/main".to_string()))
            .unwrap();
        assert_eq!(store.resolve("HEAD").unwrap(), oid);
    }

    #[test]
    fn test_head_operations() {
        let store = RefStore::new();

        let oid = ObjectId::from_content(b"commit");

        // Symbolic HEAD
        store
            .set_head(Ref::Symbolic("refs/heads/main".to_string()))
            .unwrap();
        assert_eq!(store.current_branch(), Some("main".to_string()));

        // Detached HEAD
        store.set_head(Ref::Direct(oid)).unwrap();
        assert_eq!(store.current_branch(), None);
    }

    #[test]
    fn test_ref_name_validation() {
        // Valid names
        assert!(validate_ref_name("main").is_ok());
        assert!(validate_ref_name("feature/test").is_ok());
        assert!(validate_ref_name("v1.0.0").is_ok());

        // Invalid names
        assert!(validate_ref_name("").is_err());
        assert!(validate_ref_name(".hidden").is_err());
        assert!(validate_ref_name("bad..name").is_err());
        assert!(validate_ref_name("has space").is_err());
    }
}
