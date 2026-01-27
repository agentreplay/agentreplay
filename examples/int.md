# Flowtrace + LangGraph Testing Plan

## Executive Summary
This plan outlines a systematic approach to testing Flowtrace's observability capabilities using LangGraph as the traced application. The goal is to validate that Flowtrace correctly captures, stores, and queries agentic workflow traces from real LangGraph applications.

---

## Phase 1: Environment Setup (Days 1-2)

### 1.1 Flowtrace Setup
```bash
# Build Flowtrace from source
cd flowtrace
cargo build --release

# Initialize test database
./target/release/flowtrace -d ./test-data init

# Start server (if using HTTP API)
cd flowtrace-server
cargo run --release -- --port 8080 --db-path ../test-data
```

### 1.2 LangGraph Environment Setup
```bash
# Create Python virtual environment
python3 -m venv venv-langgraph
source venv-langgraph/bin/activate

# Install LangGraph and dependencies
pip install langgraph langchain langchain-openai langchain-anthropic
pip install httpx requests  # For sending traces to Flowtrace
pip install python-dotenv  # For API keys
```

### 1.3 Create Flowtrace Python Client
```python
# flowtrace_client.py
import httpx
import time
from typing import Optional, Dict, Any
from enum import Enum

class SpanType(Enum):
    ROOT = 0
    AGENT = 1
    TOOL = 2
    LLM = 3
    RETRIEVER = 4
    CHAIN = 5
    EMBEDDING = 6

class FlowtraceClient:
    def __init__(self, base_url: str = "http://localhost:8080", 
                 tenant_id: int = 1, project_id: int = 0):
        self.base_url = base_url
        self.tenant_id = tenant_id
        self.project_id = project_id
        self.client = httpx.Client()
    
    def create_trace(self, agent_id: int, session_id: int, 
                     span_type: SpanType, parent_id: Optional[int] = None,
                     metadata: Optional[Dict[str, Any]] = None) -> Dict:
        """Create a new trace/span in Flowtrace"""
        payload = {
            "tenant_id": self.tenant_id,
            "project_id": self.project_id,
            "agent_id": agent_id,
            "session_id": session_id,
            "span_type": span_type.name.lower(),
            "parent_id": parent_id or 0,
            "timestamp_us": int(time.time() * 1_000_000),
            "metadata": metadata or {}
        }
        response = self.client.post(f"{self.base_url}/api/v1/traces", json=payload)
        return response.json()
    
    def update_trace(self, edge_id: str, duration_us: int, 
                     token_count: int = 0, payload: Optional[Dict] = None):
        """Update trace with completion data"""
        data = {
            "duration_us": duration_us,
            "token_count": token_count,
            "payload": payload or {}
        }
        response = self.client.put(f"{self.base_url}/api/v1/traces/{edge_id}", json=data)
        return response.json()
    
    def query_traces(self, start_ts: int, end_ts: int, 
                     agent_id: Optional[int] = None,
                     session_id: Optional[int] = None):
        """Query traces in time range"""
        params = {
            "start": start_ts,
            "end": end_ts,
            "tenant": self.tenant_id,
        }
        if agent_id:
            params["agent"] = agent_id
        if session_id:
            params["session"] = session_id
        
        response = self.client.get(f"{self.base_url}/api/v1/traces", params=params)
        return response.json()
    
    def get_trace_graph(self, trace_id: str):
        """Get causal graph for a trace"""
        response = self.client.get(f"{self.base_url}/api/v1/traces/{trace_id}/graph")
        return response.json()
```

---

## Phase 2: LangGraph Test Applications (Days 3-5)

### 2.1 Simple Agent Test (Baseline)
Test Flowtrace with a basic single-agent LangGraph workflow.

