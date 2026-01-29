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

"""Unified observability interface for vendor-agnostic tracing.

This module provides a single API that works with multiple observability
backends (Agentreplay, LangSmith, Langfuse, Opik) based on environment variables.

This allows users to:
- Switch vendors without code changes
- A/B test different observability tools
- Send traces to multiple backends simultaneously

Example:
    # Set environment variable to choose backend
    export OBSERVABILITY_BACKEND="agentreplay"  # or "langsmith", "langfuse", "opik"
    
    # Python code - works with any backend!
    from agentreplay.unified import UnifiedObservability
    
    obs = UnifiedObservability()
    
    with obs.trace("operation") as span:
        span.set_attribute("key", "value")
        result = do_work()
        span.set_output(result)
"""

import os
import logging
from typing import Optional, Dict, Any, ContextManager
from contextlib import contextmanager
from abc import ABC, abstractmethod

logger = logging.getLogger(__name__)


class ObservabilityBackend(ABC):
    """Abstract interface for observability backends."""
    
    @abstractmethod
    def trace(self, name: str, **kwargs) -> ContextManager:
        """Create a trace span.
        
        Args:
            name: Span name
            **kwargs: Backend-specific parameters
            
        Returns:
            Context manager for span
        """
        pass
    
    @abstractmethod
    def set_attribute(self, span: Any, key: str, value: Any) -> None:
        """Set attribute on span.
        
        Args:
            span: Span object
            key: Attribute key
            value: Attribute value
        """
        pass
    
    @abstractmethod
    def close(self) -> None:
        """Close backend and flush any buffered data."""
        pass


class AgentreplayBackend(ObservabilityBackend):
    """Agentreplay observability backend."""
    
    def __init__(self, **config):
        """Initialize Agentreplay backend.
        
        Args:
            **config: Configuration passed to AgentreplayClient
        """
        from agentreplay import AgentreplayClient
        
        url = config.get("url", os.getenv("AGENTREPLAY_URL", "http://localhost:8080"))
        tenant_id = config.get("tenant_id", int(os.getenv("AGENTREPLAY_TENANT_ID", "1")))
        project_id = config.get("project_id", int(os.getenv("AGENTREPLAY_PROJECT_ID", "0")))
        
        self.client = AgentreplayClient(
            url=url,
            tenant_id=tenant_id,
            project_id=project_id,
        )
        
        logger.info("AgentreplayBackend initialized")
    
    @contextmanager
    def trace(self, name: str, **kwargs):
        """Create Agentreplay span."""
        span = self.client.trace(**kwargs)
        span_ctx = span.__enter__()
        try:
            yield span_ctx
        finally:
            span.__exit__(None, None, None)
    
    def set_attribute(self, span: Any, key: str, value: Any) -> None:
        """Set attribute on Agentreplay span."""
        # Agentreplay uses specific methods for common attributes
        if key == "token_count" and hasattr(span, "set_token_count"):
            span.set_token_count(int(value))
        elif key == "confidence" and hasattr(span, "set_confidence"):
            span.set_confidence(float(value))
        # Other attributes stored in payload
    
    def close(self) -> None:
        """Close Agentreplay client."""
        self.client.close()


class LangSmithBackend(ObservabilityBackend):
    """LangSmith observability backend."""
    
    def __init__(self, **config):
        """Initialize LangSmith backend.
        
        Args:
            **config: Configuration for LangSmith
        """
        try:
            from langsmith import Client
            
            api_key = config.get("api_key", os.getenv("LANGCHAIN_API_KEY"))
            if not api_key:
                raise ValueError("LANGCHAIN_API_KEY environment variable required for LangSmith")
            
            self.client = Client(api_key=api_key)
            logger.info("LangSmithBackend initialized")
            
        except ImportError:
            raise ImportError(
                "LangSmith not installed. Install with: pip install langsmith"
            )
    
    @contextmanager
    def trace(self, name: str, **kwargs):
        """Create LangSmith span."""
        from langsmith import traceable
        
        # Use LangSmith's traceable decorator as context manager
        with traceable(name=name, **kwargs) as span:
            yield span
    
    def set_attribute(self, span: Any, key: str, value: Any) -> None:
        """Set attribute on LangSmith span."""
        if hasattr(span, "metadata"):
            span.metadata[key] = value
    
    def close(self) -> None:
        """Close LangSmith client."""
        pass  # LangSmith handles cleanup automatically


