# Flowtrace Plugins

This directory contains all plugin-related code for Flowtrace.

## Directory Structure

```
flowtrace-plugins/
├── core/           # Core plugin system (Rust crate: flowtrace-plugins)
│   ├── src/        # Plugin runtime, loader, manager, registry
│   └── wit/        # WIT interface definitions for WASM plugins
│
├── sdk/            # SDKs for plugin developers
│   ├── rust/       # Rust SDK (crate: flowtrace-plugin-sdk)
│   ├── python/     # Python SDK (package: flowtrace-plugin)
│   └── typescript/ # TypeScript SDK (package: @flowtrace/plugin-sdk)
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
The core plugin system that runs inside Flowtrace. Provides:
- Plugin manifest parsing and validation
- WASM runtime (wasmtime) for executing plugins
- Plugin lifecycle management (install, enable, disable, uninstall)
- Capability-based security
- Dependency resolution

### SDK (`plugins/sdk`)
Libraries that plugin developers use to build plugins:

| Language | Package | Build Command |
|----------|---------|---------------|
| Rust | `flowtrace-plugin-sdk` | `cargo build --target wasm32-wasip1` |
| Python | `flowtrace-plugin` | `componentize-py componentize -o plugin.wasm` |
| TypeScript | `@flowtrace/plugin-sdk` | `npm run build` |

### Examples (`plugins/examples`)
Ready-to-use example plugins demonstrating different plugin types:
- **sample-evaluator**: Basic evaluator in Rust
- **sentiment-evaluator**: Sentiment analysis evaluator in Python

### Templates (`plugins/templates`)
Scaffolding templates used by `flowtrace plugin init`:
```bash
flowtrace plugin init my-plugin --template rust-evaluator
flowtrace plugin init my-plugin --template python-evaluator
```

## Quick Start

### Install a Plugin
```bash
# From GitHub
flowtrace plugin install https://github.com/user/my-plugin

# From local directory
flowtrace plugin install ./path/to/plugin
```

### Create a Plugin
```bash
# Create from template
flowtrace plugin init my-evaluator --template rust-evaluator

# Build
cd my-evaluator
cargo build --target wasm32-wasip1 --release

# Install locally
flowtrace plugin install .
```

### List Plugins
```bash
flowtrace plugin list
```
