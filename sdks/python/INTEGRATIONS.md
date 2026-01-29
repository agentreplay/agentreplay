# Agentreplay Framework Integrations

Comprehensive Python SDK integrations for popular AI agent frameworks with seamless Agentreplay observability.

## Overview

Agentreplay provides production-ready integrations for all major AI agent frameworks, enabling automatic tracing, token tracking, cost calculation, and performance monitoring without requiring code changes to your existing agent workflows.

### Supported Frameworks

| Framework | Status | Version | Features |
|-----------|--------|---------|----------|
| **LangChain / LangGraph** | ✅ Ready | 0.3.x+ | Callbacks, chains, agents, workflows |
| **LlamaIndex** | ✅ Ready | 0.11.x+ | Query engines, agents, workflows |
| **OpenAI Agents SDK** | ✅ Ready | 0.5.x+ | Agent wrappers, sessions, handoffs |
| **Microsoft AutoGen** | ✅ Ready | 0.4.x+ | Multi-agent, GroupChat, tools |
| **Semantic Kernel** | ✅ Ready | 1.x+ | Kernel functions, planners |
| **CrewAI** | ✅ Ready | 0.x+ | Crews, tasks, delegation |
| **Hugging Face smolagents** | ✅ Ready | 1.x+ | Code agents, tool calling |
| **PydanticAI** | ✅ Ready | 1.x+ | Type-safe agents, structured outputs |
| **Strands Agents** | ✅ Ready | 1.x+ | AWS agents, multi-provider |
| **Google ADK** | ✅ Ready | 1.x+ | Gemini agents, multi-agent systems |

---

## Quick Start

### Installation

```bash
# Install Agentreplay SDK
pip install agentreplay

# Install framework integrations (optional dependencies)
pip install agentreplay[langchain]      # LangChain support
pip install agentreplay[llamaindex]     # LlamaIndex support
pip install agentreplay[openai-agents]  # OpenAI Agents support
pip install agentreplay[all-frameworks] # All frameworks
```

### Basic Usage

All integrations follow a consistent pattern:

```python
from agentreplay.integrations.{framework} import Wrapper/Handler

# Create integration wrapper/handler
handler = Handler(
    url="http://localhost:8080",
    tenant_id=1,
    agent_id=1,
    session_id=1001
)

# Use with your framework
# ... framework-specific code ...
```

---

## Framework-Specific Guides

### 1. LangChain / LangGraph

**Install**: `pip install langchain langchain-openai langgraph`

#### Callback Handler Approach

```python
from langchain_openai import ChatOpenAI
from langchain.chains import LLMChain
from langchain.prompts import ChatPromptTemplate
from agentreplay.integrations.langchain import AgentreplayCallbackHandler

# Create callback
callback = AgentreplayCallbackHandler(
    url="http://localhost:8080",
    tenant_id=1,
    session_id=1001
)

# Use with chains
llm = ChatOpenAI(model="gpt-4o-mini")
prompt = ChatPromptTemplate.from_template("Tell me about {topic}")
chain = LLMChain(llm=llm, prompt=prompt, callbacks=[callback])

result = chain.run(topic="AI agents")
# ✓ Automatically tracked in Agentreplay
```

#### Wrapper Approach

```python
from agentreplay.integrations.langchain import wrap_langchain_with_agentreplay

# Wrap any LangChain component
chain = wrap_langchain_with_agentreplay(
    chain,
    agentreplay_url="http://localhost:8080",
    tenant_id=1
)
```

#### LangGraph Workflows

```python
from agentreplay.integrations.langchain import AgentreplayLangGraphTracer

tracer = AgentreplayLangGraphTracer(
    url="http://localhost:8080",
    tenant_id=1
)

# Trace graph execution
result = await tracer.trace_graph_async(graph_app, inputs)
```

**Examples**: `examples/integrations/langchain_langgraph_example.py`

---

### 2. LlamaIndex

**Install**: `pip install llama-index`

#### Callback Manager

```python
from llama_index.core import VectorStoreIndex, Settings
from agentreplay.integrations.llamaindex import create_callback_manager

# Create callback manager
callback_manager = create_callback_manager(
    agentreplay_url="http://localhost:8080",
    tenant_id=1,
    session_id=2001
)

# Configure LlamaIndex
Settings.callback_manager = callback_manager

# Use as normal
index = VectorStoreIndex.from_documents(documents)
query_engine = index.as_query_engine()
response = query_engine.query("What is Agentreplay?")
# ✓ Automatically tracked
```

#### Workflow Observability

