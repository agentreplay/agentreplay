#!/bin/bash

# Copyright 2025 Sushanth (https://github.com/sushanthpy)
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# Agentreplay SDK Build Script
# Builds and packages all SDK distributions

set -e  # Exit on error

# Version management
VERSION_FILE="$(dirname "${BASH_SOURCE[0]}")/VERSION"
DEFAULT_VERSION="0.1.0"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="$SCRIPT_DIR/dist"

# Get current version
get_version() {
    if [ -f "$VERSION_FILE" ]; then
        cat "$VERSION_FILE" | tr -d '\n'
    else
        echo "$DEFAULT_VERSION"
    fi
}

# Set version in a file
set_version_in_file() {
    local file="$1"
    local version="$2"
    local pattern="$3"
    local replacement="$4"
    
    if [ -f "$file" ]; then
        sed -i.bak "s/$pattern/$replacement/" "$file" && rm -f "${file}.bak"
    fi
}

# Print with color
print_header() {
    echo -e "\n${BLUE}=== $1 ===${NC}\n"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

# Clean previous builds
clean_builds() {
    print_header "Cleaning previous builds"
    rm -rf "$BUILD_DIR"
    mkdir -p "$BUILD_DIR"
    print_success "Cleaned build directory"
}

# Build Python SDK
build_python() {
    print_header "Building Python SDK"
    
    local version=$(get_version)
    echo "Version: $version"
    
    PYTHON_DIR="$SCRIPT_DIR/python"
    
    if [ ! -d "$PYTHON_DIR" ]; then
        print_error "Python SDK directory not found: $PYTHON_DIR"
        return 1
    fi
    
    cd "$PYTHON_DIR"
    
    # Update version in pyproject.toml
    if [ -f "pyproject.toml" ]; then
        sed -i.bak "s/^version = \".*\"/version = \"$version\"/" pyproject.toml && rm -f pyproject.toml.bak
        print_success "Updated pyproject.toml to version $version"
    fi
    
    # Update version in __init__.py if exists
    if [ -f "src/agentreplay/__init__.py" ]; then
        sed -i.bak "s/__version__ = \".*\"/__version__ = \"$version\"/" src/agentreplay/__init__.py && rm -f src/agentreplay/__init__.py.bak
    fi
    
    # Clean previous builds
    rm -rf dist/ build/ *.egg-info src/*.egg-info
    
    # Check for virtual environment
    if [ -d ".venv" ]; then
        print_success "Found virtual environment"
        source .venv/bin/activate 2>/dev/null || true
    fi
    
    # Ensure build tools are installed
    echo "Installing build dependencies..."
    pip install --quiet --upgrade pip setuptools wheel build twine
    
    # Build wheel and sdist
    echo "Building wheel and source distribution..."
    python -m build
    
    # Copy to main dist folder
    cp dist/*.whl "$BUILD_DIR/" 2>/dev/null || true
    cp dist/*.tar.gz "$BUILD_DIR/" 2>/dev/null || true
    
    # List built packages
    echo ""
    print_success "Python packages built:"
    ls -la dist/
    
    # Validate packages
    echo ""
    echo "Validating packages with twine..."
    twine check dist/* || print_warning "Twine check had warnings"
    
    cd "$SCRIPT_DIR"
    print_success "Python SDK build complete"
}

# Build JavaScript/TypeScript SDK
build_js() {
    print_header "Building JavaScript SDK"
    
    local version=$(get_version)
    echo "Version: $version"
    
    JS_DIR="$SCRIPT_DIR/js"
    
    if [ ! -d "$JS_DIR" ]; then
        print_warning "JavaScript SDK directory not found: $JS_DIR"
        return 0
    fi
    
    cd "$JS_DIR"
    
    # Check for package.json
    if [ ! -f "package.json" ]; then
        print_warning "No package.json found in JS SDK"
        return 0
    fi
    
    # Update version in package.json
    if command -v jq &> /dev/null; then
        jq ".version = \"$version\"" package.json > package.json.tmp && mv package.json.tmp package.json
        print_success "Updated package.json to version $version"
    else
        sed -i.bak "s/\"version\": \".*\"/\"version\": \"$version\"/" package.json && rm -f package.json.bak
    fi
    
    # Install dependencies
    echo "Installing dependencies..."
    npm install --silent
    
    # Build TypeScript if tsconfig exists
    if [ -f "tsconfig.json" ]; then
        echo "Compiling TypeScript..."
        npm run build 2>/dev/null || npx tsc
    fi
    
    # Create package tarball
    echo "Creating npm package..."
    npm pack
    
    # Copy to main dist folder
    cp *.tgz "$BUILD_DIR/" 2>/dev/null || true
    
    cd "$SCRIPT_DIR"
    print_success "JavaScript SDK build complete"
}

# Build Rust SDK
build_rust() {
    print_header "Building Rust SDK"
    
    local version=$(get_version)
    echo "Version: $version"
    
    RUST_DIR="$SCRIPT_DIR/rust"
    
    if [ ! -d "$RUST_DIR" ]; then
        print_warning "Rust SDK directory not found: $RUST_DIR"
        return 0
    fi
    
    cd "$RUST_DIR"
    
    # Check for Cargo.toml
    if [ ! -f "Cargo.toml" ]; then
        print_warning "No Cargo.toml found in Rust SDK"
        return 0
    fi
    
    # Update version in Cargo.toml
    sed -i.bak "s/^version = \".*\"/version = \"$version\"/" Cargo.toml && rm -f Cargo.toml.bak
    print_success "Updated Cargo.toml to version $version"
    
    # Build release
    echo "Building Rust crate..."
    cargo build --release
    
    # Package for crates.io
    echo "Packaging crate..."
    cargo package --allow-dirty 2>/dev/null || print_warning "Cargo package had warnings"
    
    # Copy to main dist folder
    cp target/package/*.crate "$BUILD_DIR/" 2>/dev/null || true
    
    cd "$SCRIPT_DIR"
    print_success "Rust SDK build complete"
}

# Build Go SDK
build_go() {
    print_header "Building Go SDK"
    
    local version=$(get_version)
    echo "Version: $version"
    
    GO_DIR="$SCRIPT_DIR/golang"
    
    if [ ! -d "$GO_DIR" ]; then
        print_warning "Go SDK directory not found: $GO_DIR"
        return 0
    fi
    
    cd "$GO_DIR"
    
    # Check for go.mod
    if [ ! -f "go.mod" ]; then
        print_warning "No go.mod found in Go SDK"
        return 0
    fi
    
    # Update version constant if version.go exists
    if [ -f "version.go" ]; then
        sed -i.bak "s/Version = \".*\"/Version = \"$version\"/" version.go && rm -f version.go.bak
        print_success "Updated version.go to version $version"
    fi
    
    # Download dependencies
    echo "Downloading Go dependencies..."
    go mod download
    
    # Build
    echo "Building Go module..."
    go build ./...
    
    # Run tests
    echo "Running tests..."
    go test ./... 2>/dev/null || print_warning "Some tests failed or skipped"
    
    cd "$SCRIPT_DIR"
    print_success "Go SDK build complete"
}

# Print summary
print_summary() {
    print_header "Build Summary"
    
    echo "Built packages in $BUILD_DIR:"
    echo ""
    
    if [ -d "$BUILD_DIR" ] && [ "$(ls -A $BUILD_DIR 2>/dev/null)" ]; then
        ls -lah "$BUILD_DIR"
    else
        print_warning "No packages found in dist directory"
    fi
    
    echo ""
    print_success "All SDK builds complete!"
}

# Bump version
bump_version() {
    local bump_type="$1"  # major, minor, patch
    local current=$(get_version)
    
    IFS='.' read -r major minor patch <<< "$current"
    
    case $bump_type in
        major)
            major=$((major + 1))
            minor=0
            patch=0
            ;;
        minor)
            minor=$((minor + 1))
            patch=0
            ;;
        patch)
            patch=$((patch + 1))
            ;;
        *)
            print_error "Invalid bump type: $bump_type (use: major, minor, patch)"
            return 1
            ;;
    esac
    
    local new_version="$major.$minor.$patch"
    echo "$new_version" > "$VERSION_FILE"
    print_success "Bumped version: $current -> $new_version"
}

# Set specific version
set_version() {
    local new_version="$1"
    
    # Validate semver format
    if [[ ! "$new_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
        print_error "Invalid version format: $new_version (expected: X.Y.Z or X.Y.Z-suffix)"
        return 1
    fi
    
    echo "$new_version" > "$VERSION_FILE"
    print_success "Set version to: $new_version"
}

# Show usage
usage() {
    echo "Agentreplay SDK Build Script"
    echo ""
    echo "Usage: $0 [OPTIONS] [SDK...]"
    echo ""
    echo "SDKs:"
    echo "  python    Build Python SDK"
    echo "  js        Build JavaScript/TypeScript SDK"
    echo "  rust      Build Rust SDK"
    echo "  go        Build Go SDK"
    echo "  all       Build all SDKs (default)"
    echo ""
    echo "Options:"
    echo "  --clean              Clean build directories before building"
    echo "  --version VER        Set version to VER (e.g., 1.2.3)"
    echo "  --bump TYPE          Bump version (major|minor|patch)"
    echo "  --show-version       Show current version and exit"
    echo "  --help               Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                         # Build all SDKs"
    echo "  $0 python                  # Build only Python SDK"
    echo "  $0 --version 1.0.0 all     # Set version and build all"
    echo "  $0 --bump patch python     # Bump patch version and build Python"
    echo "  $0 --clean all             # Clean and rebuild all"
}

# Main
main() {
    local do_clean=false
    local sdks=()
    local new_version=""
    local bump_type=""
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --clean)
                do_clean=true
                shift
                ;;
            --version|-v)
                new_version="$2"
                shift 2
                ;;
            --bump|-b)
                bump_type="$2"
                shift 2
                ;;
            --show-version)
                echo "Current version: $(get_version)"
                exit 0
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            python|js|rust|go|all)
                sdks+=("$1")
                shift
                ;;
            *)
                print_error "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done
    
    # Handle version changes
    if [ -n "$new_version" ]; then
        set_version "$new_version"
    elif [ -n "$bump_type" ]; then
        bump_version "$bump_type"
    fi
    
    # Default to all if no SDKs specified
    if [ ${#sdks[@]} -eq 0 ]; then
        sdks=("all")
    fi
    
    print_header "Agentreplay SDK Builder"
    echo "Version: $(get_version)"
    echo "Building: ${sdks[*]}"
    
    # Clean if requested
    if [ "$do_clean" = true ]; then
        clean_builds
    fi
    
    # Create dist directory
    mkdir -p "$BUILD_DIR"
    
    # Build requested SDKs
    for sdk in "${sdks[@]}"; do
        case $sdk in
            python)
                build_python
                ;;
            js)
                build_js
                ;;
            rust)
                build_rust
                ;;
            go)
                build_go
                ;;
            all)
                build_python
                build_js
                build_rust
                build_go
                ;;
        esac
    done
    
    print_summary
}

# Run main
main "$@"
