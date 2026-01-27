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

"""
Host functions provided by Flowtrace to plugins.

These functions allow plugins to interact with the Flowtrace runtime.
"""

import json
from typing import Dict, List, Optional, Any

from .types import TraceContext, TraceId, Embedding, LogLevel, HttpResponse


class Host:
    """
    Host provides access to Flowtrace runtime functions.
    
    These functions are implemented by the Flowtrace WASM runtime
    and allow plugins to interact with the host system.
    """
    
    @staticmethod
    def log(level: LogLevel, message: str) -> None:
        """
        Log a message to the Flowtrace logs.
        
        Args:
            level: Log level (TRACE, DEBUG, INFO, WARN, ERROR)
            message: The message to log
        """
        # This will be implemented by the WASM host
        _host_log(level.value, message)
    
    @staticmethod
    def trace(message: str) -> None:
        """Log at trace level."""
        Host.log(LogLevel.TRACE, message)
    
    @staticmethod
    def debug(message: str) -> None:
        """Log at debug level."""
        Host.log(LogLevel.DEBUG, message)
    
    @staticmethod
    def info(message: str) -> None:
        """Log at info level."""
        Host.log(LogLevel.INFO, message)
    
    @staticmethod
    def warn(message: str) -> None:
        """Log at warn level."""
        Host.log(LogLevel.WARN, message)
    
    @staticmethod
    def error(message: str) -> None:
        """Log at error level."""
        Host.log(LogLevel.ERROR, message)
    
    @staticmethod
    def get_config() -> Dict[str, Any]:
        """
        Get plugin configuration.
        
        Returns:
            Configuration dictionary
        """
        config_json = _host_get_config()
        return json.loads(config_json) if config_json else {}
    
    @staticmethod
    def get_config_value(key: str) -> Optional[str]:
        """
        Get a specific configuration value.
        
        Args:
            key: Configuration key
            
        Returns:
            Value string or None if not set
        """
        return _host_get_config_value(key)
    
    @staticmethod
    def query_traces(filter_json: str, limit: int = 100) -> List[TraceContext]:
        """
        Query traces from the database.
        
        Requires trace-read capability.
        
        Args:
            filter_json: JSON filter object
            limit: Maximum number of traces to return
            
        Returns:
            List of matching traces
        """
        result_json = _host_query_traces(filter_json, limit)
        return _parse_traces(result_json)
    
    @staticmethod
    def get_trace(trace_id: TraceId) -> Optional[TraceContext]:
        """
        Get a single trace by ID.
        
        Requires trace-read capability.
        
        Args:
            trace_id: The trace ID
            
        Returns:
            TraceContext or None if not found
        """
        result_json = _host_get_trace(trace_id.to_uuid())
        if not result_json:
            return None
        traces = _parse_traces(f"[{result_json}]")
        return traces[0] if traces else None
    
    @staticmethod
    def http_request(
        method: str,
        url: str,
        headers: Optional[Dict[str, str]] = None,
        body: Optional[bytes] = None
    ) -> HttpResponse:
        """
        Make an HTTP request.
        
        Requires network capability.
        
        Args:
            method: HTTP method (GET, POST, etc.)
            url: Request URL
            headers: Optional request headers
            body: Optional request body
            
        Returns:
            HttpResponse with status, headers, and body
        """
        headers_json = json.dumps(headers or {})
        result_json = _host_http_request(method, url, headers_json, body or b"")
        result = json.loads(result_json)
        return HttpResponse(
            status=result["status"],
            headers=result["headers"],
            body=bytes(result["body"])
        )
    
    @staticmethod
    def embed_text(text: str) -> Embedding:
        """
        Generate text embedding using host provider.
        
        Requires embedding capability.
        
        Args:
            text: Text to embed
            
        Returns:
            Embedding vector
        """
        result_json = _host_embed_text(text)
        return json.loads(result_json)
    
    @staticmethod
    def embed_batch(texts: List[str]) -> List[Embedding]:
        """
        Batch embed multiple texts.
        
        Requires embedding capability.
        
        Args:
            texts: List of texts to embed
            
        Returns:
            List of embedding vectors
        """
        texts_json = json.dumps(texts)
        result_json = _host_embed_batch(texts_json)
        return json.loads(result_json)
    
    @staticmethod
    def get_env(name: str) -> Optional[str]:
        """
        Get environment variable.
        
        Requires env-vars capability.
        
        Args:
            name: Environment variable name
            
        Returns:
            Value or None if not set
        """
        return _host_get_env(name)


def _parse_traces(json_str: str) -> List[TraceContext]:
    """Parse JSON string to list of TraceContext."""
    from .types import Span, SpanType
    
    data = json.loads(json_str)
    traces = []
    
    for t in data:
        spans = []
        for s in t.get("spans", []):
            span = Span(
                id=TraceId(high=s["id"]["high"], low=s["id"]["low"]),
                parent_id=TraceId(high=s["parent_id"]["high"], low=s["parent_id"]["low"]) if s.get("parent_id") else None,
                span_type=SpanType(s.get("span_type", "custom")),
                name=s["name"],
                input=s.get("input"),
                output=s.get("output"),
                model=s.get("model"),
                timestamp_us=s["timestamp_us"],
                duration_us=s.get("duration_us"),
                token_count=s.get("token_count"),
                cost_usd=s.get("cost_usd"),
                metadata=s.get("metadata", {}),
            )
            spans.append(span)
        
        trace = TraceContext(
            trace_id=TraceId(high=t["trace_id"]["high"], low=t["trace_id"]["low"]),
            spans=spans,
            input=t.get("input"),
            output=t.get("output"),
            metadata=t.get("metadata", {}),
        )
        traces.append(trace)
    
    return traces


# Stub host functions (implemented by WASM runtime)
def _host_log(level: int, message: str) -> None:
    """Log to host."""
    print(f"[{level}] {message}")

def _host_get_config() -> str:
    """Get config from host."""
    return "{}"

def _host_get_config_value(key: str) -> Optional[str]:
    """Get config value from host."""
    return None

def _host_query_traces(filter_json: str, limit: int) -> str:
    """Query traces from host."""
    return "[]"

def _host_get_trace(trace_id: str) -> Optional[str]:
    """Get trace from host."""
    return None

def _host_http_request(method: str, url: str, headers_json: str, body: bytes) -> str:
    """Make HTTP request via host."""
    raise NotImplementedError("HTTP requests require WASM runtime")

def _host_embed_text(text: str) -> str:
    """Embed text via host."""
    raise NotImplementedError("Embeddings require WASM runtime")

def _host_embed_batch(texts_json: str) -> str:
    """Batch embed via host."""
    raise NotImplementedError("Embeddings require WASM runtime")

def _host_get_env(name: str) -> Optional[str]:
    """Get env var from host."""
    import os
    return os.environ.get(name)
