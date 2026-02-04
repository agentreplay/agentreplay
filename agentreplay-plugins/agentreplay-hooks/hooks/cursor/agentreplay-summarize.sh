#!/bin/bash
# Agent Replay: Summarize Hook for Cursor
# Generates session summary and marks session as complete

INPUT=$(cat)

# Transform Cursor payload format
TRANSFORMED=$(echo "$INPUT" | jq '. + {session_id: .conversation_id, cwd: .workspace_roots[0]}' 2>/dev/null || echo "$INPUT")

# Forward to agentreplay-hook CLI
echo "$TRANSFORMED" | agentreplay-hook summarize --platform cursor

exit 0