```python
# test_01_simple_agent.py
from langgraph.graph import StateGraph, END
from langchain_anthropic import ChatAnthropic
from typing import TypedDict
from flowtrace_client import FlowtraceClient, SpanType
import time

class AgentState(TypedDict):
    messages: list
    session_id: int
    trace_id: str

# Initialize Flowtrace client
cl_client = FlowtraceClient()

def create_agent_node(name: str, agent_id: int):
    def agent_function(state: AgentState):
        start_time = time.time()
        
        # Create trace in Flowtrace
        trace = cl_client.create_trace(
            agent_id=agent_id,
            session_id=state["session_id"],
            span_type=SpanType.AGENT,
            parent_id=state.get("trace_id"),
            metadata={"agent_name": name}
        )
        
        # Execute LLM call
        llm = ChatAnthropic(model="claude-3-5-sonnet-20241022")
        response = llm.invoke(state["messages"])
        
        # Update trace with completion data
        duration_us = int((time.time() - start_time) * 1_000_000)
        cl_client.update_trace(
            edge_id=trace["edge_id"],
            duration_us=duration_us,
            token_count=response.response_metadata.get("usage", {}).get("total_tokens", 0),
            payload={
                "model": "claude-3-5-sonnet-20241022",
                "response": response.content
            }
        )
        
        state["messages"].append(response)
        state["trace_id"] = trace["edge_id"]
        return state
    
    return agent_function

# Build simple graph
workflow = StateGraph(AgentState)
workflow.add_node("agent", create_agent_node("simple_agent", agent_id=1))
workflow.set_entry_point("agent")
workflow.add_edge("agent", END)
app = workflow.compile()

# Test execution
if __name__ == "__main__":
    session_id = int(time.time())
    result = app.invoke({
        "messages": ["What is the capital of France?"],
        "session_id": session_id,
        "trace_id": None
    })
    
    print(f"Session ID: {session_id}")
    print(f"Trace ID: {result['trace_id']}")
    
    # Query traces from Flowtrace
    traces = cl_client.query_traces(
        start_ts=session_id * 1_000_000,
        end_ts=int(time.time() * 1_000_000),
        session_id=session_id
    )
    print(f"Found {len(traces)} traces")
```

### 2.2 Multi-Agent Test (Complex Workflow)
Test with multiple agents, tool calls, and conditional routing.

```python
# test_02_multi_agent.py
from langgraph.graph import StateGraph, END
from langgraph.prebuilt import ToolNode
from langchain_core.tools import tool
from typing import TypedDict, Literal
from flowtrace_client import FlowtraceClient, SpanType
import time

@tool
def search_web(query: str) -> str:
    """Search the web for information"""
    # Simulate search with trace
    cl_client = FlowtraceClient()
    start = time.time()
    trace = cl_client.create_trace(
        agent_id=99,
        session_id=current_session_id,
        span_type=SpanType.TOOL,
        metadata={"tool_name": "search_web", "query": query}
    )
    
    # Simulate work
    time.sleep(0.5)
    result = f"Search results for: {query}"
    
    duration_us = int((time.time() - start) * 1_000_000)
    cl_client.update_trace(trace["edge_id"], duration_us, payload={"result": result})
    
    return result

class MultiAgentState(TypedDict):
    messages: list
    session_id: int
    trace_id: str
    next: str

def router(state: MultiAgentState) -> Literal["agent1", "agent2", "tools", END]:
    """Route to next node based on state"""
    return state.get("next", END)

# Build multi-agent graph with conditional routing
workflow = StateGraph(MultiAgentState)
workflow.add_node("agent1", create_traced_agent("planner", 1))
workflow.add_node("agent2", create_traced_agent("executor", 2))
workflow.add_node("tools", ToolNode([search_web]))

workflow.set_entry_point("agent1")
workflow.add_conditional_edges("agent1", router)
workflow.add_conditional_edges("agent2", router)
workflow.add_edge("tools", "agent2")

app = workflow.compile()
```

### 2.3 LangGraph Examples to Integrate

Use these official LangGraph examples and add Flowtrace instrumentation:

