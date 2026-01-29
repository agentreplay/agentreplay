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
Agentreplay + LangGraph Example (Mocked)
Auto-instrumented multi-agent system with web search (Mocked)

Project ID: 31696 (LangGraph tracking)
"""
import os
# from dotenv import load_dotenv
# load_dotenv()

# Agentreplay auto-instrumentation
import agentreplay
agentreplay.init_otel_instrumentation(
    service_name="agentreplay-langgraph-demo",
    otlp_endpoint=os.getenv("AGENTREPLAY_OTLP_ENDPOINT", "127.0.0.1:4317"),
    project_id=35455,  # Updated Project ID
    tenant_id=int(os.getenv("AGENTREPLAY_TENANT_ID", "1")),
    debug=True
)
print("✅ Agentreplay initialized for LangGraph")

from typing import Annotated, TypedDict
from langchain_core.messages import AIMessage
from langchain_core.language_models import FakeListChatModel
from langgraph.graph import StateGraph, START, END
from langgraph.graph.message import add_messages
from langgraph.prebuilt import ToolNode

# Define tools (Mocked)
class MockTool:
    name = "tavily_search_results_json"
    description = "A search engine optimized for comprehensive, accurate, and trusted results."
    
    def invoke(self, input):
        return [{"url": "https://example.com", "content": "AI agents are evolving rapidly in 2024."}]

class AgentState(TypedDict):
    """Multi-agent state"""
    messages: Annotated[list, add_messages]
    iterations: int


def research_agent(state: AgentState):
    """Research agent with web search"""
    print(f"\n🔍 [Research Agent] Processing (iteration {state['iterations']})...")
    
    # Mock LLM with FakeListChatModel to trigger instrumentation (hopefully)
    # Note: FakeListChatModel might not fully support tool binding in a way that triggers tool_calls cleanly for auto-instrumentation if it checks specifically for OpenAI/Anthropic classes.
    # But let's try.
    
    if state['iterations'] == 0:
        # We manually construct the response but we want to wrap it in an invoke call if possible.
        # Or just simulate it. 
        # Since we can't easily make FakeListChatModel produce tool_calls via invoke without setup,
        # we will just return the message but we'll create a dummy span using agentreplay manual instrumentation if needed,
        # OR we trust that LangGraph node execution itself creates a span.
        
        # If LangGraph creates a span for "researcher" node, its name will be "researcher".
        # The user issue is about "Unknown Agent (1)" which implies agent_id fallback.
        pass

    # Let's try to use a fake LLM to generate a span
    llm = FakeListChatModel(responses=["dummy"])
    try:
        llm.invoke("test")
    except:
        pass

    if state['iterations'] == 0:
        # First iteration: Call tool
        response = AIMessage(
            content="", 
            tool_calls=[{"name": "tavily_search_results_json", "args": {"query": "AI agent frameworks 2024"}, "id": "call_1"}]
        )
    else:
        # Second iteration: Summarize
        response = AIMessage(content="I have found information about AI agents.")
    
    return {
        "messages": [response],
        "iterations": state["iterations"] + 1
    }


def analyst_agent(state: AgentState):
    """Analyst agent that synthesizes findings"""
    print(f"\n📊 [Analyst] Synthesizing information...")
    
    response = AIMessage(content="Here is the final report on AI agents in 2024: They are great.")
    
    return {"messages": [response]}


def should_continue(state: AgentState):
    """Router: continue research or move to analyst"""
    if state["iterations"] >= 2:
        return "analyst"
    
    # Check if last message has tool calls
    last_message = state["messages"][-1]
    if hasattr(last_message, 'tool_calls') and last_message.tool_calls:
        return "tools"
    
    return "analyst"


# Build graph
print("\n🏗️  Building LangGraph workflow...")
workflow = StateGraph(AgentState)

workflow.add_node("researcher", research_agent)
# Mock tool node
# workflow.add_node("tools", ToolNode([MockTool()])) 
# We can't easily mock ToolNode behavior without actual tools or compatible interface.
# So we'll define a simple function for tools node
def tool_node(state):
    last_message = state["messages"][-1]
    tool_calls = last_message.tool_calls
    results = []
    for tool_call in tool_calls:
        results.append({
            "tool_call_id": tool_call["id"],
            "role": "tool",
            "name": tool_call["name"],
            "content": '[{"url": "https://example.com", "content": "AI agents are evolving."}]' 
        })
    return {"messages": results}

workflow.add_node("tools", tool_node)
workflow.add_node("analyst", analyst_agent)

workflow.add_edge(START, "researcher")
workflow.add_conditional_edges(
    "researcher",
    should_continue,
    {
        "tools": "tools",
        "analyst": "analyst"
    }
)
workflow.add_edge("tools", "researcher")
workflow.add_edge("analyst", END)

app = workflow.compile()
print("   ✅ Workflow compiled\n")


if __name__ == "__main__":
    print("="*60)
    print("LangGraph Multi-Agent Demo - Auto-Instrumented (Mock)")
    print("="*60)
    
    result = app.invoke({
        "messages": [{
            "role": "user",
            "content": "Research the latest AI agent frameworks in 2024 and summarize their key features"
        }],
        "iterations": 0
    })
    
    print("\n" + "="*60)
    print("📄 FINAL REPORT")
    print("="*60)
    print(result["messages"][-1].content)
    print("\n✅ View traces: http://localhost:5173/projects/31696/traces")
