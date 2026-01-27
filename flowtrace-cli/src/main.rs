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

//! Flowtrace CLI
//!
//! Command-line interface for Flowtrace database operations.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use flowtrace_core::{AgentFlowEdge, SpanType};
use flowtrace_plugins::{PluginConfig, PluginManager, UninstallMode};
use flowtrace_query::Flowtrace;
use std::path::PathBuf;
use std::process::Command;
use tracing::{info, Level};

#[derive(Parser)]
#[command(name = "flowtrace")]
#[command(about = "Flowtrace - AgentFlow Format Database", long_about = None)]
struct Cli {
    /// Database directory
    #[arg(short, long, default_value = "./flowtrace-data")]
    db_path: PathBuf,

    /// Verbose mode
    #[arg(short, long)]
    verbose: bool,

    /// Output as JSON (machine-readable)
    #[arg(long)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new database
    Init,

    /// Insert a test edge
    Insert {
        /// Tenant ID
        #[arg(long, default_value = "1")]
        tenant_id: u64,

        /// Project ID
        #[arg(long, default_value = "0")]
        project_id: u16,

        /// Agent ID
        #[arg(long)]
        agent_id: u64,

        /// Session ID
        #[arg(long)]
        session_id: u64,

        /// Span type
        #[arg(long, default_value = "root")]
        span_type: String,

        /// Parent edge ID (hex)
        #[arg(long, default_value = "0")]
        parent: String,
    },

    /// Get an edge by ID
    Get {
        /// Edge ID (hex)
        edge_id: String,

        /// Filter by tenant ID (optional tenant isolation)
        #[arg(long)]
        tenant: Option<u64>,
    },

    /// Query edges in time range
    Query {
        /// Start timestamp (nanoseconds)
        #[arg(long)]
        start: u64,

        /// End timestamp (nanoseconds)
        #[arg(long)]
        end: u64,

        /// Filter by agent ID
        #[arg(long)]
        agent: Option<u64>,

        /// Filter by session ID
        #[arg(long)]
        session: Option<u64>,

        /// Filter by tenant ID (optional tenant isolation)
        #[arg(long)]
        tenant: Option<u64>,

        /// Filter by project ID (optional tenant isolation)
        #[arg(long)]
        project: Option<u16>,
    },

    /// Get children of an edge
    Children {
        /// Edge ID (hex)
        edge_id: String,

        /// Filter by tenant ID (optional tenant isolation)
        #[arg(long)]
        tenant: Option<u64>,
    },

    /// Get ancestors of an edge
    Ancestors {
        /// Edge ID (hex)
        edge_id: String,

        /// Filter by tenant ID (optional tenant isolation)
        #[arg(long)]
        tenant: Option<u64>,
    },

    /// Get database statistics
    Stats,

    /// Load test data
    LoadTest {
        /// Number of edges to generate
        #[arg(default_value = "1000")]
        count: usize,

        /// Number of agents
        #[arg(long, default_value = "10")]
        agents: u64,
    },

    /// Benchmark write performance
    Benchmark {
        /// Number of writes
        #[arg(default_value = "10000")]
        writes: usize,
    },

    /// Backup and restore commands
    Backup {
        #[command(subcommand)]
        command: BackupCommands,
    },

    /// Plugin management commands
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },
}

#[derive(Subcommand)]
enum PluginCommands {
    /// List all installed plugins
    List,

    /// Install a plugin from a directory
    Install {
        /// Path to the plugin directory
        path: PathBuf,

        /// Install as development plugin (symlink for hot reload)
        #[arg(long)]
        dev: bool,
    },

    /// Uninstall a plugin
    Uninstall {
        /// Plugin ID to uninstall
        plugin_id: String,

        /// Force uninstall even if other plugins depend on it
        #[arg(long)]
        force: bool,

        /// Preserve plugin data for reinstallation
        #[arg(long)]
        preserve_data: bool,
    },

    /// Enable a plugin
    Enable {
        /// Plugin ID to enable
        plugin_id: String,
    },

    /// Disable a plugin
    Disable {
        /// Plugin ID to disable
        plugin_id: String,
    },

    /// Show details about a plugin
    Info {
        /// Plugin ID to show details for
        plugin_id: String,
    },

    /// Scan for new plugins
    Scan,

    /// Get the plugins directory path
    Dir,

    /// Search for plugins
    Search {
        /// Search query
        query: String,
    },

    /// Reload a development plugin
    Reload {
        /// Plugin ID to reload
        plugin_id: String,
    },

    /// Initialize a new plugin from a template
    Init {
        /// Plugin name (will be used as directory name)
        name: String,

        /// Template to use: rust-evaluator, python-evaluator, typescript-evaluator,
        /// rust-embedding, python-embedding, rust-exporter, python-exporter
        #[arg(short, long, default_value = "rust-evaluator")]
        template: String,

        /// Output directory (defaults to current directory)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Plugin description
        #[arg(short, long)]
        description: Option<String>,

        /// Author name
        #[arg(short, long)]
        author: Option<String>,
    },

    /// Build a plugin to WASM
    Build {
        /// Path to the plugin directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Build in release mode
        #[arg(long)]
        release: bool,

        /// Target platform (wasm32-wasip1, wasm32-wasip2)
        #[arg(long, default_value = "wasm32-wasip1")]
        target: String,
    },

