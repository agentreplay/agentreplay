// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! OWASP LLM Top 10 scanner for agent skills
//!
//! Assesses skills against the OWASP Top 10 for LLM Applications:
//! - LLM01: Prompt Injection
//! - LLM02: Insecure Output Handling
//! - LLM03: Training Data Poisoning
//! - LLM04: Denial of Service
//! - LLM05: Supply Chain Vulnerabilities
//! - LLM06: Sensitive Information Disclosure
//! - LLM07: Insecure Plugin Design
//! - LLM08: Excessive Agency
//! - LLM09: Overreliance
//! - LLM10: Model Theft

use crate::manifest::SkillManifest;
use serde::{Deserialize, Serialize};

/// Risk level for an OWASP finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OwaspRisk {
    Pass,
    Low,
    Medium,
    High,
}

impl OwaspRisk {
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Pass => "🟢",
            Self::Low => "🟢",
            Self::Medium => "🟡",
            Self::High => "🔴",
        }
    }
}

/// Finding for a single OWASP category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwaspFinding {
    pub id: String,
    pub name: String,
    pub risk: OwaspRisk,
    pub description: String,
    pub detail: Option<String>,
    pub recommendation: Option<String>,
}

/// Complete OWASP scan result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwaspScanResult {
    pub skill_name: String,
    pub findings: Vec<OwaspFinding>,
    pub high_risk_count: usize,
    pub medium_risk_count: usize,
    pub safe_for_production: bool,
    pub verdict: String,
}

/// OWASP LLM Top 10 scanner
pub struct OwaspScanner;

impl OwaspScanner {
    pub fn new() -> Self {
        Self
    }

