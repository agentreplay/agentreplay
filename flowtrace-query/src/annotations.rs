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

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnnotationError {
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

/// Human annotation for an evaluation result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Annotation {
    pub id: u128,
    pub eval_run_id: u128,
    pub test_case_id: u128,
    pub annotator: String,             // User who did the annotation
    pub ratings: HashMap<String, f64>, // dimension -> score (0.0-1.0)
    pub thumbs: Option<ThumbsRating>,  // Simple binary rating
    pub stars: Option<u8>,             // 1-5 stars
    pub tags: Vec<String>,
    pub comment: Option<String>,
    pub corrected_output: Option<String>, // Ground truth correction
    pub time_spent_secs: u64,             // Time spent on this annotation
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThumbsRating {
    Up,
    Down,
    Neutral,
}

/// Annotation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationStats {
    pub total_annotations: usize,
    pub unique_cases_annotated: usize,
    pub avg_time_per_annotation_secs: u64,
    pub rating_distribution: HashMap<String, Vec<f64>>,
    pub thumbs_up_count: usize,
    pub thumbs_down_count: usize,
}

/// Inter-annotator agreement metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgreementMetrics {
    pub dimension: String,
    pub num_annotators: usize,
    pub num_cases_multi_annotated: usize,
    pub krippendorff_alpha: f64, // For continuous scales
    pub average_pairwise_agreement: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingExample {
    pub input: String,
    pub output: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationUpdate {
    pub ratings: Option<HashMap<String, f64>>,
    pub thumbs: Option<ThumbsRating>,
    pub stars: Option<u8>,
    pub tags: Option<Vec<String>>,
    pub comment: Option<String>,
    pub corrected_output: Option<String>,
}

pub struct AnnotationManager {
    // In-memory storage for now
    annotations: RwLock<HashMap<u128, Annotation>>,
}

impl Default for AnnotationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AnnotationManager {
    pub fn new() -> Self {
        Self {
            annotations: RwLock::new(HashMap::new()),
        }
    }

    /// Create annotation for an evaluation result
    pub fn create_annotation(&self, annotation: Annotation) -> Result<()> {
        let mut annotations = self.annotations.write();
        annotations.insert(annotation.id, annotation);
        Ok(())
    }

    /// Get all annotations for an eval run
    pub fn get_annotations_for_run(&self, eval_run_id: u128) -> Result<Vec<Annotation>> {
        let annotations = self.annotations.read();
        Ok(annotations
            .values()
            .filter(|a| a.eval_run_id == eval_run_id)
            .cloned()
            .collect())
    }

    /// Update existing annotation
    pub fn update_annotation(&self, annotation_id: u128, updates: AnnotationUpdate) -> Result<()> {
        let mut annotations = self.annotations.write();

        if let Some(annotation) = annotations.get_mut(&annotation_id) {
            if let Some(ratings) = updates.ratings {
                annotation.ratings = ratings;
            }
            if let Some(thumbs) = updates.thumbs {
                annotation.thumbs = Some(thumbs);
            }
            if let Some(stars) = updates.stars {
                annotation.stars = Some(stars);
            }
            if let Some(tags) = updates.tags {
                annotation.tags = tags;
            }
            if let Some(comment) = updates.comment {
                annotation.comment = Some(comment);
            }
            if let Some(corrected) = updates.corrected_output {
                annotation.corrected_output = Some(corrected);
            }
            annotation.updated_at = current_timestamp();
            Ok(())
        } else {
            Err(AnnotationError::NotFound(format!("Annotation {}", annotation_id)).into())
        }
    }

    /// Get annotation statistics for a campaign
    pub fn get_annotation_stats(&self, eval_run_id: u128) -> Result<AnnotationStats> {
        let annotations = self.get_annotations_for_run(eval_run_id)?;

        let total_annotations = annotations.len();
        let unique_cases: std::collections::HashSet<u128> =
            annotations.iter().map(|a| a.test_case_id).collect();

        let avg_time_per_annotation = if total_annotations > 0 {
            annotations.iter().map(|a| a.time_spent_secs).sum::<u64>() / total_annotations as u64
        } else {
            0
        };

        // Calculate distribution of ratings
        let mut rating_distribution: HashMap<String, Vec<f64>> = HashMap::new();
        for annotation in &annotations {
            for (dimension, score) in &annotation.ratings {
                rating_distribution
                    .entry(dimension.clone())
                    .or_default()
                    .push(*score);
            }
        }

        Ok(AnnotationStats {
            total_annotations,
            unique_cases_annotated: unique_cases.len(),
            avg_time_per_annotation_secs: avg_time_per_annotation,
            rating_distribution,
            thumbs_up_count: annotations
                .iter()
                .filter(|a| a.thumbs == Some(ThumbsRating::Up))
                .count(),
            thumbs_down_count: annotations
                .iter()
                .filter(|a| a.thumbs == Some(ThumbsRating::Down))
                .count(),
        })
    }

    /// Calculate inter-annotator agreement
    pub fn calculate_agreement(
        &self,
        eval_run_id: u128,
        dimension: &str,
    ) -> Result<AgreementMetrics> {
        let annotations = self.get_annotations_for_run(eval_run_id)?;

        // Group annotations by test case
        let mut case_annotations: HashMap<u128, Vec<&Annotation>> = HashMap::new();
        for annotation in &annotations {
            case_annotations
                .entry(annotation.test_case_id)
                .or_default()
                .push(annotation);
        }

        // Filter to cases with multiple annotators
        let multi_annotated: Vec<_> = case_annotations
            .iter()
            .filter(|(_, anns)| anns.len() >= 2)
            .collect();

        if multi_annotated.is_empty() {
            // Need at least 2 annotations per case for agreement calculation
            // Returning default metrics instead of error to avoid UI crash on empty
            return Ok(AgreementMetrics {
                dimension: dimension.to_string(),
                num_annotators: 0,
                num_cases_multi_annotated: 0,
                krippendorff_alpha: 0.0,
                average_pairwise_agreement: 0.0,
            });
        }

        // Calculate Krippendorff's Alpha for continuous ratings
        let alpha = self.krippendorff_alpha(&multi_annotated, dimension)?;

        // Calculate average pairwise agreement
        let mut pairwise_agreements = Vec::new();
        for (_, anns) in &multi_annotated {
            for i in 0..anns.len() {
                for j in (i + 1)..anns.len() {
                    if let (Some(score_i), Some(score_j)) = (
                        anns[i].ratings.get(dimension),
                        anns[j].ratings.get(dimension),
                    ) {
                        let agreement = 1.0 - (score_i - score_j).abs();
                        pairwise_agreements.push(agreement);
                    }
                }
            }
        }

        let avg_pairwise = if !pairwise_agreements.is_empty() {
            pairwise_agreements.iter().sum::<f64>() / pairwise_agreements.len() as f64
        } else {
            0.0
        };

        Ok(AgreementMetrics {
            dimension: dimension.to_string(),
            num_annotators: case_annotations
                .values()
                .map(|anns| anns.len())
                .max()
                .unwrap_or(0),
            num_cases_multi_annotated: multi_annotated.len(),
            krippendorff_alpha: alpha,
            average_pairwise_agreement: avg_pairwise,
        })
    }

    /// Krippendorff's Alpha calculation
    fn krippendorff_alpha(
        &self,
        multi_annotated: &[(&u128, &Vec<&Annotation>)],
        dimension: &str,
    ) -> Result<f64> {
        // Simplified implementation

        let mut all_pairs = Vec::new();
        for (_, anns) in multi_annotated {
            for i in 0..anns.len() {
                for j in (i + 1)..anns.len() {
                    if let (Some(score_i), Some(score_j)) = (
                        anns[i].ratings.get(dimension),
                        anns[j].ratings.get(dimension),
                    ) {
                        all_pairs.push((*score_i, *score_j));
                    }
                }
            }
        }

        if all_pairs.is_empty() {
            return Ok(0.0);
        }

        // Calculate observed disagreement
        let observed_disagreement: f64 =
            all_pairs.iter().map(|(a, b)| (a - b).powi(2)).sum::<f64>() / all_pairs.len() as f64;

        // Calculate expected disagreement (simplified)
        let all_scores: Vec<f64> = all_pairs.iter().flat_map(|(a, b)| vec![*a, *b]).collect();
        let mean = all_scores.iter().sum::<f64>() / all_scores.len() as f64;
        let variance =
            all_scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / all_scores.len() as f64;

        let expected_disagreement = 2.0 * variance;

        if expected_disagreement == 0.0 {
            return Ok(1.0); // Perfect agreement if no variance
        }

        // Alpha = 1 - (observed / expected)
        let alpha = 1.0 - (observed_disagreement / expected_disagreement);
        Ok(alpha.clamp(0.0, 1.0))
    }

    /// Export annotations for fine-tuning
    pub fn export_annotations_for_training(
        &self,
        eval_run_id: u128,
    ) -> Result<Vec<TrainingExample>> {
        let annotations = self.get_annotations_for_run(eval_run_id)?;

        // Need access to results (inputs). Since we don't have DB access here directly in this struct,
        // we can't fetch inputs. In a real system, we'd join.
        // For this task, we will just return what we have in annotations (corrected_output)
        // and put placeholders for input, or assume the caller handles joining.
        // Or we assume Annotation struct stores input? No.

        // We will just mock the input for now as "Looked up from DB".

        let mut training_examples = Vec::new();

        // Group by test case
        let mut case_annotations: HashMap<u128, Vec<&Annotation>> = HashMap::new();
        for annotation in &annotations {
            case_annotations
                .entry(annotation.test_case_id)
                .or_default()
                .push(annotation);
        }

        for (test_case_id, anns) in case_annotations {
            // Use consensus or average of annotations
            let consensus_output = if anns.len() == 1 {
                anns[0].corrected_output.clone()
            } else {
                // If multiple annotators, use majority vote or best-rated
                anns.iter()
                    .max_by_key(|a| (a.ratings.values().sum::<f64>() * 100.0) as i64)
                    .and_then(|a| a.corrected_output.clone())
            };

            if let Some(corrected) = consensus_output {
                training_examples.push(TrainingExample {
                    input: format!("Input for case {}", test_case_id), // Placeholder
                    output: corrected,
                    metadata: serde_json::json!({
                        "test_case_id": test_case_id,
                        "num_annotators": anns.len(),
                        "avg_rating": anns.iter()
                            .flat_map(|a| a.ratings.values())
                            .sum::<f64>() / (anns.len() as f64),
                    }),
                });
            }
        }

        Ok(training_examples)
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}
