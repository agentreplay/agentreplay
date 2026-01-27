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

# Helper script to monitor Flowtrace server logs

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}📝 Flowtrace Log Viewer${NC}"
echo "=================================="

# Find the most recent log file
LATEST_LOG=$(ls -t logs/flowtrace-server-*.log 2>/dev/null | head -1)

if [ -z "$LATEST_LOG" ]; then
    echo -e "${YELLOW}No log files found in logs/ directory${NC}"
    echo "Start the server first with: sh start-web.sh"
    exit 1
fi

echo -e "${GREEN}Viewing: ${LATEST_LOG}${NC}"
echo ""

# Show different views based on argument
case "$1" in
    "otel")
        echo -e "${BLUE}🔵 Filtering for OTEL INGEST logs...${NC}"
        grep "OTEL INGEST" "$LATEST_LOG" | tail -50
        ;;
    "errors")
        echo -e "${BLUE}❌ Filtering for errors...${NC}"
        grep -i "error\|failed\|panic" "$LATEST_LOG" | tail -50
        ;;
    "projects")
        echo -e "${BLUE}📁 Filtering for project operations...${NC}"
        grep -E "project_[0-9]+|project=|Opening Flowtrace" "$LATEST_LOG" | tail -50
        ;;
    "live")
        echo -e "${BLUE}📡 Live tail (Ctrl+C to stop)...${NC}"
        tail -f "$LATEST_LOG"
        ;;
    "all"|"")
        echo -e "${BLUE}📜 Last 100 lines...${NC}"
        tail -100 "$LATEST_LOG"
        ;;
    *)
        echo "Usage: $0 [otel|errors|projects|live|all]"
        echo ""
        echo "Examples:"
        echo "  $0          - Show last 100 lines"
        echo "  $0 otel     - Show OTEL ingestion logs"
        echo "  $0 errors   - Show errors only"
        echo "  $0 projects - Show project operations"
        echo "  $0 live     - Live tail"
        exit 1
        ;;
esac
