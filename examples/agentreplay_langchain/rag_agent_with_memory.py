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
Agentreplay + LangChain Example
Auto-instrumented RAG chain with memory

Project ID: 31697 (LangChain tracking)
"""
import os
from dotenv import load_dotenv
load_dotenv()

# Agentreplay auto-instrumentation
import agentreplay
agentreplay.init_otel_instrumentation(
    service_name="agentreplay-langchain-demo",
    otlp_endpoint=os.getenv("AGENTREPLAY_OTLP_ENDPOINT", "localhost:4317"),
    project_id=31697,  # LangChain project
    tenant_id=int(os.getenv("AGENTREPLAY_TENANT_ID", "1")),
    debug=True
)
print("✅ Agentreplay initialized for LangChain")

from langchain_openai import AzureChatOpenAI
from langchain.prompts import ChatPromptTemplate, MessagesPlaceholder
from langchain.schema import StrOutputParser
from langchain.memory import ConversationBufferMemory
from langchain_community.tools.tavily_search import TavilySearchResults
from langchain.agents import AgentExecutor, create_openai_tools_agent


def create_rag_chain():
    """Create a RAG chain with memory"""
    print("\n🔗 Creating RAG chain...")
    
    llm = AzureChatOpenAI(
        azure_endpoint=os.getenv("AZURE_OPENAI_ENDPOINT"),
        azure_deployment=os.getenv("AZURE_OPENAI_DEPLOYMENT", "gpt-4o"),
        api_version="2024-12-01-preview",
        temperature=0.7
    )
    
    prompt = ChatPromptTemplate.from_messages([
        ("system", "You are a helpful AI assistant with access to web search. Answer questions accurately and cite sources."),
        MessagesPlaceholder(variable_name="chat_history"),
        ("human", "{input}"),
        MessagesPlaceholder(variable_name="agent_scratchpad"),
    ])
    
    tools = [TavilySearchResults(max_results=3, name="web_search")]
    
    agent = create_openai_tools_agent(llm, tools, prompt)
    agent_executor = AgentExecutor(
        agent=agent,
        tools=tools,
        verbose=True,
        max_iterations=3
    )
    
    return agent_executor


def conversational_agent():
    """Run a conversational agent with memory"""
    print("="*60)
    print("LangChain Conversational Agent - Auto-Instrumented")
    print("="*60)
    
    agent = create_rag_chain()
    chat_history = []
    
    questions = [
        "What are the latest developments in Large Language Models?",
        "Which companies are leading in this space?",
        "What did you just tell me about companies? Summarize briefly."
    ]
    
    for i, question in enumerate(questions, 1):
        print(f"\n{'='*60}")
        print(f"Question {i}: {question}")
        print('='*60)
        
        result = agent.invoke({
            "input": question,
            "chat_history": chat_history
        })
        
        print(f"\n💬 Answer: {result['output']}\n")
        
        # Update history
        chat_history.extend([
            {"role": "human", "content": question},
            {"role": "assistant", "content": result['output']}
        ])
    
    print("\n✅ View traces: http://localhost:5173/projects/31697/traces")


if __name__ == "__main__":
    conversational_agent()
