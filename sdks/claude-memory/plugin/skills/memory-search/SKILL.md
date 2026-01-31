---
name: memory-search
description: Search your local coding memory. Use when user asks about past work, previous sessions, how something was implemented, what they worked on before, or wants to recall information from earlier sessions. All data is stored locally on your machine.
allowed-tools: Bash(node:*)
---

# Memory Search

Search Agent Replay's local memory for past coding sessions, decisions, and saved information.

## How to Search

Run the search script with the user's query:

```bash
node "${CLAUDE_PLUGIN_ROOT}/scripts/search-memory.cjs" "USER_QUERY_HERE"
```

Replace `USER_QUERY_HERE` with what the user is searching for.

## Examples

- User asks "what did I work on yesterday":
  ```bash
  node "${CLAUDE_PLUGIN_ROOT}/scripts/search-memory.cjs" "work yesterday recent activity"
  ```

- User asks "how did I implement auth":
  ```bash
  node "${CLAUDE_PLUGIN_ROOT}/scripts/search-memory.cjs" "authentication implementation"
  ```

- User asks about coding conventions:
  ```bash
  node "${CLAUDE_PLUGIN_ROOT}/scripts/search-memory.cjs" "coding conventions style preferences"
  ```

## Present Results

The script outputs formatted memory results. Present them clearly to the user and offer to search again with different terms if needed.

## Privacy

All memories are stored locally on your machine via Agent Replay. No data is sent to external servers.
