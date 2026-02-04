#!/bin/bash
# Agent Replay: Session End Hook for Cursor
# Called when the conversation/session ends

INPUT=$(cat)

# Debug logging if enabled
if [ "$AGENTREPLAY_HOOK_DEBUG" = "true" ]; then
    echo "$(date -Iseconds) [SESSION_END] INPUT: $INPUT" >> "${AGENTREPLAY_HOOK_LOG:-/tmp/agentreplay-hook.log}"
fi

# Transform and pass to summarize
TRANSFORMED=$(echo "$INPUT" | jq '. + {session_id: .conversation_id, cwd: .workspace_roots[0]}' 2>/dev/null || echo "$INPUT")
echo "$TRANSFORMED" | agentreplay-hook summarize --platform cursor >> "${AGENTREPLAY_HOOK_LOG:-/tmp/agentreplay-hook.log}" 2>&1

echo '{"continue": true}'
exit 0
