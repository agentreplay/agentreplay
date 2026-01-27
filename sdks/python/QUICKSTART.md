# Flowtrace Python SDK - Quick Start

## Zero-Code Instrumentation 🚀

Flowtrace provides **true zero-code observability** - just like LangSmith!

### Installation

```bash
pip install flowtrace
```

### Setup (One-Time)

```bash
# Set environment variables
export FLOWTRACE_ENABLED=true
export FLOWTRACE_PROJECT_ID=your-project-id
```

### Usage

**That's it!** Run your existing code with ZERO changes:

```bash
python your_app.py  # ✅ Automatically traced!
```

## How It Works

Flowtrace uses Python's `.pth` file mechanism to automatically instrument your code **before it runs**:

1. Install SDK → `.pth` file added to site-packages
2. Set `FLOWTRACE_ENABLED=true`
3. Run your code → Auto-instrumented!

**No imports. No decorators. No code changes.**

## Example: Pure OpenAI Code

```python
# your_app.py - NO FLOWTRACE IMPORTS!
from openai import AzureOpenAI
import os

client = AzureOpenAI(
    azure_endpoint=os.environ['AZURE_OPENAI_ENDPOINT'],
    api_key=os.environ['AZURE_OPENAI_API_KEY'],
    api_version='2024-12-01-preview'
)

response = client.chat.completions.create(
    model='gpt-4',
    messages=[{'role': 'user', 'content': 'Hello!'}]
)

print(response.choices[0].message.content)
```

**Run it:**

```bash
export FLOWTRACE_ENABLED=true
export FLOWTRACE_PROJECT_ID=my-project
python your_app.py  # ✅ Traces automatically appear in Flowtrace!
```

## Supported Frameworks

Flowtrace automatically instruments:

- ✅ **OpenAI** - Direct API calls
- ✅ **LangChain** - Chains, agents, tools
- ✅ **LangGraph** - Multi-agent workflows
- ✅ **LlamaIndex** - Query engines, agents
- ✅ **CrewAI** - Multi-agent systems
- ✅ **AutoGen** - Conversational agents
- ✅ **Any framework using the above**

**All with zero code changes!**

## Environment Variables

### Required

```bash
export FLOWTRACE_ENABLED=true  # Enable auto-instrumentation
```

### Optional

```bash
export FLOWTRACE_OTLP_ENDPOINT=localhost:4317  # Default: localhost:4317
export FLOWTRACE_PROJECT_ID=0                  # Default: 0
export FLOWTRACE_TENANT_ID=1                   # Default: 1
export FLOWTRACE_SERVICE_NAME=my-app           # Default: python-app
export FLOWTRACE_LOG_LEVEL=DEBUG               # Default: INFO
```

## Comparison: Flowtrace vs Others

### LangSmith

```bash
export LANGCHAIN_API_KEY=xxx
export LANGCHAIN_TRACING_V2=true
python app.py  # ✅ Auto-traces
```

### Flowtrace

```bash
export FLOWTRACE_ENABLED=true
export FLOWTRACE_OTLP_ENDPOINT=localhost:4317
python app.py  # ✅ Auto-traces (SAME UX!)
```

| Feature | Flowtrace | LangSmith | Manual OTEL |
|---------|-----------|-----------|-------------|
| **Zero code changes** | ✅ | ✅ | ❌ |
| **Local deployment** | ✅ | ❌ | ✅ |
| **Framework agnostic** | ✅ | ⚠️ | ✅ |
| **W3C standard** | ✅ OTLP | ⚠️ | ✅ OTLP |
| **Setup complexity** | 1 env var | 2 env vars | ~50 lines |

## Examples

### LangGraph Multi-Agent

