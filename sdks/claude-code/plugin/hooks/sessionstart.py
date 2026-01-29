#!/usr/bin/env python3
"""
SessionStart hook for Agentreplay plugin.
Creates a root trace when a Claude Code session starts.
"""

import json
import os
import sys

# Add plugin root to path
PLUGIN_ROOT = os.environ.get("CLAUDE_PLUGIN_ROOT", os.path.dirname(os.path.dirname(__file__)))
if PLUGIN_ROOT not in sys.path:
    sys.path.insert(0, PLUGIN_ROOT)

from core.client import send_trace, set_parent_edge_id, SpanType, get_config


def main():
    """Main entry point for SessionStart hook."""
    try:
        # Read input from stdin (may be empty for SessionStart)
        try:
            input_data = json.load(sys.stdin)
        except json.JSONDecodeError:
            input_data = {}

        config = get_config()
        if not config["enabled"]:
            print(json.dumps({}))
            sys.exit(0)

        # Create root trace for this session
        edge_id = send_trace(
            span_type=SpanType.Root,
            metadata={
                "event": "session_start",
                "agent": "claude-code",
                "workspace": os.environ.get("PWD", ""),
                "user": os.environ.get("USER", ""),
            },
        )

        if edge_id:
            set_parent_edge_id(edge_id)

        # Return system message about tracing (optional)
        result = {
            "systemMessage": f"📊 Agentreplay: Tracing to {config['url']}"
        }
        print(json.dumps(result))

    except Exception as e:
        # Silently continue - don't disrupt Claude Code
        print(json.dumps({}))

    sys.exit(0)


if __name__ == "__main__":
    main()
