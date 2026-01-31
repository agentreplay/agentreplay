---
description: Clear Agent Replay settings and start fresh
allowed-tools: ["Bash"]
---

# Clear Settings

Remove Agent Replay plugin settings to reset to defaults.

## Steps

1. Use Bash to remove the settings directory:
   ```bash
   rm -rf ~/.agentreplay-claude
   ```

2. Confirm to the user:
   ```
   Successfully cleared Agent Replay settings.

   Your local settings have been removed. The plugin will use default settings
   (http://localhost:9600) on the next session.
   
   Note: Your memories stored in Agent Replay are NOT deleted. Only the plugin
   settings file has been removed.
   ```
