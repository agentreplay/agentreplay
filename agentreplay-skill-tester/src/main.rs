// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Skill Tester CLI entry point
//!
//! Commands:
//!   skill-test <path>           — Run tests (opens web UI by default)
//!   skill-inspect <path>        — Parse and validate SKILL.md
//!   skill-scan <path>           — Security scan (OWASP LLM Top 10)
//!   skill-diff <v1> <v2>        — Compare two skill versions
//!   skill-test init <path>      — Generate test scaffold
//!   skill-test generate-adversarial <path> — Auto-generate attack cases

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "agentreplay-skill-tester",
    about = "AgentReplay Skill Tester — test, debug, and certify AI agent skills",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Test a skill (opens web UI by default)
    #[command(name = "skill-test")]
    SkillTest {
        /// Path to skill directory or SKILL.md
        skill_path: PathBuf,

        /// Path to test suite directory
        #[arg(long = "tests")]
        tests_dir: Option<PathBuf>,

        /// Run in headless CI mode (no web UI)
        #[arg(long)]
        ci: bool,

        /// Output format for CI mode
        #[arg(long, default_value = "json")]
        output: String,

        /// Fail on specific categories (e.g., "safety")
        #[arg(long)]
        fail_on: Option<String>,

        /// Filter tests by tag
        #[arg(long, value_delimiter = ',')]
        tags: Option<Vec<String>>,

        /// Filter tests by expression
        #[arg(long)]
        filter: Option<String>,

        /// Port for web UI
        #[arg(long, default_value = "6274")]
        port: u16,

        /// Generate test scaffold
        #[command(subcommand)]
        subcommand: Option<SkillTestSubcommand>,
    },

    /// Parse and validate SKILL.md without running tests
    #[command(name = "skill-inspect")]
    SkillInspect {
        /// Path to skill directory or SKILL.md
        skill_path: PathBuf,
    },

    /// Security scan (OWASP LLM Top 10)
    #[command(name = "skill-scan")]
    SkillScan {
        /// Path to skill directory or SKILL.md
        skill_path: PathBuf,

        /// Run OWASP LLM Top 10 assessment
        #[arg(long)]
        owasp: bool,
    },

    /// Compare two skill versions
    #[command(name = "skill-diff")]
    SkillDiff {
        /// Path to first skill version
        v1_path: PathBuf,
        /// Path to second skill version
        v2_path: PathBuf,
    },

    /// Monitor skill in production
    #[command(name = "skill-monitor")]
    SkillMonitor {
        /// Skill name to monitor
        skill_name: String,

        /// Baseline run ID for comparison
        #[arg(long)]
        baseline: Option<String>,
    },

    /// Detect distribution drift
    #[command(name = "skill-drift")]
    SkillDrift {
        /// Skill name to check
        skill_name: String,

        /// Time window (e.g., "24h", "7d")
        #[arg(long, default_value = "24h")]
        window: String,
    },
}

#[derive(Subcommand)]
enum SkillTestSubcommand {
    /// Generate test scaffold from SKILL.md
    Init {
        /// Path to skill directory
        skill_path: PathBuf,
    },
    /// Generate adversarial probes
    GenerateAdversarial {
        /// Path to skill directory
        skill_path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let _cli = Cli::parse();

    // TODO: Dispatch commands to appropriate handlers
    println!("AgentReplay Skill Tester v{}", agentreplay_skill_tester::VERSION);
    println!("Skill testing features coming soon!");

    Ok(())
}