```python
from agentreplay.integrations.llamaindex import AgentreplayWorkflowObserver

observer = AgentreplayWorkflowObserver(
    url="http://localhost:8080",
    tenant_id=1
)

result = await observer.run_workflow(workflow, **inputs)
```

**Examples**: `examples/integrations/llamaindex_example.py`

---

### 3. OpenAI Agents SDK

**Install**: `pip install openai-agents`

#### Agent Wrapper

```python
from openai_agents import Agent
from agentreplay.integrations.openai_agents import AgentreplayAgentWrapper

agent = Agent(
    name="assistant",
    instructions="You are helpful",
    model="gpt-4o-mini"
)

# Wrap agent
wrapped = AgentreplayAgentWrapper(
    agent=agent,
    agentreplay_url="http://localhost:8080",
    tenant_id=1
)

# Create session and run
session = wrapped.create_session()
response = wrapped.run(session, "Hello!")
# ✓ Tracked in Agentreplay
```

#### Session Manager

```python
from agentreplay.integrations.openai_agents import AgentreplaySessionManager

manager = AgentreplaySessionManager(
    agentreplay_url="http://localhost:8080",
    tenant_id=1
)

session = manager.create_session()
response = manager.run_agent(agent, session, "Hello!")
```

**Examples**: `examples/integrations/openai_agents_example.py`

---

### 4. Microsoft AutoGen

**Install**: `pip install autogen-agentchat`

```python
from autogen import ConversableAgent
from agentreplay.integrations.autogen import wrap_autogen_agent

# Create and wrap agent
agent = ConversableAgent(
    name="assistant",
    llm_config={"model": "gpt-4o-mini"}
)

agent = wrap_autogen_agent(
    agent,
    agentreplay_url="http://localhost:8080",
    tenant_id=1
)

# Use normally - automatically tracked
```

**Multi-Agent Workflows**:

```python
from agentreplay.integrations.autogen import AgentreplayAutoGenTracer

tracer = AgentreplayAutoGenTracer(
    url="http://localhost:8080",
    tenant_id=1
)

tracer.initiate_chat(user_proxy, assistant, "Hello!")
```

---

### 5. Semantic Kernel

**Install**: `pip install semantic-kernel`

```python
from semantic_kernel import Kernel
from agentreplay.integrations.semantic_kernel import AgentreplaySemanticKernelTracer

tracer = AgentreplaySemanticKernelTracer(
    url="http://localhost:8080",
    tenant_id=1
)

kernel = Kernel()
kernel = tracer.wrap_kernel(kernel)

# All kernel invocations tracked
result = await kernel.invoke("MyPlugin", "MyFunction", input="test")
```

---

### 6. CrewAI

**Install**: `pip install crewai`

```python
from crewai import Agent, Task, Crew
from agentreplay.integrations.crewai import wrap_crewai_crew

# Create crew
agent = Agent(role="Researcher", goal="Research", backstory="Expert")
task = Task(description="Research AI", agent=agent)
crew = Crew(agents=[agent], tasks=[task])

# Wrap with tracking
crew = wrap_crewai_crew(
    crew,
    agentreplay_url="http://localhost:8080",
    tenant_id=1
)

# Run - automatically tracked
result = crew.kickoff()
```

---

### 7. Hugging Face smolagents

**Install**: `pip install smolagents`

```python
from smolagents import CodeAgent
from agentreplay.integrations.smolagents import wrap_smolagents_agent

agent = CodeAgent(tools=[], model=model)
agent = wrap_smolagents_agent(
    agent,
    agentreplay_url="http://localhost:8080",
    tenant_id=1
)

result = agent.run("Calculate fibonacci(10)")
# ✓ Code execution tracked
```

---

### 8. PydanticAI

**Install**: `pip install pydantic-ai`

```python
from pydantic_ai import Agent
from agentreplay.integrations.pydantic_ai import wrap_pydantic_ai_agent

agent = Agent("openai:gpt-4o-mini")
agent = wrap_pydantic_ai_agent(
    agent,
    agentreplay_url="http://localhost:8080",
    tenant_id=1
)

result = agent.run_sync("Hello!")
# ✓ Type-safe tracking
```

---

### 9. Strands Agents (AWS)

**Install**: `pip install strands-sdk`

```python
from strands import Agent, AgentConfig
from agentreplay.integrations.strands import wrap_strands_agent

config = AgentConfig(model="anthropic.claude-3-sonnet", provider="bedrock")
agent = Agent(config=config)

agent = wrap_strands_agent(
    agent,
    agentreplay_url="http://localhost:8080",
    tenant_id=1
)

result = agent.run("Process this request")
```

---

### 10. Google ADK

**Install**: `pip install google-adk`

