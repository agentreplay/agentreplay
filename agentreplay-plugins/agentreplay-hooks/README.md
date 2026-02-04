# Agent Replay Hooks - Coding Agent Observability Plugin

**Agent Replay Hooks** captures traces from coding agents (Claude Code, Cursor, Copilot, etc.) and forwards them to Agent Replay for observability and analysis.

## Features

- 🔗 Session tracking for coding agent conversations
- 📝 Observation capture for file reads, edits, bash commands
- 📊 Automatic summarization on session end
- 🎯 Support for Cursor, Claude Code, and other hook-enabled agents

## Installation

### Quick Install

```bash
# From agentreplay directory
./agentreplay-plugins/agentreplay-hooks/install.sh
```

### Manual Install

1. Copy the `agentreplay` hook script to your PATH:
   ```bash
   cp agentreplay-plugins/agentreplay-hooks/agentreplay-hook /usr/local/bin/
   chmod +x /usr/local/bin/agentreplay-hook
   ```

2. Set up hook configurations for your coding agent (see below)

## Configuration

### Environment Variables

Create a `.env` file or set these environment variables:

```bash
export AGENTREPLAY_URL="http://localhost:47100"
export AGENTREPLAY_PROJECT_ID="1"
```

### Cursor Integration

Copy the Cursor hooks configuration:

```bash
mkdir -p ~/.cursor/hooks
cp agentreplay-plugins/agentreplay-hooks/hooks/cursor/* ~/.cursor/hooks/
chmod +x ~/.cursor/hooks/*.sh
```

Then add to your Cursor settings (`.cursor/hooks.json`):

```json
{
  "version": 1,
  "hooks": {
    "sessionStart": [{"command": "~/.cursor/hooks/agentreplay-session-start.sh"}],
    "preToolUse": [{"command": "~/.cursor/hooks/agentreplay-observation.sh"}],
    "postToolUse": [{"command": "~/.cursor/hooks/agentreplay-observation.sh"}],
    "beforeReadFile": [{"command": "~/.cursor/hooks/agentreplay-observation.sh"}],
    "afterFileEdit": [{"command": "~/.cursor/hooks/agentreplay-observation.sh"}],
    "beforeShellExecution": [{"command": "~/.cursor/hooks/agentreplay-observation.sh"}],
    "afterShellExecution": [{"command": "~/.cursor/hooks/agentreplay-observation.sh"}],
    "stop": [{"command": "~/.cursor/hooks/agentreplay-summarize.sh"}],
    "sessionEnd": [{"command": "~/.cursor/hooks/agentreplay-summarize.sh"}]
  }
}
```

### Claude Code Integration

Add to your Claude Code settings (`.claude/settings.json`):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": ".*",
        "hooks": [
          {"type": "command", "command": "agentreplay-hook session-init --platform claude", "async": true}
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": ".*",
        "hooks": [
          {"type": "command", "command": "agentreplay-hook observation --platform claude", "async": true}
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {"type": "command", "command": "agentreplay-hook summarize --platform claude", "async": true}
        ]
      }
    ]
  }
}
```

## Usage

### CLI Commands

```bash
# Initialize a session
agentreplay-hook session-init --platform cursor

# Record an observation
agentreplay-hook observation --platform cursor

# Summarize and end a session  
agentreplay-hook summarize --platform cursor

# Check status
agentreplay-hook status
```

### Viewing Sessions

Open Agent Replay and navigate to **Coding Sessions** in the sidebar to view:
- Active and completed sessions
- Timeline of all observations
- File read/edit statistics
- Session summaries

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Cursor/Claude  │────>│ agentreplay-hook│────>│  Agent Replay   │
│    Code Hooks   │     │  (transforms)   │     │  Coding API     │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                              │
                              ▼
                    POST /api/v1/coding-sessions
                    POST /api/v1/coding-sessions/:id/observations
```

## License

Apache-2.0
