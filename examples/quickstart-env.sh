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

#
# Agentreplay Environment-Based Tracing - Quick Start
#
# This script demonstrates how to use Agentreplay with environment variables
# for automatic tracing across all frameworks.
#

set -e

echo "=============================================================================="
echo "🚀 Agentreplay Environment-Based Auto-Instrumentation"
echo "=============================================================================="
echo ""

# Set Agentreplay environment variables
export AGENTREPLAY_ENABLED=1
export AGENTREPLAY_URL=${AGENTREPLAY_URL:-http://localhost:8080}
export AGENTREPLAY_TENANT_ID=${AGENTREPLAY_TENANT_ID:-1}
export AGENTREPLAY_PROJECT_ID=${AGENTREPLAY_PROJECT_ID:-0}
export AGENTREPLAY_SERVICE_NAME=${AGENTREPLAY_SERVICE_NAME:-quickstart-env}
export AGENTREPLAY_FRAMEWORKS=${AGENTREPLAY_FRAMEWORKS:-all}
export AGENTREPLAY_SAMPLE_RATE=${AGENTREPLAY_SAMPLE_RATE:-1.0}
export AGENTREPLAY_CAPTURE_CONTENT=${AGENTREPLAY_CAPTURE_CONTENT:-true}
export AGENTREPLAY_LOG_LEVEL=${AGENTREPLAY_LOG_LEVEL:-INFO}

echo "Environment Configuration:"
echo "  AGENTREPLAY_ENABLED=$AGENTREPLAY_ENABLED"
echo "  AGENTREPLAY_URL=$AGENTREPLAY_URL"
echo "  AGENTREPLAY_TENANT_ID=$AGENTREPLAY_TENANT_ID"
echo "  AGENTREPLAY_PROJECT_ID=$AGENTREPLAY_PROJECT_ID"
echo "  AGENTREPLAY_SERVICE_NAME=$AGENTREPLAY_SERVICE_NAME"
echo "  AGENTREPLAY_FRAMEWORKS=$AGENTREPLAY_FRAMEWORKS"
echo "  AGENTREPLAY_SAMPLE_RATE=$AGENTREPLAY_SAMPLE_RATE"
echo "  AGENTREPLAY_CAPTURE_CONTENT=$AGENTREPLAY_CAPTURE_CONTENT"
echo ""

# Check if API keys are set
if [ -z "$OPENAI_API_KEY" ] && [ -z "$ANTHROPIC_API_KEY" ]; then
    echo "⚠️  Warning: No API keys set"
    echo "   export OPENAI_API_KEY=sk-..."
    echo "   export ANTHROPIC_API_KEY=sk-ant-..."
    echo ""
fi

# Check if Python virtual environment exists
if [ -d "venv" ]; then
    echo "✓ Using virtual environment: venv"
    source venv/bin/activate
else
    echo "⚠️  No virtual environment found. Using system Python."
fi

echo ""
echo "=============================================================================="
echo "Running Environment-Based Tracing Example"
echo "=============================================================================="
echo ""

# Run the example
python env_based_tracing_example.py

echo ""
echo "=============================================================================="
echo "✅ Complete! View traces at: $AGENTREPLAY_URL"
echo "=============================================================================="
echo ""
echo "Usage in your own scripts:"
echo ""
echo "  # 1. Set environment variables:"
echo "  export AGENTREPLAY_ENABLED=1"
echo "  export AGENTREPLAY_URL=http://localhost:8080"
echo "  export AGENTREPLAY_SERVICE_NAME=my-app"
echo ""
echo "  # 2. In your Python script:"
echo "  import agentreplay.env_init  # Auto-instruments based on env vars"
echo "  from openai import OpenAI   # Now traced automatically!"
echo ""
echo "  # 3. Run your script:"
echo "  python your_script.py"
echo ""
echo "That's it! All LLM calls are automatically traced."
echo ""
