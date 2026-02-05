---
name: memory-search
description: Search local coding memory for past work, previous sessions, implementation details, or recalled information. All data stored locally via Agent Replay.
allowed-tools: Bash(node:*)
---

# Memory Search

Query Agent Replay's local memory for past sessions and saved knowledge.

## Search Command

```bash
node "${CLAUDE_PLUGIN_ROOT}/scripts/search-memory.cjs" "QUERY"
```

## Examples

**Past work:**
```bash
node "${CLAUDE_PLUGIN_ROOT}/scripts/search-memory.cjs" "recent activity yesterday"
```

**Implementation:**
```bash
node "${CLAUDE_PLUGIN_ROOT}/scripts/search-memory.cjs" "authentication implementation"
```

**Conventions:**
```bash
node "${CLAUDE_PLUGIN_ROOT}/scripts/search-memory.cjs" "coding style conventions"
```

## Output

The script returns formatted memory matches. Present clearly and offer to refine the search if needed.

## Privacy

All data stays on your machine - no external servers involved.
