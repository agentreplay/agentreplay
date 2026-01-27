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

//! MCP Tool Adapter
//!
//! Bridges the unified tool registry with MCP protocol tools.
//! Converts between MCP's JSON-RPC format and the unified tool system.

use crate::mcp::protocol::{CallToolResult, Tool, ToolContent};
use crate::mcp::tools::get_tool_definitions;
use crate::tool_registry::{ToolRegistry, ToolRegistryError};
use async_trait::async_trait;
use flowtrace_core::{
    MCPTransport, ToolExecutionError, ToolKind, ToolVersion, UnifiedToolDefinition,
};
use std::sync::Arc;

/// MCP Tool Adapter
///
/// Provides bidirectional conversion between MCP tools and unified tool definitions
pub struct McpToolAdapter {
    registry: Arc<ToolRegistry>,
}

impl McpToolAdapter {
    /// Create a new adapter
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }

    /// Import existing MCP tool definitions into the unified registry
    pub fn import_mcp_tools(&self) -> Result<Vec<String>, ToolRegistryError> {
        let mcp_tools = get_tool_definitions();
        let mut imported = Vec::new();

        for mcp_tool in mcp_tools {
            let unified = self.mcp_to_unified(&mcp_tool, "mcp", ToolVersion::new(1, 0, 0));
            let registration = self.registry.register(unified)?;
            imported.push(registration.tool_id);
        }

        Ok(imported)
    }

    /// Convert MCP Tool to UnifiedToolDefinition
    pub fn mcp_to_unified(
        &self,
        mcp_tool: &Tool,
        namespace: &str,
        version: ToolVersion,
    ) -> UnifiedToolDefinition {
        let mut tool = UnifiedToolDefinition::new(
            namespace,
            &mcp_tool.name,
            version,
            ToolKind::MCP {
                server_uri: "local".to_string(),
                transport: MCPTransport::Stdio,
            },
            mcp_tool.input_schema.clone(),
        );

        tool.metadata.description = mcp_tool.description.clone().unwrap_or_default();

        tool
    }

    /// Convert UnifiedToolDefinition to MCP Tool format
    pub fn unified_to_mcp(&self, tool: &UnifiedToolDefinition) -> Tool {
        Tool {
            name: tool.name.clone(),
            description: Some(tool.metadata.description.clone()),
            input_schema: tool.input_schema.clone(),
        }
    }

    /// Get all MCP-compatible tools from the registry
    pub fn get_mcp_tools(&self) -> Vec<Tool> {
        self.registry
            .list_latest()
            .iter()
            .map(|tool| self.unified_to_mcp(tool))
            .collect()
    }

    /// Execute a tool via MCP format
    pub async fn execute_mcp_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, ToolExecutionError> {
        // This is a placeholder - actual execution would go through the executor
        // For now, we just validate the tool exists
        let _lookup = self
            .registry
            .lookup(Some("mcp"), tool_name, None)
            .map_err(|e| ToolExecutionError::not_found(&e.to_string()))?;

        // Return placeholder - real implementation would execute
        Ok(CallToolResult {
            content: vec![ToolContent::Text {
                text: serde_json::json!({
                    "status": "executed",
                    "tool": tool_name,
                    "arguments": arguments
                })
                .to_string(),
            }],
            is_error: None,
        })
    }

    /// Register a dynamic MCP tool from external source
    pub fn register_dynamic_mcp_tool(
        &self,
        name: &str,
        description: &str,
        input_schema: serde_json::Value,
        server_uri: &str,
        transport: MCPTransport,
    ) -> Result<String, ToolRegistryError> {
        let mut tool = UnifiedToolDefinition::new(
            "mcp",
            name,
            ToolVersion::new(1, 0, 0),
            ToolKind::MCP {
                server_uri: server_uri.to_string(),
                transport,
            },
            input_schema,
        );

        tool.metadata.description = description.to_string();

        let registration = self.registry.register(tool)?;
        Ok(registration.tool_id)
    }
}

/// Extension trait for integrating with existing MCP server
#[allow(dead_code)]
#[async_trait]
pub trait McpServerExtension {
    /// Get tool definitions from unified registry
    fn get_unified_tool_definitions(&self) -> Vec<Tool>;

    /// Handle tool call via unified registry
    async fn handle_unified_tool_call(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, ToolExecutionError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_registry::ToolRegistryConfig;

    #[test]
    fn test_import_mcp_tools() {
        let registry = Arc::new(ToolRegistry::new(ToolRegistryConfig::default()));
        let adapter = McpToolAdapter::new(registry.clone());

        let imported = adapter.import_mcp_tools().unwrap();
        assert!(!imported.is_empty());

        // Should be able to look up imported tools
        let result = registry.lookup(Some("mcp"), "search_traces", None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unified_to_mcp_conversion() {
        let registry = Arc::new(ToolRegistry::new(ToolRegistryConfig::default()));
        let adapter = McpToolAdapter::new(registry);

        let unified = UnifiedToolDefinition::new(
            "test",
            "my_tool",
            ToolVersion::new(1, 0, 0),
            ToolKind::Native {
                handler_id: "test".to_string(),
            },
            serde_json::json!({ "type": "object" }),
        );

        let mcp = adapter.unified_to_mcp(&unified);
        assert_eq!(mcp.name, "my_tool");
    }
}
