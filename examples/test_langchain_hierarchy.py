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

"""
Test Flowtrace's LangChain callback handler with hierarchical traces.

This creates synthetic spans to test parent-child relationships without
needing actual LLM credentials.
"""

import time
import os
os.environ["FLOWTRACE_PROJECT_ID"] = "11635"
os.environ["OTEL_EXPORTER_OTLP_ENDPOINT"] = "http://localhost:4317"

# Initialize Flowtrace instrumentation FIRST
import flowtrace.auto_instrument

from flowtrace.langchain_tracer import FlowtraceCallbackHandler
from uuid import uuid4
from langchain_core.outputs import LLMResult, Generation

print("🚀 Testing Flowtrace LangChain Tracer")
print("=" * 60)

# Create callback handler
handler = FlowtraceCallbackHandler()

# Simulate a chain execution with nested LLM calls
print("\n📊 Simulating: Chain → LLM calls (parent-child)")
print("-" * 60)

# Chain starts (root span)
chain_run_id = uuid4()
handler.on_chain_start(
    serialized={"name": "MyChain", "id": ["langchain", "chains", "MyChain"]},
    inputs={"question": "What is machine learning?"},
    run_id=chain_run_id,
    parent_run_id=None
)
print(f"✓ Chain started: {chain_run_id}")

time.sleep(0.1)

# LLM call within chain (child span)
llm_run_id = uuid4()
handler.on_llm_start(
    serialized={"name": "gpt-4"},
    prompts=["Explain machine learning in simple terms"],
    run_id=llm_run_id,
    parent_run_id=chain_run_id  # Parent is the chain
)
print(f"  ✓ LLM call started (child of chain): {llm_run_id}")

time.sleep(0.2)

# LLM completes
handler.on_llm_end(
    response=LLMResult(
        generations=[[Generation(text="Machine learning is...")]],
        llm_output={"token_usage": {"total_tokens": 50, "prompt_tokens": 10, "completion_tokens": 40}}
    ),
    run_id=llm_run_id,
    parent_run_id=chain_run_id
)
print(f"  ✓ LLM call completed")

time.sleep(0.1)

# Chain completes
handler.on_chain_end(
    outputs={"answer": "Machine learning is..."},
    run_id=chain_run_id,
    parent_run_id=None
)
print(f"✓ Chain completed")

# Simulate tool call within chain
print("\n📊 Simulating: Chain → Tool call → LLM")
print("-" * 60)

# Chain starts
chain2_run_id = uuid4()
handler.on_chain_start(
    serialized={"name": "ToolChain", "id": ["langchain", "chains", "ToolChain"]},
    inputs={"task": "Get weather and summarize"},
    run_id=chain2_run_id,
    parent_run_id=None
)
print(f"✓ Chain started: {chain2_run_id}")

time.sleep(0.1)

# Tool call within chain
tool_run_id = uuid4()
handler.on_tool_start(
    serialized={"name": "get_weather", "description": "Get weather for a location"},
    input_str="San Francisco",
    run_id=tool_run_id,
    parent_run_id=chain2_run_id  # Parent is the chain
)
print(f"  ✓ Tool call started (child of chain): {tool_run_id}")

time.sleep(0.15)

handler.on_tool_end(
    output="Sunny, 72°F",
    run_id=tool_run_id,
    parent_run_id=chain2_run_id
)
print(f"  ✓ Tool call completed")

time.sleep(0.1)

# LLM call to synthesize tool result
llm2_run_id = uuid4()
handler.on_llm_start(
    serialized={"name": "gpt-4"},
    prompts=["Summarize this weather: Sunny, 72°F"],
    run_id=llm2_run_id,
    parent_run_id=chain2_run_id  # Also child of chain
)
print(f"  ✓ LLM call started (child of chain): {llm2_run_id}")

time.sleep(0.2)

handler.on_llm_end(
    response=LLMResult(
        generations=[[Generation(text="The weather is nice!")]],
        llm_output={"token_usage": {"total_tokens": 30}}
    ),
    run_id=llm2_run_id,
    parent_run_id=chain2_run_id
)
print(f"  ✓ LLM call completed")

time.sleep(0.1)

# Chain completes
handler.on_chain_end(
    outputs={"result": "The weather is nice!"},
    run_id=chain2_run_id,
    parent_run_id=None
)
print(f"✓ Chain completed")

print("\n" + "=" * 60)
print("✅ Test complete! Spans have been sent to OTLP.")
print("   Check Flowtrace UI at http://localhost:5173")
print("   Expected hierarchy:")
print("   1. MyChain → LLM (gpt-4)")
print("   2. ToolChain → Tool (get_weather) + LLM (gpt-4)")
print("=" * 60)

# Force flush - import the provider and flush
print("\n⏳ Flushing spans to OTLP...")
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry import trace as trace_api

provider = trace_api.get_tracer_provider()
if hasattr(provider, 'force_flush'):
    provider.force_flush()
time.sleep(2)
print("✅ Done!")
