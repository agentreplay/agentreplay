# Claude-AgentReplay

<div align="center">

### 🧠 Local-First Persistent Memory for Claude Code

**No cloud. No subscriptions. Your data stays on your machine.**

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Node](https://img.shields.io/badge/node-18%2B-green.svg)](https://nodejs.org/)

</div>

---

A Claude Code plugin that gives your AI persistent memory across sessions using [Agent Replay](https://agentreplay.dev).
Your agent remembers what you worked on - across sessions, across projects - all stored locally.

## ✨ Features

- **🏠 Local-First**: All data stored on your machine via Agent Replay
- **🔒 Private**: No cloud accounts, no data leaving your machine
- **💾 Unlimited**: Use your full disk, no monthly limits
- **🔄 Context Injection**: On session start, relevant memories are automatically injected
- **📝 Automatic Capture**: Conversation turns are captured and stored for future context
- **🔍 Semantic Search**: Find relevant memories using vector similarity
- **📚 Codebase Indexing**: Index your project's architecture and conventions

## 📦 Installation

### Prerequisites

1. **Agent Replay Desktop** must be running:
   ```bash
   # macOS
   open /Applications/Agent\ Replay.app
   
   # Or run from source
   cd agentreplay && ./run-tauri.sh
   ```

2. **Claude Code** installed

### Install the Plugin

```bash
# From the agentreplay/sdks/claude-memory directory
npm install
npm run build

# Install to Claude Code plugins directory
cp -r plugin ~/.claude/plugins/claude-agentreplay
```

Or add via Claude Code:

```bash
# Add from local directory
/plugin marketplace add /path/to/agentreplay/sdks/claude-memory/plugin

# Install the plugin
/plugin install claude-agentreplay
```

## 🚀 How It Works

### On Session Start

The plugin fetches relevant memories from your local Agent Replay and injects them into Claude's context:

```
<agentreplay-context>
The following is recalled context from your local Agent Replay memory.
Data stored locally on this machine.

## User Preferences (Persistent)
- Prefers TypeScript over JavaScript
- Uses pnpm as package manager

## Recent Context
- Working on authentication flow
- Fixed issue with database connection

</agentreplay-context>
```

### During Session

Conversation turns are automatically captured when you stop and stored for future context.

### Skills

**memory-search**: When you ask about past work, previous sessions, or want to recall information, the agent automatically searches your local memories.

## 📋 Commands

### /claude-agentreplay:index

Index your codebase into Agent Replay. Explores project structure, architecture, conventions, and key files.

```
/claude-agentreplay:index
```

### /claude-agentreplay:status

Check Agent Replay connection and memory statistics.

```
/claude-agentreplay:status
```

### /claude-agentreplay:clear

Clear plugin settings (not memories).

```
/claude-agentreplay:clear
```

## ⚙️ Configuration

### Environment Variables

```bash
# Optional - defaults to localhost:9600
AGENTREPLAY_URL=http://localhost:9600

# Optional - for multi-tenant setups
AGENTREPLAY_TENANT_ID=1
AGENTREPLAY_PROJECT_ID=1

# Optional
AGENTREPLAY_SKIP_TOOLS=Read,Glob,Grep    # Tools to not capture
AGENTREPLAY_DEBUG=true                    # Enable debug logging
```

### Settings File

Create `~/.agentreplay-claude/settings.json`:

```json
{
  "url": "http://localhost:9600",
  "tenantId": 1,
  "projectId": 1,
  "skipTools": ["Read", "Glob", "Grep", "TodoWrite"],
  "captureTools": ["Edit", "Write", "Bash", "Task"],
  "maxProfileItems": 5,
  "debug": false
}
```

## 🏗️ Architecture

```
claude-memory/
├── package.json           # Build tools package
├── biome.json            # Linting config
├── scripts/
│   └── build.js          # esbuild bundler
├── src/
│   ├── context-hook.js   # SessionStart - injects memories
│   ├── summary-hook.js   # Stop - saves conversation
│   ├── prompt-hook.js    # UserPromptSubmit handler
│   ├── observation-hook.js # PostToolUse handler
│   ├── search-memory.js  # CLI search tool
│   ├── add-memory.js     # CLI add tool
│   └── lib/
│       ├── agentreplay-client.js  # API client
│       ├── settings.js            # Config management
│       ├── container-tag.js       # Workspace ID
│       ├── format-context.js      # Context formatting
│       ├── stdin.js               # Hook I/O
│       ├── transcript-formatter.js # Session parsing
│       └── validate.js            # Input validation
└── plugin/               # Claude Code plugin (built)
    ├── .claude-plugin/
    │   └── plugin.json
    ├── hooks/
    │   └── hooks.json
    ├── commands/
    │   ├── index.md
    │   ├── status.md
    │   └── clear.md
    ├── skills/
    │   └── memory-search/
    │       └── SKILL.md
    └── scripts/          # Built CJS bundles
```

## 🔒 Privacy

Unlike cloud-based memory solutions:

- **All data stays local**: Memories are stored in Agent Replay on your machine
- **No external API calls**: The plugin only talks to localhost:9600
- **No accounts required**: No signup, no API keys to manage
- **Full control**: Delete your data anytime by clearing Agent Replay storage

## 🛠️ Development

```bash
# Install dependencies
npm install

# Build plugin
npm run build

# Watch mode (rebuild on changes)
npm run build -- --watch

# Lint
npm run lint

# Format
npm run format
```

## 📄 License

MIT - See [LICENSE](LICENSE)

---

<div align="center">
  <p>Built with ❤️ by the Agent Replay team</p>
  <p>
    <a href="https://agentreplay.dev">Website</a> •
    <a href="https://github.com/sochdb/agentreplay">GitHub</a>
  </p>
</div>
