// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! AgentReplay Skill Tester
//!
//! A standalone developer tool for testing, debugging, and certifying AI agent skills.
//! Inspired by MCP Inspector, built for the AgentSkills era.
//!
//! # Architecture
//!
//! ```text
//! agentreplay-skill-tester/
//! ├── manifest/    — SKILL.md parsing, registry, validation
//! ├── runner/      — Test case execution, sandbox, assertions
//! ├── evaluators/  — Skill-specific evaluators (selection, contract, adversarial)
//! ├── security/    — OWASP scanning, supply chain, sensitivity
//! ├── viz/         — Sankey flow, confusion matrix, calibration data export
//! └── server/      — Embedded web UI + local REST API
//! ```

pub mod manifest;
pub mod runner;
pub mod evaluators;
pub mod security;
pub mod viz;
pub mod server;

// Re-exports for convenience
pub use manifest::{SkillManifest, SkillRegistry, SkillValidator, ManifestError};
pub use runner::{Assertion, TestCase, TestRunner, TestResult, TestSuite, AssertionEngine};
pub use evaluators::{
    SkillSelectionEvaluator, ContractTestEvaluator, AdversarialEvaluator,
    ToolPolicyEvaluator, DistributionShiftEvaluator,
};
pub use security::{OwaspScanner, SupplyChainVerifier, SensitivityDetector};
pub use viz::{SankeyExporter, ConfusionMatrixRenderer, CalibrationExporter};
pub use server::SkillTesterServer;

/// Skill Tester version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