1. **ReAct Agent** - https://github.com/langchain-ai/langgraph/tree/main/examples/react-agent
   - Test: Single agent with tool calls
   - Traces: Agent nodes, tool execution spans, LLM calls

2. **Multi-Agent Collaboration** - https://github.com/langchain-ai/langgraph/tree/main/examples/multi-agent
   - Test: Multiple agents coordinating
   - Traces: Inter-agent communication, causal graph validation

3. **Plan-and-Execute** - https://github.com/langchain-ai/langgraph/tree/main/examples/plan-and-execute
   - Test: Hierarchical planning and execution
   - Traces: Parent-child relationships, nested spans

4. **Human-in-the-Loop** - https://github.com/langchain-ai/langgraph/tree/main/examples/human-in-the-loop
   - Test: Interrupt and resume workflows
   - Traces: Session continuity across interruptions

5. **RAG with Citations** - https://github.com/langchain-ai/langgraph/tree/main/examples/rag
   - Test: Retrieval augmented generation
   - Traces: Retriever spans, embedding generation

---

## Phase 3: Core Functionality Tests (Days 6-8)

### 3.1 Temporal Query Tests
```python
# test_temporal_queries.py
def test_temporal_range_query():
    """Test querying traces within time ranges"""
    # Generate traces over 1 hour
    start_time = time.time() * 1_000_000
    
    for i in range(100):
        session_id = int(start_time) + i * 10000
        # Execute LangGraph workflow
        # ...
        time.sleep(0.1)
    
    end_time = time.time() * 1_000_000
    
    # Query different time ranges
    results_1min = cl_client.query_traces(
        start_ts=int(end_time - 60_000_000),
        end_ts=int(end_time)
    )
    
    results_10min = cl_client.query_traces(
        start_ts=int(end_time - 600_000_000),
        end_ts=int(end_time)
    )
    
    assert len(results_1min) < len(results_10min)
    print(f"1min: {len(results_1min)}, 10min: {len(results_10min)}")
```

### 3.2 Causal Graph Tests
```python
# test_causal_graph.py
def test_parent_child_relationships():
    """Test that causal relationships are correctly captured"""
    session_id = int(time.time())
    
    # Create root trace
    root = cl_client.create_trace(
        agent_id=1,
        session_id=session_id,
        span_type=SpanType.ROOT
    )
    
    # Create child traces
    child1 = cl_client.create_trace(
        agent_id=2,
        session_id=session_id,
        span_type=SpanType.AGENT,
        parent_id=root["edge_id"]
    )
    
    child2 = cl_client.create_trace(
        agent_id=3,
        session_id=session_id,
        span_type=SpanType.TOOL,
        parent_id=child1["edge_id"]
    )
    
    # Verify graph structure
    graph = cl_client.get_trace_graph(root["edge_id"])
    assert len(graph["nodes"]) == 3
    assert len(graph["edges"]) == 2
    
    # Verify hierarchy
    edge_map = {edge["target"]: edge["source"] for edge in graph["edges"]}
    assert edge_map[child1["edge_id"]] == root["edge_id"]
    assert edge_map[child2["edge_id"]] == child1["edge_id"]
```

### 3.3 Multi-Tenant Isolation Tests
```python
# test_tenant_isolation.py
def test_tenant_isolation():
    """Verify that tenant data is properly isolated"""
    tenant1_client = FlowtraceClient(tenant_id=1)
    tenant2_client = FlowtraceClient(tenant_id=2)
    
    session_id = int(time.time())
    
    # Tenant 1 creates traces
    trace1 = tenant1_client.create_trace(
        agent_id=1,
        session_id=session_id,
        span_type=SpanType.AGENT
    )
    
    # Tenant 2 creates traces
    trace2 = tenant2_client.create_trace(
        agent_id=1,
        session_id=session_id,
        span_type=SpanType.AGENT
    )
    
    # Verify isolation
    tenant1_traces = tenant1_client.query_traces(
        start_ts=session_id * 1_000_000,
        end_ts=int(time.time() * 1_000_000)
    )
    
    tenant2_traces = tenant2_client.query_traces(
        start_ts=session_id * 1_000_000,
        end_ts=int(time.time() * 1_000_000)
    )
    
    # Each tenant should only see their own traces
    assert len(tenant1_traces) == 1
    assert len(tenant2_traces) == 1
    assert tenant1_traces[0]["edge_id"] != tenant2_traces[0]["edge_id"]
```