    /// Scan a skill manifest for OWASP LLM Top 10 risks
    pub fn scan(&self, manifest: &SkillManifest) -> OwaspScanResult {
        let mut findings = Vec::new();

        // LLM01: Prompt Injection
        let processes_user_content = !manifest.instructions.is_empty();
        let processes_external_data = manifest.requires.mcp.len() > 0
            || manifest.instructions.to_lowercase().contains("user input")
            || manifest.instructions.to_lowercase().contains("issue body")
            || manifest.instructions.to_lowercase().contains("api response");

        findings.push(OwaspFinding {
            id: "LLM01".to_string(),
            name: "Prompt Injection".to_string(),
            risk: if processes_external_data { OwaspRisk::High } else if processes_user_content { OwaspRisk::Medium } else { OwaspRisk::Pass },
            description: if processes_external_data {
                "Skill processes user-supplied or external content".to_string()
            } else {
                "Skill does not appear to process untrusted input".to_string()
            },
            detail: if processes_external_data {
                Some("External data sources may contain injected instructions".to_string())
            } else {
                None
            },
            recommendation: if processes_external_data {
                Some("Add explicit instructions to ignore suspicious commands in external data".to_string())
            } else {
                None
            },
        });

        // LLM02: Insecure Output Handling
        let has_output_sanitization = manifest.instructions.to_lowercase().contains("sanitiz")
            || manifest.instructions.to_lowercase().contains("escape")
            || manifest.instructions.to_lowercase().contains("validate output");

        findings.push(OwaspFinding {
            id: "LLM02".to_string(),
            name: "Insecure Output Handling".to_string(),
            risk: if has_output_sanitization { OwaspRisk::Pass } else { OwaspRisk::Medium },
            description: if has_output_sanitization {
                "Output sanitization references found in instructions".to_string()
            } else {
                "No explicit output sanitization found".to_string()
            },
            detail: None,
            recommendation: if !has_output_sanitization {
                Some("Add output validation before passing to external APIs".to_string())
            } else {
                None
            },
        });

        // LLM03: Training Data Poisoning
        let uses_external_model = manifest.instructions.to_lowercase().contains("model")
            || manifest.metadata.is_some();

        findings.push(OwaspFinding {
            id: "LLM03".to_string(),
            name: "Training Data Poisoning".to_string(),
            risk: if uses_external_model { OwaspRisk::Medium } else { OwaspRisk::Pass },
            description: if uses_external_model {
                "Skill references external model — cannot verify training data".to_string()
            } else {
                "No external model dependencies detected".to_string()
            },
            detail: None,
            recommendation: None,
        });

        // LLM04: Denial of Service
        let has_rate_limits = manifest.instructions.to_lowercase().contains("rate limit")
            || manifest.instructions.to_lowercase().contains("timeout")
            || manifest.instructions.to_lowercase().contains("max_");

        findings.push(OwaspFinding {
            id: "LLM04".to_string(),
            name: "Denial of Service".to_string(),
            risk: if has_rate_limits { OwaspRisk::Pass } else { OwaspRisk::Low },
            description: if has_rate_limits {
                "Rate limits or timeouts referenced in skill".to_string()
            } else {
                "No explicit rate limiting found; verify at platform level".to_string()
            },
            detail: None,
            recommendation: if !has_rate_limits {
                Some("Add explicit rate limits and timeouts to prevent resource exhaustion".to_string())
            } else {
                None
            },
        });

        // LLM05: Supply Chain Vulnerabilities
        let has_unverified_deps = manifest.requires.mcp.iter().any(|_| true); // All MCP deps need verification
        let has_bin_deps = !manifest.requires.bins.is_empty();

        findings.push(OwaspFinding {
            id: "LLM05".to_string(),
            name: "Supply Chain Vulnerabilities".to_string(),
            risk: if has_unverified_deps { OwaspRisk::High } else if has_bin_deps { OwaspRisk::Medium } else { OwaspRisk::Pass },
            description: if has_unverified_deps {
                format!("Depends on {} unverified MCP server(s)", manifest.requires.mcp.len())
            } else if has_bin_deps {
                format!("Depends on {} binary tool(s)", manifest.requires.bins.len())
            } else {
                "No external dependencies".to_string()
            },
            detail: if has_unverified_deps {
                Some(format!("MCP deps: {}", manifest.requires.mcp.join(", ")))
            } else {
                None
            },
            recommendation: if has_unverified_deps {
                Some("Pin MCP server versions with integrity hashes".to_string())
            } else {
                None
            },
        });

        // LLM06: Sensitive Information Disclosure
        let mentions_secrets = manifest.requires.env.iter().any(|e| {
            let lower = e.to_lowercase();
            lower.contains("token") || lower.contains("key") || lower.contains("secret") || lower.contains("password")
        });

        findings.push(OwaspFinding {
            id: "LLM06".to_string(),
            name: "Sensitive Information Disclosure".to_string(),
            risk: if mentions_secrets { OwaspRisk::Medium } else { OwaspRisk::Pass },
            description: if mentions_secrets {
                "Skill uses sensitive environment variables (tokens/keys)".to_string()
            } else {
                "No sensitive credentials detected in requirements".to_string()
            },
            detail: if mentions_secrets {
                Some(format!("Sensitive env vars: {:?}", manifest.requires.env))
            } else {
                None
            },
            recommendation: if mentions_secrets {
                Some("Ensure secrets are not included in skill output or logs".to_string())
            } else {
                None
            },
        });

        // LLM07-LLM10 (simplified assessments)
        for (id, name) in [
            ("LLM07", "Insecure Plugin Design"),
            ("LLM08", "Excessive Agency"),
            ("LLM09", "Overreliance"),
            ("LLM10", "Model Theft"),
        ] {
            findings.push(OwaspFinding {
                id: id.to_string(),
                name: name.to_string(),
                risk: OwaspRisk::Low,
                description: "Assessment requires runtime analysis".to_string(),
                detail: Some("Run full test suite with adversarial probes for dynamic analysis".to_string()),
                recommendation: None,
            });
        }

        // Summary
        let high_risk_count = findings.iter().filter(|f| f.risk == OwaspRisk::High).count();
        let medium_risk_count = findings.iter().filter(|f| f.risk == OwaspRisk::Medium).count();
        let safe_for_production = high_risk_count == 0;

        let verdict = if !safe_for_production {
            format!("🔴 NOT SAFE FOR PRODUCTION — {} high-risk finding(s) must be remediated", high_risk_count)
        } else if medium_risk_count > 0 {
            format!("🟡 CONDITIONALLY SAFE — {} medium-risk finding(s) should be addressed", medium_risk_count)
        } else {
            "🟢 SAFE FOR PRODUCTION — no significant risks detected".to_string()
        };

        OwaspScanResult {
            skill_name: manifest.name.clone(),
            findings,
            high_risk_count,
            medium_risk_count,
            safe_for_production,
            verdict,
        }
    }
}

impl Default for OwaspScanner {
    fn default() -> Self {
        Self::new()
    }
}
