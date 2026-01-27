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
Flowtrace Auto-Instrumentation Example
Demonstrates zero-config tracing for OpenAI and LangGraph

This example shows how to enable automatic tracing with just 2 lines of code.
"""
import os
from openai import OpenAI
from langgraph.graph import StateGraph, END
from typing import TypedDict, Annotated
import operator

# ============================================================================
# STEP 1: Enable Auto-Instrumentation (2 lines!)
# ============================================================================
from flowtrace import Flowtrace

# Initialize Flowtrace
ft = Flowtrace(
    api_url=os.getenv("FLOWTRACE_API_URL", "http://localhost:9600"),
    project="auto-instrumented-example",
    tags=["example", "auto-trace"]
)

# That's it! All OpenAI and LangGraph calls are now traced automatically.


# ============================================================================
# Define Agent State
# ============================================================================
class AgentState(TypedDict):
    messages: Annotated[list[dict], operator.add]
    iterations: int


# ============================================================================
# Define Agent Node
# ============================================================================
def call_model(state: AgentState):
    """LLM call - automatically traced by Flowtrace"""
    client = OpenAI()
    
    messages = state["messages"]
    
    # This call is automatically captured with:
    # - Full conversation history
    # - Token usage
    # - Cost calculation
    # - Response time
    response = client.chat.completions.create(
        model="gpt-4o-mini",
        messages=messages,
        temperature=0.7,
    )
    
    assistant_message = {
        "role": "assistant",
        "content": response.choices[0].message.content
    }
    
    return {
        "messages": [assistant_message],
        "iterations": state["iterations"] + 1
    }


def should_continue(state: AgentState):
    """Routing logic"""
    if state["iterations"] >= 3:
        return "end"
    return "continue"


# ============================================================================
# Build LangGraph - automatically traced!
# ============================================================================
workflow = StateGraph(AgentState)

# Add nodes
workflow.add_node("agent", call_model)

# Add edges
workflow.set_entry_point("agent")
workflow.add_conditional_edges(
    "agent",
    should_continue,
    {
        "continue": "agent",
        "end": END
    }
)

# Compile graph
app = workflow.compile()


# ============================================================================
# Run Agent
# ============================================================================
if __name__ == "__main__":
    print("🚀 Running auto-instrumented agent...")
    print("📊 View traces at: http://localhost:5173/projects/14094/traces\n")
    
    # Run agent
    result = app.invoke({
        "messages": [{
            "role": "user",
            "content": "Explain quantum computing in 2 sentences"
        }],
        "iterations": 0
    })
    
    print("\n✅ Done! Check Flowtrace UI for:")
    print("  - Full conversation history")
    print("  - Token usage per call")
    print("  - Total cost breakdown")
    print("  - Response times")
    print("  - LangGraph execution flow")
    print(f"\n💬 Final response: {result['messages'][-1]['content']}")
