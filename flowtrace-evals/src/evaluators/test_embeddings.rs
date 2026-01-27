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

#[cfg(test)]
mod tests {
    use crate::evaluators::relevance::RelevanceEvaluator;
    use crate::llm_client::{EmbedError, EmbeddingClient};
    use crate::{Evaluator, MetricValue, TraceContext};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;

    struct MockEmbeddingClient {
        embeddings: HashMap<String, Vec<f64>>,
    }

    impl MockEmbeddingClient {
        fn new() -> Self {
            let mut embeddings = HashMap::new();
            // "king"
            embeddings.insert("king".to_string(), vec![1.0, 0.0]);
            // "queen" - similar to king
            embeddings.insert("queen".to_string(), vec![0.9, 0.1]);
            // "apple" - dissimilar
            embeddings.insert("apple".to_string(), vec![0.0, 1.0]);
            Self { embeddings }
        }
    }

    #[async_trait]
    impl EmbeddingClient for MockEmbeddingClient {
        async fn embed(&self, text: &str) -> Result<Vec<f64>, EmbedError> {
            self.embeddings
                .get(text)
                .cloned()
                .ok_or_else(|| EmbedError::ApiError("Text not found in mock".to_string()))
        }

        async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f64>>, EmbedError> {
            let mut results = Vec::new();
            for text in texts {
                results.push(self.embed(text).await?);
            }
            Ok(results)
        }
    }

    #[tokio::test]
    async fn test_semantic_relevance() {
        let client = Arc::new(MockEmbeddingClient::new());
        let evaluator = RelevanceEvaluator::new().with_embedding_client(client);

        // High relevance
        let trace_high = TraceContext {
            trace_id: 1,
            edges: vec![],
            input: Some("king".to_string()),
            output: Some("queen".to_string()),
            context: None,
            metadata: HashMap::new(),
            eval_trace: None,
            timestamp_us: 0,
        };

        let result_high = evaluator.evaluate(&trace_high).await.unwrap();
        assert!(result_high.passed);
        if let Some(MetricValue::Float(score)) = result_high.metrics.get("relevance_score") {
            assert!(*score > 0.8);
        } else {
            panic!("Missing score");
        }

        // Low relevance
        let trace_low = TraceContext {
            trace_id: 2,
            edges: vec![],
            input: Some("king".to_string()),
            output: Some("apple".to_string()),
            context: None,
            metadata: HashMap::new(),
            eval_trace: None,
            timestamp_us: 0,
        };

        let result_low = evaluator.evaluate(&trace_low).await.unwrap();
        assert!(!result_low.passed);
        if let Some(MetricValue::Float(score)) = result_low.metrics.get("relevance_score") {
            assert!(*score < 0.2);
        } else {
            panic!("Missing score");
        }
    }
}
