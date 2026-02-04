#!/bin/bash
# Agent Replay: Observation Hook for Cursor
# Records tool uses (file reads, edits, shell commands, etc.)

INPUT=$(cat)

# Transform Cursor payload format
TRANSFORMED=$(echo "$INPUT" | jq '. + {session_id: .conversation_id, cwd: .workspace_roots[0]}' 2>/dev/null || echo "$INPUT")

# Ensure session exists (idempotent), then record observation
echo "$TRANSFORMED" | agentreplay-hook session-init --platform cursor >/dev/null 2>&1
echo "$TRANSFORMED" | agentreplay-hook observation --platform cursor

exit 0
