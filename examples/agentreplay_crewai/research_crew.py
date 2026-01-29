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
Agentreplay + CrewAI Example  
Auto-instrumented crew with specialized agents

Project ID: 31699 (CrewAI tracking)
"""
import os
from dotenv import load_dotenv
load_dotenv()

# Agentreplay auto-instrumentation
import agentreplay
agentreplay.init_otel_instrumentation(
    service_name="agentreplay-crewai-demo",
    otlp_endpoint=os.getenv("AGENTREPLAY_OTLP_ENDPOINT", "localhost:4317"),
    project_id=31699,  # CrewAI project
    tenant_id=int(os.getenv("AGENTREPLAY_TENANT_ID", "1")),
    debug=True
)
print("✅ Agentreplay initialized for CrewAI")

from crewai import Agent, Task, Crew, Process
from langchain_openai import AzureChatOpenAI


def create_research_crew():
    """Create CrewAI research crew"""
    print("\n🚀 Creating CrewAI crew...")
    
    # LLM setup
    llm = AzureChatOpenAI(
        azure_endpoint=os.getenv("AZURE_OPENAI_ENDPOINT"),
        azure_deployment=os.getenv("AZURE_OPENAI_DEPLOYMENT", "gpt-4o"),
        api_version="2024-12-01-preview",
        temperature=0.7
    )
    
    # Create specialized agents
    researcher = Agent(
        role="Senior Research Analyst",
        goal="Discover cutting-edge developments in AI and machine learning",
        backstory="""You are an expert research analyst with deep knowledge of AI trends.
        You excel at finding the most relevant and impactful information.""",
        llm=llm,
        verbose=True,
        allow_delegation=False
    )
    
    tech_writer = Agent(
        role="Technical Content Writer",
        goal="Create engaging, accurate technical content",
        backstory="""You are a skilled technical writer who can explain complex AI concepts
        in clear, accessible language while maintaining technical accuracy.""",
        llm=llm,
        verbose=True,
        allow_delegation=False
    )
    
    editor = Agent(
        role="Content Editor",
        goal="Ensure content quality and coherence",
        backstory="""You are a meticulous editor who ensures all content is well-structured,
        factually accurate, and engaging for technical audiences.""",
        llm=llm,
        verbose=True,
        allow_delegation=False
    )
    
    # Define tasks
    research_task = Task(
        description="""Research the latest AI agent frameworks and tools released in 2024.
        Focus on:
        - New framework capabilities
        - Industry adoption trends
        - Performance benchmarks
        - Key differentiators
        
        Provide a comprehensive analysis with sources.""",
        agent=researcher,
        expected_output="Detailed research findings with key insights and trends"
    )
    
    writing_task = Task(
        description="""Using the research findings, write a technical blog post about
        AI agent frameworks in 2024. Make it engaging and informative for developers.""",
        agent=tech_writer,
        expected_output="Well-structured technical blog post (800-1000 words)"
    )
    
    editing_task = Task(
        description="""Review and refine the blog post for:
        - Technical accuracy
        - Clear structure
        - Engaging narrative
        - Proper citations
        
        Provide the final polished version.""",
        agent=editor,
        expected_output="Final polished blog post ready for publication"
    )
    
    # Create crew
    crew = Crew(
        agents=[researcher, tech_writer, editor],
        tasks=[research_task, writing_task, editing_task],
        process=Process.sequential,
        verbose=True
    )
    
    return crew


def run_crewai_workflow():
    """Run CrewAI workflow"""
    print("="*60)
    print("CrewAI Research Crew - Auto-Instrumented")
    print("="*60)
    
    crew = create_research_crew()
    
    print("\n🎯 Starting crew execution...\n")
    result = crew.kickoff()
    
    print("\n" + "="*60)
    print("📄 FINAL OUTPUT")
    print("="*60)
    print(result)
    print("\n✅ View traces: http://localhost:5173/projects/31699/traces")


if __name__ == "__main__":
    run_crewai_workflow()
