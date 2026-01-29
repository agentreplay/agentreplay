"""
Agentreplay client for Claude Code plugin.
Sends traces to the Agentreplay server.
"""

import os
import json
import hashlib
from datetime import datetime
from typing import Optional, Dict, Any
from enum import IntEnum

try:
    import urllib.request
    import urllib.error
except ImportError:
    urllib = None


class SpanType(IntEnum):
    """Span types matching Agentreplay SDK."""
    Root = 0
    Planning = 1
    Reasoning = 2
    ToolCall = 3
    ToolResponse = 4
    Synthesis = 5
    Response = 6
    Error = 7
    Retrieval = 8
    Embedding = 9
    HttpCall = 10
    Database = 11
    Function = 12
    Reranking = 13
    Parsing = 14
    Generation = 15
    Custom = 255


def get_config() -> Dict[str, Any]:
    """Get Agentreplay configuration from environment or defaults."""
    return {
        "enabled": os.environ.get("AGENTREPLAY_ENABLED", "true").lower() != "false",
        "url": os.environ.get("AGENTREPLAY_URL", "http://localhost:9600"),
        "tenant_id": int(os.environ.get("AGENTREPLAY_TENANT_ID", "1")),
        "project_id": int(os.environ.get("AGENTREPLAY_PROJECT_ID", "1")),
    }


def hash_string(s: str) -> int:
    """Generate a numeric hash from a string."""
    return int(hashlib.md5(s.encode()).hexdigest()[:8], 16)


def get_session_id() -> int:
    """Get session ID from environment or generate one."""
    session_key = os.environ.get("CLAUDE_SESSION_ID", "")
    if session_key:
        return hash_string(session_key)
    # Use current date as fallback
    return hash_string(datetime.now().strftime("%Y-%m-%d-%H"))


def get_agent_id() -> int:
    """Get agent ID - for Claude Code, use fixed agent ID."""
    return hash_string("claude-code")


def send_trace(
    span_type: SpanType,
    metadata: Optional[Dict[str, Any]] = None,
    parent_edge_id: Optional[str] = None,
    token_count: Optional[int] = None,
) -> Optional[str]:
    """Send a generic trace to Agentreplay server."""
    config = get_config()
    if not config["enabled"]:
        return None

    payload = {
        "tenant_id": config["tenant_id"],
        "project_id": config["project_id"],
        "agent_id": get_agent_id(),
        "session_id": get_session_id(),
        "span_type": int(span_type),
    }

    if parent_edge_id:
        payload["parent_edge_id"] = parent_edge_id
    if token_count:
        payload["token_count"] = token_count
    if metadata:
        payload["metadata"] = metadata

    return _send_request(f"{config['url']}/api/v1/traces", payload)


def send_tool_trace(
    tool_name: str,
    tool_input: Optional[Any] = None,
    tool_output: Optional[Any] = None,
    duration_ms: Optional[int] = None,
    parent_edge_id: Optional[str] = None,
    metadata: Optional[Dict[str, Any]] = None,
) -> Optional[str]:
    """Send a tool call trace to Agentreplay server."""
    config = get_config()
    if not config["enabled"]:
        return None

    payload = {
        "tenant_id": config["tenant_id"],
        "project_id": config["project_id"],
        "agent_id": get_agent_id(),
        "session_id": get_session_id(),
        "tool_name": tool_name,
    }

    if tool_input is not None:
        # Truncate large inputs
        input_str = json.dumps(tool_input) if not isinstance(tool_input, str) else tool_input
        if len(input_str) > 1000:
            input_str = input_str[:1000] + "..."
        payload["tool_input"] = input_str

    if tool_output is not None:
        # Truncate large outputs
        output_str = json.dumps(tool_output) if not isinstance(tool_output, str) else tool_output
        if len(output_str) > 1000:
            output_str = output_str[:1000] + "..."
        payload["tool_output"] = output_str

    if duration_ms:
        payload["duration_ms"] = duration_ms
    if parent_edge_id:
        payload["parent_edge_id"] = parent_edge_id
    if metadata:
        payload["metadata"] = metadata

    return _send_request(f"{config['url']}/api/v1/traces/tool", payload)


def _send_request(url: str, payload: Dict[str, Any]) -> Optional[str]:
    """Send HTTP POST request to Agentreplay server."""
    if urllib is None:
        return None

    try:
        data = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(
            url,
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST",
        )

        with urllib.request.urlopen(req, timeout=3) as response:
            result = json.loads(response.read().decode("utf-8"))
            return result.get("edge_id")

    except (urllib.error.URLError, urllib.error.HTTPError, Exception):
        # Silently fail - don't disrupt Claude Code
        return None


# State file for tracking session context
STATE_FILE = "/tmp/agentreplay-claude-code-state.json"


def load_state() -> Dict[str, Any]:
    """Load state from file."""
    try:
        if os.path.exists(STATE_FILE):
            with open(STATE_FILE, "r") as f:
                return json.load(f)
    except Exception:
        pass
    return {}


def save_state(state: Dict[str, Any]) -> None:
    """Save state to file."""
    try:
        with open(STATE_FILE, "w") as f:
            json.dump(state, f)
    except Exception:
        pass


def get_parent_edge_id() -> Optional[str]:
    """Get the current session's parent edge ID."""
    state = load_state()
    return state.get("parent_edge_id")


def set_parent_edge_id(edge_id: str) -> None:
    """Set the current session's parent edge ID."""
    state = load_state()
    state["parent_edge_id"] = edge_id
    save_state(state)


def record_tool_start(tool_name: str) -> None:
    """Record a tool start time for duration calculation."""
    state = load_state()
    if "tool_starts" not in state:
        state["tool_starts"] = {}
    state["tool_starts"][tool_name] = datetime.now().timestamp() * 1000
    save_state(state)


def get_tool_duration(tool_name: str) -> Optional[int]:
    """Get duration since tool start."""
    state = load_state()
    starts = state.get("tool_starts", {})
    if tool_name in starts:
        duration = int(datetime.now().timestamp() * 1000 - starts[tool_name])
        # Clean up
        del starts[tool_name]
        state["tool_starts"] = starts
        save_state(state)
        return duration
    return None
