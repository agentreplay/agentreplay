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

# Start ChronoLake: Backend Server + Web UI (No Tauri)

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color
DEBUG=true
echo -e "${BLUE}🚀 Starting ChronoLake (Web Mode)${NC}"

# Check if release binary exists
if [ ! -f "./target/release/agentreplay-server" ]; then
    echo -e "${RED}❌ Server binary not found. Building...${NC}"
    cargo build --release
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Build failed${NC}"
        exit 1
    fi
fi

# Create logs directory
mkdir -p logs
LOG_FILE="logs/agentreplay-server-$(date +%Y%m%d-%H%M%S).log"

# Kill any existing processes
echo -e "${BLUE}Cleaning up existing processes...${NC}"
lsof -ti :47100 | xargs kill -9 2>/dev/null || true
lsof -ti :47101 | xargs kill -9 2>/dev/null || true  # MCP Server
lsof -ti :47173 | xargs kill -9 2>/dev/null || true
lsof -ti :47117 | xargs kill -9 2>/dev/null || true
sleep 1

# Start backend server in background with logging
echo -e "${GREEN}Starting Agentreplay server on port 47100...${NC}"
echo -e "${BLUE}📝 Logging to: ${LOG_FILE}${NC}"
RUST_LOG=info,agentreplay_server=debug ./target/release/agentreplay-server --config agentreplay-server-config.toml > "${LOG_FILE}" 2>&1 &
SERVER_PID=$!

# Wait for server to be ready
echo -e "${BLUE}Waiting for HTTP server (port 47100)...${NC}"
for i in {1..30}; do
  if curl -s http://localhost:47100/health > /dev/null 2>&1; then
    echo -e "${GREEN}✅ HTTP server is ready!${NC}"
    break
  fi
  if [ $i -eq 30 ]; then
    echo -e "${RED}❌ HTTP server failed to start${NC}"
    kill $SERVER_PID 2>/dev/null
    exit 1
  fi
  sleep 0.5
done

# Wait for OTLP gRPC server to be ready
echo -e "${BLUE}Waiting for OTLP gRPC server (port 4317)...${NC}"
for i in {1..30}; do
  if lsof -i:47117 > /dev/null 2>&1; then
    echo -e "${GREEN}✅ OTLP gRPC server is ready!${NC}"
    break
  fi
  if [ $i -eq 30 ]; then
    echo -e "${YELLOW}⚠️  OTLP gRPC server not detected (may need project storage enabled)${NC}"
    break
  fi
  sleep 0.5
done

# Wait for MCP server to be ready
echo -e "${BLUE}Waiting for MCP server (port 47101)...${NC}"
for i in {1..30}; do
  if lsof -i:47101 > /dev/null 2>&1; then
    echo -e "${GREEN}✅ MCP server is ready!${NC}"
    break
  fi
  if [ $i -eq 30 ]; then
    echo -e "${YELLOW}⚠️  MCP server not detected${NC}"
    break
  fi
  sleep 0.5
done

# Start Vite dev server
echo -e "${GREEN}Starting Vite dev server on port 5173...${NC}"
echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}Agentreplay is running:${NC}"
echo -e "${YELLOW}  HTTP API:    http://localhost:47100${NC}"
echo -e "${YELLOW}  MCP Server:  http://localhost:47101${NC}"
echo -e "${YELLOW}  OTLP gRPC:   localhost:47117${NC}"
echo -e "${YELLOW}  UI:          http://localhost:47173${NC}"
echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

cd agentreplay-ui && npm run dev

# Cleanup on exit (when Vite is stopped with Ctrl+C)
echo -e "${BLUE}Shutting down server...${NC}"
kill $SERVER_PID 2>/dev/null

echo -e "${GREEN}✅ Stopped ChronoLake${NC}"
echo -e "${BLUE}📝 Server logs saved to: ${LOG_FILE}${NC}"
