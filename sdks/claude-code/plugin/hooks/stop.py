#!/usr/bin/env python3
"""
Stop hook for Flowtrace plugin.
Sends session end trace when Claude Code stops.
"""

import json
import os
import sys

# Add plugin root to path
PLUGIN_ROOT = os.environ.get("CLAUDE_PLUGIN_ROOT", os.path.dirname(os.path.dirname(__file__)))
if PLUGIN_ROOT not in sys.path:
    sys.path.insert(0, PLUGIN_ROOT)

from core.client import (
    send_trace,
    get_parent_edge_id,
    get_config,
    SpanType,
    load_state,
    save_state,
)


def main():
    """Main entry point for Stop hook."""
    try:
        # Read input from stdin
        try:
            input_data = json.load(sys.stdin)
        except json.JSONDecodeError:
            input_data = {}

        config = get_config()
        if not config["enabled"]:
            print(json.dumps({}))
            sys.exit(0)

        # Send session end trace
        send_trace(
            span_type=SpanType.Response,
            parent_edge_id=get_parent_edge_id(),
            metadata={
                "event": "session_end",
                "agent": "claude-code",
                "reason": input_data.get("reason", "normal"),
            },
        )

        # Clean up state
        save_state({})

        # Don't block - return empty result
        print(json.dumps({}))

    except Exception as e:
        # Silently continue - don't disrupt Claude Code
        print(json.dumps({}))

    sys.exit(0)


if __name__ == "__main__":
    main()
