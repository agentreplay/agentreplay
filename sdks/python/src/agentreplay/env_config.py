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

"""Environment-based auto-configuration for Agentreplay.

This module provides automatic configuration from environment variables,
following OTEL conventions for zero-config deployments.

Supported environment variables:
    OTEL_EXPORTER_OTLP_ENDPOINT: Agentreplay server URL
    OTEL_SERVICE_NAME: Service/agent name
    OTEL_SERVICE_NAMESPACE: Project/namespace identifier
    OTEL_TRACES_SAMPLER: Sampling strategy (always_on, always_off, traceidratio)
    OTEL_TRACES_SAMPLER_ARG: Sampling rate (0.0-1.0 for traceidratio)
    
    AGENTREPLAY_URL: Override for Agentreplay server URL
    AGENTREPLAY_TENANT_ID: Tenant identifier
    AGENTREPLAY_PROJECT_ID: Project identifier
    AGENTREPLAY_AGENT_ID: Default agent identifier
    AGENTREPLAY_AUTO_INSTRUMENT: Enable auto-instrumentation (true/false)
    AGENTREPLAY_FRAMEWORKS: Comma-separated list of frameworks to instrument

Example:
    # Set environment variables
    export OTEL_SERVICE_NAME="my-agent"
    export OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:8080"
    export AGENTREPLAY_TENANT_ID="1"
    export AGENTREPLAY_AUTO_INSTRUMENT="true"
    
    # Python code - zero configuration needed!
    from agentreplay import init_from_env
    init_from_env()
    
    # Now all your LLM calls are automatically traced
    from openai import OpenAI
    client = OpenAI()
    response = client.chat.completions.create(...)  # ✓ Traced!
"""

import os
import logging
from typing import Optional, Dict, Any, List, Tuple
from agentreplay.client import AgentreplayClient

logger = logging.getLogger(__name__)


class EnvConfig:
    """Configuration loaded from environment variables."""
    
    def __init__(self):
        """Load configuration from environment variables."""
        # Server configuration
        self.url = self._get_url()
        
        # Identity configuration
        self.service_name = os.getenv("OTEL_SERVICE_NAME", "agentreplay-agent")
        self.tenant_id = int(os.getenv("AGENTREPLAY_TENANT_ID", "1"))
        self.project_id = self._get_project_id()
        self.agent_id = int(os.getenv("AGENTREPLAY_AGENT_ID", "1"))
        
        # Sampling configuration
        self.sampler, self.sampling_rate = self._get_sampling_config()
        
        # Auto-instrumentation configuration
        self.auto_instrument = os.getenv("AGENTREPLAY_AUTO_INSTRUMENT", "false").lower() in ("true", "1", "yes")
        self.frameworks = self._get_frameworks()
        
        # Validation
        self.validate_on_init = os.getenv("AGENTREPLAY_VALIDATE", "true").lower() in ("true", "1", "yes")
        
        # OTLP compatibility
        self.enable_otlp_export = os.getenv("AGENTREPLAY_ENABLE_OTLP", "true").lower() in ("true", "1", "yes")
        
        logger.info(f"EnvConfig loaded: service={self.service_name}, url={self.url}, tenant={self.tenant_id}")
    
    def _get_url(self) -> str:
        """Get Agentreplay server URL from environment."""
        # Priority: AGENTREPLAY_URL > OTEL_EXPORTER_OTLP_ENDPOINT > default
        url = os.getenv("AGENTREPLAY_URL")
        if url:
            return url.rstrip("/")
        
        otlp_endpoint = os.getenv("OTEL_EXPORTER_OTLP_ENDPOINT")
        if otlp_endpoint:
            # OTLP endpoint may include /v1/traces suffix, remove it
            url = otlp_endpoint.rstrip("/")
            if url.endswith("/v1/traces"):
                url = url[:-10]
            return url
        
        return "http://localhost:8080"
    
    def _get_project_id(self) -> int:
        """Get project ID from environment."""
        # Try AGENTREPLAY_PROJECT_ID first
        project_id_str = os.getenv("AGENTREPLAY_PROJECT_ID")
        if project_id_str:
            try:
                return int(project_id_str)
            except ValueError:
                logger.warning(f"Invalid AGENTREPLAY_PROJECT_ID: {project_id_str}, using 0")
        
        # Fallback to hashing OTEL_SERVICE_NAMESPACE
        namespace = os.getenv("OTEL_SERVICE_NAMESPACE")
        if namespace:
            # Hash to 16-bit value
            import hashlib
            hash_val = int(hashlib.md5(namespace.encode()).hexdigest()[:4], 16)
            return hash_val
        
        return 0
    
    def _get_sampling_config(self) -> Tuple[str, float]:
        """Get sampling configuration from environment.
        
        Returns:
            Tuple of (sampler_name, sampling_rate)
        """
        sampler = os.getenv("OTEL_TRACES_SAMPLER", "always_on").lower()
        
        if sampler == "always_on":
            return ("always_on", 1.0)
        elif sampler == "always_off":
            return ("always_off", 0.0)
        elif sampler in ("traceidratio", "parentbased_traceidratio"):
            # Get sampling rate from OTEL_TRACES_SAMPLER_ARG
            arg = os.getenv("OTEL_TRACES_SAMPLER_ARG", "1.0")
            try:
                rate = float(arg)
                rate = max(0.0, min(1.0, rate))  # Clamp to [0, 1]
                return ("traceidratio", rate)
            except ValueError:
                logger.warning(f"Invalid OTEL_TRACES_SAMPLER_ARG: {arg}, using 1.0")
                return ("traceidratio", 1.0)
        else:
            logger.warning(f"Unknown sampler: {sampler}, using always_on")
            return ("always_on", 1.0)
    
    def _get_frameworks(self) -> List[str]:
        """Get list of frameworks to auto-instrument.
        
        Returns:
            List of framework names, or empty list for all
        """
        frameworks_str = os.getenv("AGENTREPLAY_FRAMEWORKS", "")
        if not frameworks_str:
            return []  # Empty = all frameworks
        
        # Parse comma-separated list
        frameworks = [fw.strip().lower() for fw in frameworks_str.split(",")]
        return [fw for fw in frameworks if fw]  # Filter empty strings
    
    def validate_connection(self) -> bool:
        """Validate connection to Agentreplay server.
        
        Returns:
            True if server is reachable
        """
        import httpx
        
        try:
            client = httpx.Client(timeout=5.0)
            # Try to hit health endpoint or root
            for endpoint in ["/health", "/api/v1/health", "/"]:
                try:
                    response = client.get(f"{self.url}{endpoint}")
                    if response.status_code < 500:
                        logger.info(f"✓ Agentreplay server reachable at {self.url}")
                        return True
                except:
                    continue
            
            logger.warning(f"✗ Cannot reach Agentreplay server at {self.url}")
            return False
            
        except Exception as e:
            logger.warning(f"✗ Failed to validate connection: {e}")
            return False
        finally:
            try:
                client.close()
            except:
                pass
    
    def create_client(self) -> AgentreplayClient:
        """Create a AgentreplayClient from this configuration.
        
        Returns:
            Configured AgentreplayClient
        """
        return AgentreplayClient(
            url=self.url,
            tenant_id=self.tenant_id,
            project_id=self.project_id,
            agent_id=self.agent_id,
        )
    
    def print_config(self) -> None:
        """Print configuration summary for debugging."""
        print("Agentreplay Configuration:")
        print(f"  Service Name:    {self.service_name}")
        print(f"  Server URL:      {self.url}")
        print(f"  Tenant ID:       {self.tenant_id}")
        print(f"  Project ID:      {self.project_id}")
        print(f"  Agent ID:        {self.agent_id}")
        print(f"  Sampler:         {self.sampler}")
        print(f"  Sampling Rate:   {self.sampling_rate}")
        print(f"  Auto-instrument: {self.auto_instrument}")
        if self.frameworks:
            print(f"  Frameworks:      {', '.join(self.frameworks)}")
        else:
            print(f"  Frameworks:      all")


