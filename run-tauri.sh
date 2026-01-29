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

# Agentreplay Tauri Runner
# This script runs the Tauri app while avoiding snap library conflicts

set -e

cd "$(dirname "$0")"

echo "🚀 Starting Agentreplay Tauri Application..."
echo ""

# Kill any existing processes
echo "🧹 Cleaning up existing processes..."
lsof -ti :9600 | xargs kill -9 2>/dev/null || true
lsof -ti :9601 | xargs kill -9 2>/dev/null || true  # MCP Server port
lsof -ti :5173 | xargs kill -9 2>/dev/null || true
lsof -ti :4317 | xargs kill -9 2>/dev/null || true
pkill -f "cargo run --no-default-features" 2>/dev/null || true
pkill -f "agentreplay" 2>/dev/null || true
pkill -f "vite" 2>/dev/null || true
sleep 1
echo "✅ Cleanup complete"
echo ""

# Set config file path for Tauri app
export AGENTREPLAY_CONFIG_PATH="$(pwd)/agentreplay-server-config.toml"
echo "📝 Using config: $AGENTREPLAY_CONFIG_PATH"
echo ""
echo "📡 Servers that will start:"
echo "   • HTTP API:    http://localhost:9600"
echo "   • MCP Server:  http://localhost:9601"
echo "   • OTLP gRPC:   localhost:4317"
echo "   • Vite Dev:    http://localhost:5173"
echo ""

# Unset snap-related environment variables that might cause glibc conflicts
unset GTK_MODULES
unset GTK_PATH
unset GTK3_MODULES
unset GTK_IM_MODULE_FILE
unset GIO_MODULE_DIR
unset XDG_DATA_HOME
unset XDG_DATA_DIRS
unset XDG_CONFIG_DIRS
unset SNAP
unset SNAP_NAME
unset SNAP_REVISION
unset SNAP_VERSION

# Run tauri dev using local binary from agentreplay-ui/node_modules
# This runs from the project root so it can find agentreplay-tauri/
echo "🔨 Building and running Tauri app..."
echo ""

if [ -f "agentreplay-ui/node_modules/.bin/tauri" ]; then
    # Start the Vite dev server in the background
    echo "🌐 Starting Vite dev server..."
    cd agentreplay-ui
    npm run dev &
    VITE_PID=$!
    cd ..
    
    # Wait for Vite to be ready
    echo "⏳ Waiting for Vite dev server to start..."
    for i in {1..30}; do
        if curl -s http://localhost:5173 > /dev/null 2>&1; then
            echo "✅ Vite dev server is ready"
            break
        fi
        sleep 1
    done
    
    # Trap to cleanup Vite on exit
    trap "kill $VITE_PID 2>/dev/null || true" EXIT
    
    cd agentreplay-tauri
    env -u GTK_MODULES -u GTK_PATH -u GTK3_MODULES -u GIO_MODULE_DIR -u XDG_DATA_HOME -u XDG_DATA_DIRS AGENTREPLAY_CONFIG_PATH="$AGENTREPLAY_CONFIG_PATH" ../agentreplay-ui/node_modules/.bin/tauri dev
else
    echo "❌ Local Tauri CLI not found. Please run 'cd agentreplay-ui && npm install' first."
    exit 1
fi
