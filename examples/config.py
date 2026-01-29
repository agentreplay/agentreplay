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
Agentreplay Examples Configuration

Centralized configuration for all example scripts.
Modify these values in one place to affect all examples.
"""

import os

# ============================================================================
# Agentreplay Server Configuration
# ============================================================================

# Agentreplay server URL
AGENTREPLAY_URL = os.getenv("AGENTREPLAY_URL", "http://localhost:8080")

# Alternative port for some examples
AGENTREPLAY_PORT_9600 = os.getenv("AGENTREPLAY_URL", "http://localhost:9600")

# Tenant ID (default: 1)
TENANT_ID = int(os.getenv("AGENTREPLAY_TENANT_ID", "1"))

# Project ID (default: 0 for no project, or use specific project)
PROJECT_ID = int(os.getenv("AGENTREPLAY_PROJECT_ID", "17444"))

# ============================================================================
# LLM API Configuration
# ============================================================================

# OpenAI Configuration
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY", "")
OPENAI_DEFAULT_MODEL = os.getenv("OPENAI_MODEL", "gpt-4o-mini")
OPENAI_MAX_TOKENS = int(os.getenv("OPENAI_MAX_TOKENS", "1000"))
OPENAI_TEMPERATURE = float(os.getenv("OPENAI_TEMPERATURE", "0.7"))

# Anthropic Configuration
ANTHROPIC_API_KEY = os.getenv("ANTHROPIC_API_KEY", "")
ANTHROPIC_DEFAULT_MODEL = os.getenv("ANTHROPIC_MODEL", "claude-3-5-sonnet-20241022")
ANTHROPIC_MAX_TOKENS = int(os.getenv("ANTHROPIC_MAX_TOKENS", "1024"))

# ============================================================================
# Auto-Instrumentation Configuration
# ============================================================================

# Service name for auto-instrumentation
SERVICE_NAME = os.getenv("AGENTREPLAY_SERVICE_NAME", "agentreplay-examples")

# Frameworks to auto-instrument (None = all)
AUTO_INSTRUMENT_FRAMEWORKS = None  # ["openai", "anthropic", "langgraph", "langchain"]

# Sampling rate (0.0 to 1.0)
SAMPLE_RATE = float(os.getenv("AGENTREPLAY_SAMPLE_RATE", "1.0"))

# Capture full content (prompts/responses)
CAPTURE_CONTENT = os.getenv("AGENTREPLAY_CAPTURE_CONTENT", "true").lower() == "true"

# Capture token usage
CAPTURE_TOKEN_USAGE = os.getenv("AGENTREPLAY_CAPTURE_TOKEN_USAGE", "true").lower() == "true"

# ============================================================================
# Example-Specific Configuration
# ============================================================================

# Batch size for batching client
BATCH_SIZE = int(os.getenv("AGENTREPLAY_BATCH_SIZE", "10"))

# Flush interval for batching (seconds)
FLUSH_INTERVAL = float(os.getenv("AGENTREPLAY_FLUSH_INTERVAL", "5.0"))

# ============================================================================
# Helper Functions
# ============================================================================

def get_auto_instrument_config():
    """Get configuration dict for auto_instrument()."""
    return {
        "service_name": SERVICE_NAME,
        "agentreplay_url": AGENTREPLAY_URL,
        "tenant_id": TENANT_ID,
        "frameworks": AUTO_INSTRUMENT_FRAMEWORKS,
        "sample_rate": SAMPLE_RATE,
        "capture_content": CAPTURE_CONTENT,
        "capture_token_usage": CAPTURE_TOKEN_USAGE,
    }


def get_agentreplay_client_config():
    """Get configuration dict for AgentreplayClient()."""
    return {
        "url": AGENTREPLAY_URL,
        "tenant_id": TENANT_ID,
        "project_id": PROJECT_ID,
    }


def get_batching_client_config():
    """Get configuration dict for BatchingAgentreplayClient()."""
    return {
        "base_url": AGENTREPLAY_URL,
        "api_key": "your-api-key",  # Placeholder
        "batch_size": BATCH_SIZE,
        "flush_interval": FLUSH_INTERVAL,
    }


def print_config():
    """Print current configuration."""
    print("=" * 80)
    print("Agentreplay Examples Configuration")
    print("=" * 80)
    print(f"Agentreplay URL:      {AGENTREPLAY_URL}")
    print(f"Tenant ID:          {TENANT_ID}")
    print(f"Project ID:         {PROJECT_ID}")
    print(f"Service Name:       {SERVICE_NAME}")
    print(f"Sample Rate:        {SAMPLE_RATE * 100}%")
    print(f"Capture Content:    {CAPTURE_CONTENT}")
    print()
    print(f"OpenAI API Key:     {'✅ Set' if OPENAI_API_KEY else '❌ Not set'}")
    print(f"OpenAI Model:       {OPENAI_DEFAULT_MODEL}")
    print(f"Anthropic API Key:  {'✅ Set' if ANTHROPIC_API_KEY else '❌ Not set'}")
    print(f"Anthropic Model:    {ANTHROPIC_DEFAULT_MODEL}")
    print("=" * 80)
    print()


def validate_config():
    """Validate configuration and return list of issues."""
    issues = []
    
    # Check Agentreplay URL
    import requests
    try:
        response = requests.get(f"{AGENTREPLAY_URL}/health", timeout=2)
        if response.status_code != 200:
            issues.append(f"Agentreplay server at {AGENTREPLAY_URL} returned status {response.status_code}")
    except requests.exceptions.RequestException:
        issues.append(f"Cannot connect to Agentreplay server at {AGENTREPLAY_URL}")
    
    # Check API keys if needed
    if not OPENAI_API_KEY:
        issues.append("OPENAI_API_KEY not set (some examples will be skipped)")
    
    if not ANTHROPIC_API_KEY:
        issues.append("ANTHROPIC_API_KEY not set (some examples will be skipped)")
    
    return issues


if __name__ == "__main__":
    print_config()
    
    print("Validating configuration...")
    issues = validate_config()
    
    if issues:
        print("\n⚠️  Configuration Issues:")
        for issue in issues:
            print(f"  - {issue}")
    else:
        print("\n✅ Configuration is valid!")
