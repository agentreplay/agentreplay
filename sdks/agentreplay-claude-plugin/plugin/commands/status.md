---
description: Check Agent Replay server status and memory statistics
allowed-tools: ["Bash"]
---

# Status Check

Verify Agent Replay server connection and view memory statistics.

## Command

```bash
curl -s http://localhost:47100/api/v1/health 2>/dev/null && echo "" && curl -s http://localhost:47100/api/v1/memory/stats 2>/dev/null || echo "Server not reachable"
```

## Present Results

**If connected:**
```
Agent Replay Status: ✅ Running

Endpoint: http://localhost:47100
Memory:
- Vectors: X
- Documents: Y
- Storage: Z MB

All data stored locally.
```

**If not running:**
```
Agent Replay Status: ❌ Offline

Start Agent Replay to enable:
- Session tracing
- Persistent memory
```
