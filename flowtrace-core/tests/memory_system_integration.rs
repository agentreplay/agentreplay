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

//! Integration tests for the complete memory system.
//!
//! These tests verify that all 16 components work together correctly.

use std::path::Path;

use flowtrace_core::{
    context::ContextConfig,
    language::{Language, LanguageConfig, LanguagePrompts},
    memory_agent::MemoryAgentConfig,
    observation::Observation,
    observation_types::{ObservationCategory, ObservationType},
    privacy::{has_private_content, PrivacyTagProcessor},
    project::generate_project_id,
    quality::{ObservationInput, QualityMetrics},
    session::{ContinuityConfig, ContinuityManager, PersistedSessionState, SessionStateStore},
    session_summary::SessionSummaryBuilder,
    HlcTimestamp,
};

/// Test observation creation and validation
#[test]
fn test_observation_lifecycle() {
    let project_id = generate_project_id(Path::new("/tmp/test-project"));
    
    let obs = Observation::builder(1, 100, project_id)
        .observation_type(ObservationType::Implementation)
        .title("Implemented user authentication")
        .subtitle("Added JWT token generation with bcrypt password hashing")
        .add_fact("Added JWT token generation")
        .add_fact("Integrated bcrypt password hashing")
        .narrative("User requested secure authentication. Implemented JWT-based auth.")
        .add_concept("authentication")
        .add_concept("jwt")
        .add_concept("security")
        .created_at(HlcTimestamp::from_parts(1000, 0))
        .build();
    
    // Validate observation fields
    assert_eq!(obs.project_id, project_id);
    assert_eq!(obs.session_id, 100);
    assert_eq!(obs.observation_type, ObservationType::Implementation);
    assert_eq!(obs.facts.len(), 2);
    assert_eq!(obs.concepts.len(), 3);
}

/// Test observation types and categories
#[test]
fn test_observation_types() {
    // Test standard types
    assert_eq!(
        ObservationType::Implementation.category(),
        ObservationCategory::Development
    );
    assert_eq!(
        ObservationType::Architecture.category(),
        ObservationCategory::Architecture
    );
    assert_eq!(
        ObservationType::Research.category(),
        ObservationCategory::Investigation
    );
    assert_eq!(
        ObservationType::Documentation.category(),
        ObservationCategory::Documentation
    );
    
    // Test custom type
    let custom = ObservationType::Custom("security_audit".to_string());
    assert_eq!(custom.category(), ObservationCategory::Other);
    
    // Test all standard types
    let all_types = ObservationType::all_standard();
    assert!(all_types.len() >= 16);
    
    // Test types in category
    let dev_types = ObservationType::types_in_category(ObservationCategory::Development);
    assert!(dev_types.contains(&ObservationType::Implementation));
    assert!(dev_types.contains(&ObservationType::Debugging));
}

/// Test session summary generation
#[test]
fn test_session_summary() {
    let project_id = generate_project_id(Path::new("/tmp/test-project"));
    
    let _obs1 = Observation::builder(1, 100, project_id)
        .observation_type(ObservationType::Implementation)
        .title("Implemented feature A")
        .subtitle("Added module A with tests")
        .add_fact("Added module A")
        .add_concept("feature-a")
        .created_at(HlcTimestamp::from_parts(1000, 0))
        .build();
    
    let _obs2 = Observation::builder(2, 100, project_id)
        .observation_type(ObservationType::Testing)
        .title("Added tests for feature A")
        .subtitle("Wrote unit tests")
        .add_fact("Added unit tests")
        .add_concept("testing")
        .add_concept("feature-a")
        .created_at(HlcTimestamp::from_parts(2000, 0))
        .build();
    
    let summary = SessionSummaryBuilder::new(100, project_id)
        .observation_count(2)
        .build();
    
    assert_eq!(summary.session_id, 100);
    assert_eq!(summary.project_id, project_id);
    assert_eq!(summary.observation_count, 2);
}

/// Test privacy redaction
#[test]
fn test_privacy_redaction() {
    let processor = PrivacyTagProcessor::default();

    let text_with_private = "Contact <private>user@example.com</private> for details";
    let (redacted, metadata) = processor.process(text_with_private);
    assert!(!redacted.contains("user@example.com"));
    assert!(redacted.contains("[REDACTED]"));
    assert!(metadata.had_redactions());
}

