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
Direct test using OpenTelemetry SDK to create hierarchical spans.
"""

import time
import os

# Set Agentreplay config
os.environ["AGENTREPLAY_PROJECT_ID"] = "11635"
os.environ["OTEL_EXPORTER_OTLP_ENDPOINT"] = "http://localhost:4317"

# Initialize OTEL manually
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
from opentelemetry.sdk.resources import Resource

# Create resource with project info
resource = Resource.create({"service.name": "test-hierarchy", "project.id": "11635"})

# Set up tracer provider
provider = TracerProvider(resource=resource)
trace.set_tracer_provider(provider)

# Configure OTLP exporter
otlp_exporter = OTLPSpanExporter(
    endpoint="localhost:4317",
    insecure=True,
)
span_processor = BatchSpanProcessor(otlp_exporter)
provider.add_span_processor(span_processor)

# Get tracer
tracer = trace.get_tracer(__name__)

print("🚀 Creating Hierarchical Spans with Direct OTEL SDK")
print("=" * 60)

# Create parent span (chain)
with tracer.start_as_current_span("chain.MyChain") as parent:
    parent.set_attribute("chain.name", "MyChain")
    parent.set_attribute("chain.inputs", "{'question': 'What is ML?'}")
    parent.set_attribute("span.type", "chain")
    print(f"✓ Parent span started: chain.MyChain")
    print(f"  Span ID: {format(parent.get_span_context().span_id, '016x')}")
    print(f"  Trace ID: {format(parent.get_span_context().trace_id, '032x')}")
    
    time.sleep(0.1)
    
    # Create child span (LLM call)
    with tracer.start_as_current_span("llm.gpt-4") as child:
        child.set_attribute("llm.model", "gpt-4")
        child.set_attribute("llm.prompts", "Explain machine learning")
        child.set_attribute("span.type", "llm")
        child.set_attribute("llm.tokens.total", 50)
        print(f"  ✓ Child span started: llm.gpt-4")
        print(f"    Span ID: {format(child.get_span_context().span_id, '016x')}")
        print(f"    Parent ID: {format(parent.get_span_context().span_id, '016x')}")
        
        time.sleep(0.2)
        
        child.set_attribute("llm.response", "Machine learning is...")
        print(f"  ✓ Child span completed")
    
    time.sleep(0.1)
    parent.set_attribute("chain.outputs", "{'answer': 'ML explained'}")
    print(f"✓ Parent span completed")

print("\n📊 Creating second hierarchy: Chain → Tool + LLM")
print("-" * 60)

# Create another parent with multiple children
with tracer.start_as_current_span("chain.ToolChain") as parent2:
    parent2.set_attribute("chain.name", "ToolChain")
    parent2.set_attribute("span.type", "chain")
    print(f"✓ Parent span started: chain.ToolChain")
    
    time.sleep(0.1)
    
    # Child 1: Tool call
    with tracer.start_as_current_span("tool.get_weather") as tool:
        tool.set_attribute("tool.name", "get_weather")
        tool.set_attribute("tool.input", "San Francisco")
        tool.set_attribute("span.type", "tool")
        print(f"  ✓ Tool span started: get_weather")
        
        time.sleep(0.15)
        
        tool.set_attribute("tool.output", "Sunny, 72°F")
        print(f"  ✓ Tool span completed")
    
    time.sleep(0.1)
    
    # Child 2: LLM call
    with tracer.start_as_current_span("llm.gpt-4") as llm:
        llm.set_attribute("llm.model", "gpt-4")
        llm.set_attribute("llm.prompts", "Summarize weather")
        llm.set_attribute("span.type", "llm")
        print(f"  ✓ LLM span started: gpt-4")
        
        time.sleep(0.2)
        
        llm.set_attribute("llm.response", "Nice weather!")
        print(f"  ✓ LLM span completed")
    
    time.sleep(0.1)
    print(f"✓ Parent span completed")

print("\n⏳ Flushing spans to OTLP (localhost:4317)...")
provider.force_flush()
time.sleep(2)

print("\n" + "=" * 60)
print("✅ Done! Check Agentreplay UI for hierarchical traces")
print("   Expected:")
print("   1. chain.MyChain → llm.gpt-4")
print("   2. chain.ToolChain → tool.get_weather + llm.gpt-4")
print("=" * 60)