class LangfuseBackend(ObservabilityBackend):
    """Langfuse observability backend."""
    
    def __init__(self, **config):
        """Initialize Langfuse backend.
        
        Args:
            **config: Configuration for Langfuse
        """
        try:
            from langfuse import Langfuse
            
            public_key = config.get("public_key", os.getenv("LANGFUSE_PUBLIC_KEY"))
            secret_key = config.get("secret_key", os.getenv("LANGFUSE_SECRET_KEY"))
            host = config.get("host", os.getenv("LANGFUSE_HOST", "https://cloud.langfuse.com"))
            
            if not public_key or not secret_key:
                raise ValueError(
                    "LANGFUSE_PUBLIC_KEY and LANGFUSE_SECRET_KEY required for Langfuse"
                )
            
            self.client = Langfuse(
                public_key=public_key,
                secret_key=secret_key,
                host=host,
            )
            logger.info("LangfuseBackend initialized")
            
        except ImportError:
            raise ImportError(
                "Langfuse not installed. Install with: pip install langfuse"
            )
    
    @contextmanager
    def trace(self, name: str, **kwargs):
        """Create Langfuse span."""
        trace = self.client.trace(name=name, **kwargs)
        try:
            yield trace
        finally:
            self.client.flush()
    
    def set_attribute(self, span: Any, key: str, value: Any) -> None:
        """Set attribute on Langfuse span."""
        if hasattr(span, "update"):
            span.update(metadata={key: value})
    
    def close(self) -> None:
        """Close Langfuse client."""
        self.client.flush()


class OpikBackend(ObservabilityBackend):
    """Opik observability backend."""
    
    def __init__(self, **config):
        """Initialize Opik backend.
        
        Args:
            **config: Configuration for Opik
        """
        try:
            import opik
            
            api_key = config.get("api_key", os.getenv("OPIK_API_KEY"))
            workspace = config.get("workspace", os.getenv("OPIK_WORKSPACE"))
            
            if api_key:
                opik.configure(api_key=api_key, workspace=workspace)
            
            self.client = opik.Opik()
            logger.info("OpikBackend initialized")
            
        except ImportError:
            raise ImportError(
                "Opik not installed. Install with: pip install opik"
            )
    
    @contextmanager
    def trace(self, name: str, **kwargs):
        """Create Opik span."""
        trace = self.client.trace(name=name, **kwargs)
        try:
            yield trace
        finally:
            trace.end()
    
    def set_attribute(self, span: Any, key: str, value: Any) -> None:
        """Set attribute on Opik span."""
        if hasattr(span, "log_metadata"):
            span.log_metadata(key, value)
    
    def close(self) -> None:
        """Close Opik client."""
        self.client.flush()