### 3.4 Vector Search Tests (If Implemented)
```python
# test_vector_search.py
def test_semantic_search():
    """Test semantic search on prompts/responses"""
    # Create traces with different prompts
    prompts = [
        "What is quantum computing?",
        "Explain machine learning basics",
        "How does photosynthesis work?",
        "What are neural networks?"
    ]
    
    for i, prompt in enumerate(prompts):
        # Execute workflow and store with embeddings
        # ...
        pass
    
    # Search for ML-related traces
    results = cl_client.semantic_search(
        query="machine learning and AI",
        limit=10
    )
    
    # Should find both ML and neural network traces
    assert len(results["results"]) >= 2
```

---

## Phase 4: Performance & Scale Tests (Days 9-11)

### 4.1 High-Throughput Test
```python
# test_high_throughput.py
import concurrent.futures
import time

def test_concurrent_writes():
    """Test handling concurrent trace writes"""
    num_workers = 10
    traces_per_worker = 1000
    
    def worker(worker_id):
        client = FlowtraceClient()
        start = time.time()
        
        for i in range(traces_per_worker):
            trace = client.create_trace(
                agent_id=worker_id,
                session_id=int(time.time() * 1000) + i,
                span_type=SpanType.AGENT
            )
            client.update_trace(trace["edge_id"], duration_us=1000 * i)
        
        return time.time() - start
    
    with concurrent.futures.ThreadPoolExecutor(max_workers=num_workers) as executor:
        futures = [executor.submit(worker, i) for i in range(num_workers)]
        durations = [f.result() for f in futures]
    
    total_traces = num_workers * traces_per_worker
    total_time = max(durations)
    throughput = total_traces / total_time
    
    print(f"Total traces: {total_traces}")
    print(f"Total time: {total_time:.2f}s")
    print(f"Throughput: {throughput:.2f} traces/sec")
    
    # Should handle >1000 traces/sec
    assert throughput > 1000
```

### 4.2 Long-Running Session Test
```python
# test_long_session.py
def test_long_running_session():
    """Test a session with hundreds of traces"""
    session_id = int(time.time())
    
    # Simulate long-running agentic workflow
    for i in range(500):
        # Execute LangGraph step
        # Each step creates multiple traces (agent + tools + LLM)
        pass
    
    # Query all traces for session
    traces = cl_client.query_traces(
        start_ts=session_id * 1_000_000,
        end_ts=int(time.time() * 1_000_000),
        session_id=session_id
    )
    
    assert len(traces) >= 500
    
    # Verify causal graph can be built
    root_trace = traces[0]
    graph = cl_client.get_trace_graph(root_trace["edge_id"])
    assert len(graph["nodes"]) >= 500
```

### 4.3 Query Performance Test
```python
# test_query_performance.py
def test_query_performance():
    """Benchmark query performance"""
    # Pre-populate database with 100K traces
    populate_database(num_traces=100_000)
    
    # Test different query patterns
    queries = [
        # Recent time range (hot data)
        {"start": now - 3600*1e6, "end": now},
        # Older time range (cold data)
        {"start": now - 7*24*3600*1e6, "end": now - 6*24*3600*1e6},
        # Specific agent
        {"start": now - 24*3600*1e6, "end": now, "agent": 42},
        # Specific session
        {"start": 0, "end": now, "session": 12345},
    ]
    
    for query in queries:
        start = time.time()
        results = cl_client.query_traces(**query)
        duration = time.time() - start
        
        print(f"Query: {query}")
        print(f"Results: {len(results)}, Time: {duration:.3f}s")
        
        # Queries should complete in <100ms
        assert duration < 0.1
```

