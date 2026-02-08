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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A saved query view that users can reuse
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedView {
    /// Unique identifier for the view
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Query filters (as JSON object)
    pub filters: serde_json::Value,
    /// Columns to display
    pub columns: Vec<String>,
    /// User who created this view (optional)
    pub user_id: Option<String>,
    /// When the view was created (microseconds since epoch)
    pub created_at: u64,
    /// When the view was last updated (microseconds since epoch)
    pub updated_at: u64,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Whether this view is shared with the team
    pub is_shared: bool,
}

impl SavedView {
    pub fn new(name: String, filters: serde_json::Value, columns: Vec<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        let id = format!("view-{}", uuid::Uuid::new_v4());

        Self {
            id,
            name,
            description: None,
            filters,
            columns,
            user_id: None,
            created_at: now,
            updated_at: now,
            tags: Vec::new(),
            is_shared: false,
        }
    }
}

/// Registry for managing saved views
pub struct SavedViewRegistry {
    views: HashMap<String, SavedView>,
    file_path: std::path::PathBuf,
}

impl SavedViewRegistry {
    pub fn new(data_dir: &std::path::Path) -> Self {
        let file_path = data_dir.join("saved_views.json");
        let mut registry = Self {
            views: HashMap::new(),
            file_path,
        };

        // Try to load existing views
        if let Err(e) = registry.load() {
            eprintln!("Warning: Failed to load saved views: {}", e);
        }

        registry
    }

    /// Load views from disk
    fn load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.file_path.exists() {
            return Ok(());
        }

        let contents = std::fs::read_to_string(&self.file_path)?;
        let views: Vec<SavedView> = serde_json::from_str(&contents)?;

        for view in views {
            self.views.insert(view.id.clone(), view);
        }

        println!("Loaded {} saved views from registry", self.views.len());
        Ok(())
    }

    /// Save views to disk
    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let views: Vec<&SavedView> = self.views.values().collect();
        let json = serde_json::to_string_pretty(&views)?;

        // Create backup before saving
        if self.file_path.exists() {
            let backup_path = self.file_path.with_extension("json.bak");
            std::fs::copy(&self.file_path, backup_path)?;
        }

        std::fs::write(&self.file_path, json)?;
        Ok(())
    }

    /// Add a new view
    pub fn add_view(&mut self, view: SavedView) -> Result<SavedView, String> {
        if self.views.contains_key(&view.id) {
            return Err(format!("View with id {} already exists", view.id));
        }

        let view_clone = view.clone();
        self.views.insert(view.id.clone(), view);

        if let Err(e) = self.save() {
            return Err(format!("Failed to save view: {}", e));
        }

        Ok(view_clone)
    }

    /// Update an existing view
    pub fn update_view(&mut self, view: SavedView) -> Result<SavedView, String> {
        if !self.views.contains_key(&view.id) {
            return Err(format!("View with id {} not found", view.id));
        }

        let mut updated_view = view;
        updated_view.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        self.views
            .insert(updated_view.id.clone(), updated_view.clone());

        if let Err(e) = self.save() {
            return Err(format!("Failed to save view: {}", e));
        }

        Ok(updated_view)
    }

    /// Get a view by ID
    pub fn get_view(&self, id: &str) -> Option<&SavedView> {
        self.views.get(id)
    }

    /// List all views
    pub fn list_views(&self) -> Vec<&SavedView> {
        self.views.values().collect()
    }

    /// Delete a view
    pub fn delete_view(&mut self, id: &str) -> Result<(), String> {
        if !self.views.contains_key(id) {
            return Err(format!("View with id {} not found", id));
        }

        self.views.remove(id);

        if let Err(e) = self.save() {
            return Err(format!("Failed to save after deletion: {}", e));
        }

        Ok(())
    }

    /// Search views by name or tags
    pub fn search_views(&self, query: &str) -> Vec<&SavedView> {
        let query_lower = query.to_lowercase();
        self.views
            .values()
            .filter(|v| {
                v.name.to_lowercase().contains(&query_lower)
                    || v.description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
                    || v.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// Export views to JSON
    pub fn export_views(&self, view_ids: Option<Vec<String>>) -> Result<String, String> {
        let views: Vec<&SavedView> = if let Some(ids) = view_ids {
            ids.iter().filter_map(|id| self.views.get(id)).collect()
        } else {
            self.views.values().collect()
        };

        serde_json::to_string_pretty(&views).map_err(|e| format!("Failed to export views: {}", e))
    }

    /// Import views from JSON
    pub fn import_views(&mut self, json: &str, overwrite: bool) -> Result<usize, String> {
        let views: Vec<SavedView> =
            serde_json::from_str(json).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        let mut imported_count = 0;

        for view in views {
            if !overwrite && self.views.contains_key(&view.id) {
                continue; // Skip existing views if overwrite is false
            }

            self.views.insert(view.id.clone(), view);
            imported_count += 1;
        }

        if let Err(e) = self.save() {
            return Err(format!("Failed to save imported views: {}", e));
        }

        Ok(imported_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_saved_view_creation() {
        let filters = serde_json::json!({
            "environment": "production",
            "agent_id": "agent-1"
        });

        let columns = vec!["timestamp".to_string(), "trace_id".to_string()];

        let view = SavedView::new("Production Traces".to_string(), filters, columns);

        assert_eq!(view.name, "Production Traces");
        assert!(view.id.starts_with("view-"));
        assert_eq!(view.columns.len(), 2);
    }

    #[test]
    fn test_view_registry() {
        let temp_dir = TempDir::new().unwrap();
        let mut registry = SavedViewRegistry::new(temp_dir.path());

        let view = SavedView::new(
            "Test View".to_string(),
            serde_json::json!({}),
            vec!["col1".to_string()],
        );

        let added_view = registry.add_view(view.clone()).unwrap();
        assert_eq!(added_view.id, view.id);

        let retrieved = registry.get_view(&view.id).unwrap();
        assert_eq!(retrieved.name, "Test View");

        registry.delete_view(&view.id).unwrap();
        assert!(registry.get_view(&view.id).is_none());
    }

    #[test]
    fn test_view_search() {
        let temp_dir = TempDir::new().unwrap();
        let mut registry = SavedViewRegistry::new(temp_dir.path());

        let mut view1 =
            SavedView::new("Production View".to_string(), serde_json::json!({}), vec![]);
        view1.tags = vec!["prod".to_string(), "monitoring".to_string()];
        registry.add_view(view1).unwrap();

        let view2 = SavedView::new(
            "Development View".to_string(),
            serde_json::json!({}),
            vec![],
        );
        registry.add_view(view2).unwrap();

        let results = registry.search_views("production");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Production View");

        let tag_results = registry.search_views("monitoring");
        assert_eq!(tag_results.len(), 1);
    }
}
