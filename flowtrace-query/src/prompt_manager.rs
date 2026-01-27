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

// Integration point for PromptManager
use flowtrace_prompts::{InMemoryDeploymentTracker, InMemoryPromptStorage, PromptManager};
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
