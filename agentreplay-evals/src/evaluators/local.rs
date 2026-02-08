// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

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
