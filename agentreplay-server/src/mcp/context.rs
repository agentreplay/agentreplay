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

//! MCP Project Context - Isolated project/tenant for MCP Memory
//!
//! MCP operates in a completely isolated context from LLM observability:
//! - **Tenant ID 2**: Dedicated tenant for MCP memory operations
//! - **Project**: Auto-created "MCP Memory" project on first use
//!
//! This ensures MCP's vector storage and RAG operations don't conflict
//! with agent tracing data (Tenant 1).

use crate::project_manager::ProjectManager;
use crate::project_registry::{ProjectMetadata, ProjectRegistry};
use anyhow::Result;
use agentreplay_query::Agentreplay;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

/// MCP Tenant ID - dedicated tenant for MCP memory operations
/// Tenant 1 = LLM Observability (agents, traces)
/// Tenant 2 = MCP Memory (vector storage, RAG)
pub const MCP_TENANT_ID: u64 = 2;

/// Default MCP project ID
pub const MCP_DEFAULT_PROJECT_ID: u16 = 1000;

/// Default MCP project name
pub const MCP_DEFAULT_PROJECT_NAME: &str = "MCP Memory";

/// MCP project info for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPProjectInfo {
    pub project_id: u16,
    pub project_name: String,
    pub tenant_id: u64,
    pub description: String,
    pub created_at: u64,
    pub vector_count: usize,
    pub collection_count: usize,
    pub last_activity: Option<u64>,
    pub storage_path: String,
}

/// MCP collection info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPCollection {
    pub name: String,
    pub document_count: usize,
    pub vector_count: usize,
    pub embedding_dimension: usize,
    pub created_at: u64,
    pub last_updated: u64,
}

/// MCP Project Context
///
/// Manages the isolated project/tenant context for MCP operations.
/// Automatically creates the MCP project on first use.
pub struct MCPContext {
    /// Project ID for MCP operations
    project_id: u16,
    /// Project name
    project_name: String,
    /// Tenant ID (always MCP_TENANT_ID = 2)
    tenant_id: u64,
    /// Reference to project manager
    #[allow(dead_code)]
    project_manager: Arc<ProjectManager>,
    /// Reference to project registry
    project_registry: Arc<ProjectRegistry>,
    /// Cached Agentreplay instance for MCP
    db: Option<Arc<Agentreplay>>,
}

impl MCPContext {
    /// Create or load the MCP context
    ///
    /// On first use, this creates the default "MCP Memory" project.
    /// Subsequent calls return the existing project.
    pub fn new(
        project_manager: Arc<ProjectManager>,
        project_registry: Arc<ProjectRegistry>,
    ) -> Result<Self> {
        let project_id = MCP_DEFAULT_PROJECT_ID;
        let project_name = MCP_DEFAULT_PROJECT_NAME.to_string();

        // Check if MCP project exists, create if not
        if project_registry.get_metadata(project_id).is_none() {
            info!(
                "Creating MCP Memory project (id={}, tenant={})",
                project_id, MCP_TENANT_ID
            );

            project_registry.register_project(
                project_id,
                project_name.clone(),
                Some("Dedicated project for MCP vector storage and RAG operations. Isolated from LLM observability tracing.".to_string()),
            )?;
        }

        // Get or open the project database
        let db = project_manager.get_or_open_project(project_id)?;

        Ok(Self {
            project_id,
            project_name,
            tenant_id: MCP_TENANT_ID,
            project_manager,
            project_registry,
            db: Some(db),
        })
    }

    /// Get the MCP project ID
    pub fn project_id(&self) -> u16 {
        self.project_id
    }

    /// Get the MCP tenant ID
    pub fn tenant_id(&self) -> u64 {
        self.tenant_id
    }

    /// Get the project name
    pub fn project_name(&self) -> &str {
        &self.project_name
    }

    /// Get the Agentreplay database for MCP operations
    pub fn db(&self) -> Option<Arc<Agentreplay>> {
        self.db.clone()
    }

    /// Get MCP project info for API responses
    pub fn get_project_info(&self) -> MCPProjectInfo {
        let metadata = self
            .project_registry
            .get_metadata(self.project_id)
            .unwrap_or_else(|| ProjectMetadata {
                project_id: self.project_id,
                name: self.project_name.clone(),
                description: Some("MCP Memory Project".to_string()),
                created_at: 0,
                last_updated: 0,
                favorite: false,
            });

        // Get storage path
        let storage_path = format!("project_{}", self.project_id);

        // Get vector count from database
        let vector_count = self
            .db
            .as_ref()
            .map(|db| db.stats().vector_count)
            .unwrap_or(0);

        MCPProjectInfo {
            project_id: self.project_id,
            project_name: metadata.name,
            tenant_id: self.tenant_id,
            description: metadata
                .description
                .unwrap_or_else(|| "MCP Memory Project".to_string()),
            created_at: metadata.created_at,
            vector_count,
            collection_count: 1, // Default collection
            last_activity: Some(metadata.last_updated),
            storage_path,
        }
    }

    /// List collections in the MCP project
    pub fn list_collections(&self) -> Vec<MCPCollection> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Get stats from database
        let (doc_count, vec_count) = self
            .db
            .as_ref()
            .map(|db| {
                let s = db.stats();
                (s.causal_nodes, s.vector_count)
            })
            .unwrap_or((0, 0));

        // Return default collection
        vec![MCPCollection {
            name: "default".to_string(),
            document_count: doc_count,
            vector_count: vec_count,
            embedding_dimension: 384, // Default for all-MiniLM-L6-v2
            created_at: self
                .project_registry
                .get_metadata(self.project_id)
                .map(|m| m.created_at)
                .unwrap_or(now),
            last_updated: now,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_constants() {
        assert_eq!(MCP_TENANT_ID, 2);
        assert_eq!(MCP_DEFAULT_PROJECT_ID, 1000);
        assert_eq!(MCP_DEFAULT_PROJECT_NAME, "MCP Memory");
    }
}
