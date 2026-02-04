#!/bin/bash
# Agent Replay: Session Start Hook for Cursor
# Initializes a new coding session in Agent Replay

INPUT=$(cat)

# Transform Cursor payload format
TRANSFORMED=$(echo "$INPUT" | jq '. + {session_id: .conversation_id, cwd: .workspace_roots[0]}' 2>/dev/null || echo "$INPUT")

# Forward to agentreplay-hook CLI
echo "$TRANSFORMED" | agentreplay-hook session-init --platform cursor

exit 0
