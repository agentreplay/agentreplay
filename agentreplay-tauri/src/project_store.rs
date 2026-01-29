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

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectStore {
    projects: HashMap<String, Project>,
    #[serde(skip)]
    file_path: PathBuf,
}

impl ProjectStore {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            projects: HashMap::new(),
            file_path,
        }
    }

    pub fn load(file_path: PathBuf) -> Result<Self> {
        if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            let mut store: ProjectStore = serde_json::from_str(&content)?;
            store.file_path = file_path;
            Ok(store)
        } else {
            Ok(Self::new(file_path))
        }
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&self.file_path, content)?;
        Ok(())
    }

    pub fn add(&mut self, project: Project) -> Result<()> {
        self.projects.insert(project.id.clone(), project);
        self.save()
    }

    pub fn list(&self) -> Vec<Project> {
        let mut projects: Vec<Project> = self.projects.values().cloned().collect();
        // Sort by created_at descending (newest first)
        projects.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        projects
    }

    pub fn get(&self, id: &str) -> Option<Project> {
        self.projects.get(id).cloned()
    }

    pub fn remove(&mut self, id: &str) -> Result<Option<Project>> {
        let removed = self.projects.remove(id);
        self.save()?;
        Ok(removed)
    }

    pub fn clear(&mut self) -> Result<()> {
        self.projects.clear();
        self.save()
    }
}
