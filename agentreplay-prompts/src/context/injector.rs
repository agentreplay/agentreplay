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

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObservationType {
    ChatMessage,
    ToolExecution,
    DataPoint,
    SystemEvent,
    ErrorTrace,
    UserMemory,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObservationConcept {
    Person,
    Project,
    Place,
    Tool,
    Topic,
    Time,
    Task,
    Outcome,
    Risk,
    Policy,
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyMode {
    PublicOnly,
    PrivateAllowed,
}

#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub max_tokens: usize,
    pub max_turns: usize,
    pub include_long_term_memory: bool,
    pub include_short_term_memory: bool,
    pub include_tool_results: bool,
    pub include_user_pref: bool,
    pub privacy_mode: PrivacyMode,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: 1536,
            max_turns: 16,
            include_long_term_memory: true,
            include_short_term_memory: true,
            include_tool_results: true,
            include_user_pref: true,
            privacy_mode: PrivacyMode::PrivateAllowed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextObservation {
    pub id: u128,
    pub observation_type: ObservationType,
    pub concepts: Vec<ObservationConcept>,
    pub content: String,
    pub summary: Option<String>,
    pub created_at: u64,
    pub is_private: bool,
    pub is_tool_result: bool,
    pub is_long_term: bool,
    pub is_user_pref: bool,
    pub relevance_score: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct ContextBlock {
    pub label: String,
    pub content: String,
    pub tokens: usize,
    pub source_id: u128,
}

#[derive(Debug, Clone)]
pub struct ConceptObservationRef {
    pub id: u128,
    pub snippet: String,
}

#[derive(Debug, Clone)]
pub struct ConceptSummary {
    pub concept: ObservationConcept,
    pub count: usize,
    pub observations: Vec<ConceptObservationRef>,
}

#[derive(Debug, Clone)]
pub struct ContextPackage {
    pub raw_blocks: Vec<ContextBlock>,
    pub summary: String,
    pub concepts: Vec<ConceptSummary>,
    pub token_count: usize,
    pub truncation_applied: bool,
}

pub struct ContextInjector {
    config: ContextConfig,
}

impl ContextInjector {
    pub fn new(config: ContextConfig) -> Self {
        Self { config }
    }

    pub fn build_context(&self, observations: &[ContextObservation]) -> ContextPackage {
        let filtered = self.filter_observations(observations);
        let filtered = self.apply_turn_limit(filtered);
        let mut scored = self.score_observations(filtered);
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut raw_blocks = Vec::new();
        let mut concepts_map: HashMap<ObservationConcept, Vec<ConceptObservationRef>> =
            HashMap::new();
        let mut summary_lines = Vec::new();
        let mut token_count = 0usize;
        let mut truncation_applied = false;

        for (obs, _score) in scored {
            let summary_text = obs
                .summary
                .clone()
                .unwrap_or_else(|| obs.content.clone());
            let block_tokens = estimate_tokens(&obs.content);
            if token_count + block_tokens > self.config.max_tokens {
                truncation_applied = true;
                continue;
            }

            token_count += block_tokens;
            raw_blocks.push(ContextBlock {
                label: format!("{:?}", obs.observation_type),
                content: obs.content.clone(),
                tokens: block_tokens,
                source_id: obs.id,
            });

            summary_lines.push(format!(
                "{:?}: {}",
                obs.observation_type,
                truncate_text(&summary_text, 160)
            ));

            for concept in &obs.concepts {
                concepts_map
                    .entry(concept.clone())
                    .or_default()
                    .push(ConceptObservationRef {
                        id: obs.id,
                        snippet: truncate_text(&summary_text, 120),
                    });
            }
        }

        let concepts = concepts_map
            .into_iter()
            .map(|(concept, observations)| ConceptSummary {
                concept,
                count: observations.len(),
                observations,
            })
            .collect::<Vec<_>>();

        ContextPackage {
            raw_blocks,
            summary: summary_lines.join("\n"),
            concepts,
            token_count,
            truncation_applied,
        }
    }

    fn filter_observations<'a>(
        &self,
        observations: &'a [ContextObservation],
    ) -> Vec<&'a ContextObservation> {
        observations
            .iter()
            .filter(|obs| match self.config.privacy_mode {
                PrivacyMode::PublicOnly => !obs.is_private,
                PrivacyMode::PrivateAllowed => true,
            })
            .filter(|obs| {
                if !self.config.include_long_term_memory && obs.is_long_term {
                    return false;
                }
                if !self.config.include_short_term_memory && !obs.is_long_term {
                    return false;
                }
                if !self.config.include_tool_results && obs.is_tool_result {
                    return false;
                }
                if !self.config.include_user_pref && obs.is_user_pref {
                    return false;
                }
                true
            })
            .collect()
    }

    fn apply_turn_limit<'a>(
        &self,
        observations: Vec<&'a ContextObservation>,
    ) -> Vec<&'a ContextObservation> {
        if self.config.max_turns == 0 {
            return observations;
        }

        let mut chat_obs = observations
            .iter()
            .copied()
            .filter(|obs| obs.observation_type == ObservationType::ChatMessage)
            .collect::<Vec<_>>();

        chat_obs.sort_by_key(|obs| obs.created_at);
        let keep_chat_ids: std::collections::HashSet<u128> = chat_obs
            .into_iter()
            .rev()
            .take(self.config.max_turns)
            .map(|obs| obs.id)
            .collect();

        observations
            .into_iter()
            .filter(|obs| {
                obs.observation_type != ObservationType::ChatMessage
                    || keep_chat_ids.contains(&obs.id)
            })
            .collect()
    }

    fn score_observations<'a>(
        &self,
        observations: Vec<&'a ContextObservation>,
    ) -> Vec<(&'a ContextObservation, f32)> {
        observations
            .into_iter()
            .map(|obs| {
                let base = match obs.observation_type {
                    ObservationType::UserMemory => 1.0,
                    ObservationType::ChatMessage => 0.9,
                    ObservationType::ToolExecution => 0.8,
                    ObservationType::DataPoint => 0.7,
                    ObservationType::SystemEvent => 0.6,
                    ObservationType::ErrorTrace => 0.5,
                };
                let recency = (obs.created_at as f32 / 1_000_000.0).min(1.0);
                let relevance = obs.relevance_score.unwrap_or(0.5);
                let score = base * 0.5 + recency * 0.3 + relevance * 0.2;
                (obs, score)
            })
            .collect()
    }
}

fn estimate_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    let approx = (chars as f32 / 4.0).ceil() as usize;
    approx.max(1)
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    let mut truncated = text.chars().take(max_len).collect::<String>();
    truncated.push_str("…");
    truncated
}
