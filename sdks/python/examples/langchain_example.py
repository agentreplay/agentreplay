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

"""Example: Using Agentreplay with LangChain.

This example demonstrates how to automatically log LangChain traces
to Agentreplay using the callback handler.
"""

import os
from langchain.chat_models import ChatOpenAI
from langchain.prompts import ChatPromptTemplate
from langchain.chains import LLMChain
from langchain.agents import AgentType, initialize_agent, load_tools

from agentreplay.integrations.langchain import (
    AgentreplayCallbackHandler,
    wrap_langchain_with_chronolake,
)


def example_basic_chain():
    """Example: Basic LLM chain with Agentreplay logging."""
    print("=== Basic Chain Example ===\n")

    # Create callback handler
    callback = AgentreplayCallbackHandler(
        url="http://localhost:8080",
        tenant_id=1,
        agent_id=1,
        session_id=1001,
    )

    # Create LLM and chain
    llm = ChatOpenAI(temperature=0.7)
    prompt = ChatPromptTemplate.from_template("Tell me a joke about {topic}")
    chain = LLMChain(llm=llm, prompt=prompt, callbacks=[callback])

    # Run chain - automatically logged to Agentreplay
    result = chain.run(topic="programming")
    print(f"Result: {result}\n")


def example_agent_with_tools():
    """Example: LangChain agent with tools."""
    print("=== Agent with Tools Example ===\n")

    # Create callback handler
    callback = AgentreplayCallbackHandler(
        url="http://localhost:8080",
        tenant_id=1,
        agent_id=2,
        session_id=1002,
    )

    # Create agent with tools
    llm = ChatOpenAI(temperature=0)
    tools = load_tools(["serpapi", "llm-math"], llm=llm)

    agent = initialize_agent(
        tools,
        llm,
        agent=AgentType.ZERO_SHOT_REACT_DESCRIPTION,
        callbacks=[callback],
        verbose=True,
    )

    # Run agent - all steps logged to Agentreplay
    result = agent.run(
        "What is the population of Tokyo? What is that number raised to the power of 2?"
    )
    print(f"Result: {result}\n")


def example_wrap_component():
    """Example: Wrap existing LangChain component."""
    print("=== Wrap Component Example ===\n")

    # Create chain without callbacks
    llm = ChatOpenAI(temperature=0.7)
    prompt = ChatPromptTemplate.from_template("Write a haiku about {subject}")
    chain = LLMChain(llm=llm, prompt=prompt)

    # Wrap with Agentreplay logging
    chain = wrap_langchain_with_chronolake(
        chain,
        chronolake_url="http://localhost:8080",
        tenant_id=1,
        agent_id=3,
        session_id=1003,
    )

    # Run chain - now logged to Agentreplay
    result = chain.run(subject="artificial intelligence")
    print(f"Result: {result}\n")


def example_multi_step_reasoning():
    """Example: Multi-step reasoning chain."""
    print("=== Multi-step Reasoning Example ===\n")

    from langchain.chains import SequentialChain

    callback = AgentreplayCallbackHandler(
        url="http://localhost:8080",
        tenant_id=1,
        agent_id=4,
        session_id=1004,
    )

    llm = ChatOpenAI(temperature=0.7)

    # First chain: Generate a topic
    prompt1 = ChatPromptTemplate.from_template("Suggest an interesting topic about {field}")
    chain1 = LLMChain(llm=llm, prompt=prompt1, output_key="topic")

    # Second chain: Write about the topic
    prompt2 = ChatPromptTemplate.from_template(
        "Write a short paragraph about: {topic}"
    )
    chain2 = LLMChain(llm=llm, prompt=prompt2, output_key="paragraph")

    # Combine chains
    overall_chain = SequentialChain(
        chains=[chain1, chain2],
        input_variables=["field"],
        output_variables=["topic", "paragraph"],
        callbacks=[callback],
    )

    # Run - both steps logged to Agentreplay
    result = overall_chain({"field": "quantum computing"})
    print(f"Topic: {result['topic']}")
    print(f"Paragraph: {result['paragraph']}\n")


if __name__ == "__main__":
    # Set OpenAI API key
    if "OPENAI_API_KEY" not in os.environ:
        print("Please set OPENAI_API_KEY environment variable")
        exit(1)

    # Run examples
    try:
        example_basic_chain()
    except Exception as e:
        print(f"Error in basic chain: {e}\n")

    try:
        example_wrap_component()
    except Exception as e:
        print(f"Error in wrap component: {e}\n")

    try:
        example_multi_step_reasoning()
    except Exception as e:
        print(f"Error in multi-step reasoning: {e}\n")

    # Note: Agent example requires SERPAPI_API_KEY
    # Uncomment if you have the key:
    # try:
    #     example_agent_with_tools()
    # except Exception as e:
    #     print(f"Error in agent: {e}\n")

    print("Examples complete! Check Agentreplay for logged traces.")
