# Flowtrace Integrations Bundle

This is an example bundle plugin that demonstrates how to package Flowtrace integrations for multiple AI coding assistants.

## Supported Targets

| Target | Type | Description |
|--------|------|-------------|
| **Claude Code** | Plugin | Native Claude Code plugin with hooks |
| **Cursor** | MCP Server | Model Context Protocol server |
| **Windsurf** | MCP Server | Model Context Protocol server |
| **VS Code** | Extension | VS Code extension (future) |

## Structure

```
flowtrace-integrations/
├── flowtrace-plugin.toml      # Bundle manifest
├── README.md
└── integrations/              # assets_root
    ├── claude/
    │   ├── install.md         # Installation instructions
    │   └── plugin/            # Claude plugin files
    │       ├── .claude-plugin/
    │       │   └── plugin.json
    │       └── hooks/
    │           ├── hooks.json
    │           └── *.py
    ├── cursor/
    │   └── install.md
    ├── vscode/
    │   └── install.md
    └── windsurf/
        └── install.md
```

## Manifest Schema (v2)

The `flowtrace-plugin.toml` uses schema version 2 with bundle configuration:

```toml
schema_version = 2

[plugin]
type = "bundle"
# ... metadata

[bundle]
bundle_version = "1.0.0"
default_install_mode = "guided"
assets_root = "integrations"

[[bundle.targets]]
id = "claude_code"
display_name = "Claude Code"
kind = "claude_plugin"
install_md = "claude/install.md"
# ... detection rules, files, ops
```

## Key Features

### Detection Rules

Each target has detection rules to check if it's installed:

```toml
[[bundle.targets.detect]]
type = "file_exists"
path = "${home}/.claude/settings.json"

[[bundle.targets.detect]]
type = "command_exists"
command = "claude"
```

### File Copy Operations

Copy files/directories during installation:

```toml
[[bundle.targets.files_to_copy]]
from = "claude/plugin"
to = "${home}/.claude/plugins/flowtrace"
strategy = "copy_dir"
overwrite = true
```

### Install Operations (ops)

Declarative operations for JSON configuration:

```toml
[[bundle.targets.ops]]
type = "json_merge"
file = "${home}/.claude/settings.json"
[bundle.targets.ops.object]
enabledPlugins = { flowtrace = true }

[[bundle.targets.ops]]
type = "json_patch"
file_candidates = ["${home}/.cursor/mcp.json"]
create_if_missing = true
[[bundle.targets.ops.patch]]
op = "add"
path = "/mcpServers/flowtrace"
value = { command = "flowtrace-mcp", args = ["--stdio"] }
```

### Variable Templates

Paths support variable substitution:

- `${home}` - User home directory
- `${config_dir}` - System config directory
- `${project_dir}` - Project directory (user-prompted)
- `${install_root}` - Computed install location

## Installation Flow

When a user clicks "Install" in the Flowtrace UI:

1. **Detection** - Check if target is available
2. **Variables** - Prompt for required variables (e.g., project_dir)
3. **Plan** - Generate installation plan
4. **Preview** - Show user what will be installed
5. **Execute** - Run install operations
6. **Receipt** - Save receipt for verify/uninstall
7. **Commands** - Run post-install commands

## Verify and Uninstall

The bundle system maintains install receipts for:

- **Verify** - Check installed files haven't been modified
- **Uninstall** - Reverse all operations cleanly

## See Also

- [Flowtrace Plugin System](../../README.md)
- [Bundle Module Source](../../core/src/bundle.rs)
- [Manifest Schema](../../core/src/manifest.rs)
