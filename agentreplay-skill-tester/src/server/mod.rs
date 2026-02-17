// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Embedded web UI server and REST API for the Skill Tester

pub mod web_ui;
pub mod api;

pub use web_ui::SkillTesterServer;
