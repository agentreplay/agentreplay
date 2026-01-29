# Copyright 2025 Sushanth (https://github.com/sushanthpy)
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""W3C Trace Context propagation for distributed tracing.

This module implements W3C Trace Context specification for propagating
trace context across process boundaries using HTTP headers.

Reference: https://www.w3.org/TR/trace-context/

The W3C Trace Context uses two headers:
    traceparent: 00-<trace-id>-<parent-id>-<trace-flags>
    tracestate: vendor-specific key-value pairs

Example:
    >>> from agentreplay.propagation import inject_trace_context, extract_trace_context
    >>> 
    >>> # Injecting context into outgoing HTTP request
    >>> headers = {}
    >>> inject_trace_context(headers, trace_id=0x123abc, span_id=0x456def)
    >>> # headers now contains: {"traceparent": "00-0000...123abc-00...456def-01"}
    >>> 
    >>> # Extracting context from incoming HTTP request
    >>> trace_id, parent_id, trace_flags = extract_trace_context(request.headers)
"""

import logging
from typing import Dict, Optional, Tuple

logger = logging.getLogger(__name__)


def inject_trace_context(
    headers: Dict[str, str],
    trace_id: int,
    span_id: int,
    trace_flags: int = 0x01,
    agentreplay_tenant_id: Optional[int] = None,
    agentreplay_project_id: Optional[int] = None,
) -> None:
    """Inject W3C Trace Context into HTTP headers.
    
    Adds traceparent and tracestate headers following W3C specification.
    
    Args:
        headers: Dict to inject headers into (modified in-place)
        trace_id: 128-bit trace ID
        span_id: 64-bit span ID
        trace_flags: 8-bit trace flags (default: 0x01 = sampled)
        agentreplay_tenant_id: Optional tenant ID for tracestate
        agentreplay_project_id: Optional project ID for tracestate
        
    Example:
        >>> import httpx
        >>> headers = {}
        >>> inject_trace_context(headers, trace_id=0x123, span_id=0x456)
        >>> response = httpx.get("https://api.example.com", headers=headers)
    """
    # Format traceparent: version-trace_id-parent_id-trace_flags
    # version: 00 (fixed)
    # trace_id: 32 hex characters (128 bits)
    # parent_id: 16 hex characters (64 bits)
    # trace_flags: 2 hex characters (8 bits)
    
    # Convert IDs to hex strings (zero-padded)
    trace_id_hex = f"{trace_id:032x}"
    span_id_hex = f"{span_id:016x}"
    trace_flags_hex = f"{trace_flags:02x}"
    
    traceparent = f"00-{trace_id_hex}-{span_id_hex}-{trace_flags_hex}"
    headers["traceparent"] = traceparent
    
    # Build tracestate with vendor-specific data
    tracestate_parts = []
    
    if agentreplay_tenant_id is not None:
        tracestate_parts.append(f"agentreplay=t{agentreplay_tenant_id}")
    
    if agentreplay_project_id is not None:
        if tracestate_parts:
            # Append to existing agentreplay state
            tracestate_parts[0] += f"p{agentreplay_project_id}"
        else:
            tracestate_parts.append(f"agentreplay=p{agentreplay_project_id}")
    
    if tracestate_parts:
        headers["tracestate"] = ",".join(tracestate_parts)
    
    logger.debug(f"Injected trace context: trace_id={trace_id_hex}, span_id={span_id_hex}")


def extract_trace_context(
    headers: Dict[str, str]
) -> Tuple[Optional[int], Optional[int], int, Dict[str, str]]:
    """Extract W3C Trace Context from HTTP headers.
    
    Parses traceparent and tracestate headers following W3C specification.
    
    Args:
        headers: HTTP headers dict (case-insensitive lookup)
        
    Returns:
        Tuple of (trace_id, parent_span_id, trace_flags, tracestate_dict)
        Returns (None, None, 0, {}) if no valid context found
        
    Example:
        >>> headers = {
        ...     "traceparent": "00-0123456789abcdef0123456789abcdef-0123456789abcdef-01"
        ... }
        >>> trace_id, parent_id, flags, state = extract_trace_context(headers)
        >>> print(f"Trace ID: {trace_id:#x}")
    """
    # Make headers case-insensitive
    headers_lower = {k.lower(): v for k, v in headers.items()}
    
    # Extract traceparent
    traceparent = headers_lower.get("traceparent")
    if not traceparent:
        logger.debug("No traceparent header found")
        return (None, None, 0, {})
    
    # Parse traceparent: version-trace_id-parent_id-trace_flags
    parts = traceparent.split("-")
    if len(parts) != 4:
        logger.warning(f"Invalid traceparent format: {traceparent}")
        return (None, None, 0, {})
    
    version, trace_id_hex, parent_id_hex, trace_flags_hex = parts
    
    # Validate version
    if version != "00":
        logger.warning(f"Unsupported traceparent version: {version}")
        # Still try to parse, may be forward-compatible
    
    try:
        # Parse trace_id (128-bit)
        if len(trace_id_hex) != 32:
            raise ValueError(f"Invalid trace_id length: {len(trace_id_hex)}")
        trace_id = int(trace_id_hex, 16)
        
        # Parse parent_id (64-bit)
        if len(parent_id_hex) != 16:
            raise ValueError(f"Invalid parent_id length: {len(parent_id_hex)}")
        parent_id = int(parent_id_hex, 16)
        
        # Parse trace_flags (8-bit)
        if len(trace_flags_hex) != 2:
            raise ValueError(f"Invalid trace_flags length: {len(trace_flags_hex)}")
        trace_flags = int(trace_flags_hex, 16)
        
        logger.debug(f"Extracted trace context: trace_id={trace_id_hex}, parent_id={parent_id_hex}")
        
    except ValueError as e:
        logger.warning(f"Failed to parse traceparent: {e}")
        return (None, None, 0, {})
    
    # Parse tracestate (optional)
    tracestate_dict = {}
    tracestate = headers_lower.get("tracestate", "")
    if tracestate:
        # Parse comma-separated list of key=value pairs
        for entry in tracestate.split(","):
            entry = entry.strip()
            if "=" in entry:
                key, value = entry.split("=", 1)
                tracestate_dict[key.strip()] = value.strip()
    
    return (trace_id, parent_id, trace_flags, tracestate_dict)


def is_sampled(trace_flags: int) -> bool:
    """Check if trace is sampled based on trace_flags.
    
    Args:
        trace_flags: 8-bit trace flags from traceparent
        
    Returns:
        True if sampled bit is set
    """
    return (trace_flags & 0x01) != 0


def create_trace_id() -> int:
    """Generate a random 128-bit trace ID.
    
    Returns:
        Random 128-bit integer for use as trace ID
    """
    import random
    return random.getrandbits(128)


def create_span_id() -> int:
    """Generate a random 64-bit span ID.
    
    Returns:
        Random 64-bit integer for use as span ID
    """
    import random
    return random.getrandbits(64)


class TraceContextPropagator:
    """Helper class for managing trace context in client code.
    
    Example:
        >>> propagator = TraceContextPropagator()
        >>> 
        >>> # Start a new trace
        >>> trace_id, span_id = propagator.start_trace()
        >>> 
        >>> # Make HTTP request with context
        >>> headers = {}
        >>> propagator.inject(headers)
        >>> response = httpx.get(url, headers=headers)
        >>> 
        >>> # Continue trace from incoming request
        >>> propagator.extract(request.headers)
        >>> new_span_id = propagator.create_child_span()
    """
    
    def __init__(self, tenant_id: Optional[int] = None, project_id: Optional[int] = None):
        """Initialize propagator.
        
        Args:
            tenant_id: Optional tenant ID for tracestate
            project_id: Optional project ID for tracestate
        """
        self.tenant_id = tenant_id
        self.project_id = project_id
        self.trace_id: Optional[int] = None
        self.span_id: Optional[int] = None
        self.trace_flags: int = 0x01  # Default: sampled
    
    def start_trace(self, sampled: bool = True) -> Tuple[int, int]:
        """Start a new trace.
        
        Args:
            sampled: Whether trace should be sampled
            
        Returns:
            Tuple of (trace_id, span_id)
        """
        self.trace_id = create_trace_id()
        self.span_id = create_span_id()
        self.trace_flags = 0x01 if sampled else 0x00
        
        logger.debug(f"Started new trace: {self.trace_id:#x}")
        
        return (self.trace_id, self.span_id)
    
    def extract(self, headers: Dict[str, str]) -> bool:
        """Extract trace context from headers.
        
        Args:
            headers: HTTP headers to extract from
            
        Returns:
            True if valid context was extracted
        """
        trace_id, parent_id, trace_flags, _ = extract_trace_context(headers)
        
        if trace_id is not None:
            self.trace_id = trace_id
            self.span_id = parent_id
            self.trace_flags = trace_flags
            return True
        
        return False
    
    def inject(self, headers: Dict[str, str]) -> None:
        """Inject trace context into headers.
        
        Args:
            headers: HTTP headers dict to inject into
        """
        if self.trace_id is None or self.span_id is None:
            # No active trace, start a new one
            self.start_trace()
        
        inject_trace_context(
            headers=headers,
            trace_id=self.trace_id,
            span_id=self.span_id,
            trace_flags=self.trace_flags,
            agentreplay_tenant_id=self.tenant_id,
            agentreplay_project_id=self.project_id,
        )
    
    def create_child_span(self) -> int:
        """Create a new child span ID.
        
        Returns:
            New span ID (updates internal state)
        """
        # Parent span ID becomes current span_id
        # Generate new span ID for child
        self.span_id = create_span_id()
        return self.span_id
    
    def is_sampled(self) -> bool:
        """Check if current trace is sampled.
        
        Returns:
            True if sampled
        """
        return is_sampled(self.trace_flags)


__all__ = [
    "inject_trace_context",
    "extract_trace_context",
    "is_sampled",
    "create_trace_id",
    "create_span_id",
    "TraceContextPropagator",
]
