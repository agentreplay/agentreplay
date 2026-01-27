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

//! Git Object Types
//!
//! Content-addressable objects: Blob, Tree, Commit
//! All objects are immutable once created.

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Object ID - BLAKE3 hash (32 bytes), like Git's SHA-1
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub [u8; 32]);

impl ObjectId {
    /// Create from content (content-addressable)
    pub fn from_content(content: &[u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(content);
        Self(hasher.finalize().into())
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Display as short hex string (like git short hash, 14 chars)
    pub fn short(&self) -> String {
        hex::encode(&self.0[..7])
    }

    /// Full hex representation
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }

    /// Parse from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self, ParseError> {
        let bytes = hex::decode(hex_str).map_err(|_| ParseError::InvalidHex)?;
        if bytes.len() != 32 {
            return Err(ParseError::InvalidLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// Check if this ID starts with the given prefix (for short ID matching)
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.to_hex().starts_with(prefix)
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short())
    }
}

impl Default for ObjectId {
    fn default() -> Self {
        Self([0u8; 32])
    }
}

/// Parse errors for ObjectId
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    InvalidHex,
    InvalidLength,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidHex => write!(f, "Invalid hex string"),
            ParseError::InvalidLength => write!(f, "Invalid length (expected 32 bytes)"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Object type enum (like Git's object types)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ObjectType {
    /// Raw content (LLM response, input, etc.)
    Blob = 1,
    /// Directory-like structure pointing to blobs
    Tree = 2,
    /// Snapshot with metadata, parent refs
    Commit = 3,
}

/// Blob object - stores raw content (response text, JSON, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blob {
    /// The actual content
    pub data: Vec<u8>,
    /// Content type hint
    pub content_type: ContentType,
}

/// Content type for blobs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentType {
    /// Plain text
    Text,
    /// JSON data
    Json,
    /// Markdown
    Markdown,
    /// Tool call JSON
    ToolCall,
    /// Binary data
    Binary,
}

impl Blob {
    /// Create a new blob
    pub fn new(data: Vec<u8>, content_type: ContentType) -> Self {
        Self { data, content_type }
    }

    /// Create a text blob
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            data: content.into().into_bytes(),
            content_type: ContentType::Text,
        }
    }

    /// Create a JSON blob
    pub fn json(value: &serde_json::Value) -> Self {
        Self {
            data: serde_json::to_vec(value).unwrap_or_default(),
            content_type: ContentType::Json,
        }
    }

    /// Compute object ID
    pub fn object_id(&self) -> ObjectId {
        let serialized = bincode::serialize(self).unwrap();
        ObjectId::from_content(&serialized)
    }

    /// Try to get content as UTF-8 text
    pub fn as_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.data).ok()
    }

    /// Try to parse as JSON
    pub fn as_json(&self) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.data).ok()
    }

    /// Get data length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Tree entry - reference to a blob or subtree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    /// Name of this entry (e.g., "input", "output", "tool_calls")
    pub name: String,
    /// Object ID of the blob or subtree
    pub oid: ObjectId,
    /// Entry mode (like git file modes)
    pub mode: EntryMode,
}

/// Entry mode (like Git file modes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryMode {
    /// Regular blob (100644)
    Blob,
    /// Executable blob (100755)
    Executable,
    /// Subtree (040000)
    Tree,
    /// Symbolic link (120000)
    Symlink,
}

/// Tree object - snapshot of a response state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tree {
    /// Sorted entries (like Git trees)
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    /// Create an empty tree
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add an entry, maintaining sort order
    pub fn add_entry(&mut self, name: String, oid: ObjectId, mode: EntryMode) {
        // Remove existing entry with same name if present
        self.entries.retain(|e| e.name != name);
        self.entries.push(TreeEntry { name, oid, mode });
        self.entries.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Compute object ID
    pub fn object_id(&self) -> ObjectId {
        let serialized = bincode::serialize(self).unwrap();
        ObjectId::from_content(&serialized)
    }

    /// Get entry by name
    pub fn get(&self, name: &str) -> Option<&TreeEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over entries
    pub fn iter(&self) -> impl Iterator<Item = &TreeEntry> {
        self.entries.iter()
    }
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

/// Commit object - versioned snapshot with parent chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    /// Tree this commit points to
    pub tree: ObjectId,
    /// Parent commit(s) - empty for initial, one for linear, two+ for merge
    pub parents: Vec<ObjectId>,
    /// Commit message
    pub message: String,
    /// Author information
    pub author: Author,
    /// Committer (may differ from author)
    pub committer: Author,
    /// Timestamp (microseconds since epoch)
    pub timestamp_us: u64,
    /// Additional metadata
    pub metadata: CommitMetadata,
}

/// Author/committer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    /// Name
    pub name: String,
    /// Email (optional)
    pub email: Option<String>,
}

impl Author {
    /// Create a new author
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: None,
        }
    }

    /// Create with email
    pub fn with_email(name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: Some(email.into()),
        }
    }

    /// System author for automated commits
    pub fn system() -> Self {
        Self {
            name: "system".to_string(),
            email: None,
        }
    }
}

impl Default for Author {
    fn default() -> Self {
        Self::system()
    }
}

/// Commit metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommitMetadata {
    /// Original trace ID this version is from
    pub trace_id: Option<u128>,
    /// Span ID
    pub span_id: Option<u128>,
    /// Model used
    pub model: Option<String>,
    /// Experiment ID (for A/B testing)
    pub experiment_id: Option<u128>,
    /// Variant name
    pub variant: Option<String>,
    /// Token usage
    pub usage: Option<TokenUsage>,
    /// Latency in milliseconds
    pub latency_ms: Option<u64>,
    /// Cost in USD
    pub cost_usd: Option<f64>,
    /// Custom labels
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

