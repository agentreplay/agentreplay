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
# Flowtrace Environment-Based Tracing - Quick Start
#
# This script demonstrates how to use Flowtrace with environment variables
# for automatic tracing across all frameworks.
#

set -e

echo "=============================================================================="
echo "🚀 Flowtrace Environment-Based Auto-Instrumentation"
echo "=============================================================================="
echo ""

# Set Flowtrace environment variables
export FLOWTRACE_ENABLED=1
export FLOWTRACE_URL=${FLOWTRACE_URL:-http://localhost:8080}
export FLOWTRACE_TENANT_ID=${FLOWTRACE_TENANT_ID:-1}
export FLOWTRACE_PROJECT_ID=${FLOWTRACE_PROJECT_ID:-0}
export FLOWTRACE_SERVICE_NAME=${FLOWTRACE_SERVICE_NAME:-quickstart-env}
export FLOWTRACE_FRAMEWORKS=${FLOWTRACE_FRAMEWORKS:-all}
export FLOWTRACE_SAMPLE_RATE=${FLOWTRACE_SAMPLE_RATE:-1.0}
export FLOWTRACE_CAPTURE_CONTENT=${FLOWTRACE_CAPTURE_CONTENT:-true}
export FLOWTRACE_LOG_LEVEL=${FLOWTRACE_LOG_LEVEL:-INFO}

echo "Environment Configuration:"
echo "  FLOWTRACE_ENABLED=$FLOWTRACE_ENABLED"
echo "  FLOWTRACE_URL=$FLOWTRACE_URL"
echo "  FLOWTRACE_TENANT_ID=$FLOWTRACE_TENANT_ID"
echo "  FLOWTRACE_PROJECT_ID=$FLOWTRACE_PROJECT_ID"
echo "  FLOWTRACE_SERVICE_NAME=$FLOWTRACE_SERVICE_NAME"
echo "  FLOWTRACE_FRAMEWORKS=$FLOWTRACE_FRAMEWORKS"
echo "  FLOWTRACE_SAMPLE_RATE=$FLOWTRACE_SAMPLE_RATE"
echo "  FLOWTRACE_CAPTURE_CONTENT=$FLOWTRACE_CAPTURE_CONTENT"
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
echo "✅ Complete! View traces at: $FLOWTRACE_URL"
echo "=============================================================================="
echo ""
echo "Usage in your own scripts:"
echo ""
echo "  # 1. Set environment variables:"
echo "  export FLOWTRACE_ENABLED=1"
echo "  export FLOWTRACE_URL=http://localhost:8080"
echo "  export FLOWTRACE_SERVICE_NAME=my-app"
echo ""
echo "  # 2. In your Python script:"
echo "  import flowtrace.env_init  # Auto-instruments based on env vars"
echo "  from openai import OpenAI   # Now traced automatically!"
echo ""
echo "  # 3. Run your script:"
echo "  python your_script.py"
echo ""
echo "That's it! All LLM calls are automatically traced."
echo ""
