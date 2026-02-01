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
Agentreplay automatic instrumentation via sitecustomize.

This file is automatically loaded by Python if it's in the site-packages directory.
It enables zero-code instrumentation - just set env vars and run!

Usage:
    export AGENTREPLAY_ENABLED=true
    export AGENTREPLAY_URL=http://localhost:47100
    python my_app.py  # Automatically instrumented!

Environment Variables:
    AGENTREPLAY_ENABLED: Set to 'true' to enable auto-instrumentation
    AGENTREPLAY_URL: Agentreplay server URL (default: http://localhost:47100)
    AGENTREPLAY_TENANT_ID: Tenant ID (default: 1)
    AGENTREPLAY_PROJECT_ID: Project ID (default: 0)
    AGENTREPLAY_DEBUG: Set to 'true' for verbose logging
    OTEL_SERVICE_NAME: Service name for traces
    OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT: Capture message content

Note:
    This module is loaded very early in the Python startup process.
    It must handle all errors gracefully to avoid breaking user applications.
"""

import os
import sys

# Only auto-instrument if explicitly enabled
if os.getenv('AGENTREPLAY_ENABLED', '').lower() == 'true':
    try:
        # Import and initialize BEFORE any user code runs
        from agentreplay.bootstrap import init_otel_instrumentation
        
        init_otel_instrumentation(
            service_name=os.getenv('OTEL_SERVICE_NAME', os.path.basename(sys.argv[0])),
            agentreplay_url=os.getenv('AGENTREPLAY_URL', 'http://localhost:47100'),
            tenant_id=int(os.getenv('AGENTREPLAY_TENANT_ID', '1')),
            project_id=int(os.getenv('AGENTREPLAY_PROJECT_ID', '0')),
            capture_content=os.getenv('OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT', 'false').lower() == 'true'
        )
        
        # Silent by default, verbose if DEBUG enabled
        if os.getenv('AGENTREPLAY_DEBUG', '').lower() == 'true':
            print("[Agentreplay] ✓ Auto-instrumentation enabled", file=sys.stderr)
            print(f"[Agentreplay]   Service: {os.getenv('OTEL_SERVICE_NAME', os.path.basename(sys.argv[0]))}", file=sys.stderr)
            print(f"[Agentreplay]   URL: {os.getenv('AGENTREPLAY_URL', 'http://localhost:47100')}", file=sys.stderr)
            print(f"[Agentreplay]   Project: {os.getenv('AGENTREPLAY_PROJECT_ID', '0')}", file=sys.stderr)
        
    except ImportError as e:
        if os.getenv('AGENTREPLAY_DEBUG', '').lower() == 'true':
            print(f"[Agentreplay] ✗ Failed to auto-instrument: {e}", file=sys.stderr)
            print("[Agentreplay]   Install: pip install opentelemetry-api opentelemetry-sdk", file=sys.stderr)
    
    except Exception as e:
        if os.getenv('AGENTREPLAY_DEBUG', '').lower() == 'true':
            print(f"[Agentreplay] ✗ Auto-instrumentation error: {e}", file=sys.stderr)
            import traceback
            traceback.print_exc(file=sys.stderr)
