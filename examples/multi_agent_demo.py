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
Agentreplay Multi-Agent Demo
Creates nested spans to demonstrate the Execution Flow canvas
"""
import os
import time
from dotenv import load_dotenv
load_dotenv()

# Agentreplay initialization
import agentreplay
agentreplay.init_otel_instrumentation(
    service_name="multi-agent-demo",
    otlp_endpoint=os.getenv("AGENTREPLAY_OTLP_ENDPOINT", "localhost:4317"),
    project_id=46635,
    tenant_id=int(os.getenv("AGENTREPLAY_TENANT_ID", "1")),
    debug=True
)

from opentelemetry import trace
from opentelemetry.trace import Status, StatusCode

tracer = trace.get_tracer(__name__)

def research_task():
    """Simulates a research agent doing web search"""
    with tracer.start_as_current_span("research_agent") as span:
        span.set_attribute("agent.name", "ResearchAgent")
        span.set_attribute("gen_ai.operation.name", "web_search")
        span.set_attribute("gen_ai.system", "openai")
        span.set_attribute("gen_ai.request.model", "gpt-4o-mini")
        
        # Simulate tool call
        with tracer.start_as_current_span("tool_tavily_search") as tool_span:
            tool_span.set_attribute("tool.name", "tavily_search")
            tool_span.set_attribute("tool.query", "AI agent frameworks 2024")
            time.sleep(0.5)  # Simulate API call
            tool_span.set_status(Status(StatusCode.OK))
        
        time.sleep(0.3)  # Simulate LLM processing
        span.set_status(Status(StatusCode.OK))
        return "Research completed: Found 5 AI frameworks"

def analysis_task(research_result):
    """Simulates an analysis agent processing research data"""
    with tracer.start_as_current_span("analysis_agent") as span:
        span.set_attribute("agent.name", "AnalysisAgent")
        span.set_attribute("gen_ai.operation.name", "analyze_data")
        span.set_attribute("gen_ai.system", "anthropic")
        span.set_attribute("gen_ai.request.model", "claude-3-5-sonnet")
        span.set_attribute("input.data", research_result)
        
        time.sleep(0.7)  # Simulate processing
        span.set_status(Status(StatusCode.OK))
        return "Analysis: LangGraph and AutoGen are leading frameworks"

def writer_task(analysis_result):
    """Simulates a writer agent creating final output"""
    with tracer.start_as_current_span("writer_agent") as span:
        span.set_attribute("agent.name", "WriterAgent")
        span.set_attribute("gen_ai.operation.name", "generate_report")
        span.set_attribute("gen_ai.system", "openai")
        span.set_attribute("gen_ai.request.model", "gpt-4o")
        span.set_attribute("input.analysis", analysis_result)
        
        time.sleep(0.6)  # Simulate generation
        span.set_status(Status(StatusCode.OK))
        return "### AI Agent Frameworks 2024\\n\\nTop frameworks include LangGraph and AutoGen..."

def main():
    """Main orchestration with root span"""
    with tracer.start_as_current_span("multi_agent_workflow") as root_span:
        root_span.set_attribute("workflow.name", "AI Research Pipeline")
        root_span.set_attribute("workflow.type", "multi_agent")
        root_span.set_attribute("session.id", "demo_session_001")
        
        print("🤖 Starting Multi-Agent Workflow...")
        
        # Step 1: Research
        print("📊 Step 1: Research Agent")
        research_result = research_task()
        
        # Step 2: Analysis  
        print("🔬 Step 2: Analysis Agent")
        analysis_result = analysis_task(research_result)
        
        # Step 3: Writing
        print("✍️  Step 3: Writer Agent")
        final_report = writer_task(analysis_result)
        
        root_span.set_status(Status(StatusCode.OK))
        print(f"\\n✅ Workflow Complete!\\n{final_report}")
        
    print(f"\\n🔗 View traces: http://localhost:5173/projects/46635/traces")

if __name__ == "__main__":
    main()
