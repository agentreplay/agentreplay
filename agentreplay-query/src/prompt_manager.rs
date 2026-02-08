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

// Integration point for PromptManager
use agentreplay_prompts::{InMemoryDeploymentTracker, InMemoryPromptStorage, PromptManager};
use std::sync::Arc;

pub struct QueryPromptManager {
    inner: PromptManager<InMemoryPromptStorage, InMemoryDeploymentTracker>,
}

impl Default for QueryPromptManager {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryPromptManager {
    pub fn new() -> Self {
        let storage = Arc::new(InMemoryPromptStorage::new());
        let tracker = Arc::new(InMemoryDeploymentTracker::new());
        Self {
            inner: PromptManager::new(storage, tracker),
        }
    }

    pub fn inner(&self) -> &PromptManager<InMemoryPromptStorage, InMemoryDeploymentTracker> {
        &self.inner
    }
}
