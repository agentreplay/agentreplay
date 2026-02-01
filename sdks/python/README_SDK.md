# Agent Replay Python SDK

**Zero-code instrumentation for LLM applications with agent context tracking.**

Agent Replay automatically captures traces from OpenAI, Anthropic, and other LLM providers using OpenTelemetry, with zero code changes required.

## Features

- ✅ **Zero-Code Instrumentation**: Just set environment variables, no code changes needed
- ✅ **Streaming Support**: Properly handles streaming responses without consuming the stream
- ✅ **Agent Context Tracking**: Track which agent made which LLM call in multi-agent systems
- ✅ **Privacy Controls**: Configurable message content capture and truncation
- ✅ **Tool Call Tracking**: Captures OpenAI and Anthropic function/tool calls
- ✅ **Standard OTLP**: Uses industry-standard OpenTelemetry Protocol
- ✅ **Cost Tracking**: Automatic token usage capture for cost calculation

## Quick Start

### 1. Install

```bash
pip install agentreplay
```

### 2. Set Environment Variables

```bash
export AGENTREPLAY_ENABLED=true
export AGENTREPLAY_URL=http://localhost:47100
export OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true
```

### 3. Run Your Code

That's it! Your LLM calls are now automatically traced.

```python
from openai import OpenAI

client = OpenAI()

# This call is automatically traced!
response = client.chat.completions.create(
    model="gpt-4o-mini",
    messages=[{"role": "user", "content": "Hello!"}]
)
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `AGENTREPLAY_ENABLED` | Enable auto-instrumentation | `false` |
| `AGENTREPLAY_URL` | Agent Replay server URL | `http://localhost:47100` |
| `AGENTREPLAY_TENANT_ID` | Tenant ID | `1` |
| `AGENTREPLAY_PROJECT_ID` | Project ID | `0` |
| `AGENTREPLAY_DEBUG` | Enable debug logging | `false` |
| `OTEL_SERVICE_NAME` | Service name for traces | Script name |
| `OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT` | Capture prompts/responses | `false` |
| `AGENTREPLAY_MAX_CONTENT_LENGTH` | Max characters per message (0 = unlimited) | `10000` |
| `AGENTREPLAY_MAX_MESSAGES` | Max messages to capture (0 = all) | `0` |
| `AGENTREPLAY_TRUNCATE_CONTENT` | Enable truncation | `true` |

### Configuration Presets

**Development (Full Capture)**
```bash
export AGENTREPLAY_ENABLED=true
export OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true
export AGENTREPLAY_MAX_MESSAGES=0
export AGENTREPLAY_TRUNCATE_CONTENT=false
```

**Production (Privacy-Safe)**
```bash
export AGENTREPLAY_ENABLED=true
export OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true
export AGENTREPLAY_MAX_MESSAGES=5
export AGENTREPLAY_TRUNCATE_CONTENT=true
export AGENTREPLAY_MAX_CONTENT_LENGTH=500
```

**Compliance (No Content)**
```bash
export AGENTREPLAY_ENABLED=true
export OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=false
```

## Advanced Usage

### Agent Context Tracking

Track which agent made which LLM call in multi-agent systems:

```python
from agentreplay.context import AgentContext
from openai import OpenAI

client = OpenAI()

# Research agent
with AgentContext(agent_id="researcher", session_id="sess-123"):
    response = client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[{"role": "user", "content": "Research topic X"}]
    )

# Writer agent
with AgentContext(agent_id="writer", session_id="sess-123"):
    response = client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[{"role": "user", "content": "Write about topic X"}]
    )
```

Traces will show:
- `gen_ai.agent.id`: "researcher" or "writer"
- `gen_ai.session.id`: "sess-123"
- Cost breakdown by agent
- Agent handoff patterns

### Streaming Responses

Streaming is automatically handled - no special code needed:

```python
stream = client.chat.completions.create(
    model="gpt-4o-mini",
    messages=[{"role": "user", "content": "Count to 10"}],
    stream=True
)

for chunk in stream:
    print(chunk.choices[0].delta.content, end="")

# Trace is automatically completed after stream exhaustion
# Token counts and full content are captured
```

### Manual Initialization (Optional)

If you prefer not to use auto-initialization:

```python
from agentreplay import init_otel_instrumentation

init_otel_instrumentation(
    service_name="my-app",
    agentreplay_url="http://localhost:47100",
    tenant_id=1,
    project_id=0,
    capture_content=True
)

# Now make LLM calls...
```

## Supported Libraries

- ✅ OpenAI (including streaming and function calling)
- ✅ Anthropic (including streaming and tool use)
- ⏳ AWS Bedrock (coming soon)
- ⏳ LangChain (coming soon)
- ⏳ LlamaIndex (coming soon)

## How It Works

1. **`.pth` File**: Installed to site-packages during `pip install`, automatically imports bootstrap module when Python starts
2. **Bootstrap**: Checks `AGENTREPLAY_ENABLED` env var and initializes OpenTelemetry if enabled
3. **Monkey Patching**: Wraps OpenAI/Anthropic methods to inject tracing
4. **Stream Wrapping**: Wraps streaming responses to collect telemetry without consuming the stream
5. **OTLP Export**: Uses standard OpenTelemetry Protocol to send traces to Agent Replay backend

## Backend Setup

Start the Agent Replay backend:

```bash
cd /path/to/chronolake
./start-web.sh
```

The backend runs on:
- HTTP: `http://localhost:47100`
- OTLP gRPC: `localhost:47117`
- OTLP HTTP: `http://localhost:4318/v1/traces`
- UI: `http://localhost:47173`

## Troubleshooting

**Traces not showing up?**

1. Check environment variables:
   ```bash
   echo $AGENTREPLAY_ENABLED  # Should be "true"
   echo $AGENTREPLAY_URL      # Should be "http://localhost:47100"
   ```

2. Enable debug mode:
   ```bash
   export AGENTREPLAY_DEBUG=true
   python your_app.py
   ```

3. Check backend is running:
   ```bash
   curl http://localhost:47100/health
   ```

4. Verify OTLP endpoint:
   ```bash
   curl -X POST http://localhost:4318/v1/traces
   ```

**Streaming not working?**

The SDK automatically handles streaming. Make sure you're iterating through the entire stream:

```python
stream = client.chat.completions.create(..., stream=True)

# ✅ Correct - iterate through all chunks
for chunk in stream:
    print(chunk.choices[0].delta.content, end="")

# ❌ Wrong - don't convert to list (consumes stream)
chunks = list(stream)
```

## Examples

See `examples/zero_code_example.py` for a complete example demonstrating:
- Simple calls
- Streaming responses
- Agent context tracking
- Tool/function calling

Run it:
```bash
export AGENTREPLAY_ENABLED=true
export OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true
export OPENAI_API_KEY=your-key-here
python examples/zero_code_example.py
```

## Development

```bash
# Install in development mode
cd sdks/python
pip install -e .

# Run tests
pytest tests/

# Format code
black src/
ruff check src/
```

## License

MIT

## Links

- [GitHub](https://github.com/sochdb/agentreplay)
- [Documentation](https://docs.agentreplay.dev)
- [Backend Setup](../../README.md)
