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

//! Diff Engine - Patience Diff Algorithm
//!
//! Computes differences between blobs and trees using the patience diff algorithm.
//! Supports both textual and semantic (embedding-based) similarity.

use super::objects::{Blob, ContentType, ObjectId, Tree};
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;

/// Diff result between two commits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitDiff {
    /// Old commit ID
    pub old_commit: ObjectId,
    /// New commit ID
    pub new_commit: ObjectId,
    /// Tree diff
    pub tree_diff: TreeDiff,
    /// Summary statistics
    pub stats: DiffStats,
}

/// Diff result between two trees
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeDiff {
    /// Files added
    pub added: Vec<AddedEntry>,
    /// Files removed
    pub removed: Vec<RemovedEntry>,
    /// Files modified
    pub modified: Vec<ModifiedEntry>,
    /// Files unchanged
    pub unchanged: Vec<String>,
}

/// An added file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddedEntry {
    pub path: String,
    pub oid: ObjectId,
}

/// A removed file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedEntry {
    pub path: String,
    pub oid: ObjectId,
}

/// A modified file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifiedEntry {
    pub path: String,
    pub old_oid: ObjectId,
    pub new_oid: ObjectId,
    /// Optional blob diff (if content is text)
    pub blob_diff: Option<BlobDiff>,
}

/// Diff result between two blobs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobDiff {
    /// Hunks of changes
    pub hunks: Vec<DiffHunk>,
    /// Similarity ratio (0.0 - 1.0)
    pub similarity: f64,
    /// Whether semantic similarity was used
    pub semantic: bool,
}

/// A contiguous group of changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    /// Starting line in old content (1-indexed)
    pub old_start: usize,
    /// Number of lines in old content
    pub old_count: usize,
    /// Starting line in new content (1-indexed)
    pub new_start: usize,
    /// Number of lines in new content
    pub new_count: usize,
    /// Lines in this hunk
    pub lines: Vec<DiffLine>,
    /// Context around this hunk
    pub header: String,
}

/// A single line in a diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    /// Change type
    pub change: LineChange,
    /// Line content (without newline)
    pub content: String,
    /// Old line number (if applicable)
    pub old_line: Option<usize>,
    /// New line number (if applicable)
    pub new_line: Option<usize>,
}

/// Type of change for a line
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineChange {
    /// Line exists in both
    Context,
    /// Line was added
    Added,
    /// Line was removed
    Removed,
}

/// Diff statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffStats {
    pub files_added: usize,
    pub files_removed: usize,
    pub files_modified: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

/// Configuration for diff engine
#[derive(Debug, Clone)]
pub struct DiffConfig {
    /// Number of context lines around changes
    pub context_lines: usize,
    /// Use semantic (embedding-based) similarity for comparison
    pub use_semantic: bool,
    /// Ignore whitespace differences
    pub ignore_whitespace: bool,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            context_lines: 3,
            use_semantic: false,
            ignore_whitespace: false,
        }
    }
}

/// Diff engine using patience diff algorithm
pub struct DiffEngine {
    #[allow(dead_code)]
    config: DiffConfig,
}

impl DiffEngine {
    /// Create a new diff engine with default config
    pub fn new() -> Self {
        Self {
            config: DiffConfig::default(),
        }
    }

    /// Create with custom config
    pub fn with_config(config: DiffConfig) -> Self {
        Self { config }
    }

    /// Diff two trees
    pub fn diff_trees(&self, old_tree: &Tree, new_tree: &Tree) -> TreeDiff {
        let old_entries: HashMap<_, _> = old_tree
            .entries
            .iter()
            .map(|e| (e.name.clone(), e))
            .collect();

        let new_entries: HashMap<_, _> = new_tree
            .entries
            .iter()
            .map(|e| (e.name.clone(), e))
            .collect();

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut modified = Vec::new();
        let mut unchanged = Vec::new();

        // Find removed and modified
        for (name, old_entry) in &old_entries {
            match new_entries.get(name) {
                Some(new_entry) => {
                    if old_entry.oid != new_entry.oid {
                        modified.push(ModifiedEntry {
                            path: name.clone(),
                            old_oid: old_entry.oid,
                            new_oid: new_entry.oid,
                            blob_diff: None, // Will be filled in by caller if needed
                        });
                    } else {
                        unchanged.push(name.clone());
                    }
                }
                None => {
                    removed.push(RemovedEntry {
                        path: name.clone(),
                        oid: old_entry.oid,
                    });
                }
            }
        }

        // Find added
        for (name, new_entry) in &new_entries {
            if !old_entries.contains_key(name) {
                added.push(AddedEntry {
                    path: name.clone(),
                    oid: new_entry.oid,
                });
            }
        }

        // Sort for consistent output
        added.sort_by(|a, b| a.path.cmp(&b.path));
        removed.sort_by(|a, b| a.path.cmp(&b.path));
        modified.sort_by(|a, b| a.path.cmp(&b.path));
        unchanged.sort();

        TreeDiff {
            added,
            removed,
            modified,
            unchanged,
        }
    }

