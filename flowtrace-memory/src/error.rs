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

//! Memory system error types

use thiserror::Error;

/// Result type for memory operations
pub type MemoryResult<T> = Result<T, MemoryError>;

/// Errors that can occur in the memory system
#[derive(Debug, Error)]
pub enum MemoryError {
    /// Observation not found
    #[error("Observation not found: {0}")]
    ObservationNotFound(String),

    /// Session not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Workspace not found
    #[error("Workspace not found: {0}")]
    WorkspaceNotFound(String),

    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Index error
    #[error("Index error: {0}")]
    IndexError(String),

    /// Embedding error
    #[error("Embedding error: {0}")]
    EmbeddingError(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Context packing error
    #[error("Context packing error: {0}")]
    ContextPackingError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Generic error
    #[error("Memory error: {0}")]
    Other(String),
}

impl From<serde_json::Error> for MemoryError {
    fn from(e: serde_json::Error) -> Self {
        MemoryError::SerializationError(e.to_string())
    }
}
