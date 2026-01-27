# Flowtrace Architecture

> This document provides a high-level overview of Flowtrace's architecture for contributors and maintainers.

## Table of Contents

- [System Overview](#system-overview)
- [SochDB Storage Backend](#sochdb-storage-backend)
- [Crate Dependency Graph](#crate-dependency-graph)
- [Data Flow](#data-flow)
- [Key Design Decisions](#key-design-decisions)
- [Module Guide](#module-guide)
- [Performance Characteristics](#performance-characteristics)
- [Security Model](#security-model)

**See Also:**
- [Unified Plugin Architecture](./docs/UNIFIED_PLUGIN_ARCHITECTURE.md) - Bundle plugins, memory system, Claude/Cursor integrations

---

## System Overview

Flowtrace is a purpose-built observability platform for LLM agents. It's designed around four core principles:

1. **Write-optimized**: Agents generate massive trace volumes; we optimize for ingestion
2. **Causality-aware**: Traces form DAGs, not just flat logs; we preserve relationships
3. **Evaluation-native**: Testing and validation are first-class, not afterthoughts
4. **Memory-integrated**: Persistent context across sessions with semantic retrieval

**Powered by SochDB**: Flowtrace uses [SochDB](https://github.com/sochdb/sochdb) as its storage backend - a high-performance embedded database designed for AI/ML workloads.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Client SDKs                                     │
│         (Python, JavaScript, Rust, Go)                                      │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         flowtrace-server                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  REST API   │  │  WebSocket  │  │    Auth     │  │   Rate Limiting     │ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          flowtrace-query                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │Query Engine │  │Aggregations │  │  Semantic   │  │ Cost Calculator     │ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
┌───────────────────────┐ ┌─────────────────┐ ┌─────────────────────────────┐
│   flowtrace-storage   │ │ flowtrace-index │ │      flowtrace-evals        │
│  ┌─────────────────┐  │ │ ┌─────────────┐ │ │  ┌──────────────────────┐   │
│  │    LSM-Tree     │  │ │ │   Causal    │ │ │  │   20+ Evaluators     │   │
│  │  ┌───────────┐  │  │ │ │    Index    │ │ │  │   (Hallucination,    │   │
│  │  │    WAL    │  │  │ │ └─────────────┘ │ │  │    Relevance, etc)   │   │
│  │  ├───────────┤  │  │ │ ┌─────────────┐ │ │  └──────────────────────┘   │
│  │  │ Memtable  │  │  │ │ │    HNSW     │ │ │  ┌──────────────────────┐   │
│  │  ├───────────┤  │  │ │ │   Vector    │ │ │  │   LLM-as-Judge       │   │
│  │  │ SSTables  │  │  │ │ └─────────────┘ │ │  └──────────────────────┘   │
│  │  └───────────┘  │  │ │ ┌─────────────┐ │ └─────────────────────────────┘
│  └─────────────────┘  │ │ │   Vamana    │ │
└───────────────────────┘ │ │  (DiskANN)  │ │
                          │ └─────────────┘ │
                          └─────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          flowtrace-core                                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │ AgentFlowEdge   │  │ Hybrid Logical  │  │    SpanType, Error,         │  │
│  │  (128 bytes)    │  │     Clock       │  │    Sensitivity Flags        │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Crate Dependency Graph

```
flowtrace-core (foundation, no dependencies on other flowtrace crates)
       │
       ├──────────────────────────────────────────────┬───────────────────┐
       │                                              │                   │
       ▼                                              ▼                   ▼
flowtrace-storage                              flowtrace-evals    flowtrace-memory
       │                                              │                   │
       ▼                                              │                   │
flowtrace-index ◄─────────────────────────────────────┴───────────────────┘
       │
       ▼
flowtrace-query
       │
       ▼
flowtrace-server
       │
       ├───────────────────────────────────────┐
       ▼                                       ▼
flowtrace-plugins                     flowtrace-tauri (Desktop)
  │
  ├── WASM runtime
  ├── Bundle installer
  └── Capability enforcer
```

**Key insight**: Dependencies flow downward. Lower crates are more stable and change less frequently. When contributing, changes to `flowtrace-core` have the widest impact.

**New in v2**: `flowtrace-memory` provides persistent agent memory, and `flowtrace-plugins` supports both WASM runtime plugins and file-based bundle plugins for Claude Code/Cursor integrations.

---

## SochDB Storage Backend

Flowtrace's storage layer is built entirely on **SochDB** - a high-performance embedded database from the same team. This provides Flowtrace with enterprise-grade storage capabilities without reinventing the wheel.

### Why SochDB?

| Feature | Benefit for Flowtrace |
|---------|----------------------|
| **LSM-tree architecture** | Write-optimized for high-throughput trace ingestion |
| **ACID transactions** | Durability guarantees for critical observability data |
| **Columnar storage (PackedRow)** | 80%+ I/O reduction on analytics queries |
| **Native vector indexes** | HNSW and Vamana for semantic search |
| **Embedded design** | No external dependencies for desktop app |
| **Crash recovery (WAL)** | Zero data loss on unexpected shutdown |

### Storage Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    FlowTraceStorage                             │
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

### Key Encoding

Flowtrace uses hierarchical key encoding for efficient range scans:

| Store | Key Format | Example |
|-------|------------|---------|
| **Traces** | `traces/{tenant}/{project}/{timestamp:020}/{edge:032x}` | `traces/1/42/00000001704067200000000/0a1b2c...` |
| **Payloads** | `payloads/{edge:032x}` | `payloads/0a1b2c3d4e5f...` |
| **Metrics** | `metrics/{granularity}/{tenant}/{project}/{timestamp:020}` | `metrics/hour/1/42/00000001704067200000000` |
| **Graph** | `graph/{direction}/{node:032x}/{related:032x}` | `graph/children/0a1b2c.../0d4e5f...` |

### Columnar Edge Storage

SochDB's `PackedRow` format enables columnar projection - queries that only need specific fields read only those columns:

```rust
// Column schema for edge storage
| Column        | Type   | Size  | Description              |
|---------------|--------|-------|--------------------------|
| edge_id       | Binary | 16    | Unique edge identifier   |
| tenant_id     | UInt   | 8     | Tenant identifier        |
| project_id    | UInt   | 2     | Project identifier       |
| timestamp_us  | UInt   | 8     | Event timestamp (micros) |
| session_id    | UInt   | 8     | Session identifier       |
| agent_id      | UInt   | 8     | Agent identifier         |
| span_type     | UInt   | 4     | Type of span             |
| duration_us   | UInt   | 4     | Duration in microseconds |
| token_count   | UInt   | 4     | Token count              |
| has_payload   | Bool   | 1     | Payload flag             |
```

**Result**: Analytics queries that only need `timestamp_us + duration_us` achieve **80%+ I/O reduction** compared to reading full edges.

### SochDB Components Used

```
flowtrace-storage
    │
    ├── sochdb (EmbeddedConnection)
    │   └── Primary database connection
    │
    ├── sochdb-storage (PackedRow, PackedTableSchema)
    │   └── Columnar storage for 80% I/O reduction
    │
    └── sochdb-core (SochValue)
        └── Type system for column values
```

---

## Data Flow

### Write Path (Hot Path - Optimized for Speed)

```
Client SDK
    │
    ▼ HTTP POST /api/v1/traces
┌───────────────────────────────────────────────────────┐
│ flowtrace-server                                      │
│   1. Validate input                                   │
│   2. Parse into AgentFlowEdge (128 bytes, fixed)     │
│   3. Assign tenant_id, project_id                     │
└───────────────────────────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────────────────────────┐
│ flowtrace-storage (SochDB Backend)                    │
│   1. Encode key: traces/{tenant}/{project}/{ts}/{id} │
│   2. Convert edge to PackedRow (columnar)            │
│   3. Write to SochDB (WAL + Memtable)                │
│   4. SochDB handles durability & compaction          │
└───────────────────────────────────────────────────────┘
    │
    ├──────────────────────────────┐
    ▼                              ▼
┌─────────────────────┐   ┌─────────────────────────────┐
│ flowtrace-index     │   │ flowtrace-storage::Payload  │
│ ::CausalIndex       │   │   (if has_payload=true)     │
│   - parent→children │   │   Store prompt/response     │
│   - child→parents   │   └─────────────────────────────┘
└─────────────────────┘
```

**Performance target**: <1ms p99 latency for single writes, 100K+ edges/sec batch throughput.

### Read Path (Query)

```
Client Query
    │
    ▼
┌───────────────────────────────────────────────────────┐
│ flowtrace-query::QueryEngine                          │
│   1. Parse query (time range, filters, semantic)      │
│   2. Plan execution (index selection, parallelism)    │
└───────────────────────────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────────────────────────┐
│ flowtrace-storage (SochDB Backend)                    │
│   1. Build scan prefix from query filters             │
│   2. SochDB range scan with columnar projection      │
│   3. Deserialize only needed columns (80% I/O saved) │
└───────────────────────────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────────────────────────┐
│ Post-Processing                                       │
│   1. Optional: Causal graph traversal                 │
│   2. Optional: Vector similarity search               │
│   3. Aggregation & filtering                          │
└───────────────────────────────────────────────────────┘
    │
    ▼
Return Results
```


```
┌─────────────────────────────────────────────────────────────┐
│                       FLOWTRACE                             │
│  ┌─────────────────┐       ┌─────────────────┐              │
│  │ flowtrace-index │       │ flowtrace-query │              │
│  │  - CausalIndex  │       │  - NL Parser    │              │
│  │  - ConceptIndex │       │  - SemanticSearch│             │
│  │  - Trace-aware  │       │  - QueryFilters │              │
│  │    embeddings   │       │  - Cost Engine  │              │
│  └────────┬────────┘       └────────┬────────┘              │
│           │ USES                    │ USES                  │
│  ┌────────▼────────────────────────▼────────┐               │
│  │              SOCHDB (Backend)            │               │
│  │  ┌─────────────┐  ┌─────────────┐       │               │
│  │  │ sochdb-index│  │ sochdb-query│       │               │
│  │  │  - HNSW     │  │  - SOCH-QL  │       │               │
│  │  │  - Vamana   │  │  - Filter IR│       │               │
│  │  │  - BM25     │  │  - Hybrid   │       │               │
│  │  └─────────────┘  └─────────────┘       │               │
│  │           ┌─────────────┐                │               │
│  │           │sochdb-storage│               │               │
│  │           │  - LSM/WAL   │               │               │
│  │           └─────────────┘                │               │
│  └──────────────────────────────────────────┘               │
└─────────────────────────────────────────────────────────────┘

```
---

## Key Design Decisions

### EvalTraceV1 Canonical Transcript

Flowtrace exposes a **versioned, canonical transcript** for eval workflows: `EvalTraceV1`.
It models the trace as an append-only event log with:

- `transcript`: ordered events (messages, tool calls/results, span start/end)
- `outcome.messages`: the full messages array (Anthropic-style expectation)
- `spans`: lightweight span summaries for reconstruction
- `stats`: tokens, cost, and latency

This provides a stable contract for evaluators, exports, and replay.

### Multi-Trial Evaluation Runs

Eval runs support multiple trials per test case with:

- `trial_id` + `seed` per trial
- `TaskAggregate` summaries (pass rate, pass@k, latency percentiles)
- Wilson confidence intervals for binomial pass rates

### Schema Versioning

All major artifacts include `schema_version` for forward evolution:

- `EvalTraceV1` for transcript/outcome
- `EvalRun` for run storage and exports

Read-time defaults provide backward compatibility for stored runs.

These are the most important architectural decisions. See `/docs/adr/` for full ADRs.

### 1. SochDB as Storage Backend

**Decision**: Use SochDB as the unified storage backend instead of a custom LSM-tree.

**Why**:
- SochDB provides ACID transactions, WAL, and crash recovery out of the box
- Columnar storage (PackedRow) enables 80%+ I/O reduction for analytics
- Native vector indexes (HNSW, Vamana) for semantic search
- Same team maintains both projects → tight integration
- Embedded design perfect for offline-first desktop app

**Trade-off**: External dependency, but SochDB is designed specifically for this use case.

### 2. Fixed 128-byte Edge Format

**Decision**: `AgentFlowEdge` is exactly 128 bytes, cache-line aligned.

**Why**:
- Fits in 2 cache lines → predictable memory access patterns
- Enables SIMD processing of edge arrays
- Zero-copy serialization possible
- Large payloads stored separately (edge just has `has_payload` flag)

**Trade-off**: Rigid format, but 128 bytes covers 99%+ of use cases. Extensions go in payload store.

### 3. Columnar Edge Storage

**Decision**: Store edges in SochDB's columnar PackedRow format.

**Why**:
- Analytics queries often need only 2-3 columns (timestamp, duration, etc.)
- Reading only needed columns saves 80%+ I/O
- SochDB's PackedRow enables projection pushdown

**Trade-off**: Slightly more complex serialization, but massive performance win.

### 3. Hybrid Logical Clock (HLC)

**Decision**: Use HLC instead of wall-clock or Lamport clocks.

**Why**:
- Wall clocks drift, causing ordering issues in distributed traces
- Pure Lamport clocks lose wall-time information
- HLC = max(local_time, incoming_time) + logical_counter
- Preserves causality AND approximate real time

**Trade-off**: Slightly more complex than wall clock, but critical for distributed correctness.

### 4. Causal Index with SmallVec

**Decision**: Use `SmallVec<[u128; 8]>` for child lists in causal index.

**Why**:
- 99.9% of nodes have ≤8 children
- SmallVec stores inline (no heap allocation) when ≤8 elements
- Bounded at 10,000 to prevent OOM from degenerate graphs

**Trade-off**: Capped fan-out, but prevents resource exhaustion.

### 5. HNSW + Vamana for Vector Search

**Decision**: Dual vector index support.

**Why**:
- HNSW: Fast, low memory, great for <1M vectors
- Vamana + PQ: 32x memory reduction, scales to 100M+ vectors
- Different use cases need different trade-offs

**Trade-off**: Code complexity of supporting both, but necessary for scale range.

---

## Module Guide

### Where to Find Things

| You want to... | Look in... |
|----------------|------------|
| Understand the core data model | `flowtrace-core/src/edge.rs` |
| See SochDB integration | `flowtrace-storage/src/sochdb_unified.rs` |
| Understand key encoding | `flowtrace-storage/src/sochdb_unified.rs` (key functions) |
| See columnar edge schema | `flowtrace-storage/src/sochdb_unified.rs` (`create_edge_schema`) |
| See payload storage | `flowtrace-storage/src/observation_store.rs` |
| See causal graph traversal | `flowtrace-index/src/causal.rs` |
| Understand vector search | `flowtrace-index/src/hnsw.rs`, `flowtrace-index/src/vamana.rs` |
| See query planning | `flowtrace-query/src/engine.rs` |
| Understand API routing | `flowtrace-server/src/api/` |
| See evaluation implementations | `flowtrace-evals/src/evaluators/` |
| Understand desktop app | `flowtrace-tauri/src/` |
| **Plugin manifest schema** | `flowtrace-plugins/core/src/manifest.rs` |
| **Bundle installation** | `flowtrace-plugins/core/src/bundle.rs` |
| **WASM plugin runtime** | `flowtrace-plugins/core/src/wasm/` |
| **Memory engine** | `flowtrace-memory/src/engine.rs` |
| **Observations & context** | `flowtrace-memory/src/observation.rs`, `context.rs` |

### Code Patterns Used

1. **Builder Pattern**: Used for configuration (e.g., `QueryBuilder`, `EvalBuilder`, `Observation::new()`)
2. **Type-State Pattern**: Compile-time state machine for protocols
3. **Interior Mutability**: `Arc<RwLock<T>>` for shared mutable state
4. **Async/Await**: Tokio runtime for all I/O operations
5. **Error Handling**: `thiserror` for error definitions, `anyhow` in applications
6. **JSON Merge Patch**: RFC 7396 semantics for bundle config merging

---

## Performance Characteristics

### Benchmarks (Target on M1 MacBook Pro)

| Operation | Target | Notes |
|-----------|--------|-------|
| Single write | <1ms p99 | With WAL fsync |
| Batch write (1K edges) | <10ms | GroupCommit WAL |
| Point lookup | <100μs | From cache |
| Range scan (1K results) | <5ms | With bloom filter |
| Vector search (10 results) | <10ms | HNSW, ef=50 |
| Causal traversal (100 nodes) | <1ms | DashMap lookup |

### Memory Usage Guidelines

| Component | Expected Usage |
|-----------|---------------|
| Memtable | 64-256 MB (configurable) |
| Block Cache | 256-512 MB (configurable) |
| Causal Index | ~128 bytes/edge |
| HNSW Index | ~1KB/vector (384 dims) |
| Vamana+PQ Index | ~50 bytes/vector |

---

## Security Model

### Tenant Isolation

- Each edge has `tenant_id` field
- All queries are tenant-scoped at the storage layer
- Cross-tenant queries are impossible by design

### Authentication

- API key authentication (header-based)
- Bearer token support for OAuth integration
- Per-tenant rate limiting

### Data Sensitivity

Each edge can be marked with sensitivity flags:
- `PII`: Personally identifiable information
- `Secret`: API keys, passwords
- `FinancialData`: Payment information
- `HealthData`: HIPAA-relevant data

These flags enable selective encryption, retention policies, and audit logging.

### Payload Encryption

- Large payloads (prompts/responses) can be encrypted at rest
- Per-tenant encryption keys supported
- Key rotation without re-encryption of data (envelope encryption)

---

## What's NOT in Scope

To stay focused, Flowtrace explicitly does NOT handle:

1. **Real-time streaming**: We batch writes, not stream
2. **Full-text search**: Use Elasticsearch/Typesense alongside
3. **Graph database queries**: Cypher/Gremlin; we do causal DAGs only
4. **ML model serving**: We trace calls to models, not serve them

---

## Further Reading

- [ADRs](./docs/adr/) - Architecture Decision Records
- [Storage Deep Dive](./docs-site/docs/storage-deep-dive.md)
- [Indexing Guide](./docs-site/docs/indexing-guide.md)
- [Contributing Guide](./CONTRIBUTING.md)