impl Commit {
    /// Compute object ID
    pub fn object_id(&self) -> ObjectId {
        let serialized = bincode::serialize(self).unwrap();
        ObjectId::from_content(&serialized)
    }

    /// Create initial commit (no parents)
    pub fn initial(tree: ObjectId, message: impl Into<String>, author: Author) -> Self {
        Self {
            tree,
            parents: vec![],
            message: message.into(),
            author: author.clone(),
            committer: author,
            timestamp_us: current_timestamp_us(),
            metadata: CommitMetadata::default(),
        }
    }

    /// Create child commit (single parent)
    pub fn child(
        parent: ObjectId,
        tree: ObjectId,
        message: impl Into<String>,
        author: Author,
    ) -> Self {
        Self {
            tree,
            parents: vec![parent],
            message: message.into(),
            author: author.clone(),
            committer: author,
            timestamp_us: current_timestamp_us(),
            metadata: CommitMetadata::default(),
        }
    }

    /// Create merge commit (multiple parents)
    pub fn merge(
        parents: Vec<ObjectId>,
        tree: ObjectId,
        message: impl Into<String>,
        author: Author,
    ) -> Self {
        Self {
            tree,
            parents,
            message: message.into(),
            author: author.clone(),
            committer: author,
            timestamp_us: current_timestamp_us(),
            metadata: CommitMetadata::default(),
        }
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: CommitMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Check if this is the initial commit
    pub fn is_initial(&self) -> bool {
        self.parents.is_empty()
    }

    /// Check if this is a merge commit
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

/// Trait for Git objects
pub trait GitObject: Sized + Serialize + for<'de> Deserialize<'de> {
    /// Object type constant
    const TYPE: ObjectType;

    /// Get object type
    fn object_type(&self) -> ObjectType {
        Self::TYPE
    }

    /// Serialize to bytes
    fn serialize_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    /// Deserialize from bytes
    fn deserialize_bytes(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }

    /// Compute object ID
    fn compute_oid(&self) -> ObjectId {
        ObjectId::from_content(&self.serialize_bytes())
    }
}

impl GitObject for Blob {
    const TYPE: ObjectType = ObjectType::Blob;
}

impl GitObject for Tree {
    const TYPE: ObjectType = ObjectType::Tree;
}

impl GitObject for Commit {
    const TYPE: ObjectType = ObjectType::Commit;
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
    fn test_object_id_from_content() {
        let content1 = b"hello world";
        let content2 = b"hello world";
        let content3 = b"different";

        let oid1 = ObjectId::from_content(content1);
        let oid2 = ObjectId::from_content(content2);
        let oid3 = ObjectId::from_content(content3);

        assert_eq!(oid1, oid2); // Same content = same ID
        assert_ne!(oid1, oid3); // Different content = different ID
    }

    #[test]
    fn test_object_id_hex_roundtrip() {
        let oid = ObjectId::from_content(b"test");
        let hex = oid.to_hex();
        let parsed = ObjectId::from_hex(&hex).unwrap();
        assert_eq!(oid, parsed);
    }

    #[test]
    fn test_object_id_short() {
        let oid = ObjectId::from_content(b"test");
        let short = oid.short();
        assert_eq!(short.len(), 14); // 7 bytes = 14 hex chars
        assert!(oid.to_hex().starts_with(&short));
    }

    #[test]
    fn test_blob_creation() {
        let blob = Blob::text("Hello, world!");
        assert_eq!(blob.as_text(), Some("Hello, world!"));
        assert_eq!(blob.content_type, ContentType::Text);
    }

    #[test]
    fn test_tree_operations() {
        let mut tree = Tree::new();
        let oid1 = ObjectId::from_content(b"blob1");
        let oid2 = ObjectId::from_content(b"blob2");

        tree.add_entry("output".to_string(), oid1, EntryMode::Blob);
        tree.add_entry("input".to_string(), oid2, EntryMode::Blob);

        assert_eq!(tree.len(), 2);

        // Should be sorted
        assert_eq!(tree.entries[0].name, "input");
        assert_eq!(tree.entries[1].name, "output");

        // Get by name
        assert_eq!(tree.get("input").unwrap().oid, oid2);
        assert_eq!(tree.get("output").unwrap().oid, oid1);
    }

    #[test]
    fn test_commit_types() {
        let tree_oid = ObjectId::from_content(b"tree");
        let author = Author::new("test");

        let initial = Commit::initial(tree_oid, "Initial commit", author.clone());
        assert!(initial.is_initial());
        assert!(!initial.is_merge());

        let child = Commit::child(
            initial.object_id(),
            tree_oid,
            "Second commit",
            author.clone(),
        );
        assert!(!child.is_initial());
        assert!(!child.is_merge());

        let merge = Commit::merge(
            vec![child.object_id(), initial.object_id()],
            tree_oid,
            "Merge commit",
            author,
        );
        assert!(!merge.is_initial());
        assert!(merge.is_merge());
    }

    #[test]
    fn test_content_addressable() {
        let blob1 = Blob::text("same content");
        let blob2 = Blob::text("same content");
        let blob3 = Blob::text("different content");

        assert_eq!(blob1.object_id(), blob2.object_id());
        assert_ne!(blob1.object_id(), blob3.object_id());
    }
}