---

## Phase 5: Integration & End-to-End Tests (Days 12-14)

### 5.1 Full LangGraph Application Test
```python
# test_e2e_customer_support.py
"""
End-to-end test simulating a customer support bot with:
- Intent classification agent
- Knowledge retrieval
- Response generation
- Human escalation
"""

from langgraph.graph import StateGraph
from langgraph.checkpoint import MemorySaver

class CustomerSupportState(TypedDict):
    messages: list
    intent: str
    knowledge: list
    requires_human: bool
    session_id: int

def build_support_bot():
    workflow = StateGraph(CustomerSupportState)
    
    # Add nodes with Flowtrace instrumentation
    workflow.add_node("classify_intent", traced_node(classify_intent, 1))
    workflow.add_node("retrieve_knowledge", traced_node(retrieve_docs, 2))
    workflow.add_node("generate_response", traced_node(generate_response, 3))
    workflow.add_node("escalate_human", traced_node(escalate, 4))
    
    # Add routing logic
    workflow.set_entry_point("classify_intent")
    workflow.add_conditional_edges("classify_intent", route_by_intent)
    # ... more edges
    
    return workflow.compile(checkpointer=MemorySaver())

def test_e2e_workflow():
    bot = build_support_bot()
    session_id = int(time.time())
    
    # Simulate customer conversation
    conversations = [
        "I need help with my order",
        "Order number 12345",
        "Can I get a refund?",
        "Never mind, found it!"
    ]
    
    state = {"messages": [], "session_id": session_id}
    
    for msg in conversations:
        state["messages"].append(msg)
        state = bot.invoke(state)
    
    # Verify complete trace in Flowtrace
    traces = cl_client.query_traces(
        start_ts=session_id * 1_000_000,
        end_ts=int(time.time() * 1_000_000),
        session_id=session_id
    )
    
    # Should have traces for all steps
    assert len(traces) >= 12  # ~3 traces per message
    
    # Get causal graph
    root_id = traces[0]["edge_id"]
    graph = cl_client.get_trace_graph(root_id)
    
    # Visualize for validation
    visualize_graph(graph)
```

### 5.2 Checkpoint Recovery Test
```python
# test_checkpoint_recovery.py
def test_workflow_resume():
    """Test that workflows can resume from Flowtrace traces"""
    session_id = int(time.time())
    
    # Start workflow
    result = app.invoke(initial_state)
    
    # Simulate crash/restart
    del app
    
    # Reconstruct state from Flowtrace traces
    traces = cl_client.query_traces(
        start_ts=session_id * 1_000_000,
        end_ts=int(time.time() * 1_000_000),
        session_id=session_id
    )
    
    # Resume workflow from last known state
    reconstructed_state = reconstruct_state_from_traces(traces)
    new_app = build_workflow()
    
    # Continue execution
    result = new_app.invoke(reconstructed_state)
    
    # Verify continuity in traces
    new_traces = cl_client.query_traces(
        start_ts=session_id * 1_000_000,
        end_ts=int(time.time() * 1_000_000),
        session_id=session_id
    )
    
    assert len(new_traces) > len(traces)
```

---

## Phase 6: Validation & Debugging (Days 15-16)

### 6.1 Data Integrity Tests
```python
# test_data_integrity.py
def test_trace_completeness():
    """Verify all trace data is correctly stored"""
    trace = create_sample_trace_with_all_fields()
    
    # Retrieve and compare
    retrieved = cl_client.get_trace(trace["edge_id"])
    
    assert retrieved["tenant_id"] == trace["tenant_id"]
    assert retrieved["agent_id"] == trace["agent_id"]
    assert retrieved["session_id"] == trace["session_id"]
    assert retrieved["timestamp_us"] == trace["timestamp_us"]
    assert retrieved["duration_us"] == trace["duration_us"]
    assert retrieved["token_count"] == trace["token_count"]
    
    # Verify payload
    payload = cl_client.get_payload(trace["edge_id"])
    assert payload["metadata"] == trace["metadata"]
```

