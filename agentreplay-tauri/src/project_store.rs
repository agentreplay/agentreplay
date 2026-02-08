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
