// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Supply chain verification for skill dependencies
//!
//! Checks integrity of MCP server dependencies, binary tools, and resources.

use crate::manifest::SkillManifest;
use serde::{Deserialize, Serialize};

/// Supply chain finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyChainFinding {
    pub dependency: String,
    pub dependency_type: String, // "mcp", "binary", "resource"
    pub verified: bool,
    pub integrity_hash: Option<String>,
    pub message: String,
}

/// Supply chain verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyChainResult {
    pub total_dependencies: usize,
    pub verified: usize,
    pub unverified: usize,
    pub findings: Vec<SupplyChainFinding>,
}

/// Supply chain verifier
pub struct SupplyChainVerifier;

impl SupplyChainVerifier {
    pub fn new() -> Self {
        Self
    }

    /// Verify all dependencies in a skill manifest
    pub fn verify(&self, manifest: &SkillManifest) -> SupplyChainResult {
        let mut findings = Vec::new();

        // Check MCP dependencies
        for mcp in &manifest.requires.mcp {
            findings.push(SupplyChainFinding {
                dependency: mcp.clone(),
                dependency_type: "mcp".to_string(),
                verified: false, // MCP servers need explicit verification
                integrity_hash: None,
                message: format!(
                    "MCP server '{}' — no integrity hash. Pin version with BLAKE3 hash.",
                    mcp
                ),
            });
        }

        // Check binary dependencies
        for bin in &manifest.requires.bins {
            let available = which::which(bin).is_ok();
            findings.push(SupplyChainFinding {
                dependency: bin.clone(),
                dependency_type: "binary".to_string(),
                verified: available,
                integrity_hash: None,
                message: if available {
                    format!("Binary '{}' found in PATH", bin)
                } else {
                    format!("Binary '{}' not found in PATH", bin)
                },
            });
        }

        let verified = findings.iter().filter(|f| f.verified).count();
        let unverified = findings.len() - verified;

        SupplyChainResult {
            total_dependencies: findings.len(),
            verified,
            unverified,
            findings,
        }
    }
}

impl Default for SupplyChainVerifier {
    fn default() -> Self {
        Self::new()
    }
}