### 6.2 Bloom Filter Tests
```python
# test_bloom_filters.py
def test_bloom_filter_accuracy():
    """Test false positive rate of bloom filters"""
    # Insert 10K traces
    inserted_ids = []
    for i in range(10_000):
        trace = cl_client.create_trace(...)
        inserted_ids.append(trace["edge_id"])
    
    # Test positive lookups (should all succeed)
    for edge_id in inserted_ids:
        result = cl_client.get_trace(edge_id)
        assert result is not None
    
    # Test negative lookups (false positive rate)
    false_positives = 0
    for i in range(10_000):
        fake_id = generate_fake_id()
        if fake_id not in inserted_ids:
            result = cl_client.get_trace(fake_id)
            if result is not None:
                false_positives += 1
    
    fp_rate = false_positives / 10_000
    print(f"False positive rate: {fp_rate:.4f}")
    
    # Should be <1% as per LSM design
    assert fp_rate < 0.01
```

### 6.3 Compression Tests
```python
# test_compression.py
def test_compression_ratio():
    """Test storage compression efficiency"""
    # Insert 1000 traces with payloads
    for i in range(1000):
        trace = cl_client.create_trace(
            agent_id=1,
            session_id=i,
            span_type=SpanType.AGENT,
            payload={
                "prompt": "This is a repeated prompt " * 100,
                "response": "This is a repeated response " * 100
            }
        )
    
    # Check storage size
    db_size = get_database_size()
    
    # Calculate theoretical uncompressed size
    uncompressed_size = 1000 * (200 * 100 + 200 * 100)  # Rough estimate
    
    compression_ratio = uncompressed_size / db_size
    print(f"Compression ratio: {compression_ratio:.2f}x")
    
    # Should achieve >3x compression with LZ4/Zstd
    assert compression_ratio > 3.0
```

---

## Phase 7: Documentation & Tooling (Days 17-18)

### 7.1 Create Testing Documentation
```markdown
# Flowtrace Testing Guide

## Quick Start
1. Build Flowtrace: `cargo build --release`
2. Setup Python env: `python -m venv venv && source venv/bin/activate`
3. Install deps: `pip install -r requirements-test.txt`
4. Run tests: `pytest tests/`

## Test Categories
- Unit tests: Core functionality
- Integration tests: LangGraph workflows
- Performance tests: Benchmarks and load tests
- E2E tests: Full application scenarios

## CI/CD Integration
Tests run automatically on:
- Every PR
- Main branch commits
- Nightly performance benchmarks
```

### 7.2 Create Visualization Tools
```python
# visualize_traces.py
import graphviz

def visualize_trace_graph(graph_data, output_path="trace_graph"):
    """Create visual graph from Flowtrace trace data"""
    dot = graphviz.Digraph(comment='Trace Graph')
    
    # Add nodes
    for node in graph_data["nodes"]:
        label = f"{node['label']}\n{node['duration_ms']:.2f}ms\n{node['tokens']} tokens"
        dot.node(node["id"], label)
    
    # Add edges
    for edge in graph_data["edges"]:
        dot.edge(edge["source"], edge["target"], label=edge["label"])
    
    dot.render(output_path, format='png', cleanup=True)
    print(f"Graph saved to {output_path}.png")

# Usage
graph = cl_client.get_trace_graph(trace_id)
visualize_trace_graph(graph)
```

### 7.3 Create Test Dashboard
```python
# dashboard.py - Simple Streamlit dashboard
import streamlit as st
import pandas as pd

st.title("Flowtrace Test Dashboard")

# Query recent traces
traces = cl_client.query_traces(
    start_ts=int(time.time() * 1e6) - 3600 * 1e6,  # Last hour
    end_ts=int(time.time() * 1e6)
)

# Convert to DataFrame
df = pd.DataFrame(traces)

st.metric("Total Traces", len(df))
st.metric("Avg Duration", f"{df['duration_us'].mean() / 1000:.2f}ms")
st.metric("Total Tokens", df['token_count'].sum())

# Plot timeline
st.line_chart(df.set_index('timestamp_us')['duration_us'])

# Show recent traces
st.dataframe(df[['edge_id', 'agent_id', 'session_id', 'span_type', 'duration_us']])
```

