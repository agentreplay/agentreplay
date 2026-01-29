# @agentreplay/clawdbot-plugin

Agent Replay observability plugin for [Clawdbot](https://github.com/anthropics/clawdbot) - automatic tracing of agent activities, tool calls, and memory operations.

## Features

- 📊 **Automatic Tracing**: Traces every agent run, tool call, and response
- 🧠 **Memory Tracking**: Automatically tracks memory store/recall/forget operations
- 🔗 **Parent-Child Relationships**: Tool calls are linked to their parent agent runs
- ⏱️ **Duration Tracking**: Measures time spent on each operation
- 🏷️ **Rich Metadata**: Captures session keys, workspace info, and more
- 📈 **Dashboard Integration**: View traces in Agent Replay UI

## Installation

```bash
npm install @agentreplay/clawdbot-plugin
```

## Setup

### 1. Start Agent Replay

Make sure Agent Replay is running. By default, it runs on `http://localhost:9600`.

```bash
# Using the Agent Replay desktop app
open /Applications/Agent Replay.app

# Or using the CLI
agentreplay serve
```

### 2. Configure the Plugin

Add to your `clawdbot.json`:

```json
{
  "plugins": {
    "@agentreplay/clawdbot-plugin": {
      "enabled": true,
      "url": "http://localhost:9600",
      "tenant_id": 1,
      "project_id": 1
    }
  }
}
```

Or use environment variables:

```bash
export AGENTREPLAY_URL="http://localhost:9600"
export AGENTREPLAY_TENANT_ID="1"
export AGENTREPLAY_PROJECT_ID="1"
```

### 3. Restart Clawdbot

```bash
clawdbot restart
```

## Configuration

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable or disable tracing |
| `url` | string | `http://localhost:9600` | Agent Replay server URL |
| `tenant_id` | number | `1` | Tenant identifier |
| `project_id` | number | `1` | Project identifier |

## Commands

### `/agentreplay`

Shows the current Agent Replay integration status:

```
📊 Agent Replay Status

• Enabled: Yes
• Server: http://localhost:9600
• Tenant: 1
• Project: 1
```

## What Gets Traced

### Agent Runs

Every time the agent processes a message:
- Session key and agent ID
- Workspace directory
- Message provider (Telegram, WhatsApp, etc.)
- Prompt length
- Duration and success status

### Tool Calls

Every tool invocation:
- Tool name
- Input parameters
- Output/result
- Duration
- Error status (if any)

### Memory Operations

When using memory plugins (e.g., `memory-lancedb`):

| Operation | Tracked Data |
|-----------|--------------|
| `memory_store` | Text, category, importance, memory ID |
| `memory_recall` | Query, result count, relevance scores |
| `memory_forget` | Memory ID, deleted text |

## Viewing Traces

1. Open Agent Replay desktop app or navigate to `http://localhost:9600`
2. Go to **Traces** section
3. Filter by project, session, or time range
4. Click on a trace to see the timeline and details

### Memory Dashboard

In Agent Replay, you can also view:
- Memory operation frequency
- Most recalled memories
- Store/recall ratio
- Memory search latency

## Troubleshooting

### Traces not appearing

1. Check that Agent Replay is running: `curl http://localhost:9600/api/v1/health`
2. Verify the plugin is enabled: Send `/agentreplay` command
3. Check logs for connection errors

### Connection refused

Make sure the Agent Replay URL is correct and the server is accessible:

```bash
curl -X POST http://localhost:9600/api/v1/traces \
  -H "Content-Type: application/json" \
  -d '{"tenant_id":1,"project_id":1,"agent_id":1,"session_id":1,"span_type":0}'
```

## API Reference

The plugin sends traces to these Agent Replay endpoints:

| Endpoint | Purpose |
|----------|---------|
| `POST /api/v1/traces` | Generic traces (agent runs, errors) |
| `POST /api/v1/traces/tool` | Tool call traces |
| `POST /api/v1/traces/memory` | Memory operation traces |

## License

MIT
