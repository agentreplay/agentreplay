// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use anyhow::Result;
use clap::Parser;
use agentreplay_server::{config::ServerConfig, run_server};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file (TOML)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// HTTP listen address (overrides config file)
    #[arg(long, env = "AGENTREPLAY_HTTP_ADDR")]
    http_addr: Option<String>,

    /// Data directory path (overrides config file)
    #[arg(long, env = "AGENTREPLAY_DATA_DIR")]
    data_dir: Option<PathBuf>,

    /// Enable authentication
    #[arg(long, env = "AGENTREPLAY_AUTH_ENABLED")]
    auth_enabled: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    let mut config = ServerConfig::load(args.config)?;

    // Apply CLI overrides
    if let Some(addr) = args.http_addr {
        config.server.listen_addr = addr;
    }
    if let Some(data_dir) = args.data_dir {
        config.storage.data_dir = data_dir;
    }
    if args.auth_enabled {
        config.auth.enabled = true;
    }

    // Run server
    run_server(config).await
}
