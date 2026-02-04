#!/bin/bash
# Agent Replay: Tool Use Hook for Cursor
# Called before/after tool invocations

INPUT=$(cat)

# Debug logging if enabled
if [ "$AGENTREPLAY_HOOK_DEBUG" = "true" ]; then
    echo "$(date -Iseconds) [TOOL] INPUT: $INPUT" >> "${AGENTREPLAY_HOOK_LOG:-/tmp/agentreplay-hook.log}"
fi

# Transform payload
TRANSFORMED=$(echo "$INPUT" | jq '. + {session_id: .conversation_id, cwd: .workspace_roots[0]}' 2>/dev/null || echo "$INPUT")

# Try to init session first (idempotent)
echo "$TRANSFORMED" | agentreplay-hook session-init --platform cursor > /dev/null 2>&1

# Record observation
echo "$TRANSFORMED" | agentreplay-hook observation --platform cursor >> "${AGENTREPLAY_HOOK_LOG:-/tmp/agentreplay-hook.log}" 2>&1

echo '{"continue": true}'
exit 0
