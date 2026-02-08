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

//! Concept extraction from observations.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Concept extraction configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptExtractionConfig {
    /// Minimum concept length.
    pub min_length: usize,
    /// Maximum concept length.
    pub max_length: usize,
    /// Whether to extract from code identifiers.
    pub extract_from_code: bool,
    /// Stopwords to exclude.
    pub stopwords: HashSet<String>,
}

impl Default for ConceptExtractionConfig {
    fn default() -> Self {
        let stopwords: HashSet<String> = [
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with",
            "by", "from", "as", "is", "was", "are", "were", "been", "be", "have", "has", "had",
            "do", "does", "did", "will", "would", "could", "should", "may", "might", "must",
            "this", "that", "these", "those", "it", "its", "they", "them", "their", "we", "our",
            "you", "your", "i", "my", "me", "he", "she", "him", "her", "his",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            min_length: 2,
            max_length: 50,
            extract_from_code: true,
            stopwords,
        }
    }
}

/// An extracted concept with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedConcept {
    /// Original form of the concept.
    pub original: String,
    /// Normalized form (lowercase-hyphenated).
    pub normalized: String,
    /// Source of extraction.
    pub source: ConceptSource,
    /// Confidence score (0.0-1.0).
    pub confidence: f32,
}

/// Source of concept extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConceptSource {
    /// Explicitly marked in <concept> tags.
    Explicit,
    /// Extracted from title.
    Title,
    /// Extracted from narrative.
    Narrative,
    /// Extracted from facts.
    Fact,
    /// Extracted from file paths.
    FilePath,
    /// Extracted from code identifiers.
    Code,
}

/// Concept extractor.
pub struct ConceptExtractor {
    config: ConceptExtractionConfig,
    camel_case_re: Regex,
    snake_case_re: Regex,
    identifier_re: Regex,
}

impl Default for ConceptExtractor {
    fn default() -> Self {
        Self::new(ConceptExtractionConfig::default())
    }
}

impl ConceptExtractor {
    /// Create a new extractor with configuration.
    pub fn new(config: ConceptExtractionConfig) -> Self {
        Self {
            config,
            camel_case_re: Regex::new(r"([a-z])([A-Z])").unwrap(),
            snake_case_re: Regex::new(r"_+").unwrap(),
            identifier_re: Regex::new(r"\b[a-zA-Z][a-zA-Z0-9_]{2,}\b").unwrap(),
        }
    }

    /// Normalize a concept to lowercase-hyphenated form.
    pub fn normalize(&self, concept: &str) -> String {
        // Split camelCase
        let expanded = self.camel_case_re.replace_all(concept, "${1}-${2}");
        // Replace underscores with hyphens
        let normalized = self.snake_case_re.replace_all(&expanded, "-");
        // Lowercase and clean
        normalized
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    }

    /// Extract concepts from explicit concept list.
    pub fn extract_explicit(&self, concepts: &[String]) -> Vec<ExtractedConcept> {
        concepts
            .iter()
            .filter(|c| self.is_valid_concept(c))
            .map(|c| ExtractedConcept {
                original: c.clone(),
                normalized: self.normalize(c),
                source: ConceptSource::Explicit,
                confidence: 1.0,
            })
            .collect()
    }

    /// Extract concepts from text (title, narrative, facts).
    pub fn extract_from_text(
        &self,
        text: &str,
        source: ConceptSource,
    ) -> Vec<ExtractedConcept> {
        let mut concepts = Vec::new();

        // Find identifiers
        if self.config.extract_from_code {
            for cap in self.identifier_re.find_iter(text) {
                let word = cap.as_str();
                if self.is_valid_concept(word) {
                    concepts.push(ExtractedConcept {
                        original: word.to_string(),
                        normalized: self.normalize(word),
                        source,
                        confidence: 0.6,
                    });
                }
            }
        }

        // Deduplicate by normalized form
        concepts.sort_by(|a, b| a.normalized.cmp(&b.normalized));
        concepts.dedup_by(|a, b| a.normalized == b.normalized);

        concepts
    }

