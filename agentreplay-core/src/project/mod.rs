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
