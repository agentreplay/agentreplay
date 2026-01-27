use flowtrace_core::PromptTemplate;
use flowtrace_query::engine::Flowtrace;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::tempdir;

#[tokio::test]
async fn test_prompt_persistence() {
    // 1. Setup
    let dir = tempdir().expect("Failed to create temp dir");
    let db_path = dir.path();
    
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    // 2. Open DB and store prompt
    {
        let db = Flowtrace::open(db_path).expect("Failed to open DB");
        
        let template = PromptTemplate {
            id: 12345,
            name: "test_prompt".to_string(),
            description: "A test prompt".to_string(),
            template: "Hello {{name}}".to_string(),
            variables: vec!["name".to_string()],
            version: 1,
            created_at: now,
            updated_at: now,
            tags: vec!["test".to_string()],
            created_by: "tester".to_string(),
            metadata: None,
        };

        db.store_prompt_template(template.clone()).expect("Failed to store prompt");
        
        // precise verification that it's in memory
        let loaded = db.get_prompt_template(12345).expect("Failed to get prompt").expect("Prompt not found");
        assert_eq!(loaded.name, "test_prompt");
    } // db is dropped here

    // 3. Re-open DB and verify persistence
    {
        let db = Flowtrace::open(db_path).expect("Failed to reopen DB");
        
        let loaded = db.get_prompt_template(12345).expect("Failed to get prompt");
        assert!(loaded.is_some(), "Prompt should persist after restart");
        
        let template = loaded.unwrap();
        assert_eq!(template.name, "test_prompt");
        assert_eq!(template.template, "Hello {{name}}");
        assert_eq!(template.id, 12345);
    }
}
