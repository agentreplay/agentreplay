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
Flowtrace + AutoGen Example
Auto-instrumented multi-agent collaboration

Project ID: 31698 (AutoGen tracking)
"""
import os
from dotenv import load_dotenv
load_dotenv()

# Flowtrace auto-instrumentation
import flowtrace
flowtrace.init_otel_instrumentation(
    service_name="flowtrace-autogen-demo",
    otlp_endpoint=os.getenv("FLOWTRACE_OTLP_ENDPOINT", "localhost:4317"),
    project_id=44205,  # AutoGen project
    tenant_id=int(os.getenv("FLOWTRACE_TENANT_ID", "1")),
    debug=True
)
print("✅ Flowtrace initialized for AutoGen")

import autogen
from autogen import AssistantAgent, UserProxyAgent, GroupChat, GroupChatManager


def create_autogen_team():
    """Create AutoGen multi-agent team"""
    print("\n👥 Creating AutoGen team...")
    
    # LLM config for Azure OpenAI
    llm_config = {
        "model": os.getenv("AZURE_OPENAI_DEPLOYMENT", "gpt-4o"),
        "api_type": "azure",
        "api_key": os.getenv("AZURE_OPENAI_API_KEY"),
        "base_url": os.getenv("AZURE_OPENAI_ENDPOINT"),
        "api_version": "2024-12-01-preview",
        "temperature": 0.7
    }
    
    # Create agents
    planner = AssistantAgent(
        name="Planner",
        system_message="You are a strategic planner. Break down complex tasks into clear steps.",
        llm_config=llm_config,
    )
    
    researcher = AssistantAgent(
        name="Researcher",
        system_message="You are a research expert. Gather information and provide factual insights.",
        llm_config=llm_config,
    )
    
    writer = AssistantAgent(
        name="Writer", 
        system_message="You are a technical writer. Synthesize information into clear, concise reports.",
        llm_config=llm_config,
    )
    
    critic = AssistantAgent(
        name="Critic",
        system_message="You are a critical reviewer. Evaluate work quality and suggest improvements.",
        llm_config=llm_config,
    )
    
    user_proxy = UserProxyAgent(
        name="UserProxy",
        human_input_mode="NEVER",
        max_consecutive_auto_reply=0,
        code_execution_config=False
    )
    
    # Create group chat
    groupchat = GroupChat(
        agents=[user_proxy, planner, researcher, writer, critic],
        messages=[],
        max_round=10,
        speaker_selection_method="round_robin"
    )
    
    manager = GroupChatManager(
        groupchat=groupchat,
        llm_config=llm_config
    )
    
    return user_proxy, manager


def run_autogen_collaboration():
    """Run AutoGen multi-agent collaboration"""
    print("="*60)
    print("AutoGen Multi-Agent Collaboration - Auto-Instrumented")
    print("="*60)
    
    user_proxy, manager =create_autogen_team()
    
    task = """
    Research and write a comprehensive report about:
    'The impact of AI agents on software development in 2024'
    
    Steps:
    1. Plan the research approach
    2. Gather key information
    3. Write a structured report
    4. Review and refine
    """
    
    print(f"\n📋 Task: {task}\n")
    
    # Initiate chat
    user_proxy.initiate_chat(
        manager,
        message=task
    )
    
    print("\n✅ View traces: http://localhost:5173/projects/44205/traces")


if __name__ == "__main__":
    run_autogen_collaboration()
