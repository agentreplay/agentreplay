# Agent Replay Memory Plugin for Claude Code

A Claude Code plugin that provides persistent, local-first memory storage using Agent Replay as the backend.

## Overview

This plugin automatically captures your coding sessions and makes relevant context available in future sessions. Unlike cloud-based alternatives, all data remains on your local machine.

**Key capabilities:**
- Automatic session capture on stop
- Context injection on session start
- Semantic memory search
- Manual memory storage via CLI

## Requirements

- Node.js 18+
- Agent Replay desktop app running locally
- Claude Code CLI

## Setup

```bash
# Build the plugin
cd sdks/claude-memory
npm install
npm run build

# Copy to Claude plugins directory
cp -r plugin ~/.claude/plugins/agentreplay-memory
```

Alternative: Add via Claude Code CLI:
```bash
/plugin marketplace add /path/to/sdks/claude-memory/plugin
/plugin install agentreplay-memory
```

## How It Works

### Session Start

When you begin a new Claude Code session, the plugin queries your local Agent Replay instance for relevant memories and injects them as context:

```xml
<memory-context>
Recalled from local Agent Replay storage.

## Preferences
• Prefers TypeScript with strict mode
• Uses pnpm for package management

## Context
• Working on user authentication module
• Recently fixed database connection pooling
</memory-context>
```

### Session End

When you stop a session, new conversation content is automatically persisted to Agent Replay for future reference.

### Manual Commands

**Index codebase:**
```
/agentreplay-memory:index
```

**Check status:**
```
/agentreplay-memory:status
```

**Reset settings:**
```
/agentreplay-memory:clear
```

## Configuration

### Environment Variables

```bash
AGENTREPLAY_URL=http://localhost:47100    # Server endpoint
AGENTREPLAY_TENANT_ID=1                   # Multi-tenant ID
AGENTREPLAY_PROJECT_ID=1                  # Project scope
AGENTREPLAY_DEBUG=true                    # Enable verbose logging
AGENTREPLAY_SKIP_TOOLS=Read,Glob,Grep     # Tools to exclude from capture
```

### Config File

Location: `~/.agentreplay-claude/config.json`

```json
{
  "serverUrl": "http://localhost:47100",
  "tenantId": 1,
  "projectId": 1,
  "ignoredTools": ["Read", "Glob", "Grep", "TodoWrite"],
  "trackedTools": ["Edit", "Write", "Bash", "Task"],
  "contextLimit": 5,
  "verbose": false,
  "autoInject": true
}
```

## Project Layout

```
claude-memory/
├── package.json
├── biome.json
├── scripts/
│   └── build.js
├── src/
│   ├── context-hook.js      # Session start handler
│   ├── summary-hook.js      # Session end handler
│   ├── prompt-hook.js       # Prompt handler
│   ├── observation-hook.js  # Tool use handler
│   ├── add-memory.js        # CLI: store content
│   ├── search-memory.js     # CLI: query memories
│   └── lib/
│       ├── agentreplay-client.js   # HTTP client
│       ├── settings.js             # Config management
│       ├── container-tag.js        # Workspace identification
│       ├── format-context.js       # Context XML builder
│       ├── stdin.js                # Hook I/O
│       ├── transcript-formatter.js # Session parsing
│       └── validate.js             # Input checks
└── plugin/
    ├── .claude-plugin/
    │   └── plugin.json
    ├── hooks/
    ├── commands/
    ├── skills/
    └── scripts/
```

## Privacy

- All data stored locally via Agent Replay
- No external network requests
- No cloud accounts or API keys required
- Full control over your data

## Development

```bash
npm install       # Install dependencies
npm run build     # Build plugin bundles
npm run lint      # Run linter
npm run format    # Format code
```

## License

MIT