    /// Validate a plugin manifest and structure
    Validate {
        /// Path to the plugin directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Package a plugin for distribution
    Package {
        /// Path to the plugin directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output path for the package
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run a plugin for testing
    Run {
        /// Path to the plugin directory or WASM file
        path: PathBuf,

        /// Input to pass to the plugin (JSON)
        #[arg(short, long)]
        input: Option<String>,

        /// Function to call (evaluate, embed, export)
        #[arg(short, long, default_value = "evaluate")]
        function: String,
    },
}

#[derive(Subcommand, Clone)]
enum BackupCommands {
    /// Create a new backup of the database
    Create {
        /// Optional backup name/description
        #[arg(short, long)]
        name: Option<String>,
    },

    /// List all available backups
    List,

    /// Restore from a backup (full replace)
    Restore {
        /// Backup ID to restore from
        backup_id: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },

    /// Delete a backup
    Delete {
        /// Backup ID to delete
        backup_id: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },

    /// Export backup as a ZIP file
    Export {
        /// Backup ID to export
        backup_id: String,

        /// Output path for the ZIP file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Import backup from a ZIP file
    Import {
        /// Path to the ZIP file to import
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let level = if cli.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };
    tracing_subscriber::fmt().with_max_level(level).init();

    // Handle plugin commands separately (don't need database)
    if let Commands::Plugin { command } = &cli.command {
        return handle_plugin_command(command, &cli.db_path, cli.json).await;
    }

    // Handle backup commands separately (don't need database open - just file operations)
    if let Commands::Backup { command } = &cli.command {
        return handle_backup_command(command.clone(), &cli.db_path, cli.json).await;
    }

    // Open database
    let db = Flowtrace::open(&cli.db_path).context("Failed to open database")?;

    match cli.command {
        Commands::Plugin { .. } => unreachable!(), // Handled above
        Commands::Init => {
            info!("Initialized database at {:?}", cli.db_path);
            println!("✓ Database initialized at {:?}", cli.db_path);
        }

        Commands::Insert {
            tenant_id,
            project_id,
            agent_id,
            session_id,
            span_type,
            parent,
        } => {
            let span = parse_span_type(&span_type)?;
            let parent_id = parse_hex_u128(&parent)?;

            let edge =
                AgentFlowEdge::new(tenant_id, project_id, agent_id, session_id, span, parent_id);
            db.insert(edge).await?;

            println!("✓ Inserted edge {:#x}", edge.edge_id);
            println!(
                "  Tenant: {}, Project: {}, Agent: {}, Session: {}, Type: {:?}",
                tenant_id, project_id, agent_id, session_id, span
            );
        }

        Commands::Get { edge_id, tenant } => {
            let id = parse_hex_u128(&edge_id)?;

            let edge_opt = if let Some(tenant_id) = tenant {
                db.get_for_tenant(id, tenant_id)?
            } else {
                db.get(id)?
            };

            match edge_opt {
                Some(edge) => {
                    println!("Edge {:#x}:", edge.edge_id);
                    println!("  Tenant: {}, Project: {}", edge.tenant_id, edge.project_id);
                    println!("  Timestamp: {} μs", edge.timestamp_us);
                    println!("  Agent: {}", edge.agent_id);
                    println!("  Session: {}", edge.session_id);
                    println!("  Type: {:?}", edge.get_span_type());
                    println!("  Parent: {:#x}", edge.causal_parent);
                    println!("  Confidence: {:.2}", edge.confidence);
                    println!("  Tokens: {}", edge.token_count);
                }
                None => {
                    if tenant.is_some() {
                        println!("✗ Edge not found or does not belong to tenant");
                    } else {
                        println!("✗ Edge not found");
                    }
                }
            }
        }

        Commands::Query {
            start,
            end,
            agent,
            session,
            tenant,
            project,
        } => {
            let mut results = if let Some(tenant_id) = tenant {
                // Use tenant-aware query
                db.query_temporal_range_for_tenant(start, end, tenant_id)?
            } else {
                // Use standard query
                db.query_temporal_range(start, end)?
            };

            // Apply additional filters
            if let Some(agent_id) = agent {
                results.retain(|e| e.agent_id == agent_id);
            }

            if let Some(session_id) = session {
                results.retain(|e| e.session_id == session_id);
            }

            if let Some(project_id) = project {
                results.retain(|e| e.project_id == project_id);
            }

            println!("Found {} edges:", results.len());
            for (i, edge) in results.iter().enumerate().take(20) {
                println!(
                    "  {}. {:#x} - Tenant:{} Project:{} Agent:{} Session:{} Type:{:?}",
                    i + 1,
                    edge.edge_id,
                    edge.tenant_id,
                    edge.project_id,
                    edge.agent_id,
                    edge.session_id,
                    edge.get_span_type()
                );
            }

            if results.len() > 20 {
                println!("  ... and {} more", results.len() - 20);
            }
        }

        Commands::Children { edge_id, tenant } => {
            let id = parse_hex_u128(&edge_id)?;

            let children = if let Some(tenant_id) = tenant {
                db.get_children_for_tenant(id, tenant_id)?
            } else {
                db.get_children(id)?
            };

            println!("Children of {:#x}: {}", id, children.len());
            for child in children {
                println!("  {:#x} - Type:{:?}", child.edge_id, child.get_span_type());
            }
        }

        Commands::Ancestors { edge_id, tenant } => {
            let id = parse_hex_u128(&edge_id)?;

            let ancestors = if let Some(tenant_id) = tenant {
                db.get_ancestors_for_tenant(id, tenant_id)?
            } else {
                db.get_ancestors(id)?
            };

            println!("Ancestors of {:#x}: {}", id, ancestors.len());
            for ancestor in ancestors {
                println!(
                    "  {:#x} - Type:{:?}",
                    ancestor.edge_id,
                    ancestor.get_span_type()
                );
            }
        }

        Commands::Stats => {
            let stats = db.stats();

            println!("Flowtrace Statistics");
            println!("=====================");
            println!();
            println!("Memory:");
            println!(
                "  Memtable: {} entries ({} bytes)",
                stats.storage.memtable_entries, stats.storage.memtable_size
            );
            println!(
                "  Immutable memtables: {}",
                stats.storage.immutable_memtables
            );
            println!(
                "  Cache: {}/{}",
                stats.storage.cache_stats.size, stats.storage.cache_stats.capacity
            );
            println!();
            println!("Storage:");
            println!("  WAL sequence: {}", stats.storage.wal_sequence);

            for level in &stats.storage.levels {
                if level.num_sstables > 0 {
                    println!(
                        "  L{}: {} SSTables, {} entries, {} bytes",
                        level.level, level.num_sstables, level.total_entries, level.total_size
                    );
                }
            }

            println!();
            println!("Indexes:");
            println!(
                "  Causal graph: {} nodes, {} edges",
                stats.causal_nodes, stats.causal_edges
            );
            println!("  Vector index: {} embeddings", stats.vector_count);
        }

        Commands::LoadTest { count, agents } => {
            info!("Loading {} test edges with {} agents", count, agents);

            let start = std::time::Instant::now();

            for i in 0..count {
                let agent_id = (i as u64) % agents;
                let session_id = (i as u64) / 100;
                let span_type = match i % 4 {
                    0 => SpanType::Root,
                    1 => SpanType::Planning,
                    2 => SpanType::ToolCall,
                    _ => SpanType::Response,
                };

                let mut edge = AgentFlowEdge::new(1, 0, agent_id, session_id, span_type, 0);
                edge.token_count = (i % 500) as u32;
                edge.confidence = 0.5 + (i % 50) as f32 / 100.0;
                edge.checksum = edge.compute_checksum();

                db.insert(edge).await?;

                if (i + 1) % 1000 == 0 {
                    println!("  Inserted {} edges...", i + 1);
                }
            }

            let duration = start.elapsed();
            let throughput = count as f64 / duration.as_secs_f64();

            println!("✓ Loaded {} edges in {:.2}s", count, duration.as_secs_f64());
            println!("  Throughput: {:.0} edges/sec", throughput);

            db.sync()?;
        }

        Commands::Benchmark { writes } => {
            info!("Benchmarking {} writes", writes);

            let mut latencies = Vec::new();
            let start = std::time::Instant::now();

            for i in 0..writes {
                let edge = AgentFlowEdge::new(1, 0, i as u64, i as u64, SpanType::Root, 0);

                let write_start = std::time::Instant::now();
                db.insert(edge).await?;
                latencies.push(write_start.elapsed());
            }

            let total_duration = start.elapsed();

            // Calculate statistics
            latencies.sort();
            let p50 = latencies[writes / 2];
            let p99 = latencies[writes * 99 / 100];
            let p999 = latencies[writes * 999 / 1000];

            let throughput = writes as f64 / total_duration.as_secs_f64();

            println!("Benchmark Results ({} writes)", writes);
            println!("================================");
            println!("Throughput: {:.0} writes/sec", throughput);
            println!("Latency:");
            println!("  P50:  {:?}", p50);
            println!("  P99:  {:?}", p99);
            println!("  P999: {:?}", p999);
            println!("Total time: {:.2}s", total_duration.as_secs_f64());
        }

        Commands::Backup { .. } => unreachable!(), // Handled above
    }

    Ok(())
}

fn parse_span_type(s: &str) -> Result<SpanType> {
    Ok(match s.to_lowercase().as_str() {
        "root" => SpanType::Root,
        "planning" => SpanType::Planning,
        "reasoning" => SpanType::Reasoning,
        "toolcall" => SpanType::ToolCall,
        "toolresponse" => SpanType::ToolResponse,
        "synthesis" => SpanType::Synthesis,
        "response" => SpanType::Response,
        "error" => SpanType::Error,
        _ => anyhow::bail!("Invalid span type: {}", s),
    })
}

fn parse_hex_u128(s: &str) -> Result<u128> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    u128::from_str_radix(s, 16).context("Invalid hex number")
}

/// Handle plugin management commands
async fn handle_plugin_command(
    command: &PluginCommands,
    data_dir: &PathBuf,
    json_output: bool,
) -> Result<()> {
    let config = PluginConfig {
        data_dir: data_dir.clone(),
        ..Default::default()
    };

    let manager = PluginManager::new(config)
        .await
        .context("Failed to initialize plugin manager")?;

    match command {
        PluginCommands::List => {
            let plugins = manager.list_plugins();

            if json_output {
                println!("{}", serde_json::to_string_pretty(&plugins)?);
            } else {
                if plugins.is_empty() {
                    println!("No plugins installed.");
                    println!("\nPlugins directory: {}", manager.plugins_dir().display());
                } else {
                    println!("Installed Plugins ({}):", plugins.len());
                    println!("{:-<60}", "");
                    for plugin in &plugins {
                        let status = if plugin.enabled { "✓" } else { "○" };
                        println!("{} {} v{}", status, plugin.name, plugin.version);
                        println!("    ID: {}", plugin.id);
                        println!("    Type: {:?}", plugin.plugin_type);
                        if !plugin.description.is_empty() {
                            println!("    Description: {}", plugin.description);
                        }
                        println!();
                    }
                }
            }
        }

        PluginCommands::Install { path, dev } => {
            println!("Installing plugin from {}...", path.display());

            let result = if *dev {
                manager.install_dev(path).await
            } else {
                manager.install_from_directory(path).await
            };

            match result {
                Ok(result) => {
                    if json_output {
                        println!("{}", serde_json::to_string_pretty(&result)?);
                    } else {
                        println!("✓ Installed {} v{}", result.plugin_id, result.version);
                        println!("  Location: {}", result.install_path.display());
                        if *dev {
                            println!("  Mode: Development (hot reload enabled)");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("✗ Installation failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        PluginCommands::Uninstall {
            plugin_id,
            force,
            preserve_data,
        } => {
            println!("Uninstalling plugin {}...", plugin_id);

            let mode = if *force {
                UninstallMode::Force
            } else {
                UninstallMode::Safe
            };

            match manager.uninstall(plugin_id, mode, *preserve_data).await {
                Ok(result) => {
                    if json_output {
                        println!("{}", serde_json::to_string_pretty(&result)?);
                    } else {
                        println!("✓ Uninstalled {}", result.plugin_id);
                        println!("  Removed {} files", result.removed_files);
                        if result.data_preserved {
                            println!("  Data preserved for reinstallation");
                        }
                        if !result.broken_dependents.is_empty() {
                            println!("  Warning: The following plugins may be broken:");
                            for dep in &result.broken_dependents {
                                println!("    - {}", dep);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("✗ Uninstallation failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        PluginCommands::Enable { plugin_id } => match manager.enable(plugin_id).await {
            Ok(()) => println!("✓ Enabled {}", plugin_id),
            Err(e) => {
                eprintln!("✗ Failed to enable: {}", e);
                std::process::exit(1);
            }
        },

        PluginCommands::Disable { plugin_id } => match manager.disable(plugin_id).await {
            Ok(()) => println!("✓ Disabled {}", plugin_id),
            Err(e) => {
                eprintln!("✗ Failed to disable: {}", e);
                std::process::exit(1);
            }
        },

        PluginCommands::Info { plugin_id } => match manager.get_plugin(plugin_id) {
            Some(plugin) => {
                if json_output {
                    println!("{}", serde_json::to_string_pretty(&plugin)?);
                } else {
                    println!("Plugin: {}", plugin.name);
                    println!("{:-<40}", "");
                    println!("ID:          {}", plugin.id);
                    println!("Version:     {}", plugin.version);
                    println!("Type:        {:?}", plugin.plugin_type);
                    println!("Enabled:     {}", if plugin.enabled { "Yes" } else { "No" });
                    println!("State:       {:?}", plugin.state);
                    println!("Source:      {}", plugin.source);
                    println!("Path:        {}", plugin.install_path.display());
                    println!("Installed:   {}", plugin.installed_at);
                    if !plugin.description.is_empty() {
                        println!("Description: {}", plugin.description);
                    }
                    if !plugin.authors.is_empty() {
                        println!("Authors:     {}", plugin.authors.join(", "));
                    }
                    if !plugin.tags.is_empty() {
                        println!("Tags:        {}", plugin.tags.join(", "));
                    }
                    if !plugin.capabilities.is_empty() {
                        println!("Capabilities:");
                        for cap in &plugin.capabilities {
                            println!("  - {}", cap);
                        }
                    }
                }
            }
            None => {
                eprintln!("Plugin '{}' not found", plugin_id);
                std::process::exit(1);
            }
        },

        PluginCommands::Scan => {
            println!("Scanning for plugins...");
            match manager.scan_plugins().await {
                Ok(plugins) => {
                    if json_output {
                        println!("{}", serde_json::to_string_pretty(&plugins)?);
                    } else {
                        println!("Found {} plugins", plugins.len());
                        for plugin in &plugins {
                            println!("  - {} v{}", plugin.name, plugin.version);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("✗ Scan failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        PluginCommands::Dir => {
            let dir = manager.plugins_dir();
            if json_output {
                println!("{}", serde_json::json!({"path": dir.to_string_lossy()}));
            } else {
                println!("{}", dir.display());
            }
        }

        PluginCommands::Search { query } => {
            let results = manager.search(query);

            if json_output {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                if results.is_empty() {
                    println!("No plugins found matching '{}'", query);
                } else {
                    println!("Found {} plugins matching '{}':", results.len(), query);
                    for plugin in &results {
                        println!(
                            "  {} v{} - {}",
                            plugin.name, plugin.version, plugin.description
                        );
                    }
                }
            }
        }

        PluginCommands::Reload { plugin_id } => match manager.reload(plugin_id).await {
            Ok(()) => println!("✓ Reloaded {}", plugin_id),
            Err(e) => {
                eprintln!("✗ Failed to reload: {}", e);
                std::process::exit(1);
            }
        },

        PluginCommands::Init {
            name,
            template,
            output,
            description,
            author,
        } => {
            let output_dir = output.clone().unwrap_or_else(|| PathBuf::from("."));
            let plugin_dir = output_dir.join(name);

            println!(
                "Creating new plugin '{}' from template '{}'...",
                name, template
            );

            // Create plugin directory
            std::fs::create_dir_all(&plugin_dir).context("Failed to create plugin directory")?;

            // Generate manifest
            let manifest_content =
                generate_plugin_manifest(name, template, description.as_deref(), author.as_deref());

            let manifest_path = plugin_dir.join("flowtrace-plugin.toml");
            std::fs::write(&manifest_path, manifest_content).context("Failed to write manifest")?;

            // Generate source files based on template
            generate_template_files(&plugin_dir, name, template)
                .context("Failed to generate template files")?;

            if json_output {
                println!(
                    "{}",
                    serde_json::json!({
                        "name": name,
                        "template": template,
                        "path": plugin_dir.display().to_string()
                    })
                );
            } else {
                println!("✓ Created plugin at {}", plugin_dir.display());
                println!("\nNext steps:");
                println!("  1. cd {}", plugin_dir.display());
                match template.as_str() {
                    t if t.starts_with("rust") => {
                        println!("  2. cargo build --target wasm32-wasip1");
                    }
                    t if t.starts_with("python") => {
                        println!("  2. pip install componentize-py");
                        println!("  3. componentize-py componentize -o plugin.wasm evaluator.py");
                    }
                    t if t.starts_with("typescript") => {
                        println!("  2. npm install");
                        println!("  3. npm run build");
                    }
                    _ => {}
                }
                println!("  4. flowtrace plugin install .");
            }
        }

        PluginCommands::Build {
            path,
            release,
            target,
        } => {
            println!("Building plugin at {}...", path.display());

            // Read manifest to determine plugin type
            let manifest_path = path.join("flowtrace-plugin.toml");
            if !manifest_path.exists() {
                eprintln!("✗ No flowtrace-plugin.toml found in {}", path.display());
                std::process::exit(1);
            }

            let manifest_content =
                std::fs::read_to_string(&manifest_path).context("Failed to read manifest")?;

            // Detect language from manifest
            let is_rust =
                manifest_content.contains("entry = \"target/") || path.join("Cargo.toml").exists();
            let is_python = manifest_content.contains(".py\"")
                || path.join("evaluator.py").exists()
                || path.join("pyproject.toml").exists();
            let is_typescript =
                manifest_content.contains(".wasm\"") && path.join("package.json").exists();

            let build_result = if is_rust {
                build_rust_plugin(path, *release, target)
            } else if is_python {
                build_python_plugin(path)
            } else if is_typescript {
                build_typescript_plugin(path)
            } else {
                Err(anyhow::anyhow!("Could not detect plugin language"))
            };

            match build_result {
                Ok(output_path) => {
                    if json_output {
                        println!(
                            "{}",
                            serde_json::json!({
                                "success": true,
                                "output": output_path.display().to_string()
                            })
                        );
                    } else {
                        println!("✓ Build successful: {}", output_path.display());
                    }
                }
                Err(e) => {
                    eprintln!("✗ Build failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        PluginCommands::Validate { path } => {
            println!("Validating plugin at {}...", path.display());

            let manifest_path = path.join("flowtrace-plugin.toml");
            if !manifest_path.exists() {
                eprintln!("✗ No flowtrace-plugin.toml found");
                std::process::exit(1);
            }

            let mut errors: Vec<String> = Vec::new();
            let mut warnings: Vec<String> = Vec::new();

            // Parse and validate manifest
            let manifest_content =
                std::fs::read_to_string(&manifest_path).context("Failed to read manifest")?;

            match toml::from_str::<flowtrace_plugins::PluginManifest>(&manifest_content) {
                Ok(manifest) => {
                    // Check entry point exists
                    if let Some(entry_file) = get_entry_path(&manifest) {
                        let entry_path = path.join(&entry_file);
                        if !entry_path.exists() && !entry_file.contains("target/") {
                            warnings.push(format!(
                                "Entry point not found: {} (may need to build first)",
                                entry_file
                            ));
                        }
                    }

                    // Check version format
                    if semver::Version::parse(&manifest.plugin.version).is_err() {
                        errors.push(format!(
                            "Invalid version format: {}",
                            manifest.plugin.version
                        ));
                    }

                    // Check for README
                    if !path.join("README.md").exists() {
                        warnings.push("No README.md found".to_string());
                    }

                    if json_output {
                        println!(
                            "{}",
                            serde_json::json!({
                                "valid": errors.is_empty(),
                                "errors": errors,
                                "warnings": warnings,
                                "manifest": manifest
                            })
                        );
                    } else {
                        if errors.is_empty() && warnings.is_empty() {
                            println!("✓ Plugin is valid");
                            println!("  Name: {}", manifest.plugin.name);
                            println!("  Version: {}", manifest.plugin.version);
                            println!("  Type: {:?}", manifest.plugin.plugin_type);
                        } else {
                            if !errors.is_empty() {
                                println!("✗ Validation errors:");
                                for err in &errors {
                                    println!("  - {}", err);
                                }
                            }
                            if !warnings.is_empty() {
                                println!("⚠ Warnings:");
                                for warn in &warnings {
                                    println!("  - {}", warn);
                                }
                            }
                            if !errors.is_empty() {
                                std::process::exit(1);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("✗ Invalid manifest: {}", e);
                    std::process::exit(1);
                }
            }
        }

        PluginCommands::Package { path, output } => {
            println!("Packaging plugin at {}...", path.display());

            let manifest_path = path.join("flowtrace-plugin.toml");
            if !manifest_path.exists() {
                eprintln!("✗ No flowtrace-plugin.toml found");
                std::process::exit(1);
            }

            let manifest_content = std::fs::read_to_string(&manifest_path)?;
            let manifest: flowtrace_plugins::PluginManifest = toml::from_str(&manifest_content)?;

            let package_name = format!("{}-{}.tar.gz", manifest.id(), manifest.version());
            let output_path = output.clone().unwrap_or_else(|| path.join(&package_name));

            // Create tarball with plugin contents
            let tar_gz = std::fs::File::create(&output_path)?;
            let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
            let mut tar = tar::Builder::new(enc);

            // Add manifest
            tar.append_path_with_name(&manifest_path, "flowtrace-plugin.toml")?;

            // Add entry point
            if let Some(entry_file) = get_entry_path(&manifest) {
                let entry_path = path.join(&entry_file);
                if entry_path.exists() {
                    tar.append_path_with_name(&entry_path, &entry_file)?;
                }
            }

            // Add README if exists
            let readme_path = path.join("README.md");
            if readme_path.exists() {
                tar.append_path_with_name(&readme_path, "README.md")?;
            }

            tar.finish()?;

            if json_output {
                println!(
                    "{}",
                    serde_json::json!({
                        "package": output_path.display().to_string(),
                        "name": manifest.plugin.name,
                        "version": manifest.plugin.version
                    })
                );
            } else {
                println!("✓ Created package: {}", output_path.display());
            }
        }

        PluginCommands::Run {
            path,
            input,
            function,
        } => {
            println!("Running plugin at {}...", path.display());

            // Determine if path is a WASM file or plugin directory
            let wasm_path = if path.extension().map(|e| e == "wasm").unwrap_or(false) {
                path.clone()
            } else {
                // Read manifest to find entry
                let manifest_path = path.join("flowtrace-plugin.toml");
                if !manifest_path.exists() {
                    eprintln!("✗ No flowtrace-plugin.toml found and path is not a WASM file");
                    std::process::exit(1);
                }
                let manifest_content = std::fs::read_to_string(&manifest_path)?;
                let manifest: flowtrace_plugins::PluginManifest =
                    toml::from_str(&manifest_content)?;

                match get_entry_path(&manifest) {
                    Some(entry) => path.join(&entry),
                    None => {
                        eprintln!("✗ No entry point defined in manifest");
                        std::process::exit(1);
                    }
                }
            };

            if !wasm_path.exists() {
                eprintln!("✗ WASM file not found: {}", wasm_path.display());
                eprintln!(
                    "  Try running 'flowtrace plugin build {}' first",
                    path.display()
                );
                std::process::exit(1);
            }

            // Parse input JSON if provided
            let input_value: serde_json::Value = input
                .as_ref()
                .map(|s| serde_json::from_str(s))
                .transpose()
                .context("Invalid input JSON")?
                .unwrap_or(serde_json::json!({}));

            println!("Function: {}", function);
            println!("Input: {}", serde_json::to_string_pretty(&input_value)?);
            println!("{:-<40}", "");

            // TODO: Execute using WASM runtime
            println!("⚠ WASM execution not yet implemented");
            println!("  Plugin: {}", wasm_path.display());
        }
    }

    Ok(())
}

// ============================================================================
// Plugin Development Helper Functions
// ============================================================================

/// Get the entry point path from a manifest
fn get_entry_path(manifest: &flowtrace_plugins::PluginManifest) -> Option<String> {
    // Check for WASM entry first
    if let Some(wasm) = &manifest.entry.wasm {
        return Some(wasm.clone());
    }

    // Check for script entry
    if let Some(script) = &manifest.entry.script {
        return Some(script.path.clone());
    }

    // Check for native entry
    if let Some(native) = &manifest.entry.native {
        return Some(native.path.clone());
    }

    None
}

/// Generate a plugin manifest from template parameters
fn generate_plugin_manifest(
    name: &str,
    template: &str,
    description: Option<&str>,
    author: Option<&str>,
) -> String {
    let plugin_type = if template.contains("evaluator") {
        "evaluator"
    } else if template.contains("embedding") {
        "embedding_provider"
    } else if template.contains("exporter") {
        "exporter"
    } else {
        "evaluator"
    };

    let id = name.to_lowercase().replace(' ', "-");
    let desc = description.unwrap_or("A Flowtrace plugin");
    let auth = author.unwrap_or("Flowtrace User");

    let entry_section = if template.starts_with("rust") {
        format!(
            r#"[entry.wasm]
path = "target/wasm32-wasip1/release/{}.wasm""#,
            id.replace('-', "_")
        )
    } else if template.starts_with("python") {
        r#"[entry.wasm]
path = "plugin.wasm""#
            .to_string()
    } else {
        r#"[entry.wasm]
path = "dist/plugin.wasm""#
            .to_string()
    };

    format!(
        r#"# Flowtrace Plugin Manifest
# Generated by flowtrace plugin init

[plugin]
id = "{id}"
name = "{name}"
version = "0.1.0"
description = "{desc}"
type = "{plugin_type}"
authors = ["{auth}"]
license = "MIT"
tags = ["flowtrace", "plugin"]

[capabilities]
read_traces = true

{entry_section}
"#
    )
}

/// Generate template files based on plugin type
fn generate_template_files(plugin_dir: &PathBuf, name: &str, template: &str) -> Result<()> {
    let id = name.to_lowercase().replace(' ', "-");
    let module_name = id.replace('-', "_");

    match template {
        "rust-evaluator" => {
            // Generate Cargo.toml
            let cargo_toml = format!(
                r#"[package]
name = "{module_name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
wit-bindgen = "0.25"

[profile.release]
opt-level = "s"
lto = true
"#
            );
            std::fs::write(plugin_dir.join("Cargo.toml"), cargo_toml)?;

            // Generate lib.rs
            let lib_rs = format!(
                r#"//! {name} - Flowtrace Evaluator Plugin

use serde::{{Deserialize, Serialize}};

#[derive(Serialize, Deserialize)]
struct TraceContext {{
    trace_id: String,
    span_id: String,
    name: String,
    input: String,
    output: String,
}}

#[derive(Serialize, Deserialize)]
struct EvalResult {{
    score: f64,
    passed: bool,
    reason: String,
    metadata: std::collections::HashMap<String, String>,
}}

#[no_mangle]
pub extern "C" fn evaluate(input_ptr: *const u8, input_len: usize) -> *const u8 {{
    // Parse input
    let input_bytes = unsafe {{ std::slice::from_raw_parts(input_ptr, input_len) }};
    let context: TraceContext = match serde_json::from_slice(input_bytes) {{
        Ok(ctx) => ctx,
        Err(_) => return std::ptr::null(),
    }};
    
    // Perform evaluation
    let result = EvalResult {{
        score: 0.8,
        passed: true,
        reason: format!("Evaluated trace {{}}", context.trace_id),
        metadata: std::collections::HashMap::new(),
    }};
    
    // Return result
    let json = serde_json::to_vec(&result).unwrap();
    let ptr = json.as_ptr();
    std::mem::forget(json);
    ptr
}}

#[no_mangle]
pub extern "C" fn plugin_info() -> *const u8 {{
    let info = serde_json::json!({{
        "name": "{name}",
        "version": "0.1.0",
        "type": "evaluator"
    }});
    let json = serde_json::to_vec(&info).unwrap();
    let ptr = json.as_ptr();
    std::mem::forget(json);
    ptr
}}
"#
            );
            std::fs::create_dir_all(plugin_dir.join("src"))?;
            std::fs::write(plugin_dir.join("src/lib.rs"), lib_rs)?;

            // Generate README
            let readme = format!(
                r#"# {name}

A Flowtrace evaluator plugin written in Rust.

## Building

```bash
cargo build --target wasm32-wasip1 --release
```

## Installing

```bash
flowtrace plugin install .
```
"#
            );
            std::fs::write(plugin_dir.join("README.md"), readme)?;
        }

        "python-evaluator" => {
            // Generate evaluator.py
            let evaluator_py = format!(
                r#"\"\"\"
{name} - Flowtrace Evaluator Plugin
\"\"\"

from dataclasses import dataclass
from typing import Dict, Optional

@dataclass
class TraceContext:
    trace_id: str
    span_id: str
    name: str
    input: str
    output: str
    metadata: Dict[str, str]

@dataclass  
class EvalResult:
    score: float
    passed: bool
    reason: str
    metadata: Dict[str, str]

class {class_name}Evaluator:
    \"\"\"Custom evaluator plugin.\"\"\"
    
    def __init__(self):
        self.name = "{name}"
        self.version = "0.1.0"
    
    def evaluate(self, context: TraceContext) -> EvalResult:
        \"\"\"Evaluate a trace and return the result.\"\"\"
        # Implement your evaluation logic here
        score = 0.8
        passed = score >= 0.5
        
        return EvalResult(
            score=score,
            passed=passed,
            reason=f"Evaluated trace {{context.trace_id}}",
            metadata={{}}
        )

# Export the evaluator
evaluator = {class_name}Evaluator()
"#,
                class_name = to_pascal_case(name)
            );
            std::fs::write(plugin_dir.join("evaluator.py"), evaluator_py)?;

            // Generate pyproject.toml
            let pyproject = format!(
                r#"[project]
name = "{id}"
version = "0.1.0"
description = "{name} Flowtrace plugin"
requires-python = ">=3.10"

[build-system]
requires = ["componentize-py"]
build-backend = "componentize_py"
"#
            );
            std::fs::write(plugin_dir.join("pyproject.toml"), pyproject)?;

            // Generate README
            let readme = format!(
                r#"# {name}

A Flowtrace evaluator plugin written in Python.

## Building

```bash
pip install componentize-py
componentize-py componentize -o plugin.wasm evaluator.py
```

## Installing

```bash
flowtrace plugin install .
```
"#
            );
            std::fs::write(plugin_dir.join("README.md"), readme)?;
        }

        "typescript-evaluator" => {
            // Generate package.json
            let package_json = format!(
                r#"{{
  "name": "{id}",
  "version": "0.1.0",
  "description": "{name} Flowtrace plugin",
  "main": "dist/index.js",
  "scripts": {{
    "build": "tsc && jco componentize -o dist/plugin.wasm dist/index.js",
    "typecheck": "tsc --noEmit"
  }},
  "devDependencies": {{
    "@anthropic-ai/jco": "^1.0.0",
    "typescript": "^5.0.0"
  }}
}}
"#
            );
            std::fs::write(plugin_dir.join("package.json"), package_json)?;

            // Generate tsconfig.json
            let tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "moduleResolution": "node",
    "outDir": "./dist",
    "strict": true,
    "esModuleInterop": true
  },
  "include": ["src/**/*"]
}
"#;
            std::fs::write(plugin_dir.join("tsconfig.json"), tsconfig)?;

            // Generate src/index.ts
            let index_ts = format!(
                r#"/**
 * {name} - Flowtrace Evaluator Plugin
 */

interface TraceContext {{
  traceId: string;
  spanId: string;
  name: string;
  input: string;
  output: string;
  metadata: Record<string, string>;
}}

interface EvalResult {{
  score: number;
  passed: boolean;
  reason: string;
  metadata: Record<string, string>;
}}

export function evaluate(context: TraceContext): EvalResult {{
  // Implement your evaluation logic here
  const score = 0.8;
  const passed = score >= 0.5;
  
  return {{
    score,
    passed,
    reason: `Evaluated trace ${{context.traceId}}`,
    metadata: {{}},
  }};
}}

export function pluginInfo(): object {{
  return {{
    name: "{name}",
    version: "0.1.0",
    type: "evaluator",
  }};
}}
"#
            );
            std::fs::create_dir_all(plugin_dir.join("src"))?;
            std::fs::write(plugin_dir.join("src/index.ts"), index_ts)?;

            // Generate README
            let readme = format!(
                r#"# {name}

A Flowtrace evaluator plugin written in TypeScript.

## Building

```bash
npm install
npm run build
```

## Installing

```bash
flowtrace plugin install .
```
"#
            );
            std::fs::write(plugin_dir.join("README.md"), readme)?;
        }

        _ => {
            // Default to Rust evaluator for unknown templates
            return generate_template_files(plugin_dir, name, "rust-evaluator");
        }
    }

    Ok(())
}

/// Convert string to PascalCase
fn to_pascal_case(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

/// Build a Rust plugin to WASM
fn build_rust_plugin(path: &PathBuf, release: bool, target: &str) -> Result<PathBuf> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(path);
    cmd.arg("build");
    cmd.arg("--target").arg(target);

    if release {
        cmd.arg("--release");
    }

    let output = cmd.output().context("Failed to run cargo build")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Cargo build failed:\n{}", stderr));
    }

    // Find the output WASM file
    let profile = if release { "release" } else { "debug" };
    let target_dir = path.join("target").join(target).join(profile);

    // Look for .wasm file
    for entry in std::fs::read_dir(&target_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "wasm").unwrap_or(false) {
            return Ok(path);
        }
    }

    Err(anyhow::anyhow!(
        "No WASM file found in {}",
        target_dir.display()
    ))
}

/// Build a Python plugin to WASM using componentize-py
fn build_python_plugin(path: &PathBuf) -> Result<PathBuf> {
    // Find the main Python file
    let main_file = if path.join("evaluator.py").exists() {
        "evaluator.py"
    } else if path.join("main.py").exists() {
        "main.py"
    } else if path.join("plugin.py").exists() {
        "plugin.py"
    } else {
        return Err(anyhow::anyhow!(
            "No Python entry file found (evaluator.py, main.py, or plugin.py)"
        ));
    };

    let output_path = path.join("plugin.wasm");

    let output = Command::new("componentize-py")
        .current_dir(path)
        .arg("componentize")
        .arg("-o")
        .arg(&output_path)
        .arg(main_file)
        .output()
        .context(
            "Failed to run componentize-py. Is it installed? Run: pip install componentize-py",
        )?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("componentize-py failed:\n{}", stderr));
    }

    Ok(output_path)
}

/// Build a TypeScript plugin to WASM using jco
fn build_typescript_plugin(path: &PathBuf) -> Result<PathBuf> {
    // First run TypeScript compiler
    let tsc_output = Command::new("npx")
        .current_dir(path)
        .arg("tsc")
        .output()
        .context("Failed to run TypeScript compiler")?;

    if !tsc_output.status.success() {
        let stderr = String::from_utf8_lossy(&tsc_output.stderr);
        return Err(anyhow::anyhow!(
            "TypeScript compilation failed:\n{}",
            stderr
        ));
    }

    // Then run jco componentize
    let output_path = path.join("dist").join("plugin.wasm");
    std::fs::create_dir_all(path.join("dist"))?;

    let jco_output = Command::new("npx")
        .current_dir(path)
        .arg("jco")
        .arg("componentize")
        .arg("-o")
        .arg(&output_path)
        .arg("dist/index.js")
        .output()
        .context("Failed to run jco. Is it installed? Run: npm install @anthropic-ai/jco")?;

    if !jco_output.status.success() {
        let stderr = String::from_utf8_lossy(&jco_output.stderr);
        return Err(anyhow::anyhow!("jco componentize failed:\n{}", stderr));
    }

    Ok(output_path)
}

/// Handle backup commands
async fn handle_backup_command(command: BackupCommands, db_path: &PathBuf, json_output: bool) -> Result<()> {
    let backup_dir = db_path.parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| PathBuf::from("./backups"));
    
    std::fs::create_dir_all(&backup_dir)?;

    match command {
        BackupCommands::Create { name } => {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            
            let backup_id = name.unwrap_or_else(|| format!("backup_{}", timestamp));
            let backup_path = backup_dir.join(&backup_id);
            
            if backup_path.exists() {
                anyhow::bail!("Backup '{}' already exists", backup_id);
            }
            
            // Copy database directory to backup
            copy_dir_recursive(db_path, &backup_path)?;
            
            let size = get_dir_size(&backup_path)?;
            
            if json_output {
                println!(r#"{{"success": true, "backup_id": "{}", "path": "{}", "size_bytes": {}}}"#,
                    backup_id, backup_path.display(), size);
            } else {
                println!("✓ Backup created: {}", backup_id);
                println!("  Path: {}", backup_path.display());
                println!("  Size: {} bytes", size);
            }
        }

        BackupCommands::List => {
            let mut backups = Vec::new();
            
            if backup_dir.exists() {
                for entry in std::fs::read_dir(&backup_dir)? {
                    let entry = entry?;
                    if entry.file_type()?.is_dir() {
                        let backup_id = entry.file_name().to_string_lossy().to_string();
                        let metadata = entry.metadata()?;
                        let created = metadata.modified()?
                            .duration_since(std::time::UNIX_EPOCH)?
                            .as_secs();
                        let size = get_dir_size(&entry.path())?;
                        
                        backups.push((backup_id, created, size, entry.path()));
                    }
                }
            }
            
            backups.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by timestamp descending
            
            if json_output {
                let backup_json: Vec<String> = backups.iter()
                    .map(|(id, ts, size, path)| {
                        format!(r#"{{"backup_id": "{}", "created_at": {}, "size_bytes": {}, "path": "{}"}}"#,
                            id, ts, size, path.display())
                    })
                    .collect();
                println!(r#"{{"backups": [{}], "total": {}}}"#, backup_json.join(", "), backups.len());
            } else {
                println!("Backups ({}):", backups.len());
                println!("{}", "=".repeat(60));
                for (id, ts, size, _path) in &backups {
                    let date = chrono::DateTime::from_timestamp(*ts as i64, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| ts.to_string());
                    println!("  {} - {} ({} bytes)", id, date, size);
                }
                if backups.is_empty() {
                    println!("  No backups found.");
                }
            }
        }

        BackupCommands::Restore { backup_id, yes } => {
            let backup_path = backup_dir.join(&backup_id);
            
            if !backup_path.exists() {
                anyhow::bail!("Backup '{}' not found", backup_id);
            }
            
            if !yes {
                println!("⚠️  WARNING: This will replace all current data with backup '{}'", backup_id);
                println!("   A pre-restore backup will be created automatically.");
                print!("   Continue? [y/N] ");
                use std::io::Write;
                std::io::stdout().flush()?;
                
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Aborted.");
                    return Ok(());
                }
            }
            
            // Create pre-restore backup
            let pre_restore_id = format!("pre_restore_{}", 
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs());
            let pre_restore_path = backup_dir.join(&pre_restore_id);
            
            if db_path.exists() {
                copy_dir_recursive(db_path, &pre_restore_path)?;
                println!("✓ Created pre-restore backup: {}", pre_restore_id);
            }
            
            // Remove current database
            if db_path.exists() {
                std::fs::remove_dir_all(db_path)?;
            }
            
            // Restore from backup
            copy_dir_recursive(&backup_path, db_path)?;
            
            if json_output {
                println!(r#"{{"success": true, "backup_id": "{}", "pre_restore_backup": "{}"}}"#,
                    backup_id, pre_restore_id);
            } else {
                println!("✓ Restored from backup: {}", backup_id);
                println!("  Pre-restore backup saved as: {}", pre_restore_id);
            }
        }

        BackupCommands::Delete { backup_id, yes } => {
            let backup_path = backup_dir.join(&backup_id);
            
            if !backup_path.exists() {
                anyhow::bail!("Backup '{}' not found", backup_id);
            }
            
            if !yes {
                print!("Delete backup '{}'? [y/N] ", backup_id);
                use std::io::Write;
                std::io::stdout().flush()?;
                
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Aborted.");
                    return Ok(());
                }
            }
            
            std::fs::remove_dir_all(&backup_path)?;
            
            if json_output {
                println!(r#"{{"success": true, "backup_id": "{}"}}"#, backup_id);
            } else {
                println!("✓ Deleted backup: {}", backup_id);
            }
        }

        BackupCommands::Export { backup_id, output } => {
            let backup_path = backup_dir.join(&backup_id);
            
            if !backup_path.exists() {
                anyhow::bail!("Backup '{}' not found", backup_id);
            }
            
            let output_path = output.unwrap_or_else(|| {
                PathBuf::from(format!("flowtrace_backup_{}.zip", 
                    backup_id.trim_start_matches("backup_")))
            });
            
            // Create ZIP file
            let file = std::fs::File::create(&output_path)?;
            let mut zip = zip::ZipWriter::new(file);
            let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            
            add_dir_to_zip(&mut zip, &backup_path, &backup_id, options)?;
            zip.finish()?;
            
            let size = std::fs::metadata(&output_path)?.len();
            
            if json_output {
                println!(r#"{{"success": true, "backup_id": "{}", "output": "{}", "size_bytes": {}}}"#,
                    backup_id, output_path.display(), size);
            } else {
                println!("✓ Exported backup: {}", backup_id);
                println!("  Output: {}", output_path.display());
                println!("  Size: {} bytes", size);
            }
        }

        BackupCommands::Import { path } => {
            if !path.exists() {
                anyhow::bail!("File not found: {}", path.display());
            }
            
            let file = std::fs::File::open(&path)?;
            let mut archive = zip::ZipArchive::new(file)?;
            
            // Extract backup ID from filename or first directory in ZIP
            let backup_id = path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("imported_{}", 
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()));
            
            let target_path = backup_dir.join(&backup_id);
            
            if target_path.exists() {
                anyhow::bail!("Backup '{}' already exists", backup_id);
            }
            
            std::fs::create_dir_all(&target_path)?;
            
            // Extract all files
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let outpath = target_path.join(file.name().split('/').skip(1).collect::<Vec<_>>().join("/"));
                
                if file.is_dir() {
                    std::fs::create_dir_all(&outpath)?;
                } else {
                    if let Some(parent) = outpath.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let mut outfile = std::fs::File::create(&outpath)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }
            
            let size = get_dir_size(&target_path)?;
            
            if json_output {
                println!(r#"{{"success": true, "backup_id": "{}", "size_bytes": {}}}"#,
                    backup_id, size);
            } else {
                println!("✓ Imported backup: {}", backup_id);
                println!("  Size: {} bytes", size);
            }
        }
    }
    
    Ok(())
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    if !src.exists() {
        anyhow::bail!("Source directory not found: {}", src.display());
    }
    
    std::fs::create_dir_all(dst)?;
    
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        
        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path)?;
        }
    }
    
    Ok(())
}

/// Calculate directory size recursively
fn get_dir_size(path: &std::path::Path) -> Result<u64> {
    let mut size = 0;
    
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                size += get_dir_size(&path)?;
            } else {
                size += entry.metadata()?.len();
            }
        }
    } else {
        size = std::fs::metadata(path)?.len();
    }
    
    Ok(size)
}

/// Add a directory to a ZIP archive recursively
fn add_dir_to_zip<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    path: &std::path::Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        let name = format!("{}/{}", prefix, entry.file_name().to_string_lossy());
        
        if entry_path.is_dir() {
            zip.add_directory(&name, options)?;
            add_dir_to_zip(zip, &entry_path, &name, options)?;
        } else {
            zip.start_file(&name, options)?;
            let mut file = std::fs::File::open(&entry_path)?;
            std::io::copy(&mut file, zip)?;
        }
    }
    Ok(())
}
