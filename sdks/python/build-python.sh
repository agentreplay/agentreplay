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

# Quick Python SDK Build Script
# Usage: ./build-python.sh [--version X.Y.Z] [--bump patch|minor|major] [--publish]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERSION_FILE="$SCRIPT_DIR/../VERSION"

# Get version from VERSION file or default
get_version() {
    if [ -f "$VERSION_FILE" ]; then
        cat "$VERSION_FILE" | tr -d '\n'
    else
        echo "0.1.0"
    fi
}

# Parse arguments
VERSION=""
PUBLISH=false
BUMP=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --version|-v)
            VERSION="$2"
            shift 2
            ;;
        --bump|-b)
            BUMP="$2"
            shift 2
            ;;
        --publish|-p)
            PUBLISH=true
            shift
            ;;
        *)
            shift
            ;;
    esac
done

# Handle version bump
if [ -n "$BUMP" ]; then
    current=$(get_version)
    IFS='.' read -r major minor patch <<< "$current"
    case $BUMP in
        major) major=$((major + 1)); minor=0; patch=0 ;;
        minor) minor=$((minor + 1)); patch=0 ;;
        patch) patch=$((patch + 1)) ;;
    esac
    VERSION="$major.$minor.$patch"
    echo "$VERSION" > "$VERSION_FILE"
    echo "📦 Bumped version: $current -> $VERSION"
elif [ -n "$VERSION" ]; then
    echo "$VERSION" > "$VERSION_FILE"
    echo "📦 Set version: $VERSION"
else
    VERSION=$(get_version)
fi

cd "$SCRIPT_DIR"

echo "🐍 Building Flowtrace Python SDK v$VERSION..."

# Update version in pyproject.toml
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" pyproject.toml && rm -f pyproject.toml.bak

# Update version in __init__.py if exists
if [ -f "src/flowtrace/__init__.py" ]; then
    sed -i.bak "s/__version__ = \".*\"/__version__ = \"$VERSION\"/" src/flowtrace/__init__.py && rm -f src/flowtrace/__init__.py.bak
fi

# Clean previous builds
rm -rf dist/ build/ *.egg-info src/*.egg-info

# Create/activate virtual environment if needed
if [ ! -d ".venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv .venv
fi

source .venv/bin/activate

# Install build dependencies
echo "Installing build tools..."
pip install --quiet --upgrade pip setuptools wheel build twine

# Build the package
echo "Building wheel and sdist..."
python -m build

# Show what was built
echo ""
echo "✅ Built packages:"
ls -la dist/

# Validate
echo ""
echo "Validating with twine..."
twine check dist/*

# Publish if requested
if [ "$PUBLISH" = true ]; then
    echo ""
    echo "📦 Publishing to PyPI..."
    read -p "Publish v$VERSION to PyPI? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        twine upload dist/*
        echo "✅ Published v$VERSION to PyPI!"
    else
        echo "Publish cancelled."
    fi
fi

echo ""
echo "🎉 Python SDK v$VERSION build complete!"
echo ""
echo "To install locally:"
echo "  pip install dist/flowtrace-$VERSION-py3-none-any.whl"
echo ""
echo "To publish to PyPI:"
echo "  ./build-python.sh --publish"
