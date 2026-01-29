# ChronoLake - Build & Development Commands
# Based on patterns from Opcode (https://github.com/winfunc/opcode)

# Show available commands
default:
    @just --list

# Install dependencies with npm
install:
    cd ui && npm install

# Build the React frontend
build-frontend:
    cd ui && npx vite build

# Build the Tauri backend (debug mode)
build-backend:
    cd src-tauri && cargo build

# Build the Tauri backend (release mode)
build-backend-release:
    cd src-tauri && cargo build --release

# Build everything (frontend + backend)
build: install build-frontend build-backend

# Run the application in development mode
dev: build-frontend
    cd src-tauri && cargo tauri dev

# Run the application in release mode
run-release: build-frontend build-backend-release
    cd src-tauri && cargo run --release

# Run all tests (Rust)
test:
    @echo "🧪 Running Rust tests..."
    cargo test --all --verbose

# Run tests for specific crate
test-crate CRATE:
    @echo "🧪 Running tests for {{CRATE}}..."
    cargo test -p {{CRATE}} --verbose

# Format Rust code
fmt:
    @echo "🎨 Formatting Rust code..."
    cargo fmt --all

# Check Rust code without building
check:
    @echo "🔍 Checking Rust code..."
    cargo check --all

# Lint Rust code
clippy:
    @echo "📎 Running Clippy linter..."
    cargo clippy --all -- -D warnings

# Clean all build artifacts
clean:
    @echo "🧹 Cleaning build artifacts..."
    rm -rf ui/node_modules ui/dist
    cargo clean

# Quick development cycle: build frontend and run
quick: build-frontend
    cd src-tauri && cargo run

# Full clean rebuild from scratch
rebuild: clean build dev

# Run embedded HTTP server standalone (for testing)
run-server:
    cd agentreplay-server && cargo run

# Kill processes on ports 5173 and 9432 (useful for cleanup)
kill-ports:
    @echo "🔪 Killing processes on ports 5173 and 9432..."
    lsof -ti :5173 | xargs kill -9 2>/dev/null || true
    lsof -ti :9432 | xargs kill -9 2>/dev/null || true
    @echo "✅ Ports cleaned"

# Get local IP addresses for network access
ip:
    @echo "🌐 Your Mac's IP addresses:"
    @ipconfig getifaddr en0 2>/dev/null && echo "(WiFi: en0)" || echo "WiFi not connected"
    @ipconfig getifaddr en1 2>/dev/null && echo "(Ethernet: en1)" || echo "Ethernet not connected"
    @echo ""
    @echo "📱 Access ChronoLake from network:"
    @echo "   Frontend: http://YOUR_IP:5173"
    @echo "   Backend:  http://YOUR_IP:9432"

# Run benchmarks
bench:
    @echo "⚡ Running benchmarks..."
    cargo bench --all

# Update all dependencies
update:
    @echo "📦 Updating dependencies..."
    cd ui && bun update
    cargo update

# Generate documentation
docs:
    @echo "📚 Generating documentation..."
    cargo doc --all --no-deps --open

# Development server with hot reload
dev-hot:
    @echo "🔥 Starting dev server with hot reload..."
    cd ui && bun run dev &
    cd src-tauri && cargo watch -x 'run'

# Build for production (all platforms)
build-prod: build-frontend build-backend-release
    @echo "🚀 Production build complete!"

# Install development tools
install-tools:
    @echo "🔧 Installing development tools..."
    cargo install cargo-watch
    cargo install cargo-edit
    @echo "✅ Development tools installed"

# Show build information
info:
    @echo "🚀 ChronoLake - LLM Observability Platform"
    @echo "Built with Tauri 2 for macOS"
    @echo ""
    @echo "📦 Frontend: React + TypeScript + Vite"
    @echo "🦀 Backend: Rust + Tauri + Axum"
    @echo "💾 Database: AgentReplay (Temporal Database)"
    @echo "🏗️  Build System: Bun + Just + Cargo"
    @echo ""
    @echo "💡 Common commands:"
    @echo "  just dev          - Build and run in dev mode"
    @echo "  just test         - Run all tests"
    @echo "  just quick        - Quick dev cycle"
    @echo "  just rebuild      - Full clean rebuild"
    @echo "  just kill-ports   - Clean up ports 5173 & 9432"
    @echo "  just ip           - Show IP for network access"
    @echo ""
    @echo "📝 Use 'just --list' to see all available commands"

# Build Linux AppImage using Docker
build-appimage:
    @echo "🐳 Building Linux AppImage..."
    docker build -f Dockerfile.appimage -t vizu-appimage .
    @echo "✅ Build complete. Run 'just run-appimage' to start."

# Run Linux AppImage
run-appimage:
    docker run --rm -v "${PWD}":/app vizu-appimage

# Build Windows Installer using Docker (Windows host only)
build-windows:
    @echo "🐳 Building Windows Installer..."
    docker build -f Dockerfile.windows -t vizu-windows .