def get_env_config() -> EnvConfig:
    """Get configuration from environment variables.
    
    Returns:
        EnvConfig instance
    """
    return EnvConfig()


def init_from_env(
    validate: bool = True,
    auto_instrument: Optional[bool] = None,
    verbose: bool = False,
) -> AgentreplayClient:
    """Initialize Agentreplay from environment variables.
    
    This is the recommended way to set up Agentreplay for production deployments.
    All configuration is read from environment variables following OTEL conventions.
    
    Args:
        validate: If True, validate connection to server on initialization
        auto_instrument: Override auto-instrumentation setting from env
        verbose: If True, print configuration details
        
    Returns:
        Configured AgentreplayClient
        
    Raises:
        ConnectionError: If validate=True and server is unreachable
        ValueError: If required environment variables are missing
        
    Example:
        >>> # Set environment variables first
        >>> import os
        >>> os.environ["OTEL_SERVICE_NAME"] = "my-agent"
        >>> os.environ["AGENTREPLAY_TENANT_ID"] = "1"
        >>> 
        >>> # Initialize with zero code configuration
        >>> from agentreplay import init_from_env
        >>> client = init_from_env()
        >>> 
        >>> # Now use the client or just rely on auto-instrumentation
        >>> with client.trace("operation"):
        ...     pass
    """
    config = get_env_config()
    
    if verbose:
        config.print_config()
    
    # Validate connection if requested
    if validate and config.validate_on_init:
        if not config.validate_connection():
            raise ConnectionError(
                f"Cannot reach Agentreplay server at {config.url}\n"
                f"Troubleshooting:\n"
                f"  1. Check server is running: curl {config.url}/health\n"
                f"  2. Verify firewall/network settings\n"
                f"  3. Set AGENTREPLAY_URL or OTEL_EXPORTER_OTLP_ENDPOINT\n"
                f"  4. Disable validation: init_from_env(validate=False)"
            )
    
    # Set up auto-instrumentation if requested
    should_auto_instrument = auto_instrument if auto_instrument is not None else config.auto_instrument
    
    if should_auto_instrument:
        try:
            from agentreplay.auto_instrument import auto_instrument as do_auto_instrument
            
            frameworks = config.frameworks if config.frameworks else None
            
            do_auto_instrument(
                service_name=config.service_name,
                agentreplay_url=config.url,
                tenant_id=config.tenant_id,
                project_id=config.project_id,
                frameworks=frameworks,
                sample_rate=config.sampling_rate,
                enable_otel_export=config.enable_otlp_export,
            )
            
            logger.info("✓ Auto-instrumentation enabled")
            
        except ImportError as e:
            logger.warning(f"Auto-instrumentation failed: {e}")
        except Exception as e:
            logger.error(f"Error during auto-instrumentation: {e}")
    
    # Create and return client
    client = config.create_client()
    
    logger.info("✓ Agentreplay initialized from environment")
    
    return client


__all__ = [
    "EnvConfig",
    "get_env_config",
    "init_from_env",
]
