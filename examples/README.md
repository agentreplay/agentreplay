# Agentreplay Framework Examples

This directory contains auto-instrumented examples for different AI agent frameworks.

## Project Structure

Each framework has its own folder and **dedicated project ID** for tracking:

- `agentreplay_langgraph/` - LangGraph examples (Project ID: 31696)
- `agentreplay_langchain/` - LangChain examples (Project ID: 31697)
- `agentreplay_autogen/` - AutoGen examples (Project ID: 31698)
- `agentreplay_crewai/` - CrewAI examples (Project ID: 31699)

## Setup

1. **Install dependencies:**
```bash
pip install agentreplay langgraph langchain autogen crewai tavily-python
```

2. **Configure .env:**
```bash
# Agentreplay
AGENTREPLAY_OTLP_ENDPOINT=localhost:4317
AGENTREPLAY_TENANT_ID=1

# Azure OpenAI (required for all examples)
AZURE_OPENAI_API_KEY=your-key
AZURE_OPENAI_ENDPOINT=https://your-endpoint.openai.azure.com
AZURE_OPENAI_DEPLOYMENT=gpt-4o
AZURE_OPENAI_API_VERSION=2024-12-01-preview

# Tavily (optional for web search)
TAVILY_API_KEY=your-key
```

3. **Start Agentreplay server:**
```bash
cd agentreplay-server
cargo run --release
```

## Running Examples

### LangGraph (Project 31696)
```bash
cd agentreplay_langgraph
python multi_agent_research.py
```
**Features:**
- Research agent with web search
- Analyst agent for synthesis
- Conditional routing
- Tool execution

### LangChain (Project 31697)
```bash
cd agentreplay_langchain
python rag_agent_with_memory.py
```
**Features:**
- RAG chain with memory
- Conversational context
- Tool integration
- Multi-turn dialogue

### AutoGen (Project 31698)
```bash
cd agentreplay_autogen
python multi_agent_collaboration.py
```
**Features:**
- 4-agent collaboration
- Group chat coordination
- Role-based agents (Planner, Researcher, Writer, Critic)
- Round-robin speaker selection

### CrewAI (Project 31699)
```bash
cd agentreplay_crewai
python research_crew.py
```
**Features:**
- Sequential task execution
- Specialized agent roles
- Task dependencies
- Crew coordination

## Viewing Traces

Each framework has its own project for isolated tracking:

- **LangGraph**: http://localhost:5173/projects/31696/traces
- **LangChain**: http://localhost:5173/projects/31697/traces
- **AutoGen**: http://localhost:5173/projects/31698/traces
- **CrewAI**: http://localhost:5173/projects/31699/traces

## What's Captured

Agentreplay automatically captures:
- ✅ All LLM calls with prompts/completions
- ✅ Token usage per agent
- ✅ Tool/function calls
- ✅ Agent interactions
- ✅ Execution flow
- ✅ Response times
- ✅ Errors and exceptions

## Zero-Code Instrumentation

All examples use **2 lines** of Agentreplay initialization:
```python
import agentreplay
agentreplay.init_otel_instrumentation(
    service_name="your-service",
    project_id=31696,  # Framework-specific ID
    otlp_endpoint="localhost:4317",
    tenant_id=1
)
```

That's it! Everything else is automatic.

## Comparing Frameworks

Since each framework has its own project ID, you can easily compare:
- Agent behavior patterns
- Token efficiency
- Execution speed
- Error rates
- Tool usage patterns

Navigate between projects in the Agentreplay UI to compare!
