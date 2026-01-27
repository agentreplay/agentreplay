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
Flowtrace + LangGraph Example
Auto-instrumented multi-agent system with web search

Project ID: 31696 (LangGraph tracking)
"""
import os
from dotenv import load_dotenv
load_dotenv()

# Flowtrace auto-instrumentation
import flowtrace
flowtrace.init_otel_instrumentation(
    service_name="flowtrace-langgraph-demo",
    otlp_endpoint=os.getenv("FLOWTRACE_OTLP_ENDPOINT", "localhost:4317"),
    project_id=46635,  # LangGraph project
    tenant_id=int(os.getenv("FLOWTRACE_TENANT_ID", "1")),
    debug=True
)
print("✅ Flowtrace initialized for LangGraph")

from typing import Annotated, TypedDict
from langchain_openai import AzureChatOpenAI
from langchain_community.tools.tavily_search import TavilySearchResults
from langgraph.graph import StateGraph, START, END
from langgraph.graph.message import add_messages
from langgraph.prebuilt import ToolNode, tools_condition


class AgentState(TypedDict):
    """Multi-agent state"""
    messages: Annotated[list, add_messages]
    iterations: int


def research_agent(state: AgentState):
    """Research agent with web search"""
    print(f"\n🔍 [Research Agent] Processing (iteration {state['iterations']})...")
    
    llm = AzureChatOpenAI(
        azure_endpoint=os.getenv("AZURE_OPENAI_ENDPOINT"),
        azure_deployment=os.getenv("AZURE_OPENAI_DEPLOYMENT", "gpt-4o"),
        api_version="2024-12-01-preview",
        temperature=0
    )
    tools = [TavilySearchResults(max_results=3)]
    llm_with_tools = llm.bind_tools(tools)
    
    response = llm_with_tools.invoke(state["messages"])
    
    if hasattr(response, 'tool_calls') and response.tool_calls:
        print(f"   🔧 Calling {len(response.tool_calls)} tool(s)")
    
    return {
        "messages": [response],
        "iterations": state["iterations"] + 1
    }


def analyst_agent(state: AgentState):
    """Analyst agent that synthesizes findings"""
    print(f"\n📊 [Analyst] Synthesizing information...")
    
    llm = AzureChatOpenAI(
        azure_endpoint=os.getenv("AZURE_OPENAI_ENDPOINT"),
        azure_deployment=os.getenv("AZURE_OPENAI_DEPLOYMENT", "gpt-4o"),
        api_version="2024-12-01-preview",
        temperature=0.7
    )
    
    # Add analyst instructions
    messages = state["messages"] + [{
        "role": "system",
        "content": "Synthesize the research findings into a structured report with key insights."
    }]
    
    response = llm.invoke(messages)
    print(f"   ✅ Generated {len(response.content)} char report")
    
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
workflow.add_node("tools", ToolNode([TavilySearchResults(max_results=3)]))
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
    print("LangGraph Multi-Agent Demo - Auto-Instrumented")
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
    print("\n✅ View traces: http://localhost:5173/projects/43342/traces")
