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
Agentreplay Environment-based Auto-Initialization

Simplified to use pure OpenTelemetry with OTLP export.
No custom span processors or framework-specific code.

Environment Variables:
    AGENTREPLAY_ENABLED: Set to "1", "true", "yes" to enable
    AGENTREPLAY_OTLP_ENDPOINT: OTLP gRPC endpoint (default: localhost:47117)
    AGENTREPLAY_TENANT_ID: Tenant ID (default: 1)
    AGENTREPLAY_PROJECT_ID: Project ID (default: 0)
    AGENTREPLAY_SERVICE_NAME: Service name (default: "python-app")

Usage:
    export AGENTREPLAY_ENABLED=true
    export AGENTREPLAY_PROJECT_ID=19358
    python your_script.py  # Automatically instrumented!
"""

import os
import logging
import atexit

logger = logging.getLogger(__name__)


def _parse_bool(value: str) -> bool:
    """Parse boolean from environment variable."""
    return value.lower() in ("1", "true", "yes", "on", "enabled")


def init_from_env(force: bool = False) -> bool:
    """Initialize Agentreplay from environment variables.
    
    Args:
        force: Force initialization even if already initialized
        
    Returns:
        True if instrumentation was enabled, False otherwise
    """
    # Check if already initialized
    if hasattr(init_from_env, "_initialized") and not force:
        return init_from_env._initialized
    
    # Check if enabled
    enabled = os.getenv("AGENTREPLAY_ENABLED", "").strip()
    if not enabled or not _parse_bool(enabled):
        logger.debug("Agentreplay disabled (AGENTREPLAY_ENABLED not set)")
        init_from_env._initialized = False
        return False
    
    # Get configuration
    otlp_endpoint = os.getenv("AGENTREPLAY_OTLP_ENDPOINT", "localhost:47117")
    tenant_id = int(os.getenv("AGENTREPLAY_TENANT_ID", "1"))
    project_id = int(os.getenv("AGENTREPLAY_PROJECT_ID", "0"))
    service_name = os.getenv("AGENTREPLAY_SERVICE_NAME", "python-app")
    log_level = os.getenv("AGENTREPLAY_LOG_LEVEL", "INFO").upper()
    
    # Set logging level
    logging.basicConfig(
        level=getattr(logging, log_level, logging.INFO),
        format='%(asctime)s [%(name)s] %(levelname)s: %(message)s'
    )
    
    try:
        from agentreplay.auto_instrument import auto_instrument
        
        logger.info(f"🚀 Initializing Agentreplay")
        logger.info(f"   OTLP Endpoint: {otlp_endpoint}")
        logger.info(f"   Tenant: {tenant_id}, Project: {project_id}")
        logger.info(f"   Service: {service_name}")
        
        auto_instrument(
            service_name=service_name,
            otlp_endpoint=otlp_endpoint,
            tenant_id=tenant_id,
            project_id=project_id,
        )
        
        # Register atexit handler to flush spans on program exit
        def _flush_on_exit():
            try:
                from opentelemetry import trace
                from opentelemetry.sdk.trace import TracerProvider
                provider = trace.get_tracer_provider()
                if isinstance(provider, TracerProvider):
                    logger.debug("Flushing spans on exit...")
                    provider.force_flush(timeout_millis=5000)
                    logger.debug("Spans flushed successfully")
            except Exception as e:
                logger.debug(f"Failed to flush spans on exit: {e}")
        
        atexit.register(_flush_on_exit)
        
        logger.info("✅ Agentreplay auto-instrumentation enabled")
        init_from_env._initialized = True
        return True
        
    except ImportError as e:
        logger.error(f"❌ Failed to import: {e}")
        init_from_env._initialized = False
        return False
    except Exception as e:
        logger.error(f"❌ Failed to initialize: {e}")
        init_from_env._initialized = False
        return False


# Auto-initialize on module import
_AUTO_INIT = os.getenv("AGENTREPLAY_AUTO_INIT", "1")
if _parse_bool(_AUTO_INIT):
    init_from_env()
else:
    logger.debug("Agentreplay auto-init on import disabled (AGENTREPLAY_AUTO_INIT=0)")
