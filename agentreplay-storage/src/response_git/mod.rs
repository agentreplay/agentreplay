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
