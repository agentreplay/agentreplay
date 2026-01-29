#!/usr/bin/env python3
"""
PreToolUse hook for Agentreplay plugin.
Records tool start time and sends initial trace.
"""

import json
import os
import sys

# Add plugin root to path
PLUGIN_ROOT = os.environ.get("CLAUDE_PLUGIN_ROOT", os.path.dirname(os.path.dirname(__file__)))
if PLUGIN_ROOT not in sys.path:
    sys.path.insert(0, PLUGIN_ROOT)

from core.client import (
    send_tool_trace,
    record_tool_start,
    get_parent_edge_id,
    get_config,
)


def main():
    """Main entry point for PreToolUse hook."""
    try:
        # Read input from stdin
        input_data = json.load(sys.stdin)

        config = get_config()
        if not config["enabled"]:
            print(json.dumps({}))
            sys.exit(0)

        tool_name = input_data.get("tool_name", "unknown")
        tool_input = input_data.get("tool_input", {})

        # Record start time for duration calculation
        record_tool_start(tool_name)

        # Send tool start trace
        send_tool_trace(
            tool_name=tool_name,
            tool_input=tool_input,
            parent_edge_id=get_parent_edge_id(),
            metadata={
                "status": "started",
                "event": "pre_tool_use",
            },
        )

        # Don't block the tool - return empty result
        print(json.dumps({}))

    except Exception as e:
        # Silently continue - don't disrupt Claude Code
        print(json.dumps({}))

    sys.exit(0)


if __name__ == "__main__":
    main()
