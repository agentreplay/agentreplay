---
description: Check Agent Replay connection status and memory statistics
allowed-tools: ["Bash"]
---

# Status Check

Check the Agent Replay server connection and memory statistics.

## Steps

1. Check if Agent Replay is running and get stats:
   ```bash
   curl -s http://localhost:9600/api/v1/health && echo "" && curl -s http://localhost:9600/api/v1/memory/stats
   ```

2. Present the results to the user:
   - If healthy: Show memory statistics (total vectors, documents, storage size)
   - If not running: Suggest starting Agent Replay

## Example Output

```
Agent Replay Status: ✅ Connected

Server: http://localhost:9600
Memory Stats:
- Total Vectors: 1,234
- Total Documents: 567
- Storage Size: 45.2 MB
- Index Type: HNSW

All data stored locally on this machine.
```
