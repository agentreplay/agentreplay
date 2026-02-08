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

//! Git worktree detection.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Information about a detected git worktree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    /// Path to the worktree directory.
    pub worktree_path: PathBuf,
    /// Path to the parent repository.
    pub parent_path: PathBuf,
    /// Name of the worktree (usually the branch name).
    pub worktree_name: String,
    /// Project ID for this worktree.
    pub worktree_project_id: u128,
    /// Project ID for the parent repository.
    pub parent_project_id: u128,
}

impl WorktreeInfo {
    /// Get all related project IDs (parent and worktree).
    pub fn all_project_ids(&self) -> Vec<u128> {
        if self.parent_project_id != self.worktree_project_id {
            vec![self.parent_project_id, self.worktree_project_id]
        } else {
            vec![self.worktree_project_id]
        }
    }
}

/// Information about git repository status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    /// Whether this is a git repository.
    pub is_git_repo: bool,
    /// Whether this is a worktree (vs main repo).
    pub is_worktree: bool,
    /// Path to the .git directory or file.
    pub git_path: Option<PathBuf>,
    /// Worktree info if applicable.
    pub worktree_info: Option<WorktreeInfo>,
    /// Current branch name.
    pub branch: Option<String>,
}

impl Default for GitInfo {
    fn default() -> Self {
        Self {
            is_git_repo: false,
            is_worktree: false,
            git_path: None,
            worktree_info: None,
            branch: None,
        }
    }
}

/// Detector for git worktrees.
pub struct WorktreeDetector;

impl WorktreeDetector {
    /// Detect if a path is within a git worktree.
    ///
    /// Returns `Some(WorktreeInfo)` if the path is in a worktree,
    /// `None` if it's a regular repository or not a git repo.
    pub fn detect(path: &Path) -> Option<WorktreeInfo> {
        let git_path = Self::find_git_path(path)?;

        // Check if .git is a file (worktree) or directory (regular repo)
        if git_path.is_file() {
            Self::parse_worktree_gitfile(&git_path)
        } else {
            None
        }
    }

    /// Get comprehensive git information for a path.
    pub fn get_git_info(path: &Path) -> GitInfo {
        let git_path = match Self::find_git_path(path) {
            Some(p) => p,
            None => return GitInfo::default(),
        };

        let is_worktree = git_path.is_file();
        let worktree_info = if is_worktree {
            Self::parse_worktree_gitfile(&git_path)
        } else {
            None
        };

        let branch = Self::get_current_branch(&git_path);

        GitInfo {
            is_git_repo: true,
            is_worktree,
            git_path: Some(git_path),
            worktree_info,
            branch,
        }
    }

    /// Find the .git path for a given directory.
    fn find_git_path(path: &Path) -> Option<PathBuf> {
        let mut current = path.to_path_buf();

        loop {
            let git_path = current.join(".git");
            if git_path.exists() {
                return Some(git_path);
            }

            if !current.pop() {
                return None;
            }
        }
    }

    /// Parse a .git file to extract worktree information.
    fn parse_worktree_gitfile(git_file: &Path) -> Option<WorktreeInfo> {
        let content = fs::read_to_string(git_file).ok()?;

        // Parse "gitdir: /path/to/.git/worktrees/name"
        let gitdir_prefix = "gitdir: ";
        let gitdir_line = content
            .lines()
            .find(|l| l.starts_with(gitdir_prefix))?;

        let gitdir_path = PathBuf::from(gitdir_line.trim_start_matches(gitdir_prefix).trim());

        // Extract parent .git directory and worktree name
        // Path format: /path/to/repo/.git/worktrees/worktree-name
        let worktree_name = gitdir_path.file_name()?.to_str()?.to_string();

        // Navigate up to find parent .git directory
        let worktrees_dir = gitdir_path.parent()?; // .git/worktrees
        if worktrees_dir.file_name()?.to_str()? != "worktrees" {
            return None;
        }

        let parent_git_dir = worktrees_dir.parent()?; // .git
        let parent_path = parent_git_dir.parent()?.to_path_buf();

        let worktree_path = git_file.parent()?.to_path_buf();

        // Generate project IDs
        let worktree_project_id = super::generate_project_id(&worktree_path);
        let parent_project_id = super::generate_project_id(&parent_path);

        Some(WorktreeInfo {
            worktree_path,
            parent_path,
            worktree_name,
            worktree_project_id,
            parent_project_id,
        })
    }

    /// Get the current branch name.
    fn get_current_branch(git_path: &Path) -> Option<String> {
        let head_path = if git_path.is_file() {
            // For worktrees, we need to read from the linked .git directory
            let content = fs::read_to_string(git_path).ok()?;
            let gitdir_prefix = "gitdir: ";
            let gitdir_line = content.lines().find(|l| l.starts_with(gitdir_prefix))?;
            let gitdir_path =
                PathBuf::from(gitdir_line.trim_start_matches(gitdir_prefix).trim());
            gitdir_path.join("HEAD")
        } else {
            git_path.join("HEAD")
        };

        let head_content = fs::read_to_string(head_path).ok()?;

        // Parse "ref: refs/heads/branch-name"
        let ref_prefix = "ref: refs/heads/";
        if head_content.starts_with(ref_prefix) {
            Some(head_content.trim_start_matches(ref_prefix).trim().to_string())
        } else {
            // Detached HEAD - return short hash
            Some(head_content.trim()[..8].to_string())
        }
    }

    /// Check if two paths are in related repositories (worktree relationship).
    pub fn are_related(path1: &Path, path2: &Path) -> bool {
        let info1 = Self::get_git_info(path1);
        let info2 = Self::get_git_info(path2);

        match (&info1.worktree_info, &info2.worktree_info) {
            (Some(w1), Some(w2)) => {
                // Both are worktrees - check if same parent
                w1.parent_project_id == w2.parent_project_id
            }
            (Some(w), None) => {
                // One is worktree - check if other is its parent
                let parent_id = super::generate_project_id(&w.parent_path);
                let path2_id = super::generate_project_id(path2);
                parent_id == path2_id
            }
            (None, Some(w)) => {
                // One is worktree - check if other is its parent
                let parent_id = super::generate_project_id(&w.parent_path);
                let path1_id = super::generate_project_id(path1);
                parent_id == path1_id
            }
            (None, None) => {
                // Neither is a worktree - check if same repo
                super::generate_project_id(path1) == super::generate_project_id(path2)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_info_default() {
        let info = GitInfo::default();
        assert!(!info.is_git_repo);
        assert!(!info.is_worktree);
    }

    #[test]
    fn test_worktree_info_all_project_ids() {
        let info = WorktreeInfo {
            worktree_path: PathBuf::from("/worktree"),
            parent_path: PathBuf::from("/parent"),
            worktree_name: "feature".to_string(),
            worktree_project_id: 1,
            parent_project_id: 2,
        };

        let ids = info.all_project_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
    }

    #[test]
    fn test_worktree_info_same_project_id() {
        let info = WorktreeInfo {
            worktree_path: PathBuf::from("/path"),
            parent_path: PathBuf::from("/path"),
            worktree_name: "main".to_string(),
            worktree_project_id: 1,
            parent_project_id: 1,
        };

        let ids = info.all_project_ids();
        assert_eq!(ids.len(), 1);
    }
}
