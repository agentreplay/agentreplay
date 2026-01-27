# Flowtrace Python SDK

Python SDK for Flowtrace - A purpose-built agent trace engine for LLM agents.

## Installation

```bash
pip install flowtrace-client
```

### With Framework Integrations

```bash
# Install specific framework integrations
pip install flowtrace-client[langchain]      # LangChain / LangGraph
pip install flowtrace-client[llamaindex]     # LlamaIndex
pip install flowtrace-client[openai-agents]  # OpenAI Agents SDK
pip install flowtrace-client[autogen]        # Microsoft AutoGen
pip install flowtrace-client[semantic-kernel] # Semantic Kernel
pip install flowtrace-client[crewai]         # CrewAI
pip install flowtrace-client[smolagents]     # Hugging Face smolagents
pip install flowtrace-client[pydantic-ai]    # PydanticAI
pip install flowtrace-client[strands]        # AWS Strands Agents
pip install flowtrace-client[google-adk]     # Google ADK

# Or install all framework integrations at once
pip install flowtrace-client[all-frameworks]

# For development (includes all frameworks + dev tools)
pip install flowtrace-client[all]
```

## Quick Start

```python
from flowtrace import FlowtraceClient, SpanType

# Initialize client
client = FlowtraceClient(
    url="http://localhost:8080",
    tenant_id=1,
    project_id=0
)

# Log a trace
with client.trace(span_type=SpanType.ROOT) as root:
    # Planning step
    with root.child(SpanType.PLANNING) as planning:
        planning.set_token_count(50)
        planning.set_confidence(0.95)
    
    # Tool call
    with root.child(SpanType.TOOL_CALL) as tool:
        tool.set_token_count(20)
        tool.set_duration_ms(150)
    
    # Final response
    with root.child(SpanType.RESPONSE) as response:
        response.set_token_count(80)
        response.set_confidence(0.94)

# Query traces
edges = client.query_temporal_range(
    start_timestamp_us=start_time,
    end_timestamp_us=end_time
)

# Get causal relationships
children = client.get_children(edge_id)
ancestors = client.get_ancestors(edge_id)
```

## Framework Integrations

Flowtrace provides seamless integrations for all major AI agent frameworks:

### Supported Frameworks

| Framework | Install | Documentation |
|-----------|---------|---------------|
| **LangChain / LangGraph** | `pip install flowtrace-client[langchain]` | Chains, agents, workflows |
| **LlamaIndex** | `pip install flowtrace-client[llamaindex]` | Query engines, agents, workflows |
| **OpenAI Agents SDK** | `pip install flowtrace-client[openai-agents]` | Agent wrappers, sessions |
| **Microsoft AutoGen** | `pip install flowtrace-client[autogen]` | Multi-agent conversations |
| **Semantic Kernel** | `pip install flowtrace-client[semantic-kernel]` | Kernel functions, planners |
| **CrewAI** | `pip install flowtrace-client[crewai]` | Crews, tasks, collaboration |
| **Hugging Face smolagents** | `pip install flowtrace-client[smolagents]` | Code agents, tool calling |
| **PydanticAI** | `pip install flowtrace-client[pydantic-ai]` | Type-safe agents |
| **Strands Agents** | `pip install flowtrace-client[strands]` | AWS agents, multi-provider |
| **Google ADK** | `pip install flowtrace-client[google-adk]` | Gemini agents |

### Quick Integration Examples

#### LangChain

```python
from flowtrace.integrations.langchain import FlowtraceCallbackHandler
from langchain.chains import LLMChain

callback = FlowtraceCallbackHandler(
    url="http://localhost:8080",
    tenant_id=1
)

chain = LLMChain(llm=llm, callbacks=[callback])
result = chain.run("What is the weather?")
```

#### LlamaIndex

```python
from flowtrace.integrations.llamaindex import create_callback_manager
from llama_index.core import VectorStoreIndex

callback_manager = create_callback_manager(
    flowtrace_url="http://localhost:8080",
    tenant_id=1
)

index = VectorStoreIndex.from_documents(
    documents,
    callback_manager=callback_manager
)
```

#### OpenAI Agents SDK

```python
from flowtrace.integrations.openai_agents import FlowtraceAgentWrapper
from openai_agents import Agent

agent = Agent(name="assistant", instructions="You are helpful")
wrapped = FlowtraceAgentWrapper(
    agent=agent,
    flowtrace_url="http://localhost:8080",
    tenant_id=1
)

session = wrapped.create_session()
response = wrapped.run(session, "Hello!")
```

#### PydanticAI

```python
from flowtrace.integrations.pydantic_ai import wrap_pydantic_ai_agent
from pydantic_ai import Agent

agent = Agent("openai:gpt-4")
agent = wrap_pydantic_ai_agent(
    agent,
    flowtrace_url="http://localhost:8080",
    tenant_id=1
)

result = agent.run_sync("Process this request")
```

**📚 For detailed integration guides and examples, see [INTEGRATIONS.md](INTEGRATIONS.md)**

## Features

### Core SDK
- **Low-level API**: Direct control over edge creation and querying
- **Context Managers**: Pythonic span tracking with automatic parent-child relationships
- **Async Support**: Full async/await support for high-performance applications
- **Type Safety**: Full type hints and Pydantic models
- **Causal Queries**: Navigate agent reasoning graphs
- **Semantic Search**: Find similar traces using vector embeddings

### Framework Integrations (New in v2.0!)
- **10+ Framework Support**: LangChain, LlamaIndex, OpenAI Agents, AutoGen, and more
- **Automatic Tracking**: LLM calls, token usage, costs, and execution timing
- **OpenTelemetry GenAI**: Full semantic conventions support
- **Production Ready**: Async/sync, error resilience, connection pooling
- **Zero Code Changes**: Wrap existing agents with minimal modifications

## Documentation

- **Quick Start**: See above and [examples/](examples/)
- **Framework Integrations**: [INTEGRATIONS.md](INTEGRATIONS.md)
- **Full API Documentation**: https://docs.flowtrace.dev/python-sdk
- **Examples**:
  - [LangChain/LangGraph](examples/integrations/langchain_langgraph_example.py)
  - [LlamaIndex](examples/integrations/llamaindex_example.py)
  - [OpenAI Agents](examples/integrations/openai_agents_example.py)
  - [All Frameworks Quickstart](examples/integrations/all_frameworks_quickstart.py)

## License

Apache License 2.0 - Copyright 2025 Sushanth (https://github.com/sushanthpy)