    /// Extract concepts from file paths.
    pub fn extract_from_paths(&self, paths: &[String]) -> Vec<ExtractedConcept> {
        let mut concepts = Vec::new();

        for path in paths {
            // Extract file name without extension
            if let Some(file_name) = path.split('/').last() {
                let name = file_name.split('.').next().unwrap_or(file_name);
                if self.is_valid_concept(name) {
                    concepts.push(ExtractedConcept {
                        original: name.to_string(),
                        normalized: self.normalize(name),
                        source: ConceptSource::FilePath,
                        confidence: 0.7,
                    });
                }
            }

            // Extract directory names
            for part in path.split('/') {
                if part.is_empty() || part.starts_with('.') {
                    continue;
                }
                if self.is_valid_concept(part) {
                    concepts.push(ExtractedConcept {
                        original: part.to_string(),
                        normalized: self.normalize(part),
                        source: ConceptSource::FilePath,
                        confidence: 0.5,
                    });
                }
            }
        }

        // Deduplicate
        concepts.sort_by(|a, b| a.normalized.cmp(&b.normalized));
        concepts.dedup_by(|a, b| a.normalized == b.normalized);

        concepts
    }

    /// Check if a string is a valid concept.
    fn is_valid_concept(&self, s: &str) -> bool {
        let len = s.len();
        if len < self.config.min_length || len > self.config.max_length {
            return false;
        }

        // Must contain at least one letter
        if !s.chars().any(|c| c.is_alphabetic()) {
            return false;
        }

        // Check stopwords
        if self.config.stopwords.contains(&s.to_lowercase()) {
            return false;
        }

        true
    }

    /// Extract all concepts from an observation-like structure.
    pub fn extract_all(
        &self,
        explicit_concepts: &[String],
        title: &str,
        narrative: &str,
        facts: &[String],
        file_paths: &[String],
    ) -> Vec<ExtractedConcept> {
        let mut all_concepts = Vec::new();

        // Explicit concepts have highest priority
        all_concepts.extend(self.extract_explicit(explicit_concepts));

        // Title concepts
        all_concepts.extend(self.extract_from_text(title, ConceptSource::Title));

        // Narrative concepts
        all_concepts.extend(self.extract_from_text(narrative, ConceptSource::Narrative));

        // Fact concepts
        for fact in facts {
            all_concepts.extend(self.extract_from_text(fact, ConceptSource::Fact));
        }

        // File path concepts
        all_concepts.extend(self.extract_from_paths(file_paths));

        // Deduplicate, keeping highest confidence
        all_concepts.sort_by(|a, b| {
            a.normalized
                .cmp(&b.normalized)
                .then(b.confidence.partial_cmp(&a.confidence).unwrap())
        });
        all_concepts.dedup_by(|a, b| a.normalized == b.normalized);

        all_concepts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize() {
        let extractor = ConceptExtractor::default();

        assert_eq!(extractor.normalize("UserAuthentication"), "user-authentication");
        assert_eq!(extractor.normalize("api_endpoint"), "api-endpoint");
        // HTTPClient: no lowercase->uppercase transition until 'C', so just 'HTTP-Client' -> 'httpclient'
        assert_eq!(extractor.normalize("HTTPClient"), "httpclient");
        assert_eq!(extractor.normalize("simple"), "simple");
        // Mixed case: lowercase 'y' followed by uppercase 'H' triggers one split
        assert_eq!(extractor.normalize("myHTTPClient"), "my-httpclient");
    }

    #[test]
    fn test_extract_explicit() {
        let extractor = ConceptExtractor::default();

        let concepts = extractor.extract_explicit(&[
            "authentication".to_string(),
            "user-session".to_string(),
        ]);

        assert_eq!(concepts.len(), 2);
        assert_eq!(concepts[0].source, ConceptSource::Explicit);
        assert_eq!(concepts[0].confidence, 1.0);
    }

    #[test]
    fn test_extract_from_paths() {
        let extractor = ConceptExtractor::default();

        let concepts = extractor.extract_from_paths(&[
            "src/auth/user_handler.rs".to_string(),
        ]);

        let normalized: Vec<_> = concepts.iter().map(|c| c.normalized.as_str()).collect();
        assert!(normalized.contains(&"user-handler"));
        assert!(normalized.contains(&"auth"));
    }

    #[test]
    fn test_stopwords_filtered() {
        let extractor = ConceptExtractor::default();

        let concepts = extractor.extract_from_text(
            "The user is authenticated",
            ConceptSource::Narrative,
        );

        let normalized: Vec<_> = concepts.iter().map(|c| c.normalized.as_str()).collect();
        assert!(!normalized.contains(&"the"));
        assert!(normalized.contains(&"user"));
        assert!(normalized.contains(&"authenticated"));
    }
}