```python
from google.adk import Agent
from agentreplay.integrations.google_adk import wrap_google_adk_agent

agent = Agent(name="assistant", model="gemini-pro")
agent = wrap_google_adk_agent(
    agent,
    agentreplay_url="http://localhost:8080",
    tenant_id=1
)

result = agent.run(input_data)
```

---

## Features Across All Integrations

### ✅ Automatic Tracking
- LLM calls with request/response
- Token usage (prompt, completion, total)
- Cost calculation (OpenTelemetry GenAI standards)
- Execution timing and duration
- Error handling and retries

### ✅ Observability
- Real-time trace ingestion
- Causal graph navigation
- Multi-agent workflow visualization
- Tool usage tracking
- Session management

### ✅ Production Ready
- Async/sync support
- Batch processing
- Connection pooling
- Error resilience
- OpenTelemetry compatible

---

## Configuration

### Environment Variables

```bash
export AGENTREPLAY_URL="http://localhost:8080"
export AGENTREPLAY_TENANT_ID="1"
export AGENTREPLAY_PROJECT_ID="0"
export AGENTREPLAY_AGENT_ID="1"
```

### Programmatic Configuration

```python
from agentreplay.config import AgentreplayConfig

config = AgentreplayConfig(
    url="http://localhost:8080",
    tenant_id=1,
    project_id=0,
    agent_id=1,
    timeout=30.0
)
```

---

## Examples

### Quick Start Example

Run all frameworks:
```bash
python examples/integrations/all_frameworks_quickstart.py
```

### Framework-Specific Examples

```bash
python examples/integrations/langchain_langgraph_example.py
python examples/integrations/llamaindex_example.py
python examples/integrations/openai_agents_example.py
```

---

## Architecture

All integrations follow the Agentreplay observability model:

```
Framework Code
     ↓
Integration Layer (callbacks/wrappers)
     ↓
Agentreplay Client
     ↓
HTTP API (REST)
     ↓
Agentreplay Server
     ↓
LSM-Tree Storage + HNSW Index
```

---

## Best Practices

### 1. Session Management
Use consistent `session_id` for related operations:

```python
session_id = 1001  # Same session for related calls

handler1 = Handler(url=url, tenant_id=1, session_id=session_id)
handler2 = Handler(url=url, tenant_id=1, session_id=session_id)
```

### 2. Agent Hierarchy
Use `agent_id` to distinguish different agents:

```python
researcher = Handler(url=url, tenant_id=1, agent_id=1)
writer = Handler(url=url, tenant_id=1, agent_id=2)
editor = Handler(url=url, tenant_id=1, agent_id=3)
```

### 3. Error Handling
Integrations are resilient to Agentreplay failures:

```python
try:
    result = agent.run(query)
except Exception as e:
    # Agent execution continues even if tracking fails
    logging.error(f"Tracking error: {e}")
```

### 4. Production Deployment
- Use environment variables for configuration
- Enable connection pooling
- Configure appropriate timeouts
- Monitor Agentreplay server health

---

## Troubleshooting

### Import Errors

```python
# Framework not installed
pip install {framework-name}

# Integration not found
pip install agentreplay[{framework}]
```

### Connection Issues

```python
# Check Agentreplay server
curl http://localhost:8080/health

# Verify configuration
print(handler.client.url)
```

### Missing Traces

- Verify `tenant_id` and `session_id`
- Check Agentreplay server logs
- Ensure API endpoints are accessible
- Validate authentication if enabled

---

## Contributing

We welcome contributions! To add a new framework integration:

1. Create `src/agentreplay/integrations/{framework}.py`
2. Implement handler/wrapper following existing patterns
3. Add comprehensive examples
4. Update this documentation
5. Add tests

---

## Support

- **Documentation**: https://docs.agentreplay.io
- **Examples**: `examples/integrations/`
- **Issues**: https://github.com/sochdb/agentreplay/issues
- **Discussions**: https://github.com/sochdb/agentreplay/discussions

---

## License

Apache 2.0 - See LICENSE file for details

---

## Changelog

### v2.0.0 (2025-11-09)
- ✨ Added support for 10 AI agent frameworks
- 🔧 LangChain / LangGraph integration
- 🦙 LlamaIndex with Workflows 1.0
- 🤖 OpenAI Agents SDK
- 🔄 Microsoft AutoGen (AgentChat)
- 🧠 Semantic Kernel Python
- ⚓ CrewAI
- 🐶 Hugging Face smolagents
- ✨ PydanticAI
- 🌊 Strands Agents (AWS)
- 🔍 Google ADK
- 📊 OpenTelemetry GenAI semantic conventions
- 🚀 Production-ready observability

---

**Built with ❤️ for the AI Agent community**
