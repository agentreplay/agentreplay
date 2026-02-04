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
Agentreplay SDK - Ergonomic top-level API.

This module provides the developer-friendly API surface:
- init() - Initialize SDK from env vars or explicit config
- get_client() - Get singleton client
- flush() - Flush pending spans
- shutdown() - Graceful shutdown

Example:
    >>> from agentreplay import init, traceable, wrap_openai, flush
    >>> 
    >>> # Initialize (reads env vars by default)
    >>> init()
    >>> 
    >>> # Wrap OpenAI for auto-tracing
    >>> from openai import OpenAI
    >>> client = wrap_openai(OpenAI())
    >>> 
    >>> # Or use decorator
    >>> @traceable
    >>> def my_function():
    ...     return "result"
    >>> 
    >>> # Flush before exit (serverless)
    >>> flush()
"""

import os
import atexit
import signal
import threading
import logging
from typing import Optional, Dict, Any, Callable
from dataclasses import dataclass, field

logger = logging.getLogger(__name__)

# =============================================================================
# Global State
# =============================================================================

@dataclass
class SDKConfig:
    """Resolved SDK configuration."""
    # Connection
    api_key: Optional[str] = None
    base_url: str = "http://localhost:8080"
    tenant_id: int = 1
    project_id: int = 0
    agent_id: int = 1
    
    # Environment
    environment: str = "development"
    service_name: str = "agentreplay-app"
    
    # Behavior
    enabled: bool = True
    debug: bool = False
    strict: bool = False
    
    # Batching
    batch_size: int = 100
    flush_interval: float = 5.0
    max_queue_size: int = 10000
    
    # Timeouts
    timeout: float = 30.0
    flush_timeout: float = 5.0
    
    # Privacy
    capture_input: bool = True
    capture_output: bool = True
    redact_patterns: list = field(default_factory=list)
    max_payload_size: int = 100000


# Global state
_config: Optional[SDKConfig] = None
_client = None
_batching_client = None
_initialized = False
_lock = threading.Lock()


# =============================================================================
# Environment Variable Helpers
# =============================================================================

def _get_env(key: str, default: Optional[str] = None) -> Optional[str]:
    """Get environment variable."""
    return os.environ.get(key, default)


def _get_env_bool(key: str, default: bool = False) -> bool:
    """Get boolean environment variable."""
    val = os.environ.get(key, "").lower()
    if val in ("1", "true", "yes", "on"):
        return True
    if val in ("0", "false", "no", "off"):
        return False
    return default


def _get_env_int(key: str, default: int) -> int:
    """Get integer environment variable."""
    val = os.environ.get(key)
    if val is None:
        return default
    try:
        return int(val)
    except ValueError:
        return default


def _get_env_float(key: str, default: float) -> float:
    """Get float environment variable."""
    val = os.environ.get(key)
    if val is None:
        return default
    try:
        return float(val)
    except ValueError:
        return default


# =============================================================================
# Initialization
# =============================================================================

def init(
    *,
    api_key: Optional[str] = None,
    base_url: Optional[str] = None,
    tenant_id: Optional[int] = None,
    project_id: Optional[int] = None,
    agent_id: Optional[int] = None,
    environment: Optional[str] = None,
    service_name: Optional[str] = None,
    enabled: Optional[bool] = None,
    debug: Optional[bool] = None,
    strict: Optional[bool] = None,
    batch_size: Optional[int] = None,
    flush_interval: Optional[float] = None,
    max_queue_size: Optional[int] = None,
    timeout: Optional[float] = None,
    capture_input: Optional[bool] = None,
    capture_output: Optional[bool] = None,
    redact_patterns: Optional[list] = None,
) -> SDKConfig:
    """Initialize the Agentreplay SDK.
    
    Reads configuration from environment variables by default, with
    explicit parameters taking precedence.
    
    Environment Variables:
        AGENTREPLAY_API_KEY: API key for authentication
        AGENTREPLAY_URL: Base URL (default: http://localhost:8080)
        AGENTREPLAY_TENANT_ID: Tenant ID (default: 1)
        AGENTREPLAY_PROJECT_ID: Project ID (default: 0)
        AGENTREPLAY_AGENT_ID: Agent ID (default: 1)
        AGENTREPLAY_ENVIRONMENT: Environment name (default: development)
        AGENTREPLAY_SERVICE_NAME: Service name (default: agentreplay-app)
        AGENTREPLAY_ENABLED: Enable SDK (default: true)
        AGENTREPLAY_DEBUG: Enable debug logging (default: false)
        AGENTREPLAY_STRICT: Strict mode - throw on missing API key (default: false)
        AGENTREPLAY_BATCH_SIZE: Batch size (default: 100)
        AGENTREPLAY_FLUSH_INTERVAL: Flush interval seconds (default: 5.0)
        AGENTREPLAY_MAX_QUEUE_SIZE: Max queue size (default: 10000)
        AGENTREPLAY_CAPTURE_INPUT: Capture inputs (default: true)
        AGENTREPLAY_CAPTURE_OUTPUT: Capture outputs (default: true)
    
    Args:
        api_key: API key (overrides env var)
        base_url: Base URL (overrides env var)
        tenant_id: Tenant ID (overrides env var)
        project_id: Project ID (overrides env var)
        agent_id: Agent ID (overrides env var)
        environment: Environment name (overrides env var)
        service_name: Service name (overrides env var)
        enabled: Enable SDK (overrides env var)
        debug: Enable debug logging (overrides env var)
        strict: Strict mode (overrides env var)
        batch_size: Batch size (overrides env var)
        flush_interval: Flush interval (overrides env var)
        max_queue_size: Max queue size (overrides env var)
        timeout: Request timeout (overrides env var)
        capture_input: Capture inputs (overrides env var)
        capture_output: Capture outputs (overrides env var)
        redact_patterns: Patterns to redact
        
    Returns:
        SDKConfig: Resolved configuration
        
    Raises:
        ValueError: If strict=True and API key is missing
        
    Example:
        >>> # Use environment variables
        >>> init()
        
        >>> # Explicit configuration
        >>> init(
        ...     api_key="ar_xxx",
        ...     base_url="https://api.agentreplay.dev",
        ...     environment="production",
        ...     debug=True
        ... )
    """
    global _config, _client, _batching_client, _initialized
    
    with _lock:
        if _initialized:
            logger.debug("SDK already initialized, returning existing config")
            return _config
        
        # Build config from env vars + explicit params
        _config = SDKConfig(
            api_key=api_key or _get_env("AGENTREPLAY_API_KEY"),
            base_url=(base_url or _get_env("AGENTREPLAY_URL", "http://localhost:8080")).rstrip("/"),
            tenant_id=tenant_id if tenant_id is not None else _get_env_int("AGENTREPLAY_TENANT_ID", 1),
            project_id=project_id if project_id is not None else _get_env_int("AGENTREPLAY_PROJECT_ID", 0),
            agent_id=agent_id if agent_id is not None else _get_env_int("AGENTREPLAY_AGENT_ID", 1),
            environment=environment or _get_env("AGENTREPLAY_ENVIRONMENT", "development"),
            service_name=service_name or _get_env("AGENTREPLAY_SERVICE_NAME", "agentreplay-app"),
            enabled=enabled if enabled is not None else _get_env_bool("AGENTREPLAY_ENABLED", True),
            debug=debug if debug is not None else _get_env_bool("AGENTREPLAY_DEBUG", False),
            strict=strict if strict is not None else _get_env_bool("AGENTREPLAY_STRICT", False),
            batch_size=batch_size if batch_size is not None else _get_env_int("AGENTREPLAY_BATCH_SIZE", 100),
            flush_interval=flush_interval if flush_interval is not None else _get_env_float("AGENTREPLAY_FLUSH_INTERVAL", 5.0),
            max_queue_size=max_queue_size if max_queue_size is not None else _get_env_int("AGENTREPLAY_MAX_QUEUE_SIZE", 10000),
            timeout=timeout if timeout is not None else _get_env_float("AGENTREPLAY_TIMEOUT", 30.0),
            capture_input=capture_input if capture_input is not None else _get_env_bool("AGENTREPLAY_CAPTURE_INPUT", True),
            capture_output=capture_output if capture_output is not None else _get_env_bool("AGENTREPLAY_CAPTURE_OUTPUT", True),
            redact_patterns=redact_patterns or [],
        )
        
        # Validate in strict mode
        if _config.strict and not _config.api_key:
            raise ValueError(
                "Agentreplay: API key required in strict mode. "
                "Set AGENTREPLAY_API_KEY or pass api_key parameter."
            )
        
        # Setup logging
        if _config.debug:
            logging.basicConfig(level=logging.DEBUG)
            logger.setLevel(logging.DEBUG)
        
        # Log initialization
        if _config.debug:
            logger.info(f"[Agentreplay] Initializing SDK")
            logger.info(f"  base_url: {_config.base_url}")
            logger.info(f"  tenant_id: {_config.tenant_id}")
            logger.info(f"  project_id: {_config.project_id}")
            logger.info(f"  environment: {_config.environment}")
            logger.info(f"  api_key: {'***' + _config.api_key[-4:] if _config.api_key else 'not set'}")
        
        # Warn if no API key (non-strict)
        if not _config.api_key and not _config.strict:
            logger.warning(
                "[Agentreplay] No API key configured. "
                "Set AGENTREPLAY_API_KEY or pass api_key parameter."
            )
        
        # Create clients if enabled
        if _config.enabled:
            from agentreplay.client import AgentreplayClient
            from agentreplay.batching import BatchingAgentreplayClient
            
            _client = AgentreplayClient(
                url=_config.base_url,
                tenant_id=_config.tenant_id,
                project_id=_config.project_id,
                agent_id=_config.agent_id,
                timeout=_config.timeout,
            )
            
            _batching_client = BatchingAgentreplayClient(
                client=_client,
                batch_size=_config.batch_size,
                flush_interval=_config.flush_interval,
                max_buffer_size=_config.max_queue_size,
            )
            
            # Register shutdown handlers
            atexit.register(_atexit_handler)
            signal.signal(signal.SIGTERM, _signal_handler)
            signal.signal(signal.SIGINT, _signal_handler)
        
        _initialized = True
        
        if _config.debug:
            logger.info("[Agentreplay] SDK initialized successfully")
        
        return _config


def _atexit_handler():
    """Handle process exit."""
    try:
        shutdown(timeout=2.0)
    except Exception:
        pass


def _signal_handler(signum, frame):
    """Handle SIGTERM/SIGINT."""
    try:
        shutdown(timeout=2.0)
    except Exception:
        pass


# =============================================================================
# Client Access
# =============================================================================

def get_client():
    """Get the singleton AgentreplayClient.
    
    Returns:
        AgentreplayClient: The client instance
        
    Raises:
        RuntimeError: If SDK not initialized
        
    Example:
        >>> from agentreplay import init, get_client
        >>> init()
        >>> client = get_client()
        >>> with client.trace() as span:
        ...     span.set_token_count(100)
    """
    global _client
    if not _initialized:
        raise RuntimeError("Agentreplay SDK not initialized. Call init() first.")
    return _client


def get_batching_client():
    """Get the singleton BatchingAgentreplayClient.
    
    Returns:
        BatchingAgentreplayClient: The batching client instance
        
    Raises:
        RuntimeError: If SDK not initialized
    """
    global _batching_client
    if not _initialized:
        raise RuntimeError("Agentreplay SDK not initialized. Call init() first.")
    return _batching_client


def get_config() -> SDKConfig:
    """Get current SDK configuration.
    
    Returns:
        SDKConfig: Current configuration
        
    Raises:
        RuntimeError: If SDK not initialized
    """
    global _config
    if not _initialized:
        raise RuntimeError("Agentreplay SDK not initialized. Call init() first.")
    return _config


def is_initialized() -> bool:
    """Check if SDK is initialized.
    
    Returns:
        bool: True if initialized
    """
    return _initialized


# =============================================================================
# Flush & Shutdown
# =============================================================================

def flush(timeout: Optional[float] = None) -> int:
    """Flush all pending spans.
    
    Call this before serverless function exits or at the end of scripts
    to ensure all spans are sent.
    
    Args:
        timeout: Maximum seconds to wait (default: from config)
        
    Returns:
        int: Number of spans flushed
        
    Example:
        >>> from agentreplay import init, flush
        >>> init()
        >>> 
        >>> # ... your code ...
        >>> 
        >>> # Flush before exit
        >>> flush(timeout=5.0)
    """
    global _batching_client, _config
    
    if not _initialized or _batching_client is None:
        return 0
    
    timeout = timeout or (_config.flush_timeout if _config else 5.0)
    
    if _config and _config.debug:
        logger.info(f"[Agentreplay] Flushing spans (timeout={timeout}s)")
    
    count = _batching_client.flush()
    
    if _config and _config.debug:
        logger.info(f"[Agentreplay] Flushed {count} spans")
    
    return count


def shutdown(timeout: Optional[float] = None) -> None:
    """Shutdown the SDK gracefully.
    
    Flushes all pending spans and stops background threads.
    
    Args:
        timeout: Maximum seconds to wait for flush
        
    Example:
        >>> from agentreplay import init, shutdown
        >>> init()
        >>> 
        >>> # ... your code ...
        >>> 
        >>> # Shutdown before exit
        >>> shutdown()
    """
    global _client, _batching_client, _initialized, _config
    
    if not _initialized:
        return
    
    with _lock:
        if _config and _config.debug:
            logger.info("[Agentreplay] Shutting down SDK")
        
        # Close batching client (flushes pending)
        if _batching_client is not None:
            try:
                _batching_client.close()
            except Exception as e:
                if _config and _config.debug:
                    logger.error(f"[Agentreplay] Error closing batching client: {e}")
            _batching_client = None
        
        # Close HTTP client
        if _client is not None:
            try:
                _client.close()
            except Exception as e:
                if _config and _config.debug:
                    logger.error(f"[Agentreplay] Error closing client: {e}")
            _client = None
        
        _initialized = False
        
        if _config and _config.debug:
            logger.info("[Agentreplay] SDK shutdown complete")


def reset() -> None:
    """Reset SDK state (for testing).
    
    Shuts down and clears all global state.
    """
    global _config, _client, _batching_client, _initialized
    
    shutdown()
    
    with _lock:
        _config = None
        _client = None
        _batching_client = None
        _initialized = False


# =============================================================================
# Diagnostics
# =============================================================================

def get_stats() -> Dict[str, Any]:
    """Get SDK statistics for debugging.
    
    Returns:
        Dict with queue_size, dropped_count, last_error, etc.
        
    Example:
        >>> from agentreplay import init, get_stats
        >>> init(debug=True)
        >>> print(get_stats())
        {'queue_size': 0, 'dropped_count': 0, 'initialized': True}
    """
    global _batching_client, _config
    
    stats = {
        "initialized": _initialized,
        "enabled": _config.enabled if _config else False,
        "debug": _config.debug if _config else False,
    }
    
    if _batching_client is not None:
        stats["queue_size"] = len(_batching_client._buffer)
        stats["dropped_count"] = _batching_client._dropped_count
        stats["retry_queue_size"] = len(_batching_client._retry_queue)
    
    return stats


def ping() -> Dict[str, Any]:
    """Ping the server to verify connectivity.
    
    Returns:
        Dict with success, latency_ms, version, error
        
    Example:
        >>> from agentreplay import init, ping
        >>> init()
        >>> result = ping()
        >>> if result["success"]:
        ...     print(f"Connected! Latency: {result['latency_ms']}ms")
    """
    import time
    
    if not _initialized or _client is None:
        return {"success": False, "error": "SDK not initialized"}
    
    start = time.time()
    try:
        # Try health endpoint
        response = _client._client.get(f"{_client.url}/health")
        latency_ms = (time.time() - start) * 1000
        
        if response.status_code == 200:
            return {
                "success": True,
                "latency_ms": round(latency_ms, 2),
                "status_code": response.status_code,
            }
        else:
            return {
                "success": False,
                "latency_ms": round(latency_ms, 2),
                "status_code": response.status_code,
                "error": response.text[:200],
            }
    except Exception as e:
        latency_ms = (time.time() - start) * 1000
        return {
            "success": False,
            "latency_ms": round(latency_ms, 2),
            "error": str(e),
        }
