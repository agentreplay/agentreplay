// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! SKILL.md YAML frontmatter parser
//!
//! Parses skill manifests from SKILL.md files following the AgentSkills standard
//! and OpenClaw SKILL.md format. Supports YAML frontmatter with `---` delimiters.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ManifestError {
    #[error("Failed to read skill file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid YAML frontmatter: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("Missing frontmatter delimiters (---) in SKILL.md")]
    MissingFrontmatter,

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid skill structure: {0}")]
    InvalidStructure(String),
}

/// Parsed skill manifest from SKILL.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// Skill name (unique identifier)
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// Semantic version
    pub version: String,

    /// BLAKE3 content-addressable hash of SKILL.md + scripts + resources
    #[serde(default)]
    pub version_hash: String,

    /// License (Agent Skills spec optional field)
    #[serde(default)]
    pub license: Option<String>,

    /// Compatibility requirements (Agent Skills spec optional field)
    /// E.g. "Requires git, docker, jq, and access to the internet"
    #[serde(default)]
    pub compatibility: Option<String>,

    /// Pre-approved tools whitelist (Agent Skills spec experimental field)
    /// Space-delimited list, e.g. "Bash(git:*) Bash(jq:*) Read"
    #[serde(default, rename = "allowed-tools")]
    pub allowed_tools: Option<String>,

    /// Skill requirements
    #[serde(default)]
    pub requires: SkillRequires,

    /// Gating predicates — when should this skill trigger?
    #[serde(default)]
    pub gating: Vec<GatingPredicate>,

    /// Resource files referenced by the skill
    #[serde(default)]
    pub resources: Vec<String>,

    /// Progressive disclosure levels
    #[serde(default)]
    pub summary: Option<String>,

    /// Full instructions (markdown body after frontmatter)
    #[serde(skip)]
    pub instructions: String,

    /// OpenClaw-specific metadata
    #[serde(default)]
    pub metadata: Option<SkillMetadata>,

    /// Raw frontmatter for extensibility
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Skill dependency requirements
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillRequires {
    /// Required environment variables
    #[serde(default)]
    pub env: Vec<String>,

    /// Required binary executables
    #[serde(default)]
    pub bins: Vec<String>,

    /// Required MCP server dependencies
    #[serde(default)]
    pub mcp: Vec<String>,

    /// Required config keys
    #[serde(default)]
    pub config: Vec<String>,
}

/// Gating predicate — when should the skill trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatingPredicate {
    /// File pattern to match (glob)
    #[serde(default)]
    pub file_pattern: Option<String>,

    /// Context type (e.g., "pull_request", "issue", "chat")
    #[serde(default)]
    pub context: Option<String>,

    /// Custom predicate expression
    #[serde(default)]
    pub expression: Option<String>,
}

/// OpenClaw / Claude Skills metadata extensions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// OpenClaw-specific fields
    #[serde(default)]
    pub openclaw: Option<OpenClawMetadata>,

    /// Agent Skills standard compatibility
    #[serde(default)]
    pub compatibility: Option<Vec<String>>,

    /// Token budget hints
    #[serde(default)]
    pub token_budget: Option<TokenBudget>,
}

/// OpenClaw-specific metadata block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawMetadata {
    /// Primary environment variable
    #[serde(default)]
    pub primary_env: Option<String>,

    /// Required environment variables (OpenClaw-specific)
    #[serde(default)]
    pub requires: Option<SkillRequires>,
}

/// Token budget configuration for progressive disclosure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    /// Metadata scan cost in tokens (~100)
    #[serde(default = "default_metadata_tokens")]
    pub metadata_scan: u32,

    /// Full load cost in tokens (<5000)
    #[serde(default = "default_full_tokens")]
    pub full_load: u32,
}

fn default_metadata_tokens() -> u32 { 100 }
fn default_full_tokens() -> u32 { 5000 }

/// Parse a SKILL.md file into a SkillManifest
///
/// Expects YAML frontmatter between `---` delimiters followed by markdown instructions.
///
/// # Example
///
/// ```text
/// ---
/// name: my-skill
/// description: Does something useful
/// version: 1.0.0
/// requires:
///   env: [API_KEY]
///   bins: [git]
/// gating:
///   - file_pattern: "*.py"
///     context: pull_request
/// ---
///
/// # My Skill
///
/// Full instructions here...
/// ```
pub fn parse_skill_md(path: &Path) -> Result<SkillManifest, ManifestError> {
    let content = std::fs::read_to_string(path)?;
    parse_skill_md_content(&content, path)
}

/// Parse SKILL.md content from a string
pub fn parse_skill_md_content(content: &str, source_path: &Path) -> Result<SkillManifest, ManifestError> {
    let trimmed = content.trim();

    // Find YAML frontmatter between --- delimiters
    if !trimmed.starts_with("---") {
        return Err(ManifestError::MissingFrontmatter);
    }

    let after_first = &trimmed[3..];
    let end_idx = after_first.find("\n---")
        .ok_or(ManifestError::MissingFrontmatter)?;

    let yaml_str = &after_first[..end_idx].trim();
    let instructions = after_first[end_idx + 4..].trim().to_string();

    // Parse YAML frontmatter
    let mut manifest: SkillManifest = serde_yaml::from_str(yaml_str)?;
    manifest.instructions = instructions;

    // Compute BLAKE3 content hash
    let mut hasher = blake3::Hasher::new();
    hasher.update(content.as_bytes());

    // Also hash any referenced resource files
    if let Some(parent) = source_path.parent() {
        for resource in &manifest.resources {
            let resource_path = parent.join(resource);
            if resource_path.exists() {
                if let Ok(data) = std::fs::read(&resource_path) {
                    hasher.update(&data);
                }
            }
        }
    }

    manifest.version_hash = format!("b3_{}", hex::encode(&hasher.finalize().as_bytes()[..16]));

    // Validate required fields
    if manifest.name.is_empty() {
        return Err(ManifestError::MissingField("name".to_string()));
    }

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_skill_md() {
        let content = r#"---
name: test-skill
description: A test skill
version: 1.0.0
requires:
  env:
    - API_KEY
  bins:
    - git
gating:
  - file_pattern: "*.py"
    context: pull_request
---

# Test Skill

This is the full instruction body.
"#;

        let manifest = parse_skill_md_content(content, &PathBuf::from("SKILL.md")).unwrap();
        assert_eq!(manifest.name, "test-skill");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.requires.env, vec!["API_KEY"]);
        assert_eq!(manifest.requires.bins, vec!["git"]);
        assert_eq!(manifest.gating.len(), 1);
        assert!(manifest.version_hash.starts_with("b3_"));
        assert!(manifest.instructions.contains("Test Skill"));
    }
}
