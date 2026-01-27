# @sochdb/flowtrace-claude-code

Flowtrace observability plugin for [Claude Code](https://github.com/anthropics/claude-code) - automatic tracing of all tool calls and sessions.

## Features

- 📊 **Automatic Tracing**: Traces every tool call (Bash, Edit, Write, Read, etc.)
- 🔗 **Session Tracking**: Links all tool calls within a session
- ⏱️ **Duration Tracking**: Measures time spent on each tool execution
- 🚫 **Non-blocking**: Never interrupts or slows down Claude Code
- 📈 **Dashboard Integration**: View traces in Flowtrace UI

## Installation

```bash
npm install -g @sochdb/flowtrace-claude-code
```

This will automatically install the plugin to `~/.claude/plugins/flowtrace`.

### Manual Installation

If auto-install doesn't work, run:

```bash
npx @sochdb/flowtrace-claude-code
```

Or copy manually:

```bash
cp -r node_modules/@sochdb/flowtrace-claude-code/plugin ~/.claude/plugins/flowtrace
```

## Configuration

The plugin is configured via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `FLOWTRACE_ENABLED` | `true` | Enable/disable tracing |
| `FLOWTRACE_URL` | `http://localhost:9600` | Flowtrace server URL |
| `FLOWTRACE_TENANT_ID` | `1` | Tenant identifier |
| `FLOWTRACE_PROJECT_ID` | `1` | Project identifier |

### Example

```bash
# Set Flowtrace URL
export FLOWTRACE_URL="http://localhost:9600"

# Run Claude Code
claude
```

## What Gets Traced

### Session Events

- **SessionStart**: When a Claude Code session begins
- **Stop**: When a session ends

### Tool Calls

Every tool invocation is traced with:

| Tool | Traced Data |
|------|-------------|
| `Bash` | Command, output, exit code, duration |
| `Edit` | File path, changes, duration |
| `Write` | File path, content length, duration |
| `Read` | File path, lines read, duration |
| `Glob` | Pattern, matches, duration |
| `Grep` | Pattern, file, matches, duration |
| `LS` | Directory, entries, duration |
| `Task` | Subtask description, result, duration |

## Viewing Traces

1. Start Flowtrace:
   ```bash
   open /Applications/Flowtrace.app
   # or
   flowtrace serve
   ```

2. Open the Flowtrace UI at `http://localhost:9600`

3. Navigate to **Traces** to see all Claude Code activity

4. Filter by:
   - Project
   - Session
   - Tool name
   - Time range

## Plugin Structure

```
flowtrace/
├── .claude-plugin/
│   └── plugin.json          # Plugin metadata
├── core/
│   ├── __init__.py
│   └── client.py            # Flowtrace API client
├── hooks/
│   ├── __init__.py
│   ├── hooks.json           # Hook definitions
│   ├── sessionstart.py      # Session start handler
│   ├── pretooluse.py        # Pre-tool execution handler
│   ├── posttooluse.py       # Post-tool execution handler
│   └── stop.py              # Session end handler
└── README.md
```

## Troubleshooting

### Traces not appearing

1. Check Flowtrace is running:
   ```bash
   curl http://localhost:9600/api/v1/health
   ```

2. Verify environment variable:
   ```bash
   echo $FLOWTRACE_URL
   ```

3. Check plugin is loaded:
   ```bash
   claude /plugin list
   ```

### Disable temporarily

```bash
export FLOWTRACE_ENABLED=false
claude
```

## Development

To modify the plugin:

1. Edit hooks in `hooks/` directory
2. Modify the client in `core/client.py`
3. Test with Claude Code:
   ```bash
   claude
   ```

## License

MIT
