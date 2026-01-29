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

//! Hallucination detection using LLM-as-judge

#[cfg(test)]
use crate::llm_client::LLMError;
use crate::{
    llm_client::LLMClient, EvalError, EvalResult, Evaluator, EvaluatorMetadata, MetricValue,
    TraceContext,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimVerification {
    pub claim: String,
    pub status: ClaimStatus,
    pub evidence: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClaimStatus {
    Supported,    // Claim is supported by context
    Contradicted, // Claim contradicts context
    Unsupported,  // Claim has no support (but doesn't contradict)
    Unverifiable, // Claim cannot be verified against context
}

/// Hallucination detector using LLM-as-judge approach
///
/// Checks if the agent's output is grounded in the provided context.
/// Returns a hallucination score (0-1) where:
/// - 0.0 = fully grounded, no hallucinations
/// - 1.0 = completely hallucinated, not grounded in context
pub struct HallucinationDetector {
    llm_client: Arc<dyn LLMClient>,
    claim_extraction_prompt: String,
    verification_prompt: String,
    threshold: f64,
}

impl HallucinationDetector {
    /// Create a new hallucination detector with default prompt template
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            claim_extraction_prompt: Self::default_claim_extraction_prompt(),
            verification_prompt: Self::default_verification_prompt(),
            threshold: 0.3, // Fail if hallucination score > 0.3
        }
    }

    /// Create with custom prompt templates
    pub fn with_prompts(mut self, extraction: String, verification: String) -> Self {
        self.claim_extraction_prompt = extraction;
        self.verification_prompt = verification;
        self
    }

    /// Create with custom prompt template (Deprecated: use with_prompts)
    /// This is maintained for backward compatibility where possible, mapping to the verification prompt.
    #[deprecated(note = "Use with_prompts instead for granular control")]
    pub fn with_prompt_template(mut self, template: String) -> Self {
        self.verification_prompt = template;
        self
    }

    /// Set threshold for pass/fail (default: 0.3)
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    fn default_claim_extraction_prompt() -> String {
        r#"You are an expert fact-checker. Extract all factual claims from the provided text.
Break complex sentences into atomic claims.
Ignore opinions, questions, or greetings.

TEXT:
{text}

Respond in JSON format:
{
  "claims": ["claim 1", "claim 2", ...]
}"#
        .to_string()
    }

    fn default_verification_prompt() -> String {
        r#"You are an expert fact-checker. Verify the following claims against the provided context.

CONTEXT:
{context}

CLAIMS:
{claims}

For each claim, determine its status:
- Supported: The claim is explicitly supported by the context.
- Contradicted: The claim contradicts the context.
- Unsupported: The claim is not mentioned in the context.
- Unverifiable: The context is insufficient to verify the claim.

Respond in JSON format:
{
  "verifications": [
    {
      "claim": "claim text",
      "status": "Supported" | "Contradicted" | "Unsupported" | "Unverifiable",
      "evidence": "quote from context supporting the verdict",
      "confidence": <float 0-1>
    },
    ...
  ]
}"#
        .to_string()
    }

    /// Extract claims from text
    async fn extract_claims(&self, text: &str) -> Result<(Vec<String>, f64), EvalError> {
        let prompt = self.claim_extraction_prompt.replace("{text}", text);
        let response = self
            .llm_client
            .evaluate(prompt)
            .await
            .map_err(|e| EvalError::LLMClientError(e.to_string()))?;

        // Calculate cost
        let (input_cost, output_cost) = self.llm_client.cost_per_token();
        let cost = response.usage.calculate_cost(input_cost, output_cost);

        let json = response.as_json().map_err(|e| {
            EvalError::LLMClientError(format!("Failed to parse claims JSON: {}", e))
        })?;

        let claims = json["claims"]
            .as_array()
            .ok_or_else(|| EvalError::LLMClientError("Missing claims array".to_string()))?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        Ok((claims, cost))
    }

    /// Verify claims against context
    async fn verify_claims(
        &self,
        claims: &[String],
        context: &str,
    ) -> Result<(Vec<ClaimVerification>, f64), EvalError> {
        let claims_text = claims
            .iter()
            .map(|c| format!("- {}", c))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = self
            .verification_prompt
            .replace("{context}", context)
            .replace("{claims}", &claims_text);

        let response = self
            .llm_client
            .evaluate(prompt)
            .await
            .map_err(|e| EvalError::LLMClientError(e.to_string()))?;

        // Calculate cost
        let (input_cost, output_cost) = self.llm_client.cost_per_token();
        let cost = response.usage.calculate_cost(input_cost, output_cost);

        let json = response.as_json().map_err(|e| {
            EvalError::LLMClientError(format!("Failed to parse verification JSON: {}", e))
        })?;

        let verifications_json = json["verifications"]
            .as_array()
            .ok_or_else(|| EvalError::LLMClientError("Missing verifications array".to_string()))?;

        let mut verifications = Vec::new();
        for v in verifications_json {
            let claim = v["claim"]
                .as_str()
                .ok_or_else(|| EvalError::LLMClientError("Missing claim text".to_string()))?
                .to_string();

            let status_str = v["status"]
                .as_str()
                .ok_or_else(|| EvalError::LLMClientError("Missing status".to_string()))?;

            let status = match status_str {
                "Supported" => ClaimStatus::Supported,
                "Contradicted" => ClaimStatus::Contradicted,
                "Unsupported" => ClaimStatus::Unsupported,
                _ => ClaimStatus::Unverifiable,
            };

            let evidence = v["evidence"].as_str().map(|s| s.to_string());
            let confidence = v["confidence"].as_f64().unwrap_or(0.8);

            verifications.push(ClaimVerification {
                claim,
                status,
                evidence,
                confidence,
            });
        }

        Ok((verifications, cost))
    }
}