class UnifiedObservability:
    """Unified interface for multiple observability backends.
    
    Automatically selects backend based on environment variables:
    - OBSERVABILITY_BACKEND: Backend name (agentreplay, langsmith, langfuse, opik)
    - Or checks for API keys: LANGCHAIN_API_KEY, LANGFUSE_PUBLIC_KEY, OPIK_API_KEY
    
    Example:
        >>> from agentreplay.unified import UnifiedObservability
        >>> 
        >>> # Auto-detects backend from environment
        >>> obs = UnifiedObservability()
        >>> 
        >>> with obs.trace("my_operation") as span:
        ...     span.set_attribute("key", "value")
        ...     result = do_work()
    """
    
    def __init__(self, backend_name: Optional[str] = None, **config):
        """Initialize unified observability.
        
        Args:
            backend_name: Override backend selection (agentreplay, langsmith, langfuse, opik)
            **config: Backend-specific configuration
        """
        # Determine backend
        if backend_name:
            backend = backend_name.lower()
        else:
            backend = self._auto_detect_backend()
        
        # Create backend
        self.backend_name = backend
        self.backend = self._create_backend(backend, **config)
        
        logger.info(f"UnifiedObservability initialized with backend: {backend}")
    
    def _auto_detect_backend(self) -> str:
        """Auto-detect backend from environment variables.
        
        Returns:
            Backend name
        """
        # Check explicit setting
        backend = os.getenv("OBSERVABILITY_BACKEND", "").lower()
        if backend in ("agentreplay", "langsmith", "langfuse", "opik"):
            return backend
        
        # Auto-detect from API keys
        if os.getenv("LANGCHAIN_API_KEY"):
            return "langsmith"
        elif os.getenv("LANGFUSE_PUBLIC_KEY"):
            return "langfuse"
        elif os.getenv("OPIK_API_KEY"):
            return "opik"
        else:
            # Default to Agentreplay
            return "agentreplay"
    
    def _create_backend(self, backend_name: str, **config) -> ObservabilityBackend:
        """Create backend instance.
        
        Args:
            backend_name: Backend name
            **config: Backend configuration
            
        Returns:
            ObservabilityBackend instance
        """
        if backend_name == "agentreplay":
            return AgentreplayBackend(**config)
        elif backend_name == "langsmith":
            return LangSmithBackend(**config)
        elif backend_name == "langfuse":
            return LangfuseBackend(**config)
        elif backend_name == "opik":
            return OpikBackend(**config)
        else:
            raise ValueError(
                f"Unknown backend: {backend_name}. "
                f"Supported: agentreplay, langsmith, langfuse, opik"
            )
    
    def trace(self, name: str, **kwargs) -> ContextManager:
        """Create a trace span.
        
        Args:
            name: Span name
            **kwargs: Backend-specific parameters
            
        Returns:
            Context manager for span
        """
        return self.backend.trace(name, **kwargs)
    
    def set_attribute(self, span: Any, key: str, value: Any) -> None:
        """Set attribute on span.
        
        Args:
            span: Span object
            key: Attribute key
            value: Attribute value
        """
        self.backend.set_attribute(span, key, value)
    
    def close(self) -> None:
        """Close backend and flush buffered data."""
        self.backend.close()
    
    def __enter__(self):
        """Context manager entry."""
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit."""
        self.close()


class MultiBackend:
    """Send traces to multiple backends simultaneously.
    
    Example:
        >>> from agentreplay.unified import MultiBackend
        >>> 
        >>> # Send to both Agentreplay and LangSmith
        >>> obs = MultiBackend(["agentreplay", "langsmith"])
        >>> 
        >>> with obs.trace("operation") as spans:
        ...     for span in spans:
        ...         span.set_attribute("key", "value")
    """
    
    def __init__(self, backend_names: list, **config):
        """Initialize multi-backend.
        
        Args:
            backend_names: List of backend names
            **config: Configuration for backends
        """
        self.backends = []
        
        for name in backend_names:
            try:
                obs = UnifiedObservability(backend_name=name, **config)
                self.backends.append(obs)
                logger.info(f"Added backend: {name}")
            except Exception as e:
                logger.warning(f"Failed to initialize {name}: {e}")
    
    @contextmanager
    def trace(self, name: str, **kwargs):
        """Create spans in all backends.
        
        Args:
            name: Span name
            **kwargs: Parameters passed to all backends
            
        Yields:
            List of span objects (one per backend)
        """
        contexts = []
        spans = []
        
        # Enter all contexts
        for backend_obs in self.backends:
            try:
                ctx = backend_obs.trace(name, **kwargs)
                span = ctx.__enter__()
                contexts.append(ctx)
                spans.append(span)
            except Exception as e:
                logger.error(f"Failed to create span in {backend_obs.backend_name}: {e}")
        
        try:
            yield spans
        finally:
            # Exit all contexts
            for ctx in contexts:
                try:
                    ctx.__exit__(None, None, None)
                except Exception as e:
                    logger.error(f"Error exiting span context: {e}")
    
    def close(self) -> None:
        """Close all backends."""
        for backend_obs in self.backends:
            try:
                backend_obs.close()
            except Exception as e:
                logger.error(f"Error closing backend: {e}")


__all__ = [
    "ObservabilityBackend",
    "UnifiedObservability",
    "MultiBackend",
]
