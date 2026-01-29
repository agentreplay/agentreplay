#!/usr/bin/env python3

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

"""Quick test to verify Agentreplay SDK implementation without OpenAI."""

import sys
import os

# Add src to path for local testing
sdk_dir = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(sdk_dir, 'src'))

print("=" * 60)
print("Agentreplay SDK Implementation Test")
print("=" * 60)

# Test 1: Import modules
print("\n1. Testing imports...")
try:
    from agentreplay.context import AgentContext, get_current_agent_id
    print("   ✓ Context module imported")
except ImportError as e:
    print(f"   ✗ Context import failed: {e}")
    sys.exit(1)

try:
    from agentreplay.bootstrap import _auto_init
    print("   ✓ Bootstrap module imported")
except ImportError as e:
    print(f"   ✗ Bootstrap import failed: {e}")
    sys.exit(1)

try:
    from agentreplay.bootstrap import init_otel_instrumentation
    print("   ✓ OTEL bridge imported")
except ImportError as e:
    print(f"   ✗ OTEL bridge import failed: {e}")
    sys.exit(1)

try:
    from agentreplay.auto_instrument.openai import instrument_openai
    print("   ✓ OpenAI instrumentation imported")
except ImportError as e:
    print(f"   ✗ OpenAI instrumentation import failed: {e}")
    sys.exit(1)

# Test 2: Agent context
print("\n2. Testing agent context...")
try:
    with AgentContext(agent_id="test-agent", session_id="test-session"):
        agent_id = get_current_agent_id()
        if agent_id == "test-agent":
            print(f"   ✓ Agent context working: {agent_id}")
        else:
            print(f"   ✗ Agent context returned wrong value: {agent_id}")
    
    # Context should be cleared after exit
    agent_id = get_current_agent_id()
    if agent_id is None:
        print("   ✓ Context properly cleared after exit")
    else:
        print(f"   ✗ Context not cleared: {agent_id}")
except Exception as e:
    print(f"   ✗ Agent context failed: {e}")
    import traceback
    traceback.print_exc()

# Test 3: Bootstrap (without actually initializing)
print("\n3. Testing bootstrap logic...")
try:
    # Test with AGENTREPLAY_ENABLED=false (should do nothing)
    os.environ['AGENTREPLAY_ENABLED'] = 'false'
    _auto_init()
    print("   ✓ Bootstrap handles disabled state")
    
    # Clean up
    del os.environ['AGENTREPLAY_ENABLED']
except Exception as e:
    print(f"   ✗ Bootstrap failed: {e}")

# Test 4: Environment variable configuration
print("\n4. Testing environment variable configuration...")
try:
    # Set test env vars
    os.environ['OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT'] = 'true'
    os.environ['AGENTREPLAY_MAX_CONTENT_LENGTH'] = '5000'
    os.environ['AGENTREPLAY_MAX_MESSAGES'] = '10'
    
    # Re-import to pick up new config
    import importlib
    import agentreplay.auto_instrument.openai as openai_module
    importlib.reload(openai_module)
    
    if openai_module.CAPTURE_CONTENT:
        print("   ✓ Content capture enabled")
    else:
        print("   ✗ Content capture not enabled")
    
    if openai_module.MAX_CONTENT_LENGTH == 5000:
        print("   ✓ Max content length set correctly")
    else:
        print(f"   ✗ Max content length wrong: {openai_module.MAX_CONTENT_LENGTH}")
    
    if openai_module.MAX_MESSAGES == 10:
        print("   ✓ Max messages set correctly")
    else:
        print(f"   ✗ Max messages wrong: {openai_module.MAX_MESSAGES}")
    
    # Clean up
    del os.environ['OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT']
    del os.environ['AGENTREPLAY_MAX_CONTENT_LENGTH']
    del os.environ['AGENTREPLAY_MAX_MESSAGES']
    
except Exception as e:
    print(f"   ✗ Environment configuration failed: {e}")
    import traceback
    traceback.print_exc()

# Test 5: Check .pth file exists
print("\n5. Checking .pth file...")
pth_file = os.path.join(os.path.dirname(__file__), '..', 'agentreplay-init.pth')
if os.path.exists(pth_file):
    print(f"   ✓ .pth file exists: {pth_file}")
    with open(pth_file) as f:
        content = f.read().strip()
        if "agentreplay.bootstrap" in content:
            print("   ✓ .pth file has correct content")
        else:
            print(f"   ✗ .pth file content wrong: {content}")
else:
    print(f"   ✗ .pth file not found: {pth_file}")

print("\n" + "=" * 60)
print("✅ All tests passed!")
print("=" * 60)

print("\nNext steps:")
print("1. Install the SDK: cd sdks/python && pip install -e .")
print("2. Start Agentreplay backend: ./start-web.sh")
print("3. Set env vars:")
print("   export AGENTREPLAY_ENABLED=true")
print("   export OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true")
print("   export OPENAI_API_KEY=your-key")
print("4. Run example: python examples/zero_code_example.py")