#[async_trait]
impl Evaluator for HallucinationDetector {
    fn id(&self) -> &str {
        "hallucination_v1"
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        let start = Instant::now();
        let mut total_cost = 0.0;

        // Extract context and output
        let context = trace
            .context
            .as_ref()
            .map(|c| c.join("\n\n"))
            .unwrap_or_else(|| "No context provided".to_string());

        let output = trace
            .output
            .as_ref()
            .ok_or_else(|| EvalError::InvalidInput("No output to evaluate".to_string()))?;

        // Step 1: Extract claims
        let (claims, cost1) = self.extract_claims(output).await?;
        total_cost += cost1;

        if claims.is_empty() {
            return Ok(EvalResult {
                evaluator_id: self.id().to_string(),
                evaluator_type: Some("llm_judge".to_string()),
                metrics: HashMap::new(),
                passed: true, // No claims to hallucinate
                explanation: Some("No factual claims found in output.".to_string()),
                assertions: Vec::new(),
                judge_votes: Vec::new(),
                evidence_refs: Vec::new(),
                confidence: 1.0,
                cost: Some(total_cost),
                duration_ms: Some(start.elapsed().as_millis() as u64),
                actionable_feedback: None,
            });
        }

        // Step 2: Verify claims
        let (verifications, cost2) = self.verify_claims(&claims, &context).await?;
        total_cost += cost2;

        // Step 3: Compute metrics
        let total = verifications.len();
        let supported = verifications
            .iter()
            .filter(|v| matches!(v.status, ClaimStatus::Supported))
            .count();
        let contradicted = verifications
            .iter()
            .filter(|v| matches!(v.status, ClaimStatus::Contradicted))
            .count();
        let unsupported = verifications
            .iter()
            .filter(|v| matches!(v.status, ClaimStatus::Unsupported))
            .count();

        // Hallucination rate = (contradicted + unsupported) / total
        let hallucination_rate = if total > 0 {
            (contradicted + unsupported) as f64 / total as f64
        } else {
            0.0
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        // Build metrics
        let mut metrics = HashMap::new();
        metrics.insert(
            "hallucination_score".to_string(),
            MetricValue::Float(hallucination_rate),
        );
        metrics.insert("claims_total".to_string(), MetricValue::Int(total as i64));
        metrics.insert(
            "claims_supported".to_string(),
            MetricValue::Int(supported as i64),
        );
        metrics.insert(
            "claims_contradicted".to_string(),
            MetricValue::Int(contradicted as i64),
        );
        metrics.insert(
            "claims_unsupported".to_string(),
            MetricValue::Int(unsupported as i64),
        );

        // Serialize verifications for detailed report
        if let Ok(details) = serde_json::to_value(&verifications) {
            metrics.insert("claim_details".to_string(), MetricValue::Json(details));
        }

        let passed = hallucination_rate <= self.threshold;
        let explanation = format!(
            "{}/{} claims supported, {}/{} contradicted, {}/{} unsupported",
            supported, total, contradicted, total, unsupported, total
        );
        let avg_confidence = if total > 0 {
            verifications.iter().map(|v| v.confidence).sum::<f64>() / total as f64
        } else {
            0.0
        };

        Ok(EvalResult {
            evaluator_id: self.id().to_string(),
            evaluator_type: Some("llm_judge".to_string()),
            metrics,
            passed,
            explanation: Some(explanation),
            assertions: Vec::new(),
            judge_votes: Vec::new(),
            evidence_refs: Vec::new(),
            confidence: avg_confidence,
            cost: Some(total_cost),
            duration_ms: Some(duration_ms),
            actionable_feedback: None,
        })
    }

    fn metadata(&self) -> EvaluatorMetadata {
        let (input_cost, output_cost) = self.llm_client.cost_per_token();
        // Estimate ~500 tokens input, ~200 tokens output per evaluation
        let estimated_cost = (500.0 * input_cost) + (200.0 * output_cost);

        EvaluatorMetadata {
            name: "Hallucination Detector".to_string(),
            version: "1.0.0".to_string(),
            description: "Detects hallucinated information using LLM-as-judge. Checks if agent output is grounded in provided context.".to_string(),
            cost_per_eval: Some(estimated_cost),
            avg_latency_ms: Some(1500), // Typical LLM call latency
            tags: vec![
                "hallucination".to_string(),
                "llm-as-judge".to_string(),
                "groundedness".to_string(),
                "factuality".to_string(),
            ],
            author: Some("Agentreplay".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm_client::{LLMResponse, TokenUsage};

    struct MockLLMClient;

    #[async_trait]
    impl LLMClient for MockLLMClient {
        async fn evaluate(&self, prompt: String) -> Result<LLMResponse, LLMError> {
            if prompt.contains("Extract all factual claims") {
                // Claim extraction response
                Ok(LLMResponse {
                    content: r#"{
                        "claims": ["Paris is the capital of France"]
                    }"#
                    .to_string(),
                    usage: TokenUsage {
                        prompt_tokens: 100,
                        completion_tokens: 20,
                        total_tokens: 120,
                    },
                    model: "mock-model".to_string(),
                })
            } else {
                // Verification response
                Ok(LLMResponse {
                    content: r#"{
                        "verifications": [
                            {
                                "claim": "Paris is the capital of France",
                                "status": "Supported",
                                "evidence": "Paris is the capital and largest city of France.",
                                "confidence": 0.95
                            }
                        ]
                    }"#
                    .to_string(),
                    usage: TokenUsage {
                        prompt_tokens: 150,
                        completion_tokens: 50,
                        total_tokens: 200,
                    },
                    model: "mock-model".to_string(),
                })
            }
        }

        fn model_name(&self) -> &str {
            "mock-model"
        }

        fn cost_per_token(&self) -> (f64, f64) {
            (0.00000015, 0.0000006)
        }
    }

    #[tokio::test]
    async fn test_hallucination_detector() {
        let llm_client = Arc::new(MockLLMClient);
        let detector = HallucinationDetector::new(llm_client);

        let trace = TraceContext {
            trace_id: 123,
            edges: vec![],
            input: Some("What is the capital of France?".to_string()),
            output: Some("Paris is the capital of France".to_string()),
            context: Some(vec![
                "Paris is the capital and largest city of France.".to_string()
            ]),
            metadata: HashMap::new(),
            eval_trace: None,
            timestamp_us: 0,
        };

        let result = detector.evaluate(&trace).await.unwrap();

        assert!(result.passed);
        assert_eq!(result.evaluator_id, "hallucination_v1");

        // Check metrics
        if let Some(MetricValue::Float(score)) = result.metrics.get("hallucination_score") {
            assert!(*score <= 0.1); // Should be 0.0 since all claims supported
        } else {
            panic!("Missing hallucination_score metric");
        }

        if let Some(MetricValue::Int(supported)) = result.metrics.get("claims_supported") {
            assert_eq!(*supported, 1);
        } else {
            panic!("Missing claims_supported metric");
        }
    }
}
