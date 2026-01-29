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

"""Example application demonstrating Agentreplay zero-code instrumentation.

This example shows:
1. Zero-code instrumentation (just set env vars)
2. Agent context tracking
3. Streaming responses
4. Tool/function calling

Setup:
    export AGENTREPLAY_ENABLED=true
    export AGENTREPLAY_URL=http://localhost:9600
    export OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true
    export AGENTREPLAY_DEBUG=true
    export OPENAI_API_KEY=your-key-here
    
    python examples/zero_code_example.py
"""

import os
import time
from openai import OpenAI

# Import agent context (optional - only if you want agent tracking)
try:
    from agentreplay.context import AgentContext
    HAS_AGENT_CONTEXT = True
except ImportError:
    HAS_AGENT_CONTEXT = False
    print("Agent context not available - traces won't include agent_id")


def example_simple_call():
    """Example 1: Simple non-streaming call."""
    print("\n" + "="*60)
    print("Example 1: Simple Non-Streaming Call")
    print("="*60)
    
    client = OpenAI()
    
    response = client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[
            {"role": "user", "content": "What is Agentreplay?"}
        ],
        temperature=0.7,
        max_tokens=100
    )
    
    print(f"Response: {response.choices[0].message.content}")
    print(f"Tokens: {response.usage.total_tokens}")


def example_streaming_call():
    """Example 2: Streaming response."""
    print("\n" + "="*60)
    print("Example 2: Streaming Response")
    print("="*60)
    
    client = OpenAI()
    
    print("Streaming response: ", end="", flush=True)
    
    stream = client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[
            {"role": "user", "content": "Count from 1 to 5, one number per line."}
        ],
        stream=True
    )
    
    for chunk in stream:
        if chunk.choices[0].delta.content:
            print(chunk.choices[0].delta.content, end="", flush=True)
    
    print("\n\nStreaming complete! Check Agentreplay UI for full trace.")


def example_with_agent_context():
    """Example 3: Agent context tracking."""
    print("\n" + "="*60)
    print("Example 3: Agent Context Tracking")
    print("="*60)
    
    if not HAS_AGENT_CONTEXT:
        print("Skipping - agent context not available")
        return
    
    client = OpenAI()
    
    # Simulate a research agent
    with AgentContext(
        agent_id="researcher",
        session_id="demo-session-001",
        workflow_id="research-workflow",
        user_id="user-123"
    ):
        print("[Researcher Agent] Making LLM call...")
        
        response = client.chat.completions.create(
            model="gpt-4o-mini",
            messages=[
                {"role": "user", "content": "Research: What are the key features of observability tools?"}
            ],
            max_tokens=150
        )
        
        print(f"Research result: {response.choices[0].message.content[:100]}...")
    
    # Simulate a writer agent
    with AgentContext(
        agent_id="writer",
        session_id="demo-session-001",
        workflow_id="research-workflow",
        user_id="user-123"
    ):
        print("\n[Writer Agent] Making LLM call...")
        
        response = client.chat.completions.create(
            model="gpt-4o-mini",
            messages=[
                {"role": "user", "content": "Write a brief summary about observability in one sentence."}
            ],
            max_tokens=50
        )
        
        print(f"Written summary: {response.choices[0].message.content}")
    
    print("\nCheck Agentreplay UI - traces should show agent_id, session_id, etc.")


def example_tool_calling():
    """Example 4: Function/tool calling."""
    print("\n" + "="*60)
    print("Example 4: Tool/Function Calling")
    print("="*60)
    
    client = OpenAI()
    
    # Define tools
    tools = [
        {
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get the current weather for a location",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "The city and state, e.g. San Francisco, CA",
                        },
                    },
                    "required": ["location"],
                },
            },
        }
    ]
    
    print("Making LLM call with tool/function definitions...")
    
    response = client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[
            {"role": "user", "content": "What's the weather in San Francisco?"}
        ],
        tools=tools,
        tool_choice="auto"
    )
    
    # Check if model wants to call a function
    message = response.choices[0].message
    
    if message.tool_calls:
        print(f"\n✓ Model decided to call {len(message.tool_calls)} tool(s):")
        for tool_call in message.tool_calls:
            print(f"  - Function: {tool_call.function.name}")
            print(f"  - Arguments: {tool_call.function.arguments}")
        print("\nCheck Agentreplay UI - tool calls should be captured!")
    else:
        print("Model didn't call any tools")


def main():
    """Run all examples."""
    print("\n" + "="*60)
    print("Agentreplay Zero-Code Instrumentation Example")
    print("="*60)
    
    # Check if auto-instrumentation is enabled
    if os.getenv('AGENTREPLAY_ENABLED') != 'true':
        print("\n⚠️  WARNING: AGENTREPLAY_ENABLED is not set to 'true'")
        print("Set environment variable: export AGENTREPLAY_ENABLED=true")
        print("\nContinuing anyway, but traces won't be sent to Agentreplay...")
        time.sleep(2)
    
    # Check if OpenAI API key is set
    if not os.getenv('OPENAI_API_KEY'):
        print("\n❌ ERROR: OPENAI_API_KEY not set")
        print("Set your API key: export OPENAI_API_KEY=sk-...")
        return
    
    try:
        # Run examples
        example_simple_call()
        time.sleep(1)
        
        example_streaming_call()
        time.sleep(1)
        
        example_with_agent_context()
        time.sleep(1)
        
        example_tool_calling()
        
        print("\n" + "="*60)
        print("✅ All examples completed!")
        print("="*60)
        print("\nCheck Agentreplay UI at http://localhost:5173")
        print("You should see 5+ traces with:")
        print("  - Simple call")
        print("  - Streaming call (with full content)")
        print("  - Agent-tagged calls (researcher, writer)")
        print("  - Tool/function call details")
        
    except Exception as e:
        print(f"\n❌ Error: {e}")
        import traceback
        traceback.print_exc()


if __name__ == "__main__":
    main()
