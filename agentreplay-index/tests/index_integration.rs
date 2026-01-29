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

//! Integration tests for indexing components

use agentreplay_index::concept::{
    ConceptEntry, ConceptExtractor, ConceptIndex, ConceptQuery, ConceptSource,
};

/// Test concept extraction from text
#[test]
fn test_concept_extraction() {
    let extractor = ConceptExtractor::default();
    
    let text = "Implemented UserAuthentication service with JWTTokenGenerator";
    let concepts = extractor.extract_from_text(text, ConceptSource::Code);
    
    assert!(concepts.iter().any(|c| c.normalized == "user-authentication"));
    assert!(concepts.iter().any(|c| c.normalized == "service"));
    assert!(concepts.iter().any(|c| c.normalized == "jwttoken-generator"));
}

/// Test concept extraction from file paths
#[test]
fn test_concept_extraction_from_path() {
    let extractor = ConceptExtractor::default();
    
    let path = "src/auth/jwt_token_generator.rs";
    let concepts = extractor.extract_from_paths(&[path.to_string()]);
    
    assert!(concepts.iter().any(|c| c.normalized == "auth"));
    assert!(concepts.iter().any(|c| c.normalized == "jwt-token-generator"));
}

/// Test concept normalization
#[test]
fn test_concept_normalization() {
    let extractor = ConceptExtractor::default();
    
    assert_eq!(extractor.normalize("UserAuthentication"), "user-authentication");
    assert_eq!(extractor.normalize("api_endpoint"), "api-endpoint");
    assert_eq!(extractor.normalize("HTTPClient"), "httpclient");
    assert_eq!(extractor.normalize("simple"), "simple");
}

/// Test concept index
#[test]
fn test_concept_index() {
    let index = ConceptIndex::new();
    let project_id = 100;
    
    // Index observations
    index_observation(&index, project_id, 1, vec!["authentication", "jwt"]);
    index_observation(&index, project_id, 2, vec!["authentication", "oauth"]);
    index_observation(&index, project_id, 3, vec!["jwt", "token"]);
    
    // Find observations by concept
    let auth_obs = index.find_observations(
        &ConceptQuery::new(project_id)
            .concepts(vec!["authentication".to_string()])
            .limit(10),
    );
    assert_eq!(auth_obs.len(), 2);
    assert!(auth_obs.contains(&1));
    assert!(auth_obs.contains(&2));
    
    let jwt_obs = index.find_observations(
        &ConceptQuery::new(project_id)
            .concepts(vec!["jwt".to_string()])
            .limit(10),
    );
    assert_eq!(jwt_obs.len(), 2);
    assert!(jwt_obs.contains(&1));
    assert!(jwt_obs.contains(&3));
}

/// Test related concepts
#[test]
fn test_related_concepts() {
    let index = ConceptIndex::new();
    let project_id = 100;
    
    // Index observations with co-occurring concepts
    index_observation(
        &index,
        project_id,
        1,
        vec!["authentication", "jwt", "security"],
    );
    index_observation(
        &index,
        project_id,
        2,
        vec!["authentication", "jwt", "token"],
    );
    index_observation(
        &index,
        project_id,
        3,
        vec!["authentication", "oauth", "security"],
    );
    
    // Find related concepts
    let related = index.find_related(project_id, "authentication", 5);
    
    // jwt, security, oauth, token should all be related to authentication
    assert!(related.contains(&"jwt".to_string()));
    assert!(related.contains(&"security".to_string()));
    assert!(related.contains(&"oauth".to_string()));
}

/// Test top concepts
#[test]
fn test_top_concepts() {
    let index = ConceptIndex::new();
    let project_id = 100;
    
    // Index multiple observations
    index_observation(&index, project_id, 1, vec!["auth", "jwt"]);
    index_observation(&index, project_id, 2, vec!["auth", "token"]);
    index_observation(&index, project_id, 3, vec!["auth", "security"]);
    index_observation(&index, project_id, 4, vec!["jwt", "token"]);
    
    // Get top concepts
    let top = index.get_top_concepts(project_id, 3);
    
    // "auth" should be #1 (appears in 3 observations)
    assert_eq!(top[0].0, "auth");
    assert_eq!(top[0].1, 3);
    
    // "jwt" and "token" should be next (appear in 2 observations each)
    assert!(top.len() >= 2);
}

/// Test concept index removal
#[test]
fn test_concept_removal() {
    let index = ConceptIndex::new();
    let project_id = 100;
    
    index_observation(&index, project_id, 1, vec!["test"]);
    
    // Verify it's indexed
    let obs = index.find_observations(
        &ConceptQuery::new(project_id)
            .concepts(vec!["test".to_string()])
            .limit(10),
    );
    assert_eq!(obs.len(), 1);
    
    // Remove observation
    index.remove_observation(1);
    
    // Verify it's removed
    let obs = index.find_observations(
        &ConceptQuery::new(project_id)
            .concepts(vec!["test".to_string()])
            .limit(10),
    );
    assert_eq!(obs.len(), 0);
}

