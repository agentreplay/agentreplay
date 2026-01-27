# Flowtrace Architecture

> This document provides a high-level overview of Flowtrace's architecture for contributors and maintainers.

## Table of Contents

- [System Overview](#system-overview)
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
│ flowtrace-storage::LSMTree                            │
│   1. Append to WAL (durability) ─────► fsync          │
│   2. Insert into Memtable (skip list)                 │
│   3. Update block cache                               │
│   4. Maybe trigger flush if memtable full             │
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
    ├─────────────────────┬───────────────────────┐
    ▼                     ▼                       ▼
┌─────────────┐   ┌─────────────────┐   ┌─────────────────┐
│ BlockCache  │   │ Memtable        │   │ SSTables        │
│ (L1 cache)  │   │ (in-memory)     │   │ (L0 → L6)       │
└─────────────┘   └─────────────────┘   └─────────────────┘
    │                     │                       │
    └─────────────────────┴───────────────────────┘
                          │
                          ▼
                    Merge & Filter
                          │
                          ▼
              ┌─────────────────────┐
              │ Optional: Causal    │
              │ graph traversal     │
              └─────────────────────┘
                          │
                          ▼
              ┌─────────────────────┐
              │ Optional: Vector    │
              │ similarity search   │
              └─────────────────────┘
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

### 1. Fixed 128-byte Edge Format

**Decision**: `AgentFlowEdge` is exactly 128 bytes, cache-line aligned.

**Why**:
- Fits in 2 cache lines → predictable memory access patterns
- Enables SIMD processing of edge arrays
- Zero-copy serialization possible
- Large payloads stored separately (edge just has `has_payload` flag)

**Trade-off**: Rigid format, but 128 bytes covers 99%+ of use cases. Extensions go in payload store.

### 2. LSM-Tree over B-Tree

**Decision**: Use LSM-tree (Log-Structured Merge-tree) for storage.

**Why**:
- Agents produce write-heavy workloads (10:1 write:read ratio typical)
- LSM writes are sequential → 10-100x faster than B-tree random writes
- Compaction amortizes write cost
- Leveled structure enables tiered compression

**Trade-off**: Read amplification (check multiple levels), but mitigated by bloom filters and block cache.

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
| See how writes are persisted | `flowtrace-storage/src/lsm.rs` |
| Understand WAL implementation | `flowtrace-storage/src/wal_*.rs` |
| See SSTable format | `flowtrace-storage/src/sstable.rs` |
| Understand compaction | `flowtrace-storage/src/compaction/` |
| See causal graph traversal | `flowtrace-index/src/causal.rs` |
| Understand vector search | `flowtrace-index/src/hnsw.rs` |
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