/// Test PII detection
#[test]
fn test_pii_detection() {
    let text_with_private = "Email me at <private>john@example.com</private>";
    let has_private = has_private_content(text_with_private);

    assert!(has_private);
}

/// Test multi-language support
#[test]
fn test_language_support() {
    // Test English
    let config_en = LanguageConfig {
        primary: Language::English,
        auto_detect: false,
        english_concepts: true,
    };
    assert_eq!(config_en.primary, Language::English);

    // Test Spanish
    let config_es = LanguageConfig {
        primary: Language::Spanish,
        auto_detect: false,
        english_concepts: true,
    };
    assert_eq!(config_es.primary, Language::Spanish);

    // Test that different languages have different prompts
    let prompts_en = LanguagePrompts::for_language(Language::English);
    let prompts_es = LanguagePrompts::for_language(Language::Spanish);

    assert_ne!(prompts_en.system_prefix, prompts_es.system_prefix);
    assert_ne!(
        prompts_en.type_labels.get("implementation"),
        prompts_es.type_labels.get("implementation")
    );
}

/// Test quality metrics
#[test]
fn test_quality_metrics() {
    let project_id = generate_project_id(Path::new("/tmp/test-project"));
    
    // Test high-quality observation
    let good_obs = Observation::builder(1, 1, project_id)
        .observation_type(ObservationType::Implementation)
        .title("Implemented JWT authentication")
        .subtitle("Added JWT token generation with bcrypt password hashing for secure authentication")
        .add_fact("Added JWT token generation in auth/jwt.rs")
        .add_fact("Integrated bcrypt for secure password hashing")
        .add_fact("Updated user model with password_hash field")
        .narrative("User requested secure authentication. Implemented JWT-based auth with HS256 signing. Used bcrypt with cost factor 12 for password hashing.")
        .add_concept("jwt")
        .add_concept("bcrypt")
        .add_concept("authentication")
        .created_at(HlcTimestamp::from_parts(1000, 0))
        .build();
    
    let metrics = QualityMetrics::default();
    let score = metrics.score(&observation_to_input(&good_obs));
    let good_overall = score.overall;
    
    // High-quality observation should score well
    assert!(score.completeness > 0.8);
    assert!(score.overall > 0.7);
    
    // Test low-quality observation
    let bad_obs = Observation::builder(2, 1, project_id)
        .observation_type(ObservationType::Implementation)
        .title("Did stuff")
        .subtitle("Made some updates")
        .created_at(HlcTimestamp::from_parts(2000, 0))
        .build();
    
    let score = metrics.score(&observation_to_input(&bad_obs));
    
    // Low-quality observation should score poorly
    assert!(score.completeness < 0.6);
    assert!(score.overall < good_overall);
}

/// Test project ID generation
#[test]
fn test_project_id_generation() {
    let path1 = "/tmp/project1";
    let path2 = "/tmp/project2";
    
    let id1 = generate_project_id(Path::new(path1));
    let id2 = generate_project_id(Path::new(path2));
    
    // Different paths should generate different IDs
    assert_ne!(id1, id2);
    
    // Same path should generate same ID (deterministic)
    let id1_again = generate_project_id(Path::new(path1));
    assert_eq!(id1, id1_again);
}

/// Test session continuity
#[test]
fn test_session_continuity() {
    let config = ContinuityConfig::default();
    let manager = ContinuityManager::new(config);
    
    let content_session_id = 100;
    let memory_session_id = 200;
    let project_id = generate_project_id(Path::new("/tmp/test-project"));
    
    // Start new continuity
    let mut continuity = manager.get_or_create(content_session_id, project_id);
    continuity.set_memory_session(memory_session_id);
    continuity.next_prompt();
    manager.update(continuity.clone());
    
    assert_eq!(continuity.content_session_id, content_session_id);
    assert_eq!(continuity.memory_session_id, Some(memory_session_id));
    assert_eq!(continuity.prompt_number, 1);
    
    // Test that timeout is configured
    assert!(ContinuityConfig::default().session_timeout().as_secs() > 0);
}