/// Test large-scale indexing
#[test]
fn test_large_scale_indexing() {
    let index = ConceptIndex::new();
    let extractor = ConceptExtractor::default();
    let project_id = 100;

    // Index many observations
    for i in 0..100 {
        let text = format!(
            "Implemented feature{} with Component{} and Service{}",
            i % 10,
            i % 5,
            i % 3
        );
        let concepts = extractor
            .extract_from_text(&text, ConceptSource::Code)
            .into_iter()
            .map(|c| c.normalized)
            .collect::<Vec<_>>();
        index_observation_strings(&index, project_id, i as u128, concepts);
    }

    // Query should still be fast
    let results = index.find_observations(
        &ConceptQuery::new(project_id)
            .concepts(vec!["feature0".to_string()])
            .limit(20),
    );
    assert!(!results.is_empty());

    // Top concepts should reflect frequency
    let top = index.get_top_concepts(project_id, 5);
    assert!(top.len() <= 5);
}

/// Test empty index
#[test]
fn test_empty_index() {
    let index = ConceptIndex::new();
    let project_id = 100;

    let results = index.find_observations(
        &ConceptQuery::new(project_id)
            .concepts(vec!["nonexistent".to_string()])
            .limit(10),
    );
    assert_eq!(results.len(), 0);

    let related = index.find_related(project_id, "nonexistent", 5);
    assert_eq!(related.len(), 0);

    let top = index.get_top_concepts(project_id, 10);
    assert_eq!(top.len(), 0);
}

/// Test concept extraction edge cases
#[test]
fn test_concept_extraction_edge_cases() {
    let extractor = ConceptExtractor::default();
    
    // Empty string
    let concepts = extractor.extract_from_text("", ConceptSource::Narrative);
    assert_eq!(concepts.len(), 0);
    
    // Only special characters
    let concepts = extractor.extract_from_text("!@#$%^&*()", ConceptSource::Narrative);
    assert_eq!(concepts.len(), 0);
    
    // Single word
    let concepts = extractor.extract_from_text("test", ConceptSource::Narrative);
    assert_eq!(concepts.len(), 1);
    assert!(concepts.iter().any(|c| c.normalized == "test"));
}

/// Test concurrent index access
#[test]
fn test_concurrent_index_access() {
    use std::sync::Arc;
    use std::thread;
    
    let index = Arc::new(ConceptIndex::new());
    let project_id = 100;
    let mut handles = vec![];
    
    // Spawn multiple threads indexing
    for i in 0..10 {
        let index_clone = Arc::clone(&index);
        let handle = thread::spawn(move || {
            index_observation_strings(
                &index_clone,
                project_id,
                i as u128,
                vec![format!("concept-{}", i % 3)],
            );
        });
        handles.push(handle);
    }
    
    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Verify indexing worked
    let results = index.find_observations(
        &ConceptQuery::new(project_id)
            .concepts(vec!["concept-0".to_string()])
            .limit(10),
    );
    assert!(!results.is_empty());
}

/// Test concept frequency tracking
#[test]
fn test_concept_frequency() {
    let index = ConceptIndex::new();
    let project_id = 100;

    // Index same concept multiple times
    index_observation(&index, project_id, 1, vec!["common"]);
    index_observation(&index, project_id, 2, vec!["common"]);
    index_observation(&index, project_id, 3, vec!["common"]);
    index_observation(&index, project_id, 4, vec!["rare"]);

    let top = index.get_top_concepts(project_id, 5);

    // "common" should have higher frequency
    assert_eq!(top[0].0, "common");
    assert_eq!(top[0].1, 3);
}

/// Test multiple concepts per observation
#[test]
fn test_multiple_concepts_per_observation() {
    let index = ConceptIndex::new();
    let project_id = 100;
    
    let concepts = vec!["auth", "jwt", "token", "security", "bcrypt"];
    
    index_observation(&index, project_id, 1, concepts.clone());
    
    // All concepts should find the observation
    for concept in concepts {
        let obs = index.find_observations(
            &ConceptQuery::new(project_id)
                .concepts(vec![concept.to_string()])
                .limit(10),
        );
        assert!(obs.contains(&1));
    }
}

/// Integration test: Extract and index
#[test]
fn test_extract_and_index() {
    let extractor = ConceptExtractor::default();
    let index = ConceptIndex::new();
    let project_id = 100;
    
    // Extract concepts from text
    let text = "Implemented UserAuthentication with JWTTokenGenerator and BcryptHasher";
    let concepts = extractor.extract_from_text(text, ConceptSource::Code);
    let normalized = concepts
        .iter()
        .map(|c| c.normalized.clone())
        .collect::<Vec<_>>();
    
    // Index observation
    index_observation_strings(&index, project_id, 1, normalized.clone());
    
    // Verify all extracted concepts are indexed
    for concept in normalized {
        let obs = index.find_observations(
            &ConceptQuery::new(project_id)
                .concepts(vec![concept.clone()])
                .limit(10),
        );
        assert!(obs.contains(&1), "Concept {} not found", concept);
    }
}

fn index_observation(
    index: &ConceptIndex,
    project_id: u128,
    observation_id: u128,
    concepts: Vec<&str>,
) {
    let entries = concepts
        .into_iter()
        .map(|concept| ConceptEntry {
            project_id,
            concept: concept.to_string(),
            observation_id,
            confidence: 1.0,
            source: "test".to_string(),
            indexed_at: 0,
        })
        .collect::<Vec<_>>();

    index.index_batch(entries);
}

fn index_observation_strings(
    index: &ConceptIndex,
    project_id: u128,
    observation_id: u128,
    concepts: Vec<String>,
) {
    let entries = concepts
        .into_iter()
        .map(|concept| ConceptEntry {
            project_id,
            concept,
            observation_id,
            confidence: 1.0,
            source: "test".to_string(),
            indexed_at: 0,
        })
        .collect::<Vec<_>>();

    index.index_batch(entries);
}
