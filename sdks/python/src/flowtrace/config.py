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

"""Configuration management for Flowtrace SDK.

Supports environment variable-based configuration following industry best practices.
"""

import os
from typing import Optional
from dataclasses import dataclass, field


@dataclass
class FlowtraceConfig:
    """Configuration for Flowtrace observability.
    
    All settings can be configured via environment variables for zero-code setup.
    
    Environment Variables:
        FLOWTRACE_ENABLED: Enable/disable observability (default: true)
        FLOWTRACE_API_KEY: API key for authentication
        FLOWTRACE_ENDPOINT: Flowtrace server endpoint (default: http://localhost:9600)
        FLOWTRACE_PROJECT: Project identifier
        FLOWTRACE_SERVICE_NAME: Service name for identification
        FLOWTRACE_ENVIRONMENT: Environment (production/staging/development)
        FLOWTRACE_BATCH_SIZE: Number of spans to batch before sending (default: 100)
        FLOWTRACE_BATCH_TIMEOUT: Max seconds to wait before sending batch (default: 1.0)
        OTEL_EXPORTER_OTLP_ENDPOINT: Alternative OTLP endpoint specification
        
    Example:
        ```python
        # Automatic configuration from environment
        config = FlowtraceConfig.from_env()
        
        # Manual configuration
        config = FlowtraceConfig(
            api_key="cl_abc123",
            endpoint="https://api.flowtrace.io",
            project="my-agent-project"
        )
        ```
    """
    
    # Core settings
    enabled: bool = True
    api_key: Optional[str] = None
    endpoint: str = "http://localhost:9600"
    project: Optional[str] = None
    
    # Service identification
    service_name: str = "flowtrace-app"
    environment: str = "development"
    version: str = "0.1.0"
    
    # Batching configuration for performance
    batch_size: int = 100
    batch_timeout: float = 1.0  # seconds
    
    # Advanced settings
    max_retries: int = 3
    timeout: float = 5.0  # seconds
    verify_ssl: bool = True
    
    @classmethod
    def from_env(cls) -> "FlowtraceConfig":
        """Create configuration from environment variables.
        
        Returns:
            FlowtraceConfig: Configuration instance with values from environment.
        """
        return cls(
            # Core settings
            enabled=cls._get_bool_env("FLOWTRACE_ENABLED", True),
            api_key=os.getenv("FLOWTRACE_API_KEY"),
            endpoint=cls._get_endpoint(),
            project=os.getenv("FLOWTRACE_PROJECT"),
            
            # Service identification
            service_name=os.getenv("FLOWTRACE_SERVICE_NAME", "flowtrace-app"),
            environment=os.getenv("FLOWTRACE_ENVIRONMENT", "development"),
            version=os.getenv("FLOWTRACE_VERSION", "0.1.0"),
            
            # Batching
            batch_size=cls._get_int_env("FLOWTRACE_BATCH_SIZE", 100),
            batch_timeout=cls._get_float_env("FLOWTRACE_BATCH_TIMEOUT", 1.0),
            
            # Advanced
            max_retries=cls._get_int_env("FLOWTRACE_MAX_RETRIES", 3),
            timeout=cls._get_float_env("FLOWTRACE_TIMEOUT", 5.0),
            verify_ssl=cls._get_bool_env("FLOWTRACE_VERIFY_SSL", True),
        )
    
    @staticmethod
    def _get_endpoint() -> str:
        """Get endpoint from multiple possible environment variables."""
        # Check Flowtrace-specific first
        if endpoint := os.getenv("FLOWTRACE_ENDPOINT"):
            return endpoint
        
        # Fall back to OTEL standard
        if endpoint := os.getenv("OTEL_EXPORTER_OTLP_ENDPOINT"):
            return endpoint
        
        # Default
        return "http://localhost:9600"
    
    @staticmethod
    def _get_bool_env(key: str, default: bool) -> bool:
        """Get boolean environment variable."""
        value = os.getenv(key)
        if value is None:
            return default
        return value.lower() in ("true", "1", "yes", "on")
    
    @staticmethod
    def _get_int_env(key: str, default: int) -> int:
        """Get integer environment variable."""
        value = os.getenv(key)
        if value is None:
            return default
        try:
            return int(value)
        except ValueError:
            return default
    
    @staticmethod
    def _get_float_env(key: str, default: float) -> float:
        """Get float environment variable."""
        value = os.getenv(key)
        if value is None:
            return default
        try:
            return float(value)
        except ValueError:
            return default
    
    def is_enabled(self) -> bool:
        """Check if observability is enabled."""
        return self.enabled
    
    def get_headers(self) -> dict:
        """Get HTTP headers for API requests."""
        headers = {
            "Content-Type": "application/json",
            "User-Agent": f"flowtrace-python-sdk/{self.version}",
        }
        
        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"
        
        if self.project:
            headers["X-Flowtrace-Project"] = self.project
        
        return headers
    
    def __repr__(self) -> str:
        """String representation with sensitive data masked."""
        api_key_masked = f"{self.api_key[:8]}..." if self.api_key else None
        return (
            f"FlowtraceConfig("
            f"enabled={self.enabled}, "
            f"endpoint={self.endpoint}, "
            f"project={self.project}, "
            f"service_name={self.service_name}, "
            f"api_key={'***' if self.api_key else None})"
        )


# Global configuration instance
_global_config: Optional[FlowtraceConfig] = None


def get_config() -> FlowtraceConfig:
    """Get the global configuration instance.
    
    Creates configuration from environment variables if not already initialized.
    
    Returns:
        FlowtraceConfig: Global configuration instance.
    """
    global _global_config
    if _global_config is None:
        _global_config = FlowtraceConfig.from_env()
    return _global_config


def set_config(config: FlowtraceConfig) -> None:
    """Set the global configuration instance.
    
    Args:
        config: Configuration to set as global.
    """
    global _global_config
    _global_config = config


def reset_config() -> None:
    """Reset the global configuration to None.
    
    Next call to get_config() will recreate from environment variables.
    """
    global _global_config
    _global_config = None
