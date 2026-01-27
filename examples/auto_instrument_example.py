#!/usr/bin/env python3

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

"""Example: Auto-instrumentation with Flowtrace.

This example demonstrates zero-code observability for AI agents.
Just call auto_instrument() and all your LLM calls are automatically traced!
"""

import os
import time
from flowtrace import auto_instrument
from config import (
    get_auto_instrument_config,
    FLOWTRACE_URL,
    OPENAI_API_KEY,
    ANTHROPIC_API_KEY,
)

# ============================================================================
# Step 1: Auto-instrument (ONE LINE!)
# ============================================================================

config = get_auto_instrument_config()
config["frameworks"] = ["openai", "anthropic", "langgraph"]  # Or None for all
auto_instrument(**config)

print("✓ Auto-instrumentation enabled!")
print("=" * 60)

# ============================================================================
# Step 2: Use LLM libraries normally - they're automatically traced!
# ============================================================================

def example_openai():
    """OpenAI calls are automatically traced."""
    print("\n📝 OpenAI Example")
    print("-" * 60)
    
    try:
        from openai import OpenAI
        
        client = OpenAI(api_key=OPENAI_API_KEY)
        
        print("Making OpenAI API call...")
        response = client.chat.completions.create(
            model="gpt-4o-mini",
            messages=[
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "What is Flowtrace?"}
            ],
            temperature=0.7,
            max_tokens=100,
        )
        
        print(f"✓ Response: {response.choices[0].message.content[:100]}...")
        print("✓ Automatically traced in Flowtrace!")
        
    except ImportError:
        print("⚠ OpenAI SDK not installed (pip install openai)")
    except Exception as e:
        print(f"⚠ Error: {e}")


def example_anthropic():
    """Anthropic calls are automatically traced."""
    print("\n🤖 Anthropic Example")
    print("-" * 60)
    
    try:
        import anthropic
        
        client = anthropic.Anthropic(api_key=ANTHROPIC_API_KEY)
        
        print("Making Anthropic API call...")
        response = client.messages.create(
            model="claude-3-5-sonnet-20241022",
            max_tokens=100,
            messages=[
                {"role": "user", "content": "What is agent observability?"}
            ]
        )
        
        print(f"✓ Response: {response.content[0].text[:100]}...")
        print("✓ Automatically traced in Flowtrace!")
        
    except ImportError:
        print("⚠ Anthropic SDK not installed (pip install anthropic)")
    except Exception as e:
        print(f"⚠ Error: {e}")


def example_langgraph():
    """LangGraph workflows are automatically traced."""
    print("\n🔗 LangGraph Example")
    print("-" * 60)
    
    try:
        from langgraph.graph import StateGraph, END
        from typing import TypedDict
        
        class State(TypedDict):
            message: str
            count: int
        
        def process_node(state: State) -> State:
            """Simulate a workflow node."""
            return {
                "message": state["message"] + " processed",
                "count": state["count"] + 1
            }
        
        # Build graph
        workflow = StateGraph(State)
        workflow.add_node("process", process_node)
        workflow.set_entry_point("process")
        workflow.add_edge("process", END)
        
        app = workflow.compile()
        
        print("Running LangGraph workflow...")
        result = app.invoke({
            "message": "Hello",
            "count": 0
        })
        
        print(f"✓ Result: {result}")
        print("✓ Workflow automatically traced in Flowtrace!")
        
    except ImportError:
        print("⚠ LangGraph not installed (pip install langgraph)")
    except Exception as e:
        print(f"⚠ Error: {e}")


def main():
    """Run examples."""
    print("\n" + "=" * 60)
    print("🚀 Flowtrace Auto-Instrumentation Demo")
    print("=" * 60)
    print("""
This demo shows zero-code observability for AI agents.
All LLM calls are automatically traced - no manual tracking needed!

Key Features:
✓ Automatic OpenAI/Anthropic/LangChain tracing
✓ Full OpenTelemetry GenAI semantic conventions
✓ Token usage & cost tracking
✓ Latency breakdown
✓ Prompt & response capture
✓ Error tracking

View your traces at: {FLOWTRACE_URL}
    """)
    
    # Run examples
    example_openai()
    
    time.sleep(1)
    example_anthropic()
    
    time.sleep(1)
    example_langgraph()
    
    print("\n" + "=" * 60)
    print("✅ Demo Complete!")
    print("=" * 60)
    print(f"""
All traces have been automatically captured and sent to Flowtrace.

View them at: {FLOWTRACE_URL}

Try the new analytics endpoints:
- Latency breakdown: /api/v1/analytics/latency-breakdown?session_id=<id>
- Cost analysis: /api/v1/analytics/cost-breakdown?session_id=<id>

No code changes required! Just one line: auto_instrument()
    """)


if __name__ == "__main__":
    # Check environment
    if not OPENAI_API_KEY and not ANTHROPIC_API_KEY:
        print("⚠️  Set OPENAI_API_KEY or ANTHROPIC_API_KEY environment variable")
        print("   export OPENAI_API_KEY=sk-...")
    
    main()
