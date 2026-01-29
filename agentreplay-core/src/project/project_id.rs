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

//! Project ID generation.

use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::path::Path;

/// A typed project identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(pub u128);

impl ProjectId {
    /// Create a project ID from a path.
    pub fn from_path(path: &Path) -> Self {
        ProjectId(generate_project_id(path))
    }

    /// Create a project ID from a raw value.
    pub fn from_raw(id: u128) -> Self {
        ProjectId(id)
    }

    /// Get the raw ID value.
    pub fn raw(&self) -> u128 {
        self.0
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

/// Generate a deterministic project ID from a path.
///
/// The ID is based on the canonicalized absolute path to ensure consistency
/// across different working directories.
pub fn generate_project_id(path: &Path) -> u128 {
    // Canonicalize the path if possible
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical.to_string_lossy();

    // Use blake3 for deterministic hashing
    let hash = blake3::hash(path_str.as_bytes());
    let bytes = hash.as_bytes();

    // Take first 16 bytes for u128
    u128::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    ])
}

/// Generate a project ID from a string (e.g., git remote URL).
pub fn generate_project_id_from_string(s: &str) -> u128 {
    let hash = blake3::hash(s.as_bytes());
    let bytes = hash.as_bytes();

    u128::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_project_id_from_path() {
        let path = PathBuf::from("/tmp/test_project");
        let id = ProjectId::from_path(&path);
        assert!(id.raw() != 0);
    }

    #[test]
    fn test_project_id_deterministic() {
        let path = PathBuf::from("/tmp/test_project");
        let id1 = generate_project_id(&path);
        let id2 = generate_project_id(&path);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_different_paths_different_ids() {
        let path1 = PathBuf::from("/tmp/project1");
        let path2 = PathBuf::from("/tmp/project2");
        let id1 = generate_project_id(&path1);
        let id2 = generate_project_id(&path2);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_project_id_display() {
        let id = ProjectId(0x12345678_9abcdef0_12345678_9abcdef0);
        let display = format!("{}", id);
        assert_eq!(display.len(), 32);
    }

    #[test]
    fn test_from_string() {
        let id1 = generate_project_id_from_string("https://github.com/user/repo.git");
        let id2 = generate_project_id_from_string("https://github.com/user/repo.git");
        assert_eq!(id1, id2);
    }
}