```python
# langgraph_agent.py - ZERO FLOWTRACE IMPORTS!
from typing import TypedDict, Annotated
from langchain_openai import ChatOpenAI
from langchain_community.tools.tavily_search import TavilySearchResults
from langgraph.graph import StateGraph, START, END
from langgraph.graph.message import add_messages
from langgraph.prebuilt import ToolNode, tools_condition

class State(TypedDict):
    messages: Annotated[list, add_messages]

def agent(state: State):
    llm = ChatOpenAI(model="gpt-4")
    tools = [TavilySearchResults(max_results=3)]
    llm_with_tools = llm.bind_tools(tools)
    response = llm_with_tools.invoke(state["messages"])
    return {"messages": [response]}

workflow = StateGraph(State)
workflow.add_node("agent", agent)
workflow.add_node("tools", ToolNode([TavilySearchResults(max_results=3)]))
workflow.add_edge(START, "agent")
workflow.add_conditional_edges("agent", tools_condition)
workflow.add_edge("tools", "agent")

graph = workflow.compile()

# Run - NO FLOWTRACE CODE!
result = graph.invoke({
    "messages": [("user", "What's the weather in SF?")]
})
print(result["messages"][-1].content)
```

**Run it:**

```bash
export FLOWTRACE_ENABLED=true
export FLOWTRACE_PROJECT_ID=my-project
python langgraph_agent.py  # ✅ All agents automatically traced!
```

### CrewAI (Existing Code)

```python
# existing_crewai_app.py - NO MODIFICATIONS!
from crewai import Agent, Task, Crew
from langchain_openai import ChatOpenAI

researcher = Agent(
    role='Researcher',
    goal='Research AI trends',
    backstory='Expert researcher',
    llm=ChatOpenAI(model="gpt-4")
)

writer = Agent(
    role='Writer',
    goal='Write articles',
    backstory='Professional writer',
    llm=ChatOpenAI(model="gpt-4")
)

research_task = Task(
    description='Research latest AI developments',
    agent=researcher
)

write_task = Task(
    description='Write article based on research',
    agent=writer
)

crew = Crew(
    agents=[researcher, writer],
    tasks=[research_task, write_task]
)

# Your existing code - unchanged!
result = crew.kickoff()
print(result)
```

**Run it:**

```bash
export FLOWTRACE_ENABLED=true
python existing_crewai_app.py  # ✅ All agents traced!
```

## Verification

### 1. Check Installation

```bash
python -c "import site; import os; pkg = site.getsitepackages()[0]; print(f'Installed: {os.path.exists(os.path.join(pkg, \"flowtrace-init.pth\"))}')"
```

**Expected:** `Installed: True`

### 2. Test Auto-Init

```bash
export FLOWTRACE_ENABLED=true
export FLOWTRACE_LOG_LEVEL=DEBUG
python -c "print('Test')"
```

**Expected:**

```
[flowtrace.env_init] INFO: 🚀 Initializing Flowtrace
[flowtrace.env_init] INFO: ✅ Flowtrace auto-instrumentation enabled
Test
```

### 3. Test with OpenAI

```bash
export FLOWTRACE_ENABLED=true
export FLOWTRACE_PROJECT_ID=my-project
export OPENAI_API_KEY=xxx

python -c "
from openai import OpenAI
client = OpenAI()
response = client.chat.completions.create(
    model='gpt-4',
    messages=[{'role': 'user', 'content': 'Hello!'}]
)
print(response.choices[0].message.content)
"
```

**Check traces:**

```bash
# View server logs
./view-logs.sh otel

# Should see:
# OTLP: Successfully stored 1 spans for project my-project
```

## Architecture

### How Auto-Instrumentation Works

```
Python Startup
    ↓
Loads flowtrace-init.pth (from site-packages)
    ↓
Imports flowtrace.env_init
    ↓
Checks FLOWTRACE_ENABLED
    ↓
Calls auto_instrument()
    ↓
Sets up OTLP gRPC exporter (localhost:4317)
    ↓
Auto-discovers libraries (OpenAI, LangChain, etc.)
    ↓
Instruments with official OTEL instrumentations
    ↓
Registers atexit handler for span flushing
    ↓
Your Code Runs (zero changes!)
    ↓
LLM calls automatically traced
    ↓
Program Exits → atexit flushes spans
    ↓
Spans sent to server via OTLP
    ↓
✅ Visible in Flowtrace UI!
```

