---
description: Reset Agent Replay plugin settings to defaults
allowed-tools: ["Bash"]
---

# Clear Settings

Remove plugin settings to reset configuration.

## Command

```bash
rm -rf ~/.agentreplay-claude && echo "Settings cleared"
```

## Confirm to User

```
Agent Replay settings cleared.

Plugin will use defaults on next session:
- Server: http://localhost:47100
- Tracing: enabled
- Memory: enabled

Note: Memories in Agent Replay are NOT deleted.
Only plugin configuration was removed.
```
