// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Skill manifest parsing, validation, and registry.

pub mod parser;
pub mod registry;
pub mod validator;

pub use parser::{SkillManifest, ManifestError, parse_skill_md};
pub use registry::SkillRegistry;
pub use validator::SkillValidator;
