# Agentreplay Python SDK

[![PyPI version](https://badge.fury.io/py/agentreplay.svg)](https://badge.fury.io/py/agentreplay)
[![Python 3.8+](https://img.shields.io/badge/python-3.8+-blue.svg)](https://www.python.org/downloads/)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![CI](https://github.com/agentreplay/agentreplay/actions/workflows/ci-python.yaml/badge.svg)](https://github.com/agentreplay/agentreplay/actions/workflows/ci-python.yaml)

**The observability platform for LLM agents and AI applications.** Trace every LLM call, tool invocation, and agent step with minimal code changes.

---

## ✨ Features

| Feature | Description |
|---------|-------------|
| 🚀 **Zero-Config Setup** | Works out of the box with environment variables |
| 🎯 **One-Liner Instrumentation** | Wrap OpenAI/Anthropic clients in one line |
| 🔧 **Decorator-Based Tracing** | `@traceable` for any function |
| 🔄 **Async Native** | Full support for async/await patterns |
| 🔒 **Privacy First** | Built-in PII redaction and scrubbing |
| 📊 **Token Tracking** | Automatic token usage capture |
| 🌐 **Framework Agnostic** | Works with LangChain, LlamaIndex, CrewAI, etc. |
| ⚡ **Batched Transport** | Efficient background sending with retry |

---

## 📦 Installation

```bash
# Basic installation (minimal dependencies)
pip install agentreplay

# With OpenTelemetry support
pip install agentreplay[otel]

# With LangChain integration
pip install agentreplay[langchain]

# With LangGraph integration
pip install agentreplay[langgraph]

# With LlamaIndex integration
pip install agentreplay[llamaindex]

# Full installation (all integrations)
pip install agentreplay[otel,langchain,langgraph,llamaindex]
```

---

## 🚀 Quick Start

### 1. Set Environment Variables

```bash
export AGENTREPLAY_API_KEY="your-api-key"
export AGENTREPLAY_PROJECT_ID="my-project"
# Optional
export AGENTREPLAY_BASE_URL="https://api.agentreplay.io"
```

### 2. Enable Auto-Instrumentation

After installing the package, run the install command to enable zero-code auto-instrumentation:

```bash
# Run this once after pip install
agentreplay-install
```

This installs a `.pth` file that automatically initializes Agentreplay when Python starts, enabling automatic tracing of OpenAI, Anthropic, and other LLM libraries **without any code changes**.

#### Option A: Zero-Code (after running agentreplay-install)

```bash
# Just run your script - instrumentation happens automatically!
python my_app.py
```

#### Option B: Code-Based Initialization

Add these two lines at the **very beginning** of your main file, before any other imports:

```python
import agentreplay
agentreplay.init()  # Must be called before importing OpenAI, Anthropic, etc.

# Now import your LLM clients - they're automatically traced!
from openai import OpenAI

client = OpenAI()
response = client.chat.completions.create(
    model="gpt-4",
    messages=[{"role": "user", "content": "Hello!"}]
)
# All OpenAI calls are automatically traced!

# Ensure traces are sent before exit
agentreplay.flush()
```

> ⚠️ **Important**: `agentreplay.init()` must be called **before** importing OpenAI, Anthropic, or other LLM libraries for auto-instrumentation to work.

#### Managing Auto-Instrumentation

```bash
# Install auto-instrumentation (run once after pip install)
agentreplay-install

# Check if installed
agentreplay-install --check

# Uninstall auto-instrumentation
agentreplay-install --uninstall
```

### 3. Manual Tracing with Decorators

```python
import agentreplay

# Initialize (reads from env vars automatically)
agentreplay.init()

# Trace any function with a simple decorator
@agentreplay.traceable
def my_ai_function(query: str) -> str:
    # Your AI logic here
    return f"Response to: {query}"

# Call your function - it's automatically traced!
result = my_ai_function("What is the capital of France?")

# Ensure all traces are sent before exit
agentreplay.flush()
```

That's it! Your function calls are now being traced and sent to Agentreplay.

---

## 🔧 Core API Reference

### Initialization

```python
import agentreplay

# Option 1: Environment variables (recommended for production)
agentreplay.init()

# Option 2: Explicit configuration
agentreplay.init(
    api_key="your-api-key",
    project_id="my-project",
    base_url="https://api.agentreplay.io",
    
    # Optional settings
    tenant_id="default",       # Multi-tenant identifier
    agent_id="default",        # Default agent ID
    enabled=True,              # Set False to disable in tests
    capture_input=True,        # Capture function inputs
    capture_output=True,       # Capture function outputs
    batch_size=100,            # Batch size before sending
    flush_interval=5.0,        # Auto-flush interval in seconds
    debug=False,               # Enable debug logging
)

# Check if SDK is initialized
from agentreplay.sdk import is_initialized
if is_initialized():
    print("SDK is ready!")

# Get current configuration
config = agentreplay.sdk.get_config()
print(f"Project: {config.project_id}")
```

---

## 🎯 The `@traceable` Decorator

The primary and most Pythonic way to instrument your code:

### Basic Usage

```python
from agentreplay import traceable

# Just add the decorator - that's it!
@traceable
def process_query(query: str) -> str:
    return call_llm(query)

# Works with any function signature
@traceable
def complex_function(data: dict, *, optional_param: str = "default") -> list:
    return process(data, optional_param)
```

### With Options

```python
from agentreplay import traceable, SpanKind

# Custom span name and kind
@traceable(name="openai_chat", kind=SpanKind.LLM)
def call_openai(messages: list) -> str:
    return openai_client.chat.completions.create(
        model="gpt-4",
        messages=messages
    )

# Disable input capture for sensitive data
@traceable(capture_input=False)
def authenticate(password: str) -> bool:
    return verify_password(password)

# Disable output capture
@traceable(capture_output=False)
def get_secret() -> str:
    return fetch_secret_from_vault()

# Add static metadata
@traceable(metadata={"version": "2.0", "model": "gpt-4", "team": "ml"})
def enhanced_query(query: str) -> str:
    return process(query)
```

### Async Functions

Full async/await support - no changes needed:

```python
import asyncio
from agentreplay import traceable, SpanKind

@traceable(kind=SpanKind.LLM)
async def async_llm_call(prompt: str) -> str:
    response = await openai_client.chat.completions.create(
        model="gpt-4",
        messages=[{"role": "user", "content": prompt}]
    )
    return response.choices[0].message.content

@traceable
async def process_batch(queries: list[str]) -> list[str]:
    # All concurrent calls are traced with proper parent-child relationships
    tasks = [async_llm_call(q) for q in queries]
    return await asyncio.gather(*tasks)

# Run
results = asyncio.run(process_batch(["query1", "query2", "query3"]))
```

---

## 📐 Context Manager: `trace()`

For more control over span attributes and timing:

```python
from agentreplay import trace, SpanKind

def complex_operation(query: str) -> dict:
    with trace("process_query", kind=SpanKind.CHAIN) as span:
        # Set input data
        span.set_input({"query": query, "timestamp": time.time()})
        
        # Nested span for document retrieval
        with trace("retrieve_documents", kind=SpanKind.RETRIEVER) as retriever_span:
            docs = vector_db.search(query, top_k=5)
            retriever_span.set_output({"document_count": len(docs)})
            retriever_span.set_attribute("vector_db", "pinecone")
        
        # Nested span for LLM generation
        with trace("generate_response", kind=SpanKind.LLM) as llm_span:
            llm_span.set_model("gpt-4", provider="openai")
            response = generate_response(query, docs)
            llm_span.set_token_usage(
                prompt_tokens=150,
                completion_tokens=200,
                total_tokens=350
            )
            llm_span.set_output({"response_length": len(response)})
        
        # Add events for debugging
        span.add_event("processing_complete", {"doc_count": len(docs)})
        
        # Set final output
        span.set_output({"response": response, "sources": len(docs)})
        
        return {"response": response, "sources": docs}
```

### Manual Span Control

For cases where you need explicit control:

```python
from agentreplay import start_span, SpanKind

def long_running_operation():
    span = start_span("background_job", kind=SpanKind.TOOL)
    span.set_input({"job_type": "data_sync"})
    
    try:
        # Long running work...
        for i in range(100):
            process_item(i)
            if i % 10 == 0:
                span.add_event("progress", {"completed": i})
        
        span.set_output({"items_processed": 100})
        
    except Exception as e:
        span.set_error(e)
        raise
        
    finally:
        span.end()  # Always call end()
```

---

## 🔌 LLM Client Wrappers

### OpenAI (Recommended)

One line to instrument all OpenAI calls:

```python
from openai import OpenAI
from agentreplay import init, wrap_openai, flush

init()

# Wrap the client - all calls are now traced automatically!
client = wrap_openai(OpenAI())

# Use normally - tracing happens in the background
response = client.chat.completions.create(
    model="gpt-4",
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Explain quantum computing in simple terms."}
    ],
    temperature=0.7,
)

print(response.choices[0].message.content)

# Embeddings are traced too
embedding = client.embeddings.create(
    model="text-embedding-ada-002",
    input="Hello world"
)

flush()
```

### Async OpenAI

```python
from openai import AsyncOpenAI
from agentreplay import init, wrap_openai

init()

# Works with async client too
async_client = wrap_openai(AsyncOpenAI())

async def main():
    response = await async_client.chat.completions.create(
        model="gpt-4",
        messages=[{"role": "user", "content": "Hello!"}]
    )
    return response.choices[0].message.content
```

### Anthropic

```python
from anthropic import Anthropic
from agentreplay import init, wrap_anthropic, flush

init()

# Wrap the Anthropic client
client = wrap_anthropic(Anthropic())

# Use normally
message = client.messages.create(
    model="claude-3-opus-20240229",
    max_tokens=1024,
    messages=[
        {"role": "user", "content": "Explain the theory of relativity."}
    ]
)

print(message.content[0].text)
flush()
```

### Disable Content Capture

For privacy-sensitive applications:

```python
# Don't capture message content, only metadata
client = wrap_openai(OpenAI(), capture_content=False)

# Traces will still include:
# - Model name
# - Token counts
# - Latency
# - Error information
# But NOT the actual messages or responses
```

---

## 🏷️ Context Management

### Global Context

Set context that applies to ALL subsequent traces:

```python
from agentreplay import set_context, get_global_context, clear_context

# Set user context (persists until cleared)
set_context(
    user_id="user-123",
    session_id="session-456",
    agent_id="support-bot",
)

# Add more context later
set_context(
    environment="production",
    version="1.2.0",
    region="us-west-2",
)

# Get current global context
context = get_global_context()
print(context)  # {'user_id': 'user-123', 'session_id': 'session-456', ...}

# Clear all context
clear_context()
```

### Request-Scoped Context

For web applications with per-request context:

```python
from agentreplay import with_context

async def handle_api_request(request):
    # Context only applies within this block
    with with_context(
        user_id=request.user_id,
        request_id=request.headers.get("X-Request-ID"),
        ip_address=request.client_ip,
    ):
        # All traces here include this context
        result = await process_request(request)
        
    # Context automatically cleared after block
    return result

# Async version works too
async def async_handler(request):
    async with with_context(user_id=request.user_id):
        return await async_process(request)
```

### Multi-Agent Context

For multi-agent systems like CrewAI or AutoGen:

```python
from agentreplay import AgentContext

def run_multi_agent_workflow():
    # Each agent gets its own context
    with AgentContext(
        agent_id="researcher",
        session_id="workflow-123",
        workflow_id="content-creation",
    ):
        # All LLM calls here are tagged with agent_id="researcher"
        research_results = researcher_agent.run()
    
    with AgentContext(
        agent_id="writer",
        session_id="workflow-123",  # Same session
        workflow_id="content-creation",
    ):
        # These calls are tagged with agent_id="writer"
        article = writer_agent.run(research_results)
    
    with AgentContext(
        agent_id="editor",
        session_id="workflow-123",
        workflow_id="content-creation",
    ):
        final_article = editor_agent.run(article)
    
    return final_article
```

---

## 🔒 Privacy & Data Redaction

### Configure Privacy Settings

```python
from agentreplay import configure_privacy

configure_privacy(
    # Enable built-in patterns for common PII
    use_builtin_patterns=True,  # Emails, credit cards, SSNs, phones, API keys
    
    # Add custom regex patterns
    redact_patterns=[
        r"secret-\w+",           # Custom secret format
        r"internal-id-\d+",      # Internal IDs
        r"password:\s*\S+",      # Password fields
    ],
    
    # Completely scrub these JSON paths
    scrub_paths=[
        "input.password",
        "input.credentials.api_key",
        "output.user.ssn",
        "metadata.internal_token",
    ],
    
    # Hash PII instead of replacing with [REDACTED]
    # Allows tracking unique values without exposing data
    hash_pii=True,
    hash_salt="your-secret-salt-here",
    
    # Custom replacement text
    redacted_text="[SENSITIVE]",
)
```

### Built-in Redaction Patterns

The SDK includes patterns for:

| Type | Example | Redacted As |
|------|---------|-------------|
| Email | user@example.com | [REDACTED] |
| Credit Card | 4111-1111-1111-1111 | [REDACTED] |
| SSN | 123-45-6789 | [REDACTED] |
| Phone (US) | +1-555-123-4567 | [REDACTED] |
| Phone (Intl) | +44-20-1234-5678 | [REDACTED] |
| API Key | sk-proj-abc123... | [REDACTED] |
| Bearer Token | Bearer eyJ... | [REDACTED] |
| JWT | eyJhbG... | [REDACTED] |
| IP Address | 192.168.1.1 | [REDACTED] |

### Manual Redaction

```python
from agentreplay import redact_payload, redact_string, hash_pii

# Redact an entire payload
data = {
    "user": {
        "email": "john@example.com",
        "phone": "+1-555-123-4567",
    },
    "message": "My credit card is 4111-1111-1111-1111",
    "api_key": "sk-proj-abcdefghijk",
}

safe_data = redact_payload(data)
# Result:
# {
#     "user": {
#         "email": "[REDACTED]",
#         "phone": "[REDACTED]",
#     },
#     "message": "My credit card is [REDACTED]",
#     "api_key": "[REDACTED]",
# }

# Redact a single string
safe_message = redact_string("Contact me at user@example.com")
# "Contact me at [REDACTED]"

# Hash for consistent anonymization (same input = same hash)
user_hash = hash_pii("user@example.com")
# "[HASH:a1b2c3d4]"

# Useful for analytics without exposing PII
print(f"User {user_hash} performed action")
```

### Temporary Privacy Context

```python
from agentreplay.privacy import privacy_context

# Add extra redaction rules for a specific block
with privacy_context(
    redact_patterns=[r"internal-token-\w+"],
    scrub_paths=["metadata.debug_info"],
):
    # These rules only apply within this block
    result = process_sensitive_data(data)

# Original rules restored after block
```

---

## 📊 Span Kinds

Use semantic span kinds for better visualization and filtering:

```python
from agentreplay import SpanKind

# Available span kinds
SpanKind.CHAIN       # Orchestration, workflows, pipelines
SpanKind.LLM         # LLM API calls (OpenAI, Anthropic, etc.)
SpanKind.TOOL        # Tool/function calls, actions
SpanKind.RETRIEVER   # Vector DB search, document retrieval
SpanKind.EMBEDDING   # Embedding generation
SpanKind.GUARDRAIL   # Safety checks, content filtering
SpanKind.CACHE       # Cache operations
SpanKind.HTTP        # HTTP requests
SpanKind.DB          # Database queries
```

Example usage:

```python
from agentreplay import traceable, SpanKind

@traceable(kind=SpanKind.RETRIEVER)
def search_documents(query: str) -> list:
    return vector_db.similarity_search(query, k=5)

@traceable(kind=SpanKind.LLM)
def generate_answer(query: str, docs: list) -> str:
    return llm.generate(query, context=docs)

@traceable(kind=SpanKind.CHAIN)
def rag_pipeline(query: str) -> str:
    docs = search_documents(query)
    return generate_answer(query, docs)
```

---

## ⚙️ Lifecycle Management

### Flushing Traces

Always ensure traces are sent before your application exits:

```python
import agentreplay

agentreplay.init()

# Your application code...

# Option 1: Manual flush with timeout
agentreplay.flush(timeout=10.0)  # Wait up to 10 seconds

# Option 2: Full graceful shutdown
agentreplay.shutdown(timeout=30.0)  # Flush and cleanup

# Option 3: Auto-registered (init() registers atexit handler automatically)
# Traces are flushed on normal program exit
```

### Serverless / AWS Lambda

**Critical**: Always flush explicitly before the function returns!

```python
import agentreplay

agentreplay.init()

@agentreplay.traceable
def process_event(event):
    # Your logic here
    return {"processed": True}

def lambda_handler(event, context):
    try:
        result = process_event(event)
        return {
            "statusCode": 200,
            "body": json.dumps(result)
        }
    finally:
        # CRITICAL: Flush before Lambda freezes
        agentreplay.flush(timeout=5.0)
```

### FastAPI / Starlette

```python
from fastapi import FastAPI
from contextlib import asynccontextmanager
import agentreplay

@asynccontextmanager
async def lifespan(app: FastAPI):
    # Startup
    agentreplay.init()
    yield
    # Shutdown
    agentreplay.shutdown(timeout=10.0)

app = FastAPI(lifespan=lifespan)

@app.post("/chat")
async def chat(request: ChatRequest):
    # Traces are sent automatically in background
    return await process_chat(request)
```

### Diagnostics

```python
import agentreplay

# Get SDK statistics
stats = agentreplay.get_stats()
print(f"Spans sent: {stats.get('spans_sent', 0)}")
print(f"Spans pending: {stats.get('spans_pending', 0)}")
print(f"Errors: {stats.get('errors', 0)}")
print(f"Batches sent: {stats.get('batches_sent', 0)}")

# Health check - verify backend connectivity
if agentreplay.ping():
    print("✅ Backend is reachable")
else:
    print("❌ Cannot reach backend")
```

---

## 🔗 Framework Integrations

### LangChain

```python
from langchain_openai import ChatOpenAI
from langchain.schema import HumanMessage
import agentreplay

agentreplay.init()

@agentreplay.traceable(name="langchain_qa", kind=agentreplay.SpanKind.CHAIN)
def answer_question(question: str) -> str:
    llm = ChatOpenAI(model="gpt-4", temperature=0)
    response = llm.invoke([HumanMessage(content=question)])
    return response.content

result = answer_question("What is machine learning?")
agentreplay.flush()
```

### LangGraph

```python
from langgraph.graph import StateGraph, END
import agentreplay

agentreplay.init()

@agentreplay.traceable(name="agent_node", kind=agentreplay.SpanKind.LLM)
def agent_node(state):
    # Agent logic
    response = llm.invoke(state["messages"])
    return {"messages": state["messages"] + [response]}

@agentreplay.traceable(name="tool_node", kind=agentreplay.SpanKind.TOOL)
def tool_node(state):
    # Tool execution
    result = execute_tool(state["tool_call"])
    return {"messages": state["messages"] + [result]}

# Build graph with traced nodes
workflow = StateGraph(State)
workflow.add_node("agent", agent_node)
workflow.add_node("tools", tool_node)
# ... rest of graph definition
```

### CrewAI

```python
from crewai import Agent, Task, Crew
import agentreplay

agentreplay.init()

# Wrap the LLM client
wrapped_llm = agentreplay.wrap_openai(OpenAI())

# Track each agent with context
with agentreplay.AgentContext(agent_id="researcher", workflow_id="article-creation"):
    researcher = Agent(
        role="Senior Researcher",
        goal="Find comprehensive information",
        llm=wrapped_llm,
    )

with agentreplay.AgentContext(agent_id="writer", workflow_id="article-creation"):
    writer = Agent(
        role="Content Writer",
        goal="Write engaging articles",
        llm=wrapped_llm,
    )

# Run the crew
crew = Crew(agents=[researcher, writer], tasks=[research_task, write_task])
result = crew.kickoff()

agentreplay.flush()
```

### LlamaIndex

```python
from llama_index.core import VectorStoreIndex, SimpleDirectoryReader
import agentreplay

agentreplay.init()

@agentreplay.traceable(name="index_documents", kind=agentreplay.SpanKind.EMBEDDING)
def build_index(directory: str):
    documents = SimpleDirectoryReader(directory).load_data()
    return VectorStoreIndex.from_documents(documents)

@agentreplay.traceable(name="query_index", kind=agentreplay.SpanKind.RETRIEVER)
def query(index, question: str):
    query_engine = index.as_query_engine()
    return query_engine.query(question)

index = build_index("./documents")
response = query(index, "What is the main topic?")
agentreplay.flush()
```

---

## 🌐 Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `AGENTREPLAY_API_KEY` | API key for authentication | **Required** |
| `AGENTREPLAY_PROJECT_ID` | Project identifier | **Required** |
| `AGENTREPLAY_BASE_URL` | API base URL | `https://api.agentreplay.io` |
| `AGENTREPLAY_TENANT_ID` | Tenant identifier | `default` |
| `AGENTREPLAY_AGENT_ID` | Default agent ID | `default` |
| `AGENTREPLAY_ENABLED` | Enable/disable tracing | `true` |
| `AGENTREPLAY_DEBUG` | Enable debug logging | `false` |
| `AGENTREPLAY_BATCH_SIZE` | Spans per batch | `100` |
| `AGENTREPLAY_FLUSH_INTERVAL` | Auto-flush interval (seconds) | `5.0` |
| `AGENTREPLAY_CAPTURE_INPUT` | Capture function inputs | `true` |
| `AGENTREPLAY_CAPTURE_OUTPUT` | Capture function outputs | `true` |

---

## 🧪 Testing

### Disable Tracing in Tests

```python
import agentreplay
import pytest

@pytest.fixture(autouse=True)
def disable_tracing():
    """Disable tracing for all tests."""
    agentreplay.init(enabled=False)
    yield
    agentreplay.reset()

def test_my_function():
    # Tracing is disabled, no network calls
    result = my_traced_function("test")
    assert result == expected
```

Or use environment variable:

```bash
AGENTREPLAY_ENABLED=false pytest
```

### Mock the SDK

```python
from unittest.mock import patch

def test_with_mock():
    with patch('agentreplay.flush'):
        # flush() won't actually send data
        result = my_function()
```

---

## 📚 Complete API Reference

### Top-Level Functions

| Function | Description |
|----------|-------------|
| `init(**config)` | Initialize the SDK with configuration |
| `flush(timeout=None)` | Send all pending traces |
| `shutdown(timeout=None)` | Graceful shutdown with flush |
| `reset()` | Reset SDK state completely |
| `get_stats()` | Get diagnostic statistics |
| `ping()` | Check backend connectivity |

### Decorators & Tracing

| Function | Description |
|----------|-------------|
| `@traceable` | Decorator for function tracing |
| `@observe` | Alias for `@traceable` (Langfuse-style) |
| `trace(name, **opts)` | Context manager for creating spans |
| `start_span(name, **opts)` | Create a manual span |
| `get_current_span()` | Get the currently active span |

### Client Wrappers

| Function | Description |
|----------|-------------|
| `wrap_openai(client, **opts)` | Wrap OpenAI client |
| `wrap_anthropic(client, **opts)` | Wrap Anthropic client |
| `wrap_method(obj, method, **opts)` | Wrap any method |

### Context Management

| Function | Description |
|----------|-------------|
| `set_context(**ctx)` | Set global context |
| `get_global_context()` | Get current global context |
| `clear_context()` | Clear all global context |
| `with_context(**ctx)` | Scoped context manager |
| `AgentContext(...)` | Class-based agent context |

### Privacy

| Function | Description |
|----------|-------------|
| `configure_privacy(**opts)` | Configure redaction settings |
| `redact_payload(data)` | Redact sensitive data from dict |
| `redact_string(text)` | Redact patterns from string |
| `hash_pii(value, salt=None)` | Hash PII for anonymization |
| `add_pattern(regex)` | Add redaction pattern at runtime |
| `add_scrub_path(path)` | Add scrub path at runtime |

### ActiveSpan Methods

| Method | Description |
|--------|-------------|
| `set_input(data)` | Set span input data |
| `set_output(data)` | Set span output data |
| `set_attribute(key, value)` | Set a single attribute |
| `set_attributes(dict)` | Set multiple attributes |
| `add_event(name, attrs)` | Add a timestamped event |
| `set_error(exception)` | Record an error |
| `set_token_usage(...)` | Set LLM token counts |
| `set_model(model, provider)` | Set model information |
| `end()` | End the span |

---

## 🤝 Contributing

We welcome contributions! See [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.

```bash
# Clone the repository
git clone https://github.com/agentreplay/agentreplay.git
cd agentreplay/sdks/python

# Create virtual environment
python -m venv .venv
source .venv/bin/activate  # or `.venv\Scripts\activate` on Windows

# Install in development mode
pip install -e ".[dev]"

# Run tests
pytest tests/ -v

# Run linter
ruff check src/

# Run type checker
mypy src/agentreplay

# Run formatter
ruff format src/
```

---

## 📄 License

Apache 2.0 - see [LICENSE](../../LICENSE) for details.

---

## 🔗 Links

- 📖 [Documentation](https://docs.agentreplay.io)
- 💻 [GitHub Repository](https://github.com/agentreplay/agentreplay)
- 📦 [PyPI Package](https://pypi.org/project/agentreplay/)
- 💬 [Discord Community](https://discord.gg/agentreplay)
- 🐦 [Twitter](https://twitter.com/agentreplay)

---

<p align="center">
  Made with ❤️ by the Agentreplay team
</p>
