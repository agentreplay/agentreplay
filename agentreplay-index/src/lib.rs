// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Agentreplay Index Layer
//!
//! Indexing structures for causal graph traversal and semantic search.
//!
//! ## Vector Indices
//!
//! This module provides two vector index implementations:
//!
//! - **HNSW** (re-exported from sochdb-index): Hierarchical Navigable Small World graphs
//!   for ANN search. Good for general purpose vector search with low latency.
//!
//! - **Vamana** (`vamana`): DiskANN-style single-layer graph with Product Quantization.
//!   Optimized for massive scale (10M+ vectors) with 32x memory reduction.
//!
//! ## Embedding Module
//!
//! The `embedding` module provides a complete embedding pipeline:
//!
//! - **Providers**: Local ONNX inference (offline) and OpenAI API integration
//! - **Pipeline**: Batched processing with background workers
//! - **Storage**: Persistent embedding storage with PQ compression
//! - **Normalization**: SIMD-optimized L2 normalization
//!
//! ## Product Quantization
//!
//! Re-exported from sochdb-index for 32x compression for embeddings:
//! - 384-dim vector (1536 bytes as f32) → 48 bytes as PQ codes
//! - 10M vectors: 15 GB → 480 MB

// =============================================================================
// Agentreplay-specific modules (not in sochdb-index)
// =============================================================================

pub mod causal;
pub mod compression;
pub mod concept;
pub mod concept_index;
pub mod embedding;
pub mod metrics;
pub mod vamana;
pub mod vector;
pub mod vector_hnsw;

// Re-export agentreplay-specific types
pub use causal::{CausalIndex, CausalStats};
pub use compression::{CompressionLevel, QuantizedVectorI8, StoredVector};
pub use concept::{ConceptEntry, ConceptExtractionConfig, ConceptExtractor, ConceptIndex, ConceptQuery, ExtractedConcept};
pub use concept_index::{ConceptIndexError, ConceptIndexStore};
pub use embedding::{
    EmbeddingError, EmbeddingIntegration, EmbeddingPipeline, EmbeddingProvider, EmbeddingRegistry,
    EmbeddingRequest, EmbeddingStorage, EmbeddingStorageConfig, IntegrationConfig,
    IntegrationError, LocalEmbeddingConfig, LocalEmbeddingProvider, MockEmbeddingProvider,
    PipelineConfig, SemanticSearchResult,
};
pub use vamana::{VamanaConfig, VamanaIndex, VamanaStats};
pub use vector::{DistanceMetric, Embedding, VectorIndex};

// =============================================================================
// Re-exports from sochdb-index (eliminates ~3200 LOC of duplicated code)
// =============================================================================

// HNSW core types (replaces local hnsw.rs - 1104 LOC)
pub use sochdb_index::hnsw::{
    HnswConfig, HnswIndex, HnswStats, MemoryStats, HnswNode,
    DistanceMetric as HnswDistanceMetric, AdaptiveSearchConfig, RngOptimizationConfig,
};

// HNSW modules for backwards compatibility
pub mod hnsw {
    //! Re-export of sochdb-index HNSW module for backwards compatibility
    pub use sochdb_index::hnsw::*;
}

// Persistence (replaces local persistence.rs - 362 LOC)
pub mod persistence {
    //! Re-export of sochdb-index persistence module
    //! HnswIndex has save_to_disk/load_from_disk methods
    pub use sochdb_index::persistence::*;
}

// Product Quantization (replaces local product_quantization.rs - 636 LOC)
pub use sochdb_index::product_quantization::{DistanceTable, PQCodebooks, PQCodes};
pub mod product_quantization {
    //! Re-export of sochdb-index product quantization module
    pub use sochdb_index::product_quantization::*;
}

// HNSW with PQ (replaces local hnsw_pq.rs - 453 LOC)
pub use sochdb_index::hnsw_pq::{ADCTable, PQSearchConfig, PQSearchResult, PQVectorStore};
pub mod hnsw_pq {
    //! Re-export of sochdb-index HNSW PQ module
    pub use sochdb_index::hnsw_pq::*;
}

// Vector types (replaces local vector_quantized.rs - 251 LOC)
pub use sochdb_index::vector_quantized::{Precision, QuantizedVector};
pub mod vector_quantized {
    //! Re-export of sochdb-index vector quantization module
    pub use sochdb_index::vector_quantized::*;
}

// Vector SIMD (replaces local vector_simd.rs - 474 LOC)
pub mod vector_simd {
    //! Re-export of sochdb-index SIMD distance module
    pub use sochdb_index::simd_distance::*;
}

// Vector storage (replaces local vector_storage.rs - 306 LOC)
pub use sochdb_index::vector_storage::{MemoryVectorStorage, MmapVectorStorage, VectorStorage};
pub mod vector_storage {
    //! Re-export of sochdb-index vector storage module
    pub use sochdb_index::vector_storage::*;
}

// Advanced HNSW features from sochdb-index
pub use sochdb_index::{
    // Lock-free entry point with packed atomic CAS
    AtomicNavigationState, AtomicNavigationStateU128,
    // CSR graph for cache-efficient traversal
    CsrGraph, CsrGraphBuilder, CsrLayer, CsrGraphStats, InternalSearchCandidate,
    // Dense ID mapping for O(1) lookup
    IdMapper, InternalId, VisitedBitmap,
    // Staged parallel construction with waves
    StagedBuilder, StagedConfig, StagedStats,
    // Hot buffer for ultra-fast inserts
    HotBufferHnsw, HotBufferConfig, HotBufferStats,
    // Buffered HNSW with delta buffer
    BufferedHnsw, BufferedHnswConfig, BufferedHnswStats, BufferStats,
    // Unified quantization pipeline
    QuantLevel, UnifiedQuantizedVector, QuantPipelineConfig, PipelineStage, 
    StageCandidates, UnifiedScorer,
    // Node ordering for cache locality
    NodeOrderer, NodePermutation, OrderingStats, OrderingStrategy,
    // AoSoA tiles for SIMD
    VectorTile, TiledVectorStore, TiledStoreStats,
};
