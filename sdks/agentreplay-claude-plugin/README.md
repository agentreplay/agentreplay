# Agent Replay Plugin for Claude Code

Unified plugin providing **observability tracing** and **persistent memory** for Claude Code sessions.

## Features

- **Tracing**: Automatic capture of all tool calls, sessions, and agent activities
- **Memory**: Context injection from previous sessions, conversation persistence
- **Local-first**: All data stays on your machine via Agent Replay server

## Installation

### Via npm (recommended)

```bash
npm install -g @agentreplay/agentreplay-claude-plugin
```

The plugin auto-installs to `~/.claude/plugins/agentreplay` on npm install.

### Manual

```bash
cd sdks/agentreplay-claude-plugin
npm install
npm run build
cp -r plugin ~/.claude/plugins/agentreplay
```

## Requirements

- Node.js 18+
- Agent Replay server running locally (default: http://localhost:47100)
- Claude Code CLI

## Configuration

### Environment Variables

```bash
AGENTREPLAY_URL=http://localhost:47100   # Server endpoint
AGENTREPLAY_TENANT_ID=1                  # Multi-tenant ID
AGENTREPLAY_PROJECT_ID=1                 # Project scope
AGENTREPLAY_TRACING=true                 # Enable/disable tracing
AGENTREPLAY_MEMORY=true                  # Enable/disable memory
AGENTREPLAY_DEBUG=true                   # Verbose logging
```

### Config File

Location: `~/.agentreplay-claude/config.json`

```json
{
  "serverUrl": "http://localhost:47100",
  "tenantId": 1,
  "projectId": 1,
  "tracingEnabled": true,
  "memoryEnabled": true,
  "verbose": false,
  "contextLimit": 5,
  "ignoredTools": ["Read", "Glob", "Grep", "TodoWrite", "LS"]
}
```

## How It Works

### Session Start

1. Creates a root trace span for the session (observability)
2. Queries memory for relevant context from previous sessions
3. Injects context into Claude's system prompt

### During Session

- **PreToolUse**: Records tool start time
- **PostToolUse**: Sends tool completion trace with duration and result
- **UserPromptSubmit**: Placeholder for future prompt-level features

### Session End

1. Sends session completion trace
2. Persists new conversation content to memory storage

## Project Structure

```
agentreplay-claude-plugin/
├── package.json
├── index.js               # npm entry point
├── bin/
│   └── install.js         # Auto-installer
├── scripts/
│   └── bundle.js          # esbuild bundler
├── src/
│   ├── api.js             # Agent Replay HTTP client
│   ├── common.js          # Shared utilities
│   ├── formatter.js       # Context XML builder
│   └── hooks/
│       ├── session-start.js
│       ├── stop.js
│       ├── pre-tool.js
│       ├── post-tool.js
│       └── prompt.js
└── plugin/                # Claude Code plugin (built)
    ├── .claude-plugin/
    │   └── plugin.json
    ├── hooks/
    │   └── hooks.json
    └── scripts/           # Bundled CJS files
```

## Development

```bash
npm install          # Install dependencies
npm run build        # Bundle hooks
npm run lint         # Check code
npm run format       # Format code
```

## License

MIT
