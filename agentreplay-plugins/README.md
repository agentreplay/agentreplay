# Agent Replay Plugins

This directory contains all plugin-related code for Agent Replay.

## Directory Structure

```
agentreplay-plugins/
├── core/           # Core plugin system (Rust crate: agentreplay-plugins)
│   ├── src/        # Plugin runtime, loader, manager, registry
│   └── wit/        # WIT interface definitions for WASM plugins
│
├── sdk/            # SDKs for plugin developers
│   ├── rust/       # Rust SDK (crate: agentreplay-plugin-sdk)
│   ├── python/     # Python SDK (package: agentreplay-plugin)
│   └── typescript/ # TypeScript SDK (package: @agentreplay/plugin-sdk)
│
├── examples/       # Example/sample plugins
│   ├── sample-evaluator/      # Basic Rust evaluator
│   └── sentiment-evaluator/   # Python sentiment analysis
│
└── templates/      # Plugin scaffolding templates
    ├── rust-evaluator/
    └── python-evaluator/
```

## Components

### Core (`plugins/core`)
The core plugin system that runs inside Agent Replay. Provides:
- Plugin manifest parsing and validation
- WASM runtime (wasmtime) for executing plugins
- Plugin lifecycle management (install, enable, disable, uninstall)
- Capability-based security
- Dependency resolution

### SDK (`plugins/sdk`)
Libraries that plugin developers use to build plugins:

| Language | Package | Build Command |
|----------|---------|---------------|
| Rust | `agentreplay-plugin-sdk` | `cargo build --target wasm32-wasip1` |
| Python | `agentreplay-plugin` | `componentize-py componentize -o plugin.wasm` |
| TypeScript | `@agentreplay/plugin-sdk` | `npm run build` |

### Examples (`plugins/examples`)
Ready-to-use example plugins demonstrating different plugin types:
- **sample-evaluator**: Basic evaluator in Rust
- **sentiment-evaluator**: Sentiment analysis evaluator in Python

### Templates (`plugins/templates`)
Scaffolding templates used by `agentreplay plugin init`:
```bash
agentreplay plugin init my-plugin --template rust-evaluator
agentreplay plugin init my-plugin --template python-evaluator
```

## Quick Start

### Install a Plugin
```bash
# From GitHub
agentreplay plugin install https://github.com/user/my-plugin

# From local directory
agentreplay plugin install ./path/to/plugin
```

### Create a Plugin
```bash
# Create from template
agentreplay plugin init my-evaluator --template rust-evaluator

# Build
cd my-evaluator
cargo build --target wasm32-wasip1 --release

# Install locally
agentreplay plugin install .
```

### List Plugins
```bash
agentreplay plugin list
```
