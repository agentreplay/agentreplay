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

# Quick start script for testing auto-instrumentation

set -e

echo "🚀 Flowtrace Auto-Instrumentation Quick Start"
echo "=============================================="
echo ""

# Check if we're in the right directory
if [ ! -f "test_auto_instrumentation.py" ]; then
    echo "❌ Error: Please run this script from the examples directory"
    echo "   cd /Users/sushanth/chronolake/examples"
    exit 1
fi

# Check if Flowtrace server is running
echo "📡 Checking Flowtrace server..."
if curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "✅ Flowtrace server is running at http://localhost:8080"
else
    echo "⚠️  Flowtrace server is not running"
    echo "   Start it with: cargo run --bin flowtrace-server"
    echo ""
    read -p "Continue anyway? (y/n) " -n 1 -r
    echo ""
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Check for API keys
echo ""
echo "🔑 Checking API keys..."
if [ -n "$OPENAI_API_KEY" ]; then
    echo "✅ OPENAI_API_KEY is set"
else
    echo "⚠️  OPENAI_API_KEY is not set (some tests will be skipped)"
fi

if [ -n "$ANTHROPIC_API_KEY" ]; then
    echo "✅ ANTHROPIC_API_KEY is set"
else
    echo "⚠️  ANTHROPIC_API_KEY is not set (some tests will be skipped)"
fi

# Check if SDK is installed
echo ""
echo "📦 Checking Flowtrace SDK..."
if python3 -c "from flowtrace import auto_instrument" 2>/dev/null; then
    echo "✅ Flowtrace SDK is installed"
else
    echo "⚠️  Flowtrace SDK not found. Installing..."
    cd ../sdks/python
    pip install -e . --quiet
    cd ../../examples
    echo "✅ Flowtrace SDK installed"
fi

# Check OpenTelemetry dependencies
echo ""
echo "📦 Checking OpenTelemetry dependencies..."
if python3 -c "import opentelemetry.trace" 2>/dev/null; then
    echo "✅ OpenTelemetry is installed"
else
    echo "⚠️  OpenTelemetry not found. Installing..."
    pip install opentelemetry-api opentelemetry-sdk opentelemetry-semantic-conventions --quiet
    echo "✅ OpenTelemetry installed"
fi

# Check LLM SDKs
echo ""
echo "📦 Checking LLM SDKs..."
if python3 -c "import openai" 2>/dev/null; then
    echo "✅ OpenAI SDK is installed"
else
    echo "⚠️  OpenAI SDK not found"
    echo "   Install with: pip install openai"
fi

if python3 -c "import anthropic" 2>/dev/null; then
    echo "✅ Anthropic SDK is installed"
else
    echo "⚠️  Anthropic SDK not found"
    echo "   Install with: pip install anthropic"
fi

# Run the test
echo ""
echo "🧪 Running auto-instrumentation tests..."
echo "=========================================="
echo ""

python3 test_auto_instrumentation.py

# Check exit code
if [ $? -eq 0 ]; then
    echo ""
    echo "🎉 Tests completed successfully!"
    echo ""
    echo "📊 Next steps:"
    echo "   1. View traces at: http://localhost:8080"
    echo "   2. Try the full demo: python3 auto_instrument_example.py"
    echo "   3. Test LangGraph: python3 langgraph_example.py --auto"
    echo ""
else
    echo ""
    echo "⚠️  Some tests failed. Check the output above."
    echo ""
    echo "💡 To run tests with real APIs:"
    echo "   export OPENAI_API_KEY=sk-..."
    echo "   export ANTHROPIC_API_KEY=sk-ant-..."
    echo "   ./quickstart.sh"
    echo ""
fi