### Pure OpenTelemetry

Flowtrace uses **official OpenTelemetry instrumentations**:

- `opentelemetry-instrumentation-openai`
- `opentelemetry-instrumentation-anthropic`
- `opentelemetry-instrumentation-langchain`
- `opentelemetry-instrumentation-llamaindex`

**No custom framework code!** We leverage the OTEL community ecosystem.

### OTLP Protocol

- **Protocol**: OTLP over gRPC (W3C standard)
- **Endpoint**: `localhost:4317` (binary + multiplexing)
- **Format**: Protobuf (efficient serialization)
- **Batching**: Automatic via `BatchSpanProcessor`
- **Flushing**: Automatic on program exit via `atexit`

## Troubleshooting

### No traces appearing

1. **Check if enabled:**
   ```bash
   echo $FLOWTRACE_ENABLED  # Should print: true
   ```

2. **Enable debug logging:**
   ```bash
   export FLOWTRACE_LOG_LEVEL=DEBUG
   python your_app.py
   
   # Should see:
   # [flowtrace.env_init] INFO: 🚀 Initializing Flowtrace
   # [flowtrace.env_init] INFO: ✅ Flowtrace auto-instrumentation enabled
   ```

3. **Check server is running:**
   ```bash
   lsof -i:4317  # Should show flowtrace-server listening
   ```

4. **View server logs:**
   ```bash
   ./view-logs.sh otel
   
   # Should see:
   # OTLP: Successfully stored X spans
   ```

### `.pth` file not loading

```bash
# Check if installed
python -c "import site; print(site.getsitepackages())"
ls /path/to/site-packages/flowtrace-init.pth

# Reinstall if missing
pip uninstall flowtrace -y
pip install flowtrace
```

### Spans not flushing

The SDK automatically flushes spans on program exit via `atexit`. If you're running in a container or notebook, you may need to manually flush:

```python
from opentelemetry import trace

# At the end of your code
provider = trace.get_tracer_provider()
provider.force_flush(timeout_millis=5000)
```

## Advanced Configuration

### Disable Auto-Init

If you want manual control:

```bash
export FLOWTRACE_AUTO_INIT=false
```

Then in your code:

```python
from flowtrace.auto_instrument import setup_instrumentation

setup_instrumentation(
    service_name="my-app",
    project_id=123
)
```

### Custom Service Name

```bash
export FLOWTRACE_SERVICE_NAME=my-cool-app
```

### Multiple Projects

```bash
# Project A
export FLOWTRACE_PROJECT_ID=project-a
python app_a.py

# Project B
export FLOWTRACE_PROJECT_ID=project-b
python app_b.py
```

## Documentation

- **Quick Start**: This file
- **Implementation Details**: `ZERO_CODE_SETUP_COMPLETE.md`
- **Status & Verification**: `IMPLEMENTATION_STATUS.md`
- **Examples**: `examples/test_zero_code.py`, `examples/langgraph_agent_with_tools.py`

## Getting Help

1. **Enable debug logging**: `export FLOWTRACE_LOG_LEVEL=DEBUG`
2. **Check server logs**: `./view-logs.sh all`
3. **Verify installation**: See "Verification" section above
4. **Open an issue**: Include debug logs and example code

## Next Steps

- ✅ Install SDK: `pip install flowtrace`
- ✅ Set env vars: `export FLOWTRACE_ENABLED=true`
- ✅ Run your code: `python your_app.py`
- ✅ View traces: `http://localhost:5173`

**That's it! Zero code changes, automatic observability.** 🎉
