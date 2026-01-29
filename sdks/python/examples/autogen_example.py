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

"""Example: Using Agentreplay with AutoGen.

This example demonstrates how to automatically log AutoGen multi-agent
conversations to Agentreplay.
"""

import os
from autogen import AssistantAgent, UserProxyAgent, GroupChat, GroupChatManager

from agentreplay.integrations.autogen import (
    AgentreplayAgentWrapper,
    AgentreplayGroupChatManager,
    wrap_autogen_function,
)
from agentreplay.client import AgentreplayClient


def example_basic_conversation():
    """Example: Basic two-agent conversation."""
    print("=== Basic Conversation Example ===\n")

    # Configure LLM
    llm_config = {
        "model": "gpt-4",
        "api_key": os.environ.get("OPENAI_API_KEY"),
    }

    # Create agents
    assistant = AssistantAgent(
        name="assistant",
        system_message="You are a helpful AI assistant.",
        llm_config=llm_config,
    )

    user_proxy = UserProxyAgent(
        name="user_proxy",
        human_input_mode="NEVER",
        max_consecutive_auto_reply=1,
    )

    # Wrap assistant with Agentreplay tracking
    wrapped_assistant = AgentreplayAgentWrapper(
        agent=assistant,
        url="http://localhost:8080",
        tenant_id=1,
        agent_id=1,
        session_id=2001,
    )

    # Start conversation - automatically logged
    user_proxy.initiate_chat(
        wrapped_assistant.agent,
        message="What are the three laws of robotics?",
    )

    print("\nConversation logged to Agentreplay!\n")


def example_group_chat():
    """Example: Multi-agent group chat."""
    print("=== Group Chat Example ===\n")

    llm_config = {
        "model": "gpt-4",
        "api_key": os.environ.get("OPENAI_API_KEY"),
    }

    # Create multiple agents
    planner = AssistantAgent(
        name="planner",
        system_message="You are a planner. Break down tasks into steps.",
        llm_config=llm_config,
    )

    researcher = AssistantAgent(
        name="researcher",
        system_message="You are a researcher. Find and analyze information.",
        llm_config=llm_config,
    )

    writer = AssistantAgent(
        name="writer",
        system_message="You are a writer. Create clear, concise content.",
        llm_config=llm_config,
    )

    user_proxy = UserProxyAgent(
        name="user_proxy",
        human_input_mode="NEVER",
        max_consecutive_auto_reply=0,
    )

    # Create group chat
    groupchat = GroupChat(
        agents=[planner, researcher, writer, user_proxy],
        messages=[],
        max_round=10,
    )

    manager = GroupChatManager(groupchat=groupchat, llm_config=llm_config)

    # Wrap with Agentreplay tracking
    chronicle_manager = AgentreplayGroupChatManager(
        manager=manager,
        url="http://localhost:8080",
        tenant_id=1,
        session_id=2002,
    )

    # Start group conversation - all agents logged
    user_proxy.initiate_chat(
        chronicle_manager.manager,
        message="Write a brief article about sustainable energy.",
    )

    print("\nGroup chat logged to Agentreplay!\n")


def example_function_calling():
    """Example: Agent with function calls tracked."""
    print("=== Function Calling Example ===\n")

    llm_config = {
        "model": "gpt-4",
        "api_key": os.environ.get("OPENAI_API_KEY"),
        "functions": [
            {
                "name": "get_weather",
                "description": "Get weather for a location",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "City name",
                        }
                    },
                    "required": ["location"],
                },
            }
        ],
    }

    # Create Agentreplay client
    client = AgentreplayClient(
        url="http://localhost:8080",
        tenant_id=1,
        agent_id=3,
    )

    # Define function
    def get_weather(location: str) -> str:
        """Get weather for a location."""
        # Simulate API call
        return f"Weather in {location}: Sunny, 72°F"

    # Wrap function with Agentreplay tracking
    wrapped_get_weather = wrap_autogen_function(
        get_weather,
        client=client,
        session_id=2003,
    )

    # Create agent
    assistant = AssistantAgent(
        name="assistant",
        system_message="You are a helpful assistant with access to weather data.",
        llm_config=llm_config,
    )

    # Wrap agent
    wrapped_assistant = AgentreplayAgentWrapper(
        agent=assistant,
        url="http://localhost:8080",
        tenant_id=1,
        agent_id=3,
        session_id=2003,
    )

    # Register function
    wrapped_assistant.agent.register_function(
        function_map={"get_weather": wrapped_get_weather}
    )

    user_proxy = UserProxyAgent(
        name="user_proxy",
        human_input_mode="NEVER",
        max_consecutive_auto_reply=1,
    )

    # Start conversation - function calls logged
    user_proxy.initiate_chat(
        wrapped_assistant.agent,
        message="What's the weather like in San Francisco?",
    )

    print("\nFunction calls logged to Agentreplay!\n")


def example_query_traces():
    """Example: Query logged traces from Agentreplay."""
    print("=== Query Traces Example ===\n")

    client = AgentreplayClient(
        url="http://localhost:8080",
        tenant_id=1,
        agent_id=1,
    )

    # Get all edges from a session
    session_edges = client.filter_by_session(session_id=2001, limit=100)

    print(f"Found {len(session_edges)} edges in session 2001:")
    for edge in session_edges[:5]:  # Show first 5
        print(f"  - Edge {edge.edge_id}: {edge.span_type} (agent {edge.agent_id})")

    # Get causal chain for a specific edge
    if session_edges:
        root_edge = session_edges[0]
        descendants = client.get_descendants(root_edge.edge_id, max_depth=10)
        print(f"\nRoot edge {root_edge.edge_id} has {len(descendants)} descendants")

    print()


if __name__ == "__main__":
    # Set OpenAI API key
    if "OPENAI_API_KEY" not in os.environ:
        print("Please set OPENAI_API_KEY environment variable")
        exit(1)

    # Run examples
    try:
        example_basic_conversation()
    except Exception as e:
        print(f"Error in basic conversation: {e}\n")

    try:
        example_function_calling()
    except Exception as e:
        print(f"Error in function calling: {e}\n")

    # Group chat requires more tokens
    # Uncomment to run:
    # try:
    #     example_group_chat()
    # except Exception as e:
    #     print(f"Error in group chat: {e}\n")

    # Query the logged traces
    try:
        example_query_traces()
    except Exception as e:
        print(f"Error querying traces: {e}\n")

    print("Examples complete! Check Agentreplay for logged traces.")