    /// Diff two blobs (text content)
    pub fn diff_blobs(&self, old_blob: &Blob, new_blob: &Blob) -> BlobDiff {
        // Handle non-text content
        if old_blob.content_type != ContentType::Text || new_blob.content_type != ContentType::Text
        {
            return self.diff_binary(old_blob, new_blob);
        }

        let old_text = old_blob.as_text().unwrap_or("");
        let new_text = new_blob.as_text().unwrap_or("");

        self.diff_text(old_text, new_text)
    }

    /// Diff two text strings
    pub fn diff_text(&self, old_text: &str, new_text: &str) -> BlobDiff {
        // Use patience algorithm from similar crate
        let diff = TextDiff::configure()
            .algorithm(similar::Algorithm::Patience)
            .diff_lines(old_text, new_text);

        let mut hunks = Vec::new();
        let mut current_hunk: Option<DiffHunk> = None;
        let mut _total_added = 0;
        let mut _total_removed = 0;

        let _ops = diff.ops().to_vec();
        let mut old_line = 0usize;
        let mut new_line = 0usize;

        for op in diff.iter_all_changes() {
            let change = match op.tag() {
                ChangeTag::Equal => LineChange::Context,
                ChangeTag::Insert => {
                    _total_added += 1;
                    LineChange::Added
                }
                ChangeTag::Delete => {
                    _total_removed += 1;
                    LineChange::Removed
                }
            };

            let (old_ln, new_ln) = match change {
                LineChange::Context => {
                    old_line += 1;
                    new_line += 1;
                    (Some(old_line), Some(new_line))
                }
                LineChange::Added => {
                    new_line += 1;
                    (None, Some(new_line))
                }
                LineChange::Removed => {
                    old_line += 1;
                    (Some(old_line), None)
                }
            };

            let line = DiffLine {
                change,
                content: op.value().trim_end_matches('\n').to_string(),
                old_line: old_ln,
                new_line: new_ln,
            };

            // Build hunks with context
            if change != LineChange::Context {
                if let Some(ref mut hunk) = current_hunk {
                    hunk.lines.push(line);
                } else {
                    current_hunk = Some(DiffHunk {
                        old_start: old_ln.unwrap_or(1),
                        old_count: 0,
                        new_start: new_ln.unwrap_or(1),
                        new_count: 0,
                        lines: vec![line],
                        header: String::new(),
                    });
                }
            } else if let Some(ref mut hunk) = current_hunk {
                // Context line near a change
                hunk.lines.push(line);
            }
        }

        // Finalize last hunk
        if let Some(mut hunk) = current_hunk {
            hunk.old_count = hunk.lines.iter().filter(|l| l.old_line.is_some()).count();
            hunk.new_count = hunk.lines.iter().filter(|l| l.new_line.is_some()).count();
            hunk.header = format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
            );
            hunks.push(hunk);
        }

        // Calculate similarity
        let similarity = diff.ratio() as f64;

        BlobDiff {
            hunks,
            similarity,
            semantic: false,
        }
    }

    /// Diff binary content (no textual diff, just similarity)
    fn diff_binary(&self, old_blob: &Blob, new_blob: &Blob) -> BlobDiff {
        let similarity = if old_blob.data == new_blob.data {
            1.0
        } else {
            0.0
        };

        BlobDiff {
            hunks: Vec::new(),
            similarity,
            semantic: false,
        }
    }

    /// Calculate diff statistics
    pub fn calculate_stats(&self, tree_diff: &TreeDiff) -> DiffStats {
        let mut stats = DiffStats {
            files_added: tree_diff.added.len(),
            files_removed: tree_diff.removed.len(),
            files_modified: tree_diff.modified.len(),
            lines_added: 0,
            lines_removed: 0,
        };

        for modified in &tree_diff.modified {
            if let Some(ref blob_diff) = modified.blob_diff {
                for hunk in &blob_diff.hunks {
                    for line in &hunk.lines {
                        match line.change {
                            LineChange::Added => stats.lines_added += 1,
                            LineChange::Removed => stats.lines_removed += 1,
                            LineChange::Context => {}
                        }
                    }
                }
            }
        }

        stats
    }
}

