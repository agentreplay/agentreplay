# Flowtrace Claude Code Bridge

This package bridges standard input/output (stdio) MCP requests from Claude Code to the Flowtrace embedded HTTP MCP server.

## Prerequisites

1. **Flowtrace Desktop must be running** - The MCP server runs on port 9601 when the desktop app is active.
2. **Node.js 18+** installed on your system.

---

## Step-by-Step Setup

### Step 1: Build the Bridge

```bash
cd flowtrace-claude-bridge
npm install
npm run build
```

This creates `dist/index.js` which is the bridge executable.

---

### Step 2: Start Flowtrace Desktop

Run the Flowtrace desktop application. This starts the MCP server on `http://127.0.0.1:9601/mcp`.

```bash
# From the flowtrace root directory
./run-tauri.sh
```

---

### Step 3: Configure Claude Code

Add the following to your Claude Code MCP settings file:

**For VS Code (Claude extension):**
Edit `~/.vscode/settings.json` or your workspace settings:

```json
{
  "claude.mcpServers": {
    "flowtrace-memory": {
      "command": "node",
      "args": ["/absolute/path/to/flowtrace/flowtrace-claude-bridge/dist/index.js"],
      "env": {
        "FLOWTRACE_URL": "http://127.0.0.1:9601/mcp"
      }
    }
  }
}
```

**For Claude Desktop (standalone app):**
Edit `~/.claude/mcp-settings.json`:

```json
{
  "mcpServers": {
    "flowtrace-memory": {
      "command": "node",
      "args": ["/absolute/path/to/flowtrace/flowtrace-claude-bridge/dist/index.js"],
      "env": {
        "FLOWTRACE_URL": "http://127.0.0.1:9601/mcp"
      }
    }
  }
}
```

> **Important:** Replace `/absolute/path/to/flowtrace` with the actual path to your Flowtrace installation.

---

### Step 4: Verify Connection

1. Restart Claude Code/Claude Desktop
2. The Flowtrace MCP tools should now appear in your available tools
3. Try asking Claude: "Search my memory for recent code changes"

---

## Available MCP Tools

Once connected, Claude can use these tools:

| Tool | Description |
|------|-------------|
| `search_traces` | Search your trace memory with natural language |
| `get_trace_details` | Get full details of a specific trace |
| `get_related_traces` | Find traces related to a given trace |
| `get_context` | Build context from relevant traces |

---

## Configuration

| Environment Variable | Description | Default |
|----------------------|-------------|---------|
| `FLOWTRACE_URL` | URL of the Flowtrace MCP HTTP endpoint | `http://127.0.0.1:9601/mcp` |

---

## Troubleshooting

### "Connection refused" error
- Ensure Flowtrace Desktop is running
- Check that port 9601 is not blocked

### "Module not found" error
- Run `npm run build` in the `flowtrace-claude-bridge` directory
- Ensure Node.js 18+ is installed

### Tools not appearing in Claude
- Restart Claude Code after updating settings
- Check the MCP settings JSON syntax is valid
