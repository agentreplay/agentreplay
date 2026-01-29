// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
