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

//! Local/offline evaluation support
//!
//! This module provides infrastructure for evaluations that don't require external APIs.
//! Currently in early stages of implementation.

use crate::llm_client::{EmbedError, EmbeddingClient};
use async_trait::async_trait;

/// Local embedding client using simple deterministic approach for testing/offline use.
/// In the future, this will integrate with ONNX Runtime for local model inference.
pub struct LocalEmbeddingClient {
    // Placeholder for ONNX session
    // session: ort::Session
}

impl LocalEmbeddingClient {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for LocalEmbeddingClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EmbeddingClient for LocalEmbeddingClient {
    async fn embed(&self, text: &str) -> Result<Vec<f64>, EmbedError> {
        // For now, return a deterministic "embedding" based on character counts
        // This is just a stub to allow offline testing of the pipeline
        // Real implementation would run a local model like all-MiniLM-L6-v2 via ONNX
        let len = text.len() as f64;
        let mut vec = vec![0.0; 384]; // Standard size for small models

        // Fill with some deterministic values based on input
        for (i, c) in text.chars().enumerate().take(384) {
            vec[i] = (c as u32 % 100) as f64 / 100.0;
        }
        vec[0] = len / 1000.0; // Normalize length feature

        Ok(vec)
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f64>>, EmbedError> {
        let mut results = Vec::new();
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
}