impl Default for DiffEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeDiff {
    /// Check if there are any changes
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }

    /// Get total number of changed files
    pub fn changed_count(&self) -> usize {
        self.added.len() + self.removed.len() + self.modified.len()
    }
}

impl BlobDiff {
    /// Check if there are any changes
    pub fn is_empty(&self) -> bool {
        self.hunks.is_empty()
    }

    /// Format as unified diff string
    pub fn to_unified(&self, old_path: &str, new_path: &str) -> String {
        let mut output = String::new();
        output.push_str(&format!("--- {}\n", old_path));
        output.push_str(&format!("+++ {}\n", new_path));

        for hunk in &self.hunks {
            output.push_str(&hunk.header);
            output.push('\n');

            for line in &hunk.lines {
                let prefix = match line.change {
                    LineChange::Context => ' ',
                    LineChange::Added => '+',
                    LineChange::Removed => '-',
                };
                output.push(prefix);
                output.push_str(&line.content);
                output.push('\n');
            }
        }

        output
    }
}

impl DiffStats {
    /// Net lines changed
    pub fn net_change(&self) -> i64 {
        self.lines_added as i64 - self.lines_removed as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_diff_simple() {
        let engine = DiffEngine::new();

        let old = "line1\nline2\nline3\n";
        let new = "line1\nmodified\nline3\n";

        let diff = engine.diff_text(old, new);

        assert!(!diff.is_empty());
        assert!(diff.similarity > 0.0);
        assert!(diff.similarity < 1.0);
    }

    #[test]
    fn test_text_diff_addition() {
        let engine = DiffEngine::new();

        let old = "line1\nline2\n";
        let new = "line1\nline2\nline3\n";

        let diff = engine.diff_text(old, new);

        let added_count: usize = diff
            .hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| l.change == LineChange::Added)
            .count();

        assert_eq!(added_count, 1);
    }

    #[test]
    fn test_text_diff_removal() {
        let engine = DiffEngine::new();

        let old = "line1\nline2\nline3\n";
        let new = "line1\nline3\n";

        let diff = engine.diff_text(old, new);

        let removed_count: usize = diff
            .hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| l.change == LineChange::Removed)
            .count();

        assert_eq!(removed_count, 1);
    }

    #[test]
    fn test_identical_text() {
        let engine = DiffEngine::new();

        let text = "line1\nline2\nline3\n";

        let diff = engine.diff_text(text, text);

        assert!(diff.is_empty());
        assert_eq!(diff.similarity, 1.0);
    }

    #[test]
    fn test_tree_diff() {
        let engine = DiffEngine::new();

        use crate::response_git::objects::{EntryMode, Tree, TreeEntry};

        let mut old_tree = Tree::new();
        old_tree.add_entry("file1".to_string(), ObjectId::default(), EntryMode::Blob);
        old_tree.add_entry("file2".to_string(), ObjectId::default(), EntryMode::Blob);

        let mut new_tree = Tree::new();
        new_tree.add_entry("file1".to_string(), ObjectId::default(), EntryMode::Blob);
        new_tree.add_entry("file3".to_string(), ObjectId::default(), EntryMode::Blob);

        let diff = engine.diff_trees(&old_tree, &new_tree);

        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.unchanged.len(), 1);
        assert_eq!(diff.added[0].path, "file3");
        assert_eq!(diff.removed[0].path, "file2");
    }

    #[test]
    fn test_blob_diff() {
        let engine = DiffEngine::new();

        let old_blob = Blob::text("Hello\nWorld\n");
        let new_blob = Blob::text("Hello\nRust\n");

        let diff = engine.diff_blobs(&old_blob, &new_blob);

        assert!(!diff.is_empty());
        assert!(!diff.semantic);
    }

    #[test]
    fn test_unified_format() {
        let engine = DiffEngine::new();

        let old = "line1\nline2\n";
        let new = "line1\nmodified\n";

        let diff = engine.diff_text(old, new);
        let unified = diff.to_unified("a/file.txt", "b/file.txt");

        assert!(unified.contains("--- a/file.txt"));
        assert!(unified.contains("+++ b/file.txt"));
        assert!(unified.contains("-line2"));
        assert!(unified.contains("+modified"));
    }
}
