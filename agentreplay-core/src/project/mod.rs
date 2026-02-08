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

//! Multi-Project and Git Worktree Support
//!
//! This module provides support for:
//! - Git worktree detection
//! - Multi-project context queries
//! - Project relationship tracking
//!
//! # Git Worktree Detection
//!
//! Git worktrees have a `.git` file (not directory) containing:
//! ```text
//! gitdir: /path/to/parent/.git/worktrees/branch-name
//! ```
//!
//! This module detects worktrees and enables shared context between
//! the parent project and worktree.

mod worktree;
mod project_id;

pub use worktree::{WorktreeDetector, WorktreeInfo, GitInfo};
pub use project_id::{ProjectId, generate_project_id};
