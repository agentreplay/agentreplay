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
LangGraph multi-agent example with ZERO CODE CHANGES.

✨ True Zero-Code Tracing ✨
Just set environment variables (or use .env file) and run - that's it!

Setup:
    1. Install: pip install flowtrace
    2. Create .env file with:
       FLOWTRACE_ENABLED=true
       FLOWTRACE_PROJECT_ID=11635
       FLOWTRACE_OTLP_ENDPOINT=localhost:4317
       AZURE_OPENAI_API_KEY=your-key
       AZURE_OPENAI_ENDPOINT=your-endpoint
       AZURE_OPENAI_DEPLOYMENT=gpt-4.1
       TAVILY_API_KEY=your-tavily-key
    3. Run: python langgraph_agent_with_tools.py
    
That's it! No imports, no manual init, no flush - just works.
"""

import os
from dotenv import load_dotenv
load_dotenv()

# ✨ EXPLICIT FLOWTRACE INITIALIZATION ✨
# (The .pth file approach requires proper installation; explicit init works reliably)
import flowtrace
flowtrace.init_otel_instrumentation(
    service_name=os.getenv("FLOWTRACE_SERVICE_NAME", "langgraph-demo"),
    otlp_endpoint=os.getenv("FLOWTRACE_OTLP_ENDPOINT", "localhost:4317"),
    project_id=int(os.getenv("FLOWTRACE_PROJECT_ID", "27986")),
    tenant_id=int(os.getenv("FLOWTRACE_TENANT_ID", "1")),
    debug=True  # Show initialization logs
)
print("[Flowtrace] ✓ Initialized")

from typing import Annotated, TypedDict
from langchain_openai import AzureChatOpenAI
from langchain_community.tools.tavily_search import TavilySearchResults
from langgraph.graph import StateGraph, START, END
from langgraph.graph.message import add_messages
from langgraph.prebuilt import ToolNode, tools_condition

print("=" * 60)
print("LangGraph Multi-Agent with Tools - ZERO CODE")
print("=" * 60)


class State(TypedDict):
    """State for the agent system."""
    messages: Annotated[list, add_messages]


def research_agent(state: State):
    """Research agent with web search capability."""
    print(f"\n[Research Agent] Processing query...")
    
    llm = AzureChatOpenAI(
        azure_endpoint=os.getenv("AZURE_OPENAI_ENDPOINT"),
        azure_deployment=os.getenv("AZURE_OPENAI_DEPLOYMENT", "gpt-4.1"),
        api_version=os.getenv("AZURE_OPENAI_API_VERSION", "2024-12-01-preview"),
        temperature=0
    )
    tools = [TavilySearchResults(max_results=3)]
    llm_with_tools = llm.bind_tools(tools)
    
    response = llm_with_tools.invoke(state["messages"])
    
    print(f"[Research Agent] Response type: {response.__class__.__name__}")
    if hasattr(response, 'tool_calls') and response.tool_calls:
        print(f"[Research Agent] Calling {len(response.tool_calls)} tool(s)")
    
    return {"messages": [response]}


def writer_agent(state: State):
    """Writer agent that synthesizes information."""
    print(f"\n[Writer Agent] Synthesizing information...")
    
    llm = AzureChatOpenAI(
        azure_endpoint=os.getenv("AZURE_OPENAI_ENDPOINT"),
        azure_deployment=os.getenv("AZURE_OPENAI_DEPLOYMENT", "gpt-4.1"),
        api_version=os.getenv("AZURE_OPENAI_API_VERSION", "2024-12-01-preview"),
        temperature=0.7
    )
    
    # Get the last few messages for context
    recent_messages = state["messages"][-5:] if len(state["messages"]) > 5 else state["messages"]
    
    response = llm.invoke(recent_messages)
    
    print(f"[Writer Agent] Generated {len(response.content)} characters")
    
    return {"messages": [response]}


# Build multi-agent graph
print("\n1. Building multi-agent workflow...")
workflow = StateGraph(State)

# Add nodes
workflow.add_node("researcher", research_agent)
workflow.add_node("tools", ToolNode([TavilySearchResults(max_results=3)]))
workflow.add_node("writer", writer_agent)

# Add edges
workflow.add_edge(START, "researcher")
workflow.add_conditional_edges(
    "researcher",
    tools_condition,
    {
        "tools": "tools",
        END: "writer"
    }
)
workflow.add_edge("tools", "researcher")
workflow.add_edge("writer", END)

graph = workflow.compile()
print("   ✓ Multi-agent workflow compiled")


if __name__ == "__main__":
    # Check API keys
    if not os.getenv('AZURE_OPENAI_API_KEY'):
        print("\n❌ Error: AZURE_OPENAI_API_KEY not set")
        exit(1)
    
    if not os.getenv('AZURE_OPENAI_ENDPOINT'):
        print("\n❌ Error: AZURE_OPENAI_ENDPOINT not set")
        exit(1)
    
    if not os.getenv('TAVILY_API_KEY'):
        print("\n⚠️  Warning: TAVILY_API_KEY not set")
        print("   Tool calls will fail. Get free key from https://tavily.com")
        print("   Continuing anyway...")
    
    print("\n2. Running multi-agent workflow...")
    print("   This will:")
    print("   - Research agent searches the web")
    print("   - Tools execute the search")
    print("   - Writer agent synthesizes results")
    
    try:
        result = graph.invoke({
            "messages": [("user", "What are the latest developments in AI agents in 2024?")]
        })
        
        print("\n3. Final Result:")
        print("-" * 60)
        print(result["messages"][-1].content)
        print("-" * 60)
        
        print("\n✅ Complete! Check Flowtrace UI:")
        print("   http://localhost:5173")
        print("\nYou should see:")
        print("   - 2 agents: researcher → writer")
        print("   - Tool calls (web search)")
        print("   - Multiple LLM calls")
        print("   - Full conversation flow")
        print("   - Token usage per agent")
        print("\n   All traced automatically - no manual flush needed!")
    
    except Exception as e:
        print(f"\n❌ Error: {e}")
        print("\nCommon issues:")
        print("   - Missing TAVILY_API_KEY (tool calls fail)")
        print("   - Network issues")
        print("   - Rate limits")
