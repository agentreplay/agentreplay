# Flowtrace Python SDK - Setup Guide

This guide walks you through setting up and using the Flowtrace Python SDK.

## Prerequisites

- Python 3.8 or higher
- Flowtrace server running (see main README for setup)
- pip or poetry for package management

## Installation

### Basic Installation

```bash
pip install flowtrace
```

### With Framework Integrations

Install optional dependencies for framework integrations:

```bash
# LangChain support
pip install flowtrace[langchain]

# AutoGen support
pip install flowtrace[autogen]

# All integrations
pip install flowtrace[langchain,autogen]

# Development tools
pip install flowtrace[dev]
```

### From Source

```bash
cd sdks/python
pip install -e .

# With optional dependencies
pip install -e ".[langchain,autogen,dev]"
```

## Quick Start

### 1. Start Flowtrace Server

First, ensure the Flowtrace server is running:

```bash
# In the main flowtrace directory
cargo build --release
./target/release/flowtrace init --path ./test-db
./target/release/flowtrace serve --path ./test-db --port 8080
```

### 2. Basic Usage Example

Create a file `test_flowtrace.py`:

```python
from flowtrace import FlowtraceClient, SpanType, Span

# Create client
client = FlowtraceClient(
    url="http://localhost:8080",
    tenant_id=1,
    agent_id=1,
)

# Use span context manager
with Span(client, SpanType.ROOT, session_id=100) as root:
    root.set_token_count(0)
    print(f"Created root span: {root.edge_id}")
    
    # Create child planning span
    with root.child(SpanType.PLANNING) as planning:
        planning.set_token_count(150).set_confidence(0.95)
        print(f"Created planning span: {planning.edge_id}")
    
    # Create tool call span
    with root.child(SpanType.TOOL_CALL) as tool:
        print(f"Created tool call span: {tool.edge_id}")
    
    # Create response span
    with root.child(SpanType.RESPONSE) as response:
        response.set_token_count(75).set_confidence(0.98)
        print(f"Created response span: {response.edge_id}")

# Query the session
edges = client.filter_by_session(session_id=100)
print(f"\nFound {len(edges)} edges in session 100")
for edge in edges:
    print(f"  Edge {edge.edge_id}: {edge.span_type} (tokens: {edge.token_count})")
```

Run it:

```bash
python test_flowtrace.py
```

### 3. Framework Integration Examples

#### LangChain

```python
from langchain.chat_models import ChatOpenAI
from langchain.chains import LLMChain
from langchain.prompts import ChatPromptTemplate
from flowtrace.integrations.langchain import FlowtraceCallbackHandler

# Create callback
callback = FlowtraceCallbackHandler(
    url="http://localhost:8080",
    tenant_id=1,
    agent_id=1,
    session_id=200,
)

# Create and run chain
llm = ChatOpenAI(temperature=0.7)
prompt = ChatPromptTemplate.from_template("Tell me about {topic}")
chain = LLMChain(llm=llm, prompt=prompt, callbacks=[callback])

result = chain.run(topic="machine learning")
print(result)

# All LLM calls, agent steps, and tool calls are now logged to Flowtrace!
```

#### AutoGen

```python
from autogen import AssistantAgent, UserProxyAgent
from flowtrace.integrations.autogen import FlowtraceAgentWrapper

# Configure and create agent
llm_config = {"model": "gpt-4", "api_key": "your-key"}
assistant = AssistantAgent(name="assistant", llm_config=llm_config)

# Wrap with Flowtrace tracking
wrapped = FlowtraceAgentWrapper(
    agent=assistant,
    url="http://localhost:8080",
    tenant_id=1,
    agent_id=1,
    session_id=300,
)

# Create user proxy
user_proxy = UserProxyAgent(name="user", human_input_mode="NEVER")

# Start conversation - automatically logged
user_proxy.initiate_chat(
    wrapped.agent,
    message="What is quantum computing?",
)

# All agent messages and tool calls are now logged to Flowtrace!
```

## Configuration

### Environment Variables

You can configure the SDK using environment variables:

```bash
export FLOWTRACE_URL="http://localhost:8080"
export FLOWTRACE_TENANT_ID="1"
export FLOWTRACE_AGENT_ID="1"
```

Then in Python:

```python
import os
from flowtrace import FlowtraceClient

client = FlowtraceClient(
    url=os.getenv("FLOWTRACE_URL", "http://localhost:8080"),
    tenant_id=int(os.getenv("FLOWTRACE_TENANT_ID", "1")),
    agent_id=int(os.getenv("FLOWTRACE_AGENT_ID", "1")),
)
```

### Async Usage

For high-throughput scenarios, use the async client:

```python
from flowtrace import AsyncFlowtraceClient
import asyncio

async def main():
    client = AsyncFlowtraceClient(
        url="http://localhost:8080",
        tenant_id=1,
        agent_id=1,
    )
    
    # Batch insert
    edges = [edge1, edge2, edge3, ...]
    inserted = await client.insert_batch(edges)
    
    # Async queries
    edges = await client.query_temporal_range(
        start_us=1234567000,
        end_us=1234568000,
    )
    
    print(f"Found {len(edges)} edges")

asyncio.run(main())
```

## Testing

Run the test suite:

```bash
cd sdks/python
pytest tests/
```

Run with coverage:

```bash
pytest --cov=flowtrace tests/
```

## Type Checking

The SDK is fully typed. Run type checking with:

```bash
mypy src/flowtrace
```

## Linting

Format code with Black:

```bash
black src/flowtrace tests/
```

Lint with Ruff:

```bash
ruff check src/flowtrace tests/
```

## Troubleshooting

### Connection Refused

If you get "Connection refused" errors:

1. Ensure Flowtrace server is running: `./target/release/flowtrace serve ...`
2. Check the port matches: default is 8080
3. Verify firewall settings

### Import Errors for Integrations

If you get import errors for LangChain or AutoGen:

```bash
# Install the integration dependencies
pip install flowtrace[langchain]  # or [autogen]
```

### Type Errors

If you get Pydantic validation errors:

1. Ensure all required fields are provided
2. Check that values match expected types (int, str, float)
3. Use `edge.model_dump()` to see the current state

### Server Not Responding

If queries timeout:

1. Check server logs for errors
2. Verify database is initialized: `./target/release/flowtrace init --path ./test-db`
3. Try reducing batch size or query limits

## Next Steps

- Read the [API Reference](API_REFERENCE.md)
- Check out [Examples](examples/)
- Join the community on [GitHub Discussions](https://github.com/sochdb/flowtrace/discussions)

## Support

- Documentation: https://docs.flowtrace.dev
- GitHub Issues: https://github.com/sochdb/flowtrace/issues
- Discussions: https://github.com/sochdb/flowtrace/discussions
