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

# Setup script for Agentreplay zero-code instrumentation testing

set -e

echo "=========================================="
echo "Agentreplay Zero-Code Instrumentation Setup"
echo "=========================================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check Python version
echo "1. Checking Python version..."
PYTHON_VERSION=$(python3 --version 2>&1 | awk '{print $2}')
echo "   Python $PYTHON_VERSION"

# Check if we're in the right directory
if [ ! -f "pyproject.toml" ]; then
    echo -e "${RED}✗ Error: pyproject.toml not found${NC}"
    echo "  Run this from: sdks/python/"
    exit 1
fi

# Install SDK in development mode
echo ""
echo "2. Installing Agentreplay SDK..."
pip install -e . --quiet
echo -e "${GREEN}✓ SDK installed${NC}"

# Install test dependencies
echo ""
echo "3. Installing test dependencies..."
pip install langchain-openai langgraph --quiet 2>/dev/null || {
    echo -e "${YELLOW}⚠ Some dependencies failed to install${NC}"
    echo "  Continuing anyway..."
}
echo -e "${GREEN}✓ Dependencies installed${NC}"

# Check for .pth file
echo ""
echo "4. Checking for .pth file..."
SITE_PACKAGES=$(python3 -c "import site; print(site.getsitepackages()[0])")
PTH_FILE="$SITE_PACKAGES/agentreplay-init.pth"

if [ -f "$PTH_FILE" ]; then
    echo -e "${GREEN}✓ .pth file found: $PTH_FILE${NC}"
else
    echo -e "${YELLOW}⚠ .pth file not found${NC}"
    echo "  Expected: $PTH_FILE"
    echo "  The .pth auto-init may not work"
fi

# Check if backend is running
echo ""
echo "5. Checking Agentreplay backend..."
if curl -s http://localhost:9600/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Backend is running${NC}"
    curl -s http://localhost:9600/health | python3 -m json.tool 2>/dev/null || echo ""
else
    echo -e "${YELLOW}⚠ Backend not running${NC}"
    echo "  Start it: cd ../.. && ./start-web.sh"
fi

# Check environment variables
echo ""
echo "6. Checking environment variables..."

check_env_var() {
    local var_name=$1
    local required=$2
    
    if [ -n "${!var_name}" ]; then
        echo -e "${GREEN}✓ $var_name=${!var_name}${NC}"
        return 0
    else
        if [ "$required" = "true" ]; then
            echo -e "${RED}✗ $var_name not set${NC}"
            return 1
        else
            echo -e "${YELLOW}⚠ $var_name not set (optional)${NC}"
            return 0
        fi
    fi
}

ALL_VARS_SET=true

check_env_var "AGENTREPLAY_ENABLED" "true" || ALL_VARS_SET=false
check_env_var "AGENTREPLAY_URL" "false"
check_env_var "OPENAI_API_KEY" "false"
check_env_var "OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT" "false"

# Create .env file if needed
if [ "$ALL_VARS_SET" = "false" ] || [ ! -f ".env" ]; then
    echo ""
    echo "7. Creating .env file..."
    cat > .env << EOF
# Agentreplay Configuration
AGENTREPLAY_ENABLED=true
AGENTREPLAY_URL=http://localhost:9600
AGENTREPLAY_TENANT_ID=1
AGENTREPLAY_PROJECT_ID=0
AGENTREPLAY_DEBUG=true

# OpenTelemetry
OTEL_SERVICE_NAME=agentreplay-test
OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true

# LLM API Keys (set these!)
OPENAI_API_KEY=${OPENAI_API_KEY:-your-key-here}
TAVILY_API_KEY=${TAVILY_API_KEY:-your-key-here}

# Privacy Controls
AGENTREPLAY_MAX_CONTENT_LENGTH=10000
AGENTREPLAY_MAX_MESSAGES=0
AGENTREPLAY_TRUNCATE_CONTENT=false
EOF
    echo -e "${GREEN}✓ .env file created${NC}"
    echo "  Edit .env and set your API keys"
fi

# Final instructions
echo ""
echo "=========================================="
echo "Setup Complete!"
echo "=========================================="
echo ""
echo "Next steps:"
echo ""
echo "1. Load environment variables:"
echo -e "   ${GREEN}source .env${NC}"
echo ""
echo "2. Run tests:"
echo -e "   ${GREEN}cd ../../examples && python3 test_zero_code_instrumentation.py${NC}"
echo ""
echo "3. Run examples:"
echo -e "   ${GREEN}cd ../../examples && python3 pure_langgraph_example.py${NC}"
echo ""
echo "4. Check UI:"
echo -e "   ${GREEN}http://localhost:5173${NC}"
echo ""
echo "Notes:"
echo "- Set OPENAI_API_KEY in .env for real tests"
echo "- Start backend with: cd ../.. && ./start-web.sh"
echo "- Enable debug: export AGENTREPLAY_DEBUG=true"
echo ""
