#!/bin/bash

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

# Run all framework examples and capture outputs

echo "================================"
echo "Running All Framework Examples"
echo "================================"

# Run LangGraph (already completed, but re-run for fresh data)
echo -e "\n[1/4] Running LangGraph..."
cd agentreplay_langgraph
python multi_agent_research.py > langraph_output.log 2>&1 &
LANGGRAPH_PID=$!
cd ..

# Run LangChain
echo -e "\n[2/4] Running LangChain..."
cd agentreplay_langchain  
python rag_agent_with_memory.py  > langchain_output.log 2>&1 &
LANGCHAIN_PID=$!
cd ..

# Wait for LangGraph to finish (it's faster)
wait $LANGGRAPH_PID
echo "✅ LangGraph complete"

# Wait for LangChain
wait $LANGCHAIN_PID
echo "✅ LangChain complete"

echo -e "\n================================"
echo "All frameworks executed!"
echo "================================"
echo "View traces:"
echo "  LangGraph: http://localhost:47173/projects/31696/traces"
echo "  LangChain: http://localhost:47173/projects/31697/traces"
