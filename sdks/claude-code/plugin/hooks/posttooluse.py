#!/usr/bin/env python3
"""
PostToolUse hook for Flowtrace plugin.
Sends tool completion trace with output and duration.
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
    get_tool_duration,
    get_parent_edge_id,
    get_config,
)


def main():
    """Main entry point for PostToolUse hook."""
    try:
        # Read input from stdin
        input_data = json.load(sys.stdin)

        config = get_config()
        if not config["enabled"]:
            print(json.dumps({}))
            sys.exit(0)

        tool_name = input_data.get("tool_name", "unknown")
        tool_input = input_data.get("tool_input", {})
        tool_output = input_data.get("tool_response", {})

        # Check for errors
        is_error = False
        error_message = None
        if isinstance(tool_output, dict):
            is_error = tool_output.get("is_error", False)
            if is_error:
                error_message = tool_output.get("content", "")

        # Get duration from recorded start time
        duration_ms = get_tool_duration(tool_name)

        # Send tool completion trace
        send_tool_trace(
            tool_name=tool_name,
            tool_input=tool_input,
            tool_output=tool_output,
            duration_ms=duration_ms,
            parent_edge_id=get_parent_edge_id(),
            metadata={
                "status": "error" if is_error else "completed",
                "event": "post_tool_use",
                "error": error_message,
            },
        )

        # Don't block - return empty result
        print(json.dumps({}))

    except Exception as e:
        # Silently continue - don't disrupt Claude Code
        print(json.dumps({}))

    sys.exit(0)


if __name__ == "__main__":
    main()
