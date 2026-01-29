# Contributing to Agentreplay

Thank you for your interest in contributing to Agentreplay! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)
- [Code Style](#code-style)
- [Documentation](#documentation)

## Code of Conduct

By participating in this project, you agree to maintain a respectful and inclusive environment. Please:

- Be respectful and constructive in discussions
- Welcome newcomers and help them get started
- Focus on what is best for the community
- Show empathy towards other community members

## Getting Started

### Prerequisites

Before contributing, ensure you have the following installed:

- **Rust** (1.75 or later): [https://rustup.rs/](https://rustup.rs/)
- **Node.js** (18 or later): [https://nodejs.org/](https://nodejs.org/)
- **Python** (3.9 or later): For SDK development and testing
- **Go** (1.21 or later): For Go SDK development

### Forking and Cloning

1. Fork the repository on GitHub
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/agentreplay.git
   cd agentreplay
   ```
3. Add the upstream remote:
   ```bash
   git remote add upstream https://github.com/sochdb/agentreplay.git
   ```

## Development Setup

### Rust Core

```bash
# Install Rust toolchain
rustup update stable
rustup component add clippy rustfmt

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run with optimizations (for benchmarks)
cargo build --release --workspace
```

### Python SDK

```bash
cd sdks/python

# Create virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install in development mode with all extras
pip install -e ".[dev,langchain,openai,llama-index]"

# Run tests
pytest tests/

# Type checking
mypy src/agentreplay
```

### JavaScript/TypeScript SDK

```bash
cd sdks/js

# Install dependencies
npm install

# Build
npm run build

# Run tests
npm test

# Type checking
npm run typecheck

# Lint
npm run lint
```

### Go SDK

```bash
cd sdks/golang

# Run tests
go test ./...

# Build
go build ./...
```

### Desktop Application (Tauri)

```bash
cd agentreplay-desktop

# Install frontend dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## Project Structure

```
agentreplay/
├── agentreplay-core/       # Core data structures (AgentFlowEdge, HLC)
├── agentreplay-storage/    # LSM-tree storage engine
├── agentreplay-index/      # Vector and causal indexing
├── agentreplay-query/      # Query engine and aggregations
├── agentreplay-server/     # REST API and WebSocket server
├── agentreplay-evals/      # Evaluation framework
├── agentreplay-desktop/    # Tauri desktop application
├── sdks/
│   ├── python/           # Python SDK
│   ├── js/               # JavaScript/TypeScript SDK
│   ├── rust/             # Rust SDK (client)
│   └── golang/           # Go SDK
├── docs-site/            # Documentation (Jekyll)
└── benchmarks/           # Performance benchmarks
```

### Crate Dependencies

```
agentreplay-core (foundation)
    ↓
agentreplay-storage (uses core)
    ↓
agentreplay-index (uses core, query)
    ↓
agentreplay-query (uses core, storage, index)
    ↓
agentreplay-server (uses all)
    ↓
agentreplay-evals (uses core, query)
```

## Making Changes

### Branching Strategy

- `main`: Stable, release-ready code
- `develop`: Integration branch for features
- `feature/*`: New features
- `fix/*`: Bug fixes
- `docs/*`: Documentation updates

Create a branch for your changes:

```bash
git checkout -b feature/my-new-feature
```

### Commit Messages

Follow conventional commits format:

```
type(scope): description

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting (no code change)
- `refactor`: Code restructuring
- `test`: Adding tests
- `chore`: Maintenance tasks

Examples:
```
feat(storage): add zstd dictionary compression

Implements dictionary compression for repetitive payload data.
Achieves 20-40% better compression for structured JSON.

Closes #123
```

```
fix(sdk-python): handle connection timeout gracefully

The client now retries with exponential backoff on timeout errors.
```

## Testing

### Rust Tests

```bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p agentreplay-storage

# Run with output
cargo test -- --nocapture

# Run benchmarks
cargo bench
```

### Integration Tests

```bash
# Start test server
cargo run --release -p agentreplay-server &

# Run integration tests
cargo test --test integration_tests

# Stop server
pkill agentreplay-server
```

### Python SDK Tests

```bash
cd sdks/python
pytest tests/ -v

# With coverage
pytest tests/ --cov=src/agentreplay --cov-report=html
```

### JavaScript SDK Tests

```bash
cd sdks/js
npm test

# With coverage
npm run test:coverage
```

## Submitting Changes

### Pull Request Process

1. **Update your fork**:
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Push your branch**:
   ```bash
   git push origin feature/my-new-feature
   ```

3. **Create a Pull Request** on GitHub with:
   - Clear title describing the change
   - Description of what and why
   - Reference to any related issues
   - Screenshots for UI changes

4. **Address review feedback** and keep the PR updated

### PR Checklist

- [ ] Code follows project style guidelines
- [ ] Tests pass locally (`cargo test --workspace`)
- [ ] New code has appropriate tests
- [ ] Documentation updated if needed
- [ ] Commit messages follow conventions
- [ ] No merge conflicts with `main`

## Code Style

### Rust

We use `rustfmt` and `clippy`:

```bash
# Format code
cargo fmt --all

# Run linter
cargo clippy --workspace -- -D warnings
```

### Python

We use `black`, `isort`, and `mypy`:

```bash
cd sdks/python

# Format
black src/ tests/
isort src/ tests/

# Type check
mypy src/agentreplay
```

### JavaScript/TypeScript

We use ESLint and Prettier:

```bash
cd sdks/js

# Lint
npm run lint

# Format
npm run format
```

### General Guidelines

- Use meaningful variable and function names
- Keep functions focused and small
- Add comments for complex logic
- Prefer explicit over implicit
- Handle errors gracefully

## Documentation

### Code Documentation

- **Rust**: Use `///` doc comments for public APIs
- **Python**: Use docstrings (Google style)
- **TypeScript**: Use JSDoc or TSDoc

### User Documentation

Documentation lives in `docs-site/` using Jekyll:

```bash
cd docs-site

# Install dependencies
bundle install

# Serve locally
bundle exec jekyll serve

# Open http://localhost:4000
```

### Adding Documentation Pages

1. Create a new `.md` file in `docs-site/docs/`
2. Add frontmatter:
   ```yaml
   ---
   layout: default
   title: Your Page Title
   nav_order: N  # Position in navigation
   ---
   ```
3. Write content using Markdown
4. Test locally before submitting

## Release Process

Releases are managed by maintainers:

1. Update version numbers in `Cargo.toml` files
2. Update `CHANGELOG.md`
3. Create release branch and tag
4. CI/CD handles publishing

## Getting Help

- **Questions**: Open a GitHub Discussion
- **Bugs**: Open a GitHub Issue
- **Security**: Email security@agentreplay.dev (do not open public issues)

## Recognition

Contributors are recognized in:
- Release notes
- GitHub contributors page
- Project README (for significant contributions)

Thank you for contributing to Agentreplay! 🎉