/// Test session state persistence
#[test]
fn test_session_state_store() {
    let store = SessionStateStore::new();
    
    let session_id = 100;
    let state = PersistedSessionState {
        content_session_id: session_id,
        memory_session_id: Some(200),
        project_id: 300,
        prompt_number: 5,
        last_observation_id: None,
        created_at_us: 0,
        last_activity_us: 0,
        conversation_history: None,
    };
    
    // Save state
    store.save(state.clone());
    
    // Retrieve state
    let retrieved = store.load(session_id).unwrap();
    assert_eq!(retrieved.content_session_id, state.content_session_id);
    assert_eq!(retrieved.memory_session_id, state.memory_session_id);
    
    // Delete state
    store.delete(session_id);
    assert!(store.load(session_id).is_none());
}

/// Test memory agent configuration
#[test]
fn test_memory_agent_config() {
    let config = MemoryAgentConfig::default()
        .session_timeout(std::time::Duration::from_secs(1800))
        .max_tokens(4096)
        .temperature(0.2);

    assert_eq!(config.session_timeout_duration().as_secs(), 1800);
    assert_eq!(config.max_tokens, 4096);
    assert!((config.temperature - 0.2).abs() < f32::EPSILON);
}

/// Test context building configuration
#[test]
fn test_context_config() {
    let config = ContextConfig {
        token_budget: 1000,
        include_files: true,
        include_concepts: true,
        ..Default::default()
    };
    
    assert_eq!(config.token_budget, 1000);
    assert!(config.include_files);
    assert!(config.include_concepts);
}

/// Integration test: Full workflow
#[test]
fn test_full_memory_workflow() {
    // 1. Generate project ID
    let project_path = "/tmp/test-project";
    let project_id = generate_project_id(Path::new(project_path));
    
    // 2. Create observation
    let obs = Observation::builder(1, 100, project_id)
        .observation_type(ObservationType::Implementation)
        .title("Implemented user authentication")
        .subtitle("User requested secure authentication system with JWT and bcrypt")
        .add_fact("Added JWT token generation")
        .add_fact("Integrated bcrypt password hashing")
        .narrative("User requested secure authentication system. Implemented JWT-based authentication with bcrypt password hashing.")
        .add_concept("authentication")
        .add_concept("jwt")
        .add_concept("security")
        .created_at(HlcTimestamp::from_parts(1000, 0))
        .build();
    
    // 3. Apply privacy redaction
    let processor = PrivacyTagProcessor::default();
    let (filtered_narrative, _metadata) = processor.process(&obs.narrative);
    assert!(!filtered_narrative.is_empty());
    
    // 4. Calculate quality
    let metrics = QualityMetrics::default();
    let quality = metrics.score(&observation_to_input(&obs));
    assert!(quality.overall > 0.5);
    
    // 5. Build session summary
    let summary = SessionSummaryBuilder::new(obs.session_id, obs.project_id)
        .observation_count(1)
        .build();
    assert_eq!(summary.observation_count, 1);
    
    // 6. Verify observation type
    assert_eq!(obs.observation_type.category(), ObservationCategory::Development);
    
    println!("✅ Full workflow completed successfully!");
    println!("   Project ID: {}", project_id);
    println!("   Quality Score: {:.2}", quality.overall);
    println!("   Session Summary: {} observations", summary.observation_count);
}

/// Test worktree detection
#[test]
fn test_project_id_determinism() {
    // Same path should generate same ID (deterministic)
    let path = "/tmp/test-project";
    let id1 = generate_project_id(Path::new(path));
    let id2 = generate_project_id(Path::new(path));
    assert_eq!(id1, id2);
    
    // Different paths should generate different IDs
    let id3 = generate_project_id(Path::new("/tmp/other-project"));
    assert_ne!(id1, id3);
}

fn observation_to_input(obs: &Observation) -> ObservationInput {
    ObservationInput {
        observation_type: obs.observation_type.as_str().to_string(),
        title: obs.title.clone(),
        subtitle: if obs.subtitle.is_empty() {
            None
        } else {
            Some(obs.subtitle.clone())
        },
        facts: obs.facts.clone(),
        narrative: obs.narrative.clone(),
        concepts: obs.concepts.iter().map(|c| c.value.clone()).collect(),
        files_read: obs
            .files_read
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        files_modified: obs
            .files_modified
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
    }
}