---

## Phase 8: Continuous Testing (Ongoing)

### 8.1 Automated Test Suite
```bash
# run_all_tests.sh
#!/bin/bash

echo "Starting Flowtrace Test Suite..."

# Start Flowtrace server
./target/release/flowtrace-server --port 8080 --db-path ./test-data &
SERVER_PID=$!

# Wait for server to start
sleep 2

# Run Python tests
pytest tests/ -v --tb=short --durations=10

# Run Rust tests
cargo test --all

# Performance benchmarks
cargo bench

# Cleanup
kill $SERVER_PID

echo "All tests complete!"
```

### 8.2 Performance Monitoring
```python
# monitor_performance.py
import time
import psutil

def monitor_test_performance():
    """Monitor system resources during tests"""
    process = psutil.Process()
    
    metrics = {
        "cpu_percent": [],
        "memory_mb": [],
        "io_read_mb": [],
        "io_write_mb": []
    }
    
    # Run tests while monitoring
    start_time = time.time()
    
    while time.time() - start_time < 300:  # 5 min test
        metrics["cpu_percent"].append(process.cpu_percent())
        metrics["memory_mb"].append(process.memory_info().rss / 1024 / 1024)
        
        io_counters = process.io_counters()
        metrics["io_read_mb"].append(io_counters.read_bytes / 1024 / 1024)
        metrics["io_write_mb"].append(io_counters.write_bytes / 1024 / 1024)
        
        time.sleep(1)
    
    # Report
    print(f"Avg CPU: {sum(metrics['cpu_percent']) / len(metrics['cpu_percent']):.2f}%")
    print(f"Avg Memory: {sum(metrics['memory_mb']) / len(metrics['memory_mb']):.2f} MB")
    print(f"Total IO Read: {max(metrics['io_read_mb']):.2f} MB")
    print(f"Total IO Write: {max(metrics['io_write_mb']):.2f} MB")
```

---

## Success Criteria

### ✅ Phase 1-2: Setup (Pass if)
- Flowtrace builds and runs successfully
- LangGraph examples execute without errors
- Python client can communicate with Flowtrace

### ✅ Phase 3-4: Core Tests (Pass if)
- Temporal queries return correct results
- Causal graphs maintain parent-child relationships
- Tenant isolation prevents data leakage
- Throughput >1000 traces/sec
- Query latency <100ms for typical queries

### ✅ Phase 5-6: Integration (Pass if)
- End-to-end workflows complete successfully
- All trace data is correctly stored and retrievable
- Compression ratios >3x
- Bloom filter false positive rate <1%

### ✅ Phase 7-8: Production Readiness (Pass if)
- Comprehensive test suite with >80% coverage
- Automated CI/CD pipeline
- Performance benchmarks tracked over time
- Documentation complete and accurate

---

## Next Steps

1. **Immediate**: Set up Flowtrace and LangGraph environments
2. **Week 1**: Implement basic LangGraph instrumentation
3. **Week 2**: Run core functionality tests
4. **Week 3**: Performance testing and optimization
5. **Week 4**: Production readiness and documentation

## Additional Resources

- LangGraph Examples: https://github.com/langchain-ai/langgraph/tree/main/examples
- LangGraph Docs: https://langchain-ai.github.io/langgraph/
- Flowtrace Architecture: See combined_project.rs
- OpenTelemetry Tracing: https://opentelemetry.io/ (for inspiration)

---

**Last Updated**: 2025-11-05
**Author**: Technical Fellow, PhD Mathematics & Computer Science
**Project**: Flowtrace Observability Testing with LangGraph