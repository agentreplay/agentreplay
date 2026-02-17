// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Sensitivity detection for PII, secrets, and internal data leakage
//!
//! Integrates with AgentReplay's existing SensitivityFlags system:
//! SENSITIVITY_PII, SENSITIVITY_SECRET, SENSITIVITY_NO_EMBED, SENSITIVITY_INTERNAL

use serde::{Deserialize, Serialize};
use regex::Regex;

/// Sensitivity category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SensitivityCategory {
    Pii,
    Secret,
    Internal,
    NoEmbed,
}

/// A detected sensitivity finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitivityFinding {
    pub category: SensitivityCategory,
    pub pattern_matched: String,
    pub location: String,
    pub severity: String,
    pub snippet: Option<String>,
}

/// Sensitivity scan result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitivityScanResult {
    pub total_findings: usize,
    pub pii_count: usize,
    pub secret_count: usize,
    pub internal_count: usize,
    pub findings: Vec<SensitivityFinding>,
    pub passed: bool,
}

/// Sensitivity detector
pub struct SensitivityDetector {
    /// Secret patterns to match
    secret_patterns: Vec<(&'static str, &'static str)>,
    /// PII patterns to match
    pii_patterns: Vec<(&'static str, &'static str)>,
    /// Internal patterns to match
    internal_patterns: Vec<(&'static str, &'static str)>,
}

impl SensitivityDetector {
    pub fn new() -> Self {
        Self {
            secret_patterns: vec![
                (r"(?i)(api[_-]?key|apikey)\s*[:=]\s*\S+", "API key"),
                (r"(?i)(secret|password|passwd)\s*[:=]\s*\S+", "Secret/Password"),
                (r"(?i)(token|auth[_-]?token)\s*[:=]\s*\S+", "Auth Token"),
                (r"-----BEGIN (RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----", "Private Key"),
                (r"(?i)ssh-(rsa|ed25519|dss)\s+\S+", "SSH Key"),
                (r"ghp_[A-Za-z0-9_]{36}", "GitHub Personal Access Token"),
                (r"sk-[A-Za-z0-9]{48}", "OpenAI API Key"),
                (r"xoxb-[0-9]+-[0-9]+-[A-Za-z0-9]+", "Slack Bot Token"),
            ],
            pii_patterns: vec![
                (r"\b\d{3}-\d{2}-\d{4}\b", "SSN"),
                (r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b", "Email Address"),
                (r"\b\d{4}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b", "Credit Card Number"),
                (r"\b\(\d{3}\)\s?\d{3}[-.]?\d{4}\b", "Phone Number"),
            ],
            internal_patterns: vec![
                (r"/etc/passwd", "System passwd file"),
                (r"/etc/shadow", "System shadow file"),
                (r"~/\.ssh/", "SSH directory"),
                (r"(?i)internal[_-]?api", "Internal API reference"),
                (r"(?i)admin[_-]?panel", "Admin panel reference"),
                (r"(?i)localhost:\d+", "Localhost reference"),
                (r"10\.\d+\.\d+\.\d+", "Internal IP (10.x)"),
                (r"192\.168\.\d+\.\d+", "Internal IP (192.168.x)"),
            ],
        }
    }

    /// Scan text for sensitive information
    pub fn scan_text(&self, text: &str, location: &str) -> Vec<SensitivityFinding> {
        let mut findings = Vec::new();

        // Check secrets
        for (pattern, description) in &self.secret_patterns {
            if let Ok(re) = Regex::new(pattern) {
                for mat in re.find_iter(text) {
                    findings.push(SensitivityFinding {
                        category: SensitivityCategory::Secret,
                        pattern_matched: description.to_string(),
                        location: location.to_string(),
                        severity: "high".to_string(),
                        snippet: Some(Self::redact_snippet(mat.as_str())),
                    });
                }
            }
        }

        // Check PII
        for (pattern, description) in &self.pii_patterns {
            if let Ok(re) = Regex::new(pattern) {
                for mat in re.find_iter(text) {
                    findings.push(SensitivityFinding {
                        category: SensitivityCategory::Pii,
                        pattern_matched: description.to_string(),
                        location: location.to_string(),
                        severity: "medium".to_string(),
                        snippet: Some(Self::redact_snippet(mat.as_str())),
                    });
                }
            }
        }

        // Check internal references
        for (pattern, description) in &self.internal_patterns {
            if let Ok(re) = Regex::new(pattern) {
                for mat in re.find_iter(text) {
                    findings.push(SensitivityFinding {
                        category: SensitivityCategory::Internal,
                        pattern_matched: description.to_string(),
                        location: location.to_string(),
                        severity: "medium".to_string(),
                        snippet: Some(mat.as_str().to_string()),
                    });
                }
            }
        }

        findings
    }

    /// Redact sensitive content for safe display
    fn redact_snippet(text: &str) -> String {
        if text.len() <= 8 {
            return "***REDACTED***".to_string();
        }
        let visible = &text[..4];
        format!("{}...***REDACTED***", visible)
    }

    /// Run full scan on skill output
    pub fn scan_output(&self, output: &str) -> SensitivityScanResult {
        let findings = self.scan_text(output, "skill_output");
        let pii_count = findings.iter().filter(|f| f.category == SensitivityCategory::Pii).count();
        let secret_count = findings.iter().filter(|f| f.category == SensitivityCategory::Secret).count();
        let internal_count = findings.iter().filter(|f| f.category == SensitivityCategory::Internal).count();

        SensitivityScanResult {
            total_findings: findings.len(),
            pii_count,
            secret_count,
            internal_count,
            findings,
            passed: secret_count == 0 && pii_count == 0,
        }
    }
}

impl Default for SensitivityDetector {
    fn default() -> Self {
        Self::new()
    }
}
