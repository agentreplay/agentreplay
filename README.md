# Agent Replay

<div align="center">

### 🖥️ Local-First Desktop Observability & AI Memory for Your Agents and Coding Tools.

**No Docker. No servers. No cloud. Just run.**

[![License](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/python-3.8%2B-blue.svg)](https://www.python.org/)

[Features](#-features) • [Quick Start](#-quick-start) • [Architecture](#-architecture) • [Documentation](#-documentation) • [Performance](#-performance) • [Contributing](#-contributing)

</div>

---

## 🎯 Why Agent Replay?

<table>
<tr>
<td width="50%">

### ❌ What We're NOT
- ~~Docker containers~~
- ~~Web servers to run~~
- ~~Cloud accounts required~~
- ~~Complex infrastructure~~
- ~~Memory limits~~
- ~~Monthly subscriptions~~

</td>
<td width="50%">

### ✅ What We ARE
- **Native desktop app** - double-click and run
- **Everything built-in** - storage, UI, APIs
- **Unlimited local memory** - use your full disk
- **Works with Claude Code, Cursor, Windsurf, Cline**
- **Zero configuration** - start tracing in seconds
- **100% offline capable** - your data stays local

</td>
</tr>
</table>

> **Built for AI coding agents** - Agent Replay gives your tools like Claude Code, Cursor, and VS Code agents persistent memory and full observability without cloud dependencies.

---

## 🚀 Quick Start

### Download Agent Replay Desktop

| Platform | Download | Architecture |
|----------|----------|--------------|
| **macOS** | [Agent Replay.dmg](https://github.com/sochdb/agentreplay/releases/latest/download/Agent Replay_aarch64.dmg) | Apple Silicon (M1/M2/M3) |
| **macOS** | [Agent Replay.dmg](https://github.com/sochdb/agentreplay/releases/latest/download/Agent Replay_x64.dmg) | Intel |
| **Windows** | [Agent Replay.exe](https://github.com/sochdb/agentreplay/releases/latest/download/Agent Replay_x64-setup.exe) | x64 (NSIS Installer) |
| **Windows** | [Agent Replay.msi](https://github.com/sochdb/agentreplay/releases/latest/download/Agent Replay_x64.msi) | x64 (MSI Installer) |
| **Linux** | [Agent Replay.AppImage](https://github.com/sochdb/agentreplay/releases/latest/download/Agent Replay_amd64.AppImage) | x64 |
| **Linux** | [Agent Replay.deb](https://github.com/sochdb/agentreplay/releases/latest/download/Agent Replay_amd64.deb) | Debian/Ubuntu |

### Or Build from Source

```bash
# Clone and run (that's it!)
git clone https://github.com/sochdb/agentreplay.git
cd agentreplay
./run-tauri.sh
```

**That's it.** No Docker. No `docker-compose up`. No environment variables. No database setup. Just a native app with everything inside.

---

## 🎯 Overview

Agent Replay is a **local-first desktop application** that gives your AI agents and coding tools:

- **🧠 Unlimited persistent memory** - stored on your machine, not in the cloud
- **👁️ Full observability** - see every decision, tool call, and reasoning step
- **⚡ Instant performance** - native desktop app, not a browser tab
- **🔒 Complete privacy** - your conversations and code never leave your machine

### Works With Your Favorite AI Tools

| Tool | Integration | Status |
|------|-------------|--------|
| **Claude Code** | [Native Plugin](https://github.com/sochdb/agentreplay-claude-plugin) | ✅ Ready |
| **Cursor** | MCP + Extension | ✅ Ready |
| **Windsurf** | MCP server | ✅ Ready |
| **Cline** | MCP server | ✅ Ready |
| **VS Code + Copilot** | Extension | ✅ Ready |
| **Custom Agents** | Python/JS/Rust SDK | ✅ Ready |

### 🔌 Claude Code Plugin (Recommended)

Install the official Agent Replay plugin for Claude Code:

```bash
# Add the marketplace
/plugin marketplace add sochdb/agentreplay-claude-plugin

# Install the plugin
/plugin install agentreplay@sochdb-agentreplay-claude-plugin
```

**Available Commands:**
- `/agentreplay:dashboard` - Open dashboard
- `/agentreplay:status` - Check server health
- `/agentreplay:remember [query]` - Search memories
- `/agentreplay:traces [count]` - List recent traces

> See [agentreplay-claude-plugin](https://github.com/sochdb/agentreplay-claude-plugin) for full documentation.

### Powered by SochDB - Everything Built-In

Unlike tools that need Postgres, Redis, or cloud databases, Agent Replay uses **SochDB** - a high-performance embedded database that lives inside the app:

| Feature | Benefit |
|---------|---------|
| **Embedded database** | No external services needed |
| **LSM-tree storage** | Write-optimized for trace ingestion |
| **Columnar storage** | 80% less I/O for analytics |
| **Vector indexes** | Semantic search over your agent memory |
| **ACID transactions** | Crash-safe, no data loss |

> Your traces, memory, and analytics all stored locally with zero setup.

---

## ✨ Features

### 🧠 AI Agent Memory (RAG Built-In)

Give your coding agents **persistent, unlimited memory** that survives restarts:

- **Semantic memory storage** - ingest content into vector-indexed collections
- **Instant retrieval** - find relevant past conversations with similarity search
- **Session continuity** - Claude Code remembers your entire project context
- **Cross-session learning** - agents learn from past interactions
- **No token limits** - store everything locally, retrieve what's relevant
- **HNSW/Vamana indexes** - 95% recall with 32x memory compression

### 👁️ Full Observability

See exactly what your AI agents are doing:

- **Every tool call** traced with inputs, outputs, and timing
- **Reasoning chains** visualized as causal graphs
- **Token usage** tracked per model, session, and project
- **Cost analytics** - automatic pricing from LiteLLM registry
- **OTLP ingestion** - accepts OpenTelemetry traces on ports 47117/47118

### 🤖 Multi-Provider LLM Support

Connect to any LLM provider from the desktop app:

| Provider | Features | Status |
|----------|----------|--------|
| **OpenAI** | GPT-4o, GPT-4 Turbo, embeddings | ✅ Ready |
| **Anthropic** | Claude 3.5 Sonnet/Haiku | ✅ Ready |
| **Google** | Gemini Pro, embeddings | ✅ Ready |
| **DeepSeek** | DeepSeek Chat/Coder | ✅ Ready |
| **Ollama** | Local models, no API key | ✅ Ready |
| **Mistral** | Mistral Large/Medium | ✅ Ready |

### ⚖️ Model Comparison Engine

Compare up to 3 models side-by-side on the same prompt:

- **Parallel execution** - all models run simultaneously
- **Independent streaming** - each model streams independently
- **Cost comparison** - see cost per model from LiteLLM pricing
- **Latency tracking** - fastest/slowest model identification
- **Error isolation** - one model failing doesn't affect others

### 📊 Evaluation Framework (20+ Evaluators)

Built-in evaluation with no external dependencies:

| Category | Evaluators |
|----------|------------|
| **Quality** | Hallucination detection, RAGAS, QAG faithfulness, G-Eval |
| **Safety** | Toxicity detection, bias detection |
| **Performance** | Latency benchmarks, cost analysis, trajectory efficiency |
| **RAG** | Context precision, faithfulness, relevancy |

**Evaluation Presets:**
- 🔍 **RAG Quality** - context precision + faithfulness
- 🔍 **RAG Deep** - QAG faithfulness with per-claim verdicts
- 🤖 **Agent Performance** - trajectory optimization + tool usage
- 🛡️ **Safety** - toxicity + compliance checks
- ⏱️ **Latency** - p50/p95/p99 with cost analysis
- 📋 **Comprehensive** - run all evaluators

**Eval Pipeline Features:**
- **Datasets** - create test case collections with expected outputs
- **Eval Runs** - track evaluation sessions with pass/fail results
- **A/B Comparison** - statistical comparison (Welch's t-test, Cohen's d)
- **G-Eval** - LLM-as-judge with configurable criteria (coherence, relevance, fluency)
- **Human Annotations** - thumbs up/down, star ratings, corrected outputs

### 📝 Prompt Registry & Versioning

Git-like version control for your prompts:

- **Automatic versioning** - each prompt update creates a new version
- **Template variables** - `{{variable}}` syntax with validation
- **Semantic versioning** - major.minor.patch for prompts
- **Deployment environments** - dev, staging, production
- **Traffic splitting** - A/B test prompts with rollout strategies
- **Lineage tracking** - parent-child version relationships
- **API endpoints** - `/api/v1/prompts` for CRUD operations

### 🔌 Plugin System

Extend Agent Replay with custom evaluators and integrations:

- **Install from directory/file** - local plugin development
- **Dev mode** - hot-reload during development
- **Plugin SDK** - Python, Rust, JavaScript
- **Evaluator plugins** - add custom quality checks
- **Bundle system** - package for Claude Code, Cursor, VS Code

### 📈 Analytics & Dashboards

Real-time metrics with DDSketch percentiles and HyperLogLog cardinality:

- **Time-series metrics** - automatic rollup (minute/hour/day)
- **True percentiles** - P50/P90/P95/P99 via DDSketch
- **Unique counts** - sessions/agents via HyperLogLog (~0.81% error)
- **Storage health** - MVCC stats, tombstone GC, write amplification
- **Bloom filter monitoring** - per-level FPR configuration

### 💾 Backup & Restore

Protect your data with built-in backup features:

- **One-click backup** - from Settings or CLI
- **Export as ZIP** - portable backup files
- **Import from ZIP** - restore from any backup
- **Pre-restore safety** - automatic backup before restore
- **Merge mode** - append traces without replacing

### Core Capabilities

- **🚀 High-Performance Ingestion**
  - 10,000 spans/minute rate limit with 50,000 burst
  - Sub-millisecond point query latency
  - Write-optimized LSM-tree storage
  - Lock-free reads for concurrent access

- **🔗 Native Causal Graph Support**
  - Track parent-child relationships between agent actions
  - Traverse reasoning chains efficiently
  - Understand multi-step agent workflows
  - Query ancestors, descendants, and siblings

- **📊 Comprehensive Evaluation Framework**
  - **Hallucination Detection**: LLM-as-judge with claim verification
  - **Relevance Scoring**: Semantic similarity between input/output
  - **Toxicity Detection**: Content safety monitoring
  - **Latency Benchmarking**: Performance profiling with percentiles
  - **Cost Tracking**: Token usage and costs across providers
  - **Anomaly Detection**: Statistical outlier identification

- **💰 Cost Intelligence**
  - Track costs per model, agent, session, and project
  - LiteLLM pricing sync for accurate cost calculation
  - Historical cost analysis and forecasting
  - Input/output token separation for accurate pricing

- **🧪 A/B Testing & Experimentation**
  - Multi-variant experiment support
  - Traffic splitting with statistical analysis
  - Prompt template versioning
  - Side-by-side performance comparison

- **🔍 Powerful Query Engine**
  - Temporal range queries with microsecond precision
  - Causal traversal (ancestors, descendants, paths)
  - Multi-tenant filtering (tenant, project, agent, session)
  - Vector similarity search (semantic search)
  - Aggregations and analytics

### 🖥️ Desktop-First Architecture

The primary way to run Agent Replay - a native desktop app:

- **Double-click to run** - no terminal, no commands
- **Everything embedded** - database, UI, API server all inside
- **Cross-platform** - Windows, macOS, Linux
- **Unlimited storage** - uses your local disk, no cloud limits
- **10x faster** - native IPC vs HTTP
- **System tray** - runs in background, always available
- **Embedded HTTP server** - REST API on port 47100
- **OTLP endpoints** - gRPC 4317, HTTP 4318

### 📦 SDKs & Integrations

- **Python SDK** with 10+ framework integrations:
  - LangChain / LangGraph
  - LlamaIndex
  - OpenAI Agents SDK
  - Microsoft AutoGen
  - CrewAI
  - Semantic Kernel
  - Hugging Face smolagents
  - PydanticAI
  - AWS Strands Agents
  - Google ADK
- **JavaScript/TypeScript SDK** (npm package)
- **Rust SDK** (crates.io package)
- **Go SDK** (Go module)

### Enterprise Features

- **📋 Compliance Reporting**
  - GDPR/CCPA compliance reports
  - Security audit trails
  - Quality metrics dashboards
  - Data retention policies

- **📈 Advanced Analytics**
  - Time-series analysis with trend detection
  - Correlation discovery across metrics
  - Custom dashboard creation
  - Export to Prometheus, Grafana, Jaeger

- **🔐 Security & Governance**
  - Multi-tenant isolation
  - Role-based access control (RBAC)
  - API key management
  - Audit logging

---

## 🚀 Quick Start

### Option 1: Desktop Application (Recommended for Local Development)

**Prerequisites:**
- Rust 1.70+ ([rustup.rs](https://rustup.rs/))
- Node.js 18+ ([nodejs.org](https://nodejs.org/))

```bash
# Clone the repository
git clone https://github.com/sochdb/agentreplay.git
cd agentreplay

# Install frontend dependencies
cd ui
npm install

# Run the desktop app
npm run tauri dev
```

The desktop app will launch with a local database at:
- **Windows**: `C:\Users\<User>\AppData\Roaming\Agent Replay\database`
- **macOS**: `~/Library/Application Support/Agent Replay/database`
- **Linux**: `~/.local/share/Agent Replay/database`

### Option 2: Server Deployment

**Prerequisites:**
- Rust 1.70+
- (Optional) Docker & Docker Compose

```bash
# Build the server
cargo build --release -p agentreplay-server

# Run with default configuration
./target/release/agentreplay-server

# Or use Docker
docker-compose up -d
```

The server will start on `http://localhost:8080` by default.

**Configuration**: Edit `agentreplay-server-config.toml` or set environment variables.

### Option 3: Python SDK

```bash
# Install from PyPI
pip install agentreplay

# Or with framework integrations
pip install agentreplay[langchain]      # LangChain/LangGraph
pip install agentreplay[llamaindex]     # LlamaIndex
pip install agentreplay[all-frameworks] # All integrations
```

**Basic Usage:**

```python
from agentreplay import Agent ReplayClient, SpanType

# Initialize client
client = Agent ReplayClient(
    url="http://localhost:8080",
    tenant_id=1,
    project_id=0
)

# Log a trace with automatic parent-child relationships
with client.trace(span_type=SpanType.ROOT) as root:
    # Planning step
    with root.child(SpanType.PLANNING) as planning:
        planning.set_token_count(50)
        planning.set_confidence(0.95)

    # Tool call
    with root.child(SpanType.TOOL_CALL) as tool:
        tool.set_token_count(20)
        tool.set_duration_ms(150)

    # Response
    with root.child(SpanType.RESPONSE) as response:
        response.set_token_count(80)
        response.set_confidence(0.94)

# Query traces
edges = client.query_temporal_range(
    start_timestamp_us=start_time,
    end_timestamp_us=end_time
)

# Get causal relationships
children = client.get_children(edge_id)
ancestors = client.get_ancestors(edge_id)
```

**Framework Integration Example (LangChain):**

```python
from agentreplay.integrations.langchain import Agent ReplayCallbackHandler
from langchain.chains import LLMChain

callback = Agent ReplayCallbackHandler(
    url="http://localhost:8080",
    tenant_id=1
)

chain = LLMChain(llm=llm, callbacks=[callback])
result = chain.run("What is the weather?")
# Automatically creates traces with full parent-child relationships
```

---

## 🏗️ Architecture

Agent Replay is built as a modular Rust workspace with 8 specialized crates:

```
┌─────────────────────────────────────────────────────────┐
│                   Client Applications                    │
│     (Python SDK, Desktop App, Custom Integrations)      │
└────────────────────┬────────────────────────────────────┘
                     │
        ┌────────────┴────────────┐
        ▼                         ▼
┌──────────────┐        ┌──────────────────┐
│  Python SDK  │        │  Tauri Desktop   │
│  (10 Frmwks) │        │  (IPC Commands)  │
└──────┬───────┘        └────────┬─────────┘
       │                         │
       └──────────┬──────────────┘
                  ▼
      ┌───────────────────────┐
      │  agentreplay-server    │  ← REST API, Auth, WebSocket
      └──────────┬────────────┘
                 ▼
      ┌───────────────────────┐
      │  agentreplay-query     │  ← Query Engine, Aggregations
      └──────────┬────────────┘
                 │
        ┌────────┴────────┐
        ▼                 ▼
┌───────────────┐  ┌──────────────────┐
│ agentreplay-   │  │ agentreplay-      │
│   index       │  │   evals          │
└───────┬───────┘  └──────────────────┘
        │          ↓ Evaluation Framework
        ▼
┌──────────────────────────────────────┐
│      agentreplay-storage              │  ← SochDB Backend
│                                      │
│  ┌─────────────────────────────────┐│
│  │           SochDB                ││
│  │  ┌──────────────────────────┐  ││
│  │  │ sochdb-storage (LSM/WAL) │  ││
│  │  ├──────────────────────────┤  ││
│  │  │ sochdb-index (HNSW/BM25) │  ││
│  │  ├──────────────────────────┤  ││
│  │  │ sochdb-query (SOCH-QL)   │  ││
│  │  └──────────────────────────┘  ││
│  └─────────────────────────────────┘│
│                                      │
│  ┌──────────────────────┐           │
│  │  Payload Store       │           │
│  │  (Variable data)     │           │
│  └──────────────────────┘           │
└─────────────────────────────────────┘
        ▼
┌───────────────────────┐
│  agentreplay-core      │  ← Edge format, data structures
└───────────────────────┘
```


### Key Components

| Crate | Purpose | Key Features |
|-------|---------|--------------|
| **agentreplay-core** | Foundational types | 128-byte AgentFlowEdge, span types, validation |
| **agentreplay-storage** | SochDB integration | Unified storage layer, columnar projection, key encoding |
| **agentreplay-index** | Indexing layer | HNSW, Vamana with Product Quantization (32x compression), Bloom filters, causal graph, temporal index |
| **agentreplay-query** | Query engine | Temporal queries, causal traversal, aggregations |
| **agentreplay-server** | HTTP API | REST endpoints, auth, multi-tenancy, WebSocket |
| **agentreplay-cli** | Command-line tool | Server management, DB inspection, benchmarks |
| **agentreplay-observability** | O11y integrations | OpenTelemetry, Prometheus, Jaeger export |
| **agentreplay-evals** | Evaluation framework | 20+ evaluators, LLM-as-judge, dataset management |

### SochDB Storage Architecture

Agent Replay's storage layer is built on **SochDB**, providing:

```
┌─────────────────────────────────────────────────────────────────┐
│                    AgentReplayStorage                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │
│  │ Trace Store  │  │ Payload Store│  │ Metrics Store│           │
│  │ (edges)      │  │ (blobs)      │  │ (aggregates) │           │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘           │
│         └─────────────────┼─────────────────┘                   │
│                           │                                     │
│                    ┌──────▼──────┐                              │
│                    │   SochDB    │                              │
│                    │ Connection  │                              │
│                    └─────────────┘                              │
└─────────────────────────────────────────────────────────────────┘
```

**Key Encoding Schema:**
- Traces: `traces/{tenant_id}/{project_id}/{timestamp:020}/{edge_id:032x}`
- Payloads: `payloads/{edge_id:032x}`
- Metrics: `metrics/{granularity}/{tenant_id}/{project_id}/{timestamp:020}`
- Graph: `graph/{direction}/{node_id:032x}/{related_id:032x}`

**Columnar Edge Storage** enables 80% I/O reduction on analytics queries by reading only needed columns instead of full edges.

### The AgentFlowEdge Format

At the heart of Agent Replay is the **128-byte fixed-size edge format**:

```rust
pub struct AgentFlowEdge {
    edge_id: u128,          // 16 bytes - Unique identifier
    causal_parent: u128,    // 16 bytes - Parent reference
    timestamp_us: u64,      // 8 bytes - Microsecond timestamp
    logical_clock: u64,     // 8 bytes - Lamport clock
    tenant_id: u64,         // 8 bytes - Multi-tenancy
    project_id: u16,        // 2 bytes - Project isolation
    agent_id: u64,          // 8 bytes - Agent ID
    session_id: u64,        // 8 bytes - Session tracking
    span_type: u64,         // 8 bytes - Span categorization
    token_count: u32,       // 4 bytes - Token usage
    duration_us: u32,       // 4 bytes - Execution time
    confidence: f32,        // 4 bytes - Confidence score
    sampling_rate: f32,     // 4 bytes - Sampling
    // ... additional fields to 128 bytes
}
```

**Why 128 bytes?**
- Cache-line aligned for optimal CPU performance
- Fixed size enables high-speed sequential writes
- Small enough to minimize storage overhead
- Large enough for essential metadata

**Variable-size data** (prompts, responses, metadata) is stored separately in the **Payload Store** and referenced by `edge_id`.

---

## 📊 Performance

### Benchmark Results

Tested against a real Agent Replay server with **240K+ traces** in the database:

#### Write Performance

| Metric | Value |
|--------|-------|
| Single Write Latency | P50: 13.7ms, P99: 19.1ms |
| Batch Throughput | 55-75 spans/sec (via HTTP API) |
| 100K Span Ingest | ~30 minutes (concurrent) |

#### Query Performance at Scale (240K+ traces)

| Metric | Value |
|--------|-------|
| Point Query (single trace) | P50: 0.96ms, P99: 4.4ms |
| Range Query (100 results) | P50: 76ms, P99: 145ms |
| Causal Traversal | P50: 47.7ms |
| Sessions List | P50: 160ms |

#### Index Architecture Performance

| Index Type | Operation | Performance |
|------------|-----------|-------------|
| LSM-Tree | Point Lookup | P50: 1.47ms |
| CSR Graph | Tree Traversal | P50: 5.56ms |
| Temporal | Range Scan | P50: 9.40ms |
| HNSW Vector | 384-dim Insert | P50: 14.37ms |
| Vamana + PQ | 384-dim Search | P50: 8.5ms (32x compression) |
| Concurrent | Mixed Workload | 319 ops/sec |

### Vamana + Product Quantization: Scaling to 10M+ Vectors

Agent Replay now includes the **Vamana index** with **Product Quantization (PQ)** for massive vector scaling:

**Memory Efficiency at Scale:**

| Vectors | F32 | F16 | PQ (Vamana) | Savings |
|---------|-----|-----|-------------|----------|
| 10K | 15.2 MB | 7.6 MB | 0.48 MB | 32x vs F32 |
| 100K | 152 MB | 76 MB | 4.8 MB | 32x vs F32 |
| 1M | 1.43 GB | 0.72 GB | 48 MB | 32x vs F32 |
| 10M | 14.31 GB | 7.15 GB | 480 MB | 32x vs F32 |

**Key Features:**

- **Product Quantization**: 384-dim vectors compressed to 48 bytes (32x compression)
- **Single-layer Graph**: Faster traversal than multi-layer HNSW
- **Beam Search**: Efficient nearest neighbor search with tunable accuracy
- **RobustPrune**: Angular diversity ensures long edges for fewer hops
- **Backedge Deltas**: Reduce write amplification during insertions
- **Integrated with LSM Storage**: Full vectors on disk via MmapVectorStorage

**Usage:**

```rust
// Create Vamana index with PQ
let config = VamanaConfig::for_dimension(384); // Auto-config for 384-dim
let index = VamanaIndex::new(config);

// Train PQ codebooks on sample vectors
index.train_codebooks(&sample_vectors);

// Insert vectors (automatically PQ-encoded)
for (id, vector) in vectors.iter().enumerate() {
    index.insert_array(id as u128, vector.clone())?;
}

// Search with PQ distance table (8.5ms for 384-dim)
let results = index.search(query, k=10)?;
```

### Architecture Benefits

1. **SochDB Storage Backend**: ACID-compliant embedded database with LSM-tree architecture
2. **Columnar Projection**: 80%+ I/O reduction for analytics queries via SochDB's PackedRow format
3. **Fixed-Size Edges**: No parsing overhead, direct memory mapping
4. **Bloom Filters**: Eliminates unnecessary disk reads
5. **CSR Causal Index**: 50-70% memory savings vs hash maps
6. **HNSW Vector Index**: ~95% recall for semantic search
7. **Vamana + Product Quantization**: 32x memory reduction + single-layer graph for 10M+ vectors
8. **Batch Writes**: Amortizes write overhead across multiple edges
9. **Compression**: LZ4/Zstd reduces disk I/O

---

## 📚 Documentation

📖 **Full Documentation**: [sochdb.github.io/agentreplay](https://sochdb.github.io/agentreplay)

### Quick Links

| Guide | Description |
|-------|-------------|
| [Getting Started](https://sochdb.github.io/agentreplay/docs/getting-started/) | Installation and first trace |
| [Python SDK](https://sochdb.github.io/agentreplay/docs/python-sdk/) | Python client with framework integrations |
| [JavaScript SDK](https://sochdb.github.io/agentreplay/docs/javascript-sdk/) | TypeScript/JavaScript client |
| [Rust SDK](https://sochdb.github.io/agentreplay/docs/rust-sdk/) | Rust client library |
| [Go SDK](https://sochdb.github.io/agentreplay/docs/go-sdk/) | Go client library |
| [API Reference](https://sochdb.github.io/agentreplay/docs/api-reference/) | REST API documentation |
| [Architecture](https://sochdb.github.io/agentreplay/docs/architecture/) | System design overview |
| [Evaluation Framework](https://sochdb.github.io/agentreplay/docs/evaluation/) | 20+ built-in evaluators |

### In-Repo Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) - System design and crate dependencies
- [CONTRIBUTING.md](CONTRIBUTING.md) - Contribution guidelines
- [SECURITY.md](SECURITY.md) - Security policy
- [ADRs](docs/adr/) - Architecture Decision Records
- [Python SDK README](sdks/python/README.md) - Python SDK details

---

## 🎯 Use Cases

### 1. LLM Agent Observability
Track multi-step agent reasoning with automatic parent-child relationship capture:
```python
with client.trace(SpanType.ROOT) as root:
    with root.child(SpanType.PLANNING) as plan:
        # Agent planning step
        pass
    with root.child(SpanType.TOOL_CALL) as tool:
        # Tool execution
        pass
```

### 2. Cost Tracking & Budget Management
Monitor LLM costs in real-time with budget alerts:
```bash
# Create budget alert
curl -X POST http://localhost:8080/api/v1/budget/alerts \
  -d '{"threshold_type": "daily_cost", "threshold_value": 100.0}'
```

### 3. A/B Testing Prompts
Compare prompt variants with statistical analysis:
```bash
# Create experiment
curl -X POST http://localhost:8080/api/v1/experiments \
  -d '{
    "name": "Temperature Test",
    "variants": [
      {"name": "temp_0.7", "config": {"temperature": 0.7}},
      {"name": "temp_0.9", "config": {"temperature": 0.9}}
    ]
  }'
```

### 4. Quality Monitoring
Automated evaluation with hallucination detection:
```python
from agentreplay_evals import HallucinationDetector

detector = HallucinationDetector()
result = await detector.evaluate(trace)
# Returns: {score: 0.02, passed: true, confidence: 0.92}
```

### 5. Performance Profiling
Identify slow agent steps with latency benchmarking:
```python
# Get latency statistics for an agent
stats = client.get_latency_stats(agent_id="my-agent")
# Returns: {p50: 120ms, p95: 450ms, p99: 890ms}
```

### 6. Framework Integration
Zero-code integration with popular frameworks:
```python
# LangChain
callback = Agent ReplayCallbackHandler(url="http://localhost:8080")
chain = LLMChain(llm=llm, callbacks=[callback])

# LlamaIndex
callback_manager = create_callback_manager(agentreplay_url="...")
index = VectorStoreIndex.from_documents(docs, callback_manager=callback_manager)

# OpenAI Agents
agent = Agent ReplayAgentWrapper(agent=openai_agent, agentreplay_url="...")
```

---

## 🔧 Configuration

### Server Configuration (`agentreplay-server-config.toml`)

```toml
[server]
host = "0.0.0.0"
port = 8080
workers = 8

[database]
path = "./data/agentreplay.db"
max_memtable_size = 67108864  # 64 MB
enable_wal = true
enable_compression = true
compression_algorithm = "lz4"  # or "zstd"

[storage]
l0_compaction_trigger = 4
max_levels = 7
target_file_size_base = 67108864  # 64 MB
bloom_filter_bits_per_key = 10

[auth]
require_auth = true
api_keys_file = "./data/api_keys.json"

[observability]
enable_metrics = true
metrics_port = 9090
enable_tracing = true
tracing_endpoint = "localhost:47117"
```

### Environment Variables

```bash
# Server
export AGENTREPLAY_HOST="0.0.0.0"
export AGENTREPLAY_PORT=8080
export AGENTREPLAY_DB_PATH="./data/agentreplay.db"

# Authentication
export AGENTREPLAY_REQUIRE_AUTH=true
export AGENTREPLAY_API_KEY="your-api-key"

# Observability
export AGENTREPLAY_ENABLE_METRICS=true
export OTEL_EXPORTER_OTLP_ENDPOINT="localhost:47117"
```

---

## 🧪 Development

### Building from Source

**Prerequisites:**
- Rust 1.70+ with cargo
- (Optional) Node.js 18+ for UI/Desktop app

```bash
# Clone repository
git clone https://github.com/sochdb/agentreplay.git
cd agentreplay

# Build all components
cargo build --release

# Build specific component
cargo build --release -p agentreplay-server
cargo build --release -p agentreplay-cli

# Build desktop app
cd ui && npm install && npm run tauri build
```

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for specific crate
cargo test -p agentreplay-storage

# Run benchmarks
cargo bench -p agentreplay-storage
```

### Project Structure

```
agentreplay/
├── agentreplay-core/          # Core data structures
├── agentreplay-storage/       # LSM-tree storage engine
├── agentreplay-index/         # Indexing layer
├── agentreplay-query/         # Query engine
├── agentreplay-server/        # HTTP API server
├── agentreplay-cli/           # Command-line interface
├── agentreplay-observability/ # O11y integrations
├── agentreplay-evals/         # Evaluation framework
├── agentreplay-plugins/       # Plugin system
│   ├── core/                # Plugin runtime
│   ├── sdk/                 # Plugin development SDKs
│   ├── examples/            # Example plugins
│   └── templates/           # Plugin templates
├── agentreplay-ui/            # Web UI (React)
├── agentreplay-tauri/         # Tauri desktop app
├── sdks/
│   └── python/              # Python SDK
├── examples/                # Example code
└── docs/                    # Documentation
```

---

## 💾 Backup & Restore

Agent Replay includes comprehensive backup and restore features to protect your data. Both the Desktop App and CLI support full backup operations.

### Desktop App Backup

Navigate to **Settings → Backup** in the desktop application to:

- **Create Backups**: Timestamped snapshots of your database
- **List Backups**: View all available backups with size and date
- **Export as ZIP**: Download backups for external storage or sharing
- **Import from ZIP**: Restore from previously exported backups
- **Restore Options**:
  - **Replace (Full Restore)**: Replace all data with backup (creates pre-restore backup automatically)
  - **Merge (Append)**: Experimental - adds backup traces to existing data
    - ⚠️ Warning: Project associations may not be preserved

### CLI Backup Commands

The CLI provides the same backup functionality for automation and scripting:

```bash
# List all backups
agentreplay --db-path <path> backup list

# Create a new backup
agentreplay --db-path <path> backup create [--name <name>]

# Restore from a backup (creates pre-restore backup automatically)
agentreplay --db-path <path> backup restore <backup_id> [-y]

# Delete a backup
agentreplay --db-path <path> backup delete <backup_id> [-y]

# Export backup as ZIP file
agentreplay --db-path <path> backup export <backup_id> [-o <output.zip>]

# Import backup from ZIP file
agentreplay --db-path <path> backup import <path.zip>
```

**Example Workflow:**

```bash
# Create a backup before making changes
agentreplay --db-path ./agentreplay-data backup create --name "before-experiment"

# List all backups
agentreplay --db-path ./agentreplay-data backup list
# Output:
# Backups (3):
# ============================================================
#   before-experiment - 2026-01-26 12:00:00 (120611 bytes)
#   backup_1769456740 - 2026-01-26 11:45:40 (120611 bytes)
#   backup_1769455267 - 2026-01-26 11:21:07 (120432 bytes)

# Export backup for archival or sharing
agentreplay --db-path ./agentreplay-data backup export before-experiment \
  -o ~/backups/agentreplay_backup_20260126.zip

# Restore from backup (with confirmation prompt)
agentreplay --db-path ./agentreplay-data backup restore before-experiment

# Or skip confirmation in scripts
agentreplay --db-path ./agentreplay-data backup restore before-experiment -y
```

**JSON Output for Automation:**

```bash
# Get machine-readable JSON output
agentreplay --db-path ./agentreplay-data --json backup list
# Output:
# {"backups": [{"backup_id": "before-experiment", "created_at": 1737889200, ...}], "total": 3}
```

**Backup Storage Location:**
- Backups are stored in `<db-path>/../backups/`
- Each backup is a complete copy of the database directory
- Pre-restore backups are automatically created with `pre_restore_<timestamp>` naming

**Best Practices:**
- Create backups before major updates or experiments
- Export important backups to external storage
- Use descriptive names for manual backups
- Regularly clean up old pre-restore backups
- Test restore procedures periodically

---

## 🤝 Contributing

We welcome contributions! Here's how to get started:

### Quick Start

```bash
git clone https://github.com/sochdb/agentreplay.git
cd agentreplay
cargo build --workspace
cargo test --workspace
```

### Contributor Resources

| Resource | Description |
|----------|-------------|
| **[CONTRIBUTING.md](CONTRIBUTING.md)** | Contribution guidelines, PR process |
| **[ARCHITECTURE.md](ARCHITECTURE.md)** | System design, crate dependencies |
| **[Developer Guide](docs-site/docs/developer-guide.md)** | Codebase tour, testing, common gotchas |
| **[ADRs](docs/adr/)** | Architecture Decision Records |
| **[SECURITY.md](SECURITY.md)** | Vulnerability reporting |

### Finding Work

| You are... | Start with... |
|-----------|---------------|
| New to the project | [Good First Issues](https://github.com/sochdb/agentreplay/labels/good%20first%20issue) |
| Experienced in Rust | [Help Wanted](https://github.com/sochdb/agentreplay/labels/help%20wanted) |
| Interested in storage | `agentreplay-storage` crate |
| Interested in ML/vectors | `agentreplay-index` crate |
| Interested in SDKs | `sdks/` directory |

### Code Style

- **Rust**: `cargo fmt --all && cargo clippy --workspace -- -D warnings`
- **Python**: Black + isort + mypy
- **TypeScript**: ESLint + Prettier
- **Commits**: [Conventional Commits](https://www.conventionalcommits.org/) format

---

## � Related Projects

Agent Replay is part of the SochDB ecosystem:

| Project | Description | Link |
|---------|-------------|------|
| **SochDB** | High-performance embedded database for AI/ML workloads | [github.com/sochdb/sochdb](https://github.com/sochdb/sochdb) |
| **SochDB Python SDK** | Python client for SochDB | [github.com/sochdb/sochdb-python-sdk](https://github.com/sochdb/sochdb-python-sdk) |
| **SochDB Node.js SDK** | Node.js client for SochDB | [github.com/sochdb/sochdb-nodejs-sdk](https://github.com/sochdb/sochdb-nodejs-sdk) |
| **SochDB Go SDK** | Go client for SochDB | [github.com/sochdb/sochdb-go](https://github.com/sochdb/sochdb-go) |

---

## �📄 License

Agent Replay core (Rust) is licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)**. See [LICENSE](LICENSE) for details.

The SDKs (Python, Node.js, Rust SDK) are licensed under the **Apache License 2.0** to allow unrestricted integration in your applications.

### Third-Party Licenses

Agent Replay uses several open-source libraries. See [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) for details.

---

## 🙏 Acknowledgments

Agent Replay builds on research and insights from:

- **Observability**: OpenTelemetry
- **LLM Frameworks**: LangChain, LlamaIndex, Hugging Face, OpenAI

Special thanks to the open-source community and all contributors.

---

## 🔗 Links

- **GitHub**: [github.com/sochdb/agentreplay](https://github.com/sochdb/agentreplay)
- **Documentation**: [sochdb.github.io/agentreplay](https://sochdb.github.io/agentreplay)
- **Python SDK**: [PyPI](https://pypi.org/project/agentreplay/)
- **JavaScript SDK**: [npm](https://www.npmjs.com/package/agentreplay)
- **Rust SDK**: [crates.io](https://crates.io/crates/agentreplay)
- **Issues**: [Bug reports & feature requests](https://github.com/sochdb/agentreplay/issues)
- **Discussions**: [GitHub Discussions](https://github.com/sochdb/agentreplay/discussions)

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/sochdb/agentreplay/issues)
- **Discussions**: [GitHub Discussions](https://github.com/sochdb/agentreplay/discussions)
- **Email**: support@agentreplay.dev (coming soon)

---

<div align="center">

**⭐ Star us on GitHub if you find Agent Replay useful! ⭐**

Made with ❤️ by the Agent Replay team

</div>
