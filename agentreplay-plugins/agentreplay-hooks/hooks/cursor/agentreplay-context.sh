#!/bin/bash
# Agent Replay: Context Injection Hook for Cursor
# Called before prompt submission to inject relevant context

# Configuration
AGENTREPLAY_URL="${AGENTREPLAY_URL:-http://127.0.0.1:47100}"
AGENTREPLAY_PROJECT_ID="${AGENTREPLAY_PROJECT_ID:-1}"
AGENTREPLAY_HOOK_LOG="${AGENTREPLAY_HOOK_LOG:-/tmp/agentreplay-hook.log}"

# Read input
INPUT=$(cat)

# Debug logging if enabled
if [ "$AGENTREPLAY_HOOK_DEBUG" = "true" ]; then
    echo "$(date -Iseconds) [CONTEXT] INPUT: $INPUT" >> "$AGENTREPLAY_HOOK_LOG"
fi

# Get working directory from input
WORKING_DIR=$(echo "$INPUT" | jq -r '.workspace_roots[0] // .cwd // empty' 2>/dev/null)

# Build query params
QUERY="project_id=$AGENTREPLAY_PROJECT_ID&limit=5&format=markdown"
if [ -n "$WORKING_DIR" ]; then
    ENCODED_DIR=$(echo "$WORKING_DIR" | jq -sRr @uri)
    QUERY="$QUERY&working_directory=$ENCODED_DIR"
fi

# Fetch context from Agent Replay
CONTEXT=$(curl -s --connect-timeout 2 --max-time 5 \
    "${AGENTREPLAY_URL}/api/v1/context?${QUERY}" 2>/dev/null)

if [ -n "$CONTEXT" ] && [ "$CONTEXT" != "null" ] && echo "$CONTEXT" | grep -q "agentreplay-context"; then
    # Write context to Cursor rules file for automatic injection
    CURSOR_RULES_DIR="$HOME/.cursor/rules"
    mkdir -p "$CURSOR_RULES_DIR"
    
    # Create the context rule file
    cat > "$CURSOR_RULES_DIR/agentreplay-context.mdc" << EOF
---
description: Agent Replay Context - Automatically injected from previous coding sessions
globs: 
alwaysApply: true
---

$CONTEXT
EOF
    
    if [ "$AGENTREPLAY_HOOK_DEBUG" = "true" ]; then
        echo "$(date -Iseconds) [CONTEXT] Wrote context to $CURSOR_RULES_DIR/agentreplay-context.mdc" >> "$AGENTREPLAY_HOOK_LOG"
    fi
fi

# Always continue
echo '{"continue": true}'
exit 0
