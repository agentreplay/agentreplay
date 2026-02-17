// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Mock MCP server for testing skills without real MCP dependencies
//!
//! Provides a lightweight mock layer that intercepts MCP tool calls
//! and returns pre-configured responses from YAML test cases.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::runner::scenario::MockResponse;

/// Mock MCP server that serves pre-configured responses
pub struct MockMcpServer {
    /// Server name (e.g., "sentry-mcp-server")
    pub name: String,

    /// Method → response mapping
    methods: HashMap<String, MockResponse>,

    /// Call log for assertions
    call_log: Vec<MockMcpCall>,
}

/// Record of a mock MCP call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockMcpCall {
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub response: serde_json::Value,
    pub timestamp_ms: u64,
}

impl MockMcpServer {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            methods: HashMap::new(),
            call_log: Vec::new(),
        }
    }

    /// Configure a mock response for a method
    pub fn mock_method(&mut self, method: &str, response: MockResponse) {
        self.methods.insert(method.to_string(), response);
    }

    /// Load mock configurations from setup
    pub fn from_mocks(name: &str, mocks: HashMap<String, MockResponse>) -> Self {
        Self {
            name: name.to_string(),
            methods: mocks,
            call_log: Vec::new(),
        }
    }

    /// Handle a mock call — returns the configured response or error
    pub async fn handle_call(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let mock = self.methods.get(method)
            .ok_or_else(|| format!("No mock configured for {}.{}", self.name, method))?;

        // Simulate latency if configured
        if let Some(delay_ms) = mock.delay_ms {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
        }

        // Return error if configured
        if let Some(error) = &mock.error {
            return Err(error.clone());
        }

        let response = mock.response.clone();

        // Log the call
        self.call_log.push(MockMcpCall {
            method: method.to_string(),
            params,
            response: response.clone(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        });

        Ok(response)
    }

    /// Get call log
    pub fn call_log(&self) -> &[MockMcpCall] {
        &self.call_log
    }

    /// Check if a method was called
    pub fn was_called(&self, method: &str) -> bool {
        self.call_log.iter().any(|c| c.method == method)
    }

    /// Count calls to a method
    pub fn call_count(&self, method: &str) -> usize {
        self.call_log.iter().filter(|c| c.method == method).count()
    }
}

/// Manager for multiple mock MCP servers
pub struct MockMcpManager {
    servers: HashMap<String, MockMcpServer>,
}

impl MockMcpManager {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    /// Register a mock server
    pub fn add_server(&mut self, server: MockMcpServer) {
        self.servers.insert(server.name.clone(), server);
    }

    /// Load all mocks from test setup
    pub fn from_test_setup(mocks: &HashMap<String, HashMap<String, MockResponse>>) -> Self {
        let mut manager = Self::new();
        for (server_name, methods) in mocks {
            let server = MockMcpServer::from_mocks(server_name, methods.clone());
            manager.add_server(server);
        }
        manager
    }

    /// Route a call to the appropriate mock server
    pub async fn handle_call(
        &mut self,
        server: &str,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let mock_server = self.servers.get_mut(server)
            .ok_or_else(|| format!("No mock server configured for '{}'", server))?;
        mock_server.handle_call(method, params).await
    }

    /// Get a server by name
    pub fn get_server(&self, name: &str) -> Option<&MockMcpServer> {
        self.servers.get(name)
    }
}

impl Default for MockMcpManager {
    fn default() -> Self {
        Self::new()
    }
}
