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

# Agentreplay Tauri Build Script
# Cross-platform build script for macOS, Linux, and Windows

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
UI_DIR="$PROJECT_ROOT/agentreplay-ui"
TAURI_DIR="$SCRIPT_DIR"

echo "=========================================="
echo "Agentreplay Tauri Build Script"
echo "=========================================="
echo ""

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Linux*)     PLATFORM="linux";;
    Darwin*)    PLATFORM="macos";;
    CYGWIN*|MINGW*|MSYS*) PLATFORM="windows";;
    *)          PLATFORM="unknown";;
esac

echo "🖥️  Platform: $PLATFORM"
echo "📁 Project Root: $PROJECT_ROOT"
echo "📁 UI Directory: $UI_DIR"
echo "📁 Tauri Directory: $TAURI_DIR"
echo ""

# Step 1: Build the UI
echo "🔨 Step 1: Building frontend..."
if [ -d "$UI_DIR" ]; then
    cd "$UI_DIR"
    
    # Check if node_modules exists
    if [ ! -d "node_modules" ]; then
        echo "📦 Installing npm dependencies..."
        npm install
    fi
    
    # Build the UI
    echo "📦 Building UI..."
    npm run build
    
    # Copy dist to Tauri
    echo "📋 Copying dist to Tauri..."
    rm -rf "$TAURI_DIR/agentreplay-ui/dist"
    mkdir -p "$TAURI_DIR/agentreplay-ui"
    cp -r dist "$TAURI_DIR/agentreplay-ui/"
    
    echo "✅ Frontend built successfully"
else
    echo "❌ UI directory not found: $UI_DIR"
    exit 1
fi

# Step 2: Build Tauri
echo ""
echo "🔨 Step 2: Building Tauri application..."
cd "$TAURI_DIR"

# Check for Tauri CLI
if command -v cargo-tauri &> /dev/null; then
    TAURI_CMD="cargo tauri"
elif [ -f "$UI_DIR/node_modules/.bin/tauri" ]; then
    TAURI_CMD="$UI_DIR/node_modules/.bin/tauri"
else
    echo "❌ Tauri CLI not found. Install with: cargo install tauri-cli"
    exit 1
fi

# Build for the current platform
echo "🏗️  Building with: $TAURI_CMD build"
$TAURI_CMD build

echo ""
echo "=========================================="
echo "✅ Build Complete!"
echo "=========================================="
echo ""

# Show output locations
case "$PLATFORM" in
    macos)
        echo "📦 Output files:"
        echo "   • App: $TAURI_DIR/target/release/bundle/macos/Agentreplay.app"
        echo "   • DMG: $TAURI_DIR/target/release/bundle/dmg/Agentreplay_*.dmg"
        ;;
    linux)
        echo "📦 Output files:"
        echo "   • AppImage: $TAURI_DIR/target/release/bundle/appimage/"
        echo "   • Deb: $TAURI_DIR/target/release/bundle/deb/"
        ;;
    windows)
        echo "📦 Output files:"
        echo "   • MSI: $TAURI_DIR/target/release/bundle/msi/"
        echo "   • NSIS: $TAURI_DIR/target/release/bundle/nsis/"
        ;;
esac
