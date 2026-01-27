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

//! Git-Like Response Versioning System
//!
//! A content-addressable version control system for LLM responses,
//! inspired by Git's object model.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Git-Like Object Model                         │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐        │
//! │  │   Blob      │     │   Tree      │     │   Commit    │        │
//! │  │ (Response)  │◄────│ (Snapshot)  │◄────│  (Version)  │        │
//! │  └─────────────┘     └─────────────┘     └─────────────┘        │
//! │       │                    │                    │                │
//! │       ▼                    ▼                    ▼                │
//! │  ┌─────────────────────────────────────────────────────┐        │
//! │  │          Content-Addressable Object Store           │        │
//! │  │              (BLAKE3 hashed, immutable)             │        │
//! │  └─────────────────────────────────────────────────────┘        │
//! │                                                                  │
//! │  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐        │
//! │  │   Branch    │     │    Tag      │     │    HEAD     │        │
//! │  │  (mutable)  │     │ (immutable) │     │  (current)  │        │
//! │  └─────────────┘     └─────────────┘     └─────────────┘        │
//! │       │                    │                    │                │
//! │       ▼                    ▼                    ▼                │
//! │  ┌─────────────────────────────────────────────────────┐        │
//! │  │               Reference Store (refs/)               │        │
//! │  └─────────────────────────────────────────────────────┘        │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Key Features
//!
//! - **Content-Addressable Storage**: Same content = same hash (deduplication)
//! - **Immutable Objects**: Blobs, trees, commits are never modified
//! - **Mutable References**: Branches point to commits, can be updated
//! - **Full History**: Parent chains enable complete version history
//! - **Branching**: Experiment variants as branches
//! - **Diffing**: Patience algorithm + semantic similarity

pub mod diff;
pub mod objects;
pub mod refs;
pub mod repository;
pub mod store;

pub use diff::{
    AddedEntry, BlobDiff, CommitDiff, DiffConfig, DiffEngine, DiffHunk, DiffLine, DiffStats,
    LineChange, ModifiedEntry, RemovedEntry, TreeDiff,
};
pub use objects::{
    Author, Blob, Commit, CommitMetadata, ContentType, EntryMode, GitObject, ObjectId, ObjectType,
    TokenUsage, Tree, TreeEntry,
};
pub use refs::{Branch, Ref, RefError, RefStore, Tag};
pub use repository::{
    Experiment, ExperimentVariant, LogEntry, RepositoryError, ResponseRepository, ResponseSnapshot,
    TokenUsage as RepoTokenUsage,
};
pub use store::{ObjectStore, StoreError, StoreStats};
