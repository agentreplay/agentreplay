// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Static analysis validator for skill manifests
//!
//! Checks:
//! - SKILL.md parseability
//! - Valid YAML frontmatter
//! - Progressive disclosure compliance
//! - Suspicious instruction detection
//! - MCP dependency availability
//! - Resource file size limits
//! - Token budget compliance

use super::parser::SkillManifest;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Severity of a validation finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Pass,
    Warning,
    Error,
}

/// A single validation finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationFinding {
    pub check: String,
    pub severity: ValidationSeverity,
    pub message: String,
    pub detail: Option<String>,
}

/// Result of validating a skill manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub skill_name: String,
    pub findings: Vec<ValidationFinding>,
    pub pass_count: usize,
    pub warn_count: usize,
    pub error_count: usize,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.error_count == 0
    }
}

/// Skill manifest validator with configurable checks
pub struct SkillValidator {
    /// Maximum resource file size in bytes (default: 50KB)
    pub max_resource_size: u64,

    /// Maximum total token budget for full skill load
    pub max_token_budget: u32,

    /// Suspicious patterns to check in instructions
    pub suspicious_patterns: Vec<String>,
}

impl Default for SkillValidator {
    fn default() -> Self {
        Self {
            max_resource_size: 50 * 1024, // 50KB
            max_token_budget: 5000,
            suspicious_patterns: vec![
                "ignore previous instructions".to_string(),
                "ignore all previous".to_string(),
                "system override".to_string(),
                "maintenance mode".to_string(),
                "cat /etc/passwd".to_string(),
                "cat ~/.ssh".to_string(),
                "eval(".to_string(),
                "exec(".to_string(),
                "subprocess".to_string(),
                "os.system".to_string(),
                "rm -rf".to_string(),
            ],
        }
    }
}

