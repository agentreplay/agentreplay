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

//! Embedding provider plugin interface
//!
//! Implement this trait to create an embedding provider plugin.

use crate::types::{Embedding, PluginMetadata};

/// Trait for embedding provider plugins
///
/// Embedding providers generate vector embeddings for text.
///
/// # Example
///
/// ```rust,ignore
/// use agentreplay_plugin_sdk::prelude::*;
///
/// #[derive(Default)]
/// struct SimpleEmbedder;
///
/// impl EmbeddingProvider for SimpleEmbedder {
///     fn embed(&self, text: &str) -> Result<Embedding, String> {
///         // Simple character-based embedding (for demonstration)
///         let mut embedding = vec![0.0f32; 128];
///         for (i, c) in text.chars().take(128).enumerate() {
///             embedding[i] = (c as u32 as f32) / 256.0;
///         }
///         Ok(embedding)
///     }
///     
///     fn dimension(&self) -> u32 {
///         128
///     }
///     
///     fn max_tokens(&self) -> u32 {
///         512
///     }
///     
///     fn get_metadata(&self) -> PluginMetadata {
///         PluginMetadata {
///             id: "simple-embedder".into(),
///             name: "Simple Embedder".into(),
///             version: "1.0.0".into(),
///             description: "A simple character-based embedder for testing".into(),
///             ..Default::default()
///         }
///     }
/// }
///
/// export_embedding_provider!(SimpleEmbedder);
/// ```
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding for a single text
    fn embed(&self, text: &str) -> Result<Embedding, String>;

    /// Generate embeddings for multiple texts (batch)
    ///
    /// Default implementation calls embed() for each text.
    /// Override for more efficient batch processing.
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Embedding>, String> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Get the dimension of the embedding vectors
    fn dimension(&self) -> u32;

    /// Get the maximum number of tokens supported
    fn max_tokens(&self) -> u32;

    /// Get plugin metadata
    fn get_metadata(&self) -> PluginMetadata;
}