impl SkillValidator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run all validation checks on a parsed manifest
    pub fn validate(&self, manifest: &SkillManifest, skill_dir: Option<&Path>) -> ValidationReport {
        let mut findings = Vec::new();

        // Check 1: SKILL.md parseable (already passed if we have a manifest)
        findings.push(ValidationFinding {
            check: "SKILL.md parseable".to_string(),
            severity: ValidationSeverity::Pass,
            message: "OK".to_string(),
            detail: None,
        });

        // Check 2: Valid YAML frontmatter
        findings.push(ValidationFinding {
            check: "Frontmatter valid YAML".to_string(),
            severity: ValidationSeverity::Pass,
            message: "OK".to_string(),
            detail: None,
        });

        // Check 2b: Name format per Agent Skills spec
        // Must be 1-64 chars, lowercase alphanumeric + hyphens, no --, no leading/trailing -
        {
            let name = &manifest.name;
            let name_re_valid = !name.is_empty()
                && name.len() <= 64
                && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && !name.starts_with('-')
                && !name.ends_with('-')
                && !name.contains("--");

            if name_re_valid {
                findings.push(ValidationFinding {
                    check: "Name format".to_string(),
                    severity: ValidationSeverity::Pass,
                    message: "OK".to_string(),
                    detail: Some(format!("'{}' matches Agent Skills spec (1-64 lowercase alphanum + hyphens)", name)),
                });
            } else {
                let mut issues = Vec::new();
                if name.is_empty() {
                    issues.push("name is empty".to_string());
                }
                if name.len() > 64 {
                    issues.push(format!("name is {} chars (max 64)", name.len()));
                }
                if name.chars().any(|c| c.is_ascii_uppercase()) {
                    issues.push("contains uppercase characters".to_string());
                }
                if name.chars().any(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-') {
                    issues.push("contains invalid characters (only a-z, 0-9, - allowed)".to_string());
                }
                if name.starts_with('-') {
                    issues.push("starts with hyphen".to_string());
                }
                if name.ends_with('-') {
                    issues.push("ends with hyphen".to_string());
                }
                if name.contains("--") {
                    issues.push("contains consecutive hyphens".to_string());
                }
                findings.push(ValidationFinding {
                    check: "Name format".to_string(),
                    severity: ValidationSeverity::Error,
                    message: "FAIL".to_string(),
                    detail: Some(format!("Name '{}' violates Agent Skills spec: {}", name, issues.join("; "))),
                });
            }
        }

        // Check 2c: Description length per Agent Skills spec (1-1024 chars)
        {
            let desc_len = manifest.description.len();
            if desc_len == 0 {
                findings.push(ValidationFinding {
                    check: "Description length".to_string(),
                    severity: ValidationSeverity::Error,
                    message: "FAIL".to_string(),
                    detail: Some("Description is empty (must be 1-1024 chars per spec)".to_string()),
                });
            } else if desc_len > 1024 {
                findings.push(ValidationFinding {
                    check: "Description length".to_string(),
                    severity: ValidationSeverity::Error,
                    message: "FAIL".to_string(),
                    detail: Some(format!("Description is {} chars (max 1024 per spec)", desc_len)),
                });
            } else {
                findings.push(ValidationFinding {
                    check: "Description length".to_string(),
                    severity: ValidationSeverity::Pass,
                    message: "OK".to_string(),
                    detail: Some(format!("{} chars (within 1-1024 limit)", desc_len)),
                });
            }
        }

        // Check 3: Progressive disclosure compliance
        if manifest.summary.is_none() {
            findings.push(ValidationFinding {
                check: "Progressive disclosure".to_string(),
                severity: ValidationSeverity::Warning,
                message: "WARN".to_string(),
                detail: Some("No summary field (loads full instructions on match)".to_string()),
            });
        } else {
            findings.push(ValidationFinding {
                check: "Progressive disclosure".to_string(),
                severity: ValidationSeverity::Pass,
                message: "OK".to_string(),
                detail: None,
            });
        }

        // Check 4: Suspicious instructions
        let instructions_lower = manifest.instructions.to_lowercase();
        let mut suspicious_found = Vec::new();
        for pattern in &self.suspicious_patterns {
            if instructions_lower.contains(&pattern.to_lowercase()) {
                suspicious_found.push(pattern.clone());
            }
        }

        if suspicious_found.is_empty() {
            findings.push(ValidationFinding {
                check: "No suspicious instructions".to_string(),
                severity: ValidationSeverity::Pass,
                message: "OK".to_string(),
                detail: None,
            });
        } else {
            findings.push(ValidationFinding {
                check: "No suspicious instructions".to_string(),
                severity: ValidationSeverity::Error,
                message: "FAIL".to_string(),
                detail: Some(format!("Suspicious patterns found: {}", suspicious_found.join(", "))),
            });
        }

        // Check 5: MCP dependency availability
        for mcp_dep in &manifest.requires.mcp {
            // In a real implementation, we'd check if the MCP server is available locally
            findings.push(ValidationFinding {
                check: format!("MCP dependency: {}", mcp_dep),
                severity: ValidationSeverity::Error,
                message: "FAIL".to_string(),
                detail: Some(format!("{} not available locally — use mcp_mocks in test cases", mcp_dep)),
            });
        }

        if manifest.requires.mcp.is_empty() {
            findings.push(ValidationFinding {
                check: "MCP dependencies".to_string(),
                severity: ValidationSeverity::Pass,
                message: "OK — no MCP dependencies".to_string(),
                detail: None,
            });
        }

        // Check 6: Resource file sizes
        if let Some(dir) = skill_dir {
            let mut all_ok = true;
            for resource in &manifest.resources {
                let resource_path = dir.join(resource);
                if resource_path.exists() {
                    if let Ok(metadata) = std::fs::metadata(&resource_path) {
                        if metadata.len() > self.max_resource_size {
                            all_ok = false;
                            findings.push(ValidationFinding {
                                check: format!("Resource size: {}", resource),
                                severity: ValidationSeverity::Error,
                                message: "FAIL".to_string(),
                                detail: Some(format!(
                                    "File is {} bytes, exceeds limit of {} bytes",
                                    metadata.len(),
                                    self.max_resource_size
                                )),
                            });
                        }
                    }
                }
            }
            if all_ok {
                findings.push(ValidationFinding {
                    check: "Resource files size".to_string(),
                    severity: ValidationSeverity::Pass,
                    message: format!("OK — all resources < {}KB", self.max_resource_size / 1024),
                    detail: None,
                });
            }
        }

        // Check 7: Required fields completeness
        let mut missing_fields = Vec::new();
        if manifest.description.is_empty() {
            missing_fields.push("description");
        }
        if manifest.version.is_empty() {
            missing_fields.push("version");
        }

        if missing_fields.is_empty() {
            findings.push(ValidationFinding {
                check: "Required fields".to_string(),
                severity: ValidationSeverity::Pass,
                message: "OK".to_string(),
                detail: None,
            });
        } else {
            findings.push(ValidationFinding {
                check: "Required fields".to_string(),
                severity: ValidationSeverity::Error,
                message: "FAIL".to_string(),
                detail: Some(format!("Missing: {}", missing_fields.join(", "))),
            });
        }

        // Check 8: Binary dependencies
        for bin in &manifest.requires.bins {
            let available = which::which(bin).is_ok();
            findings.push(ValidationFinding {
                check: format!("Binary: {}", bin),
                severity: if available { ValidationSeverity::Pass } else { ValidationSeverity::Warning },
                message: if available { "OK".to_string() } else { "NOT FOUND".to_string() },
                detail: if !available {
                    Some(format!("{} not found in PATH — tests requiring it will be skipped", bin))
                } else {
                    None
                },
            });
        }

        // Compute summary
        let pass_count = findings.iter().filter(|f| f.severity == ValidationSeverity::Pass).count();
        let warn_count = findings.iter().filter(|f| f.severity == ValidationSeverity::Warning).count();
        let error_count = findings.iter().filter(|f| f.severity == ValidationSeverity::Error).count();

        ValidationReport {
            skill_name: manifest.name.clone(),
            findings,
            pass_count,
            warn_count,
            error_count,
        }
    }
}
