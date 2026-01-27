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

// flowtrace-server/src/api/prompts.rs
//
// Prompt templates directory API endpoints

use super::query::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Data Models - Use core types
// ============================================================================

use flowtrace_core::enterprise::PromptTemplate as CorePromptTemplate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: u128,
    pub name: String,
    pub description: String,
    pub template: String,
    pub variables: Vec<String>,
    pub tags: Vec<String>,
    pub version: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub created_by: String,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl PromptTemplate {
    pub fn new(
        id: u128,
        name: String,
        template: String,
        created_by: String,
        timestamp: u64,
    ) -> Self {
        // Extract variables from template (look for {{variable}} patterns)
        let variables = extract_variables(&template);

        Self {
            id,
            name,
            description: String::new(),
            template,
            variables,
            tags: Vec::new(),
            version: 1,
            created_at: timestamp,
            updated_at: timestamp,
            created_by,
            metadata: None,
        }
    }
}

// Conversions between API and Core types (structures are identical)
impl From<PromptTemplate> for CorePromptTemplate {
    fn from(prompt: PromptTemplate) -> Self {
        CorePromptTemplate {
            id: prompt.id,
            name: prompt.name,
            description: prompt.description,
            template: prompt.template,
            variables: prompt.variables,
            tags: prompt.tags,
            version: prompt.version,
            created_at: prompt.created_at,
            updated_at: prompt.updated_at,
            created_by: prompt.created_by,
            metadata: prompt.metadata,
        }
    }
}

impl From<CorePromptTemplate> for PromptTemplate {
    fn from(core: CorePromptTemplate) -> Self {
        PromptTemplate {
            id: core.id,
            name: core.name,
            description: core.description,
            template: core.template,
            variables: core.variables,
            tags: core.tags,
            version: core.version,
            created_at: core.created_at,
            updated_at: core.updated_at,
            created_by: core.created_by,
            metadata: core.metadata,
        }
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreatePromptRequest {
    pub name: String,
    pub description: Option<String>,
    pub template: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}



#[derive(Debug, Deserialize)]
pub struct UpdatePromptRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub template: Option<String>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}



#[derive(Debug, Deserialize)]
pub struct RenderPromptRequest {
    pub variables: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct ListPromptsQuery {
    pub tag: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PromptResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub template: String,
    pub variables: Vec<String>,
    pub tags: Vec<String>,
    pub version: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub created_by: String,
}

#[derive(Debug, Serialize)]
pub struct PromptListResponse {
    pub prompts: Vec<PromptResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct RenderPromptResponse {
    pub rendered: String,
    pub template_id: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct PromptVersionResponse {
    pub version: u32,
    pub template: String,
    pub created_at: u64,
    pub created_by: String,
    pub change_summary: String,
}

#[derive(Debug, Serialize)]
pub struct PromptVersionHistoryResponse {
    pub prompt_id: String,
    pub versions: Vec<PromptVersionResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct PromptDiffResponse {
    pub prompt_id: String,
    pub version1: u32,
    pub version2: u32,
    pub diff: Vec<DiffLine>,
    pub template1: String,
    pub template2: String,
}

#[derive(Debug, Serialize)]
pub struct DiffLine {
    pub line_type: String, // "added", "removed", "unchanged"
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct VersionPerformanceMetrics {
    pub version: u32,
    pub avg_score: f64,
    pub eval_count: usize,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub avg_cost: f64,
}

#[derive(Debug, Serialize)]
pub struct PromptPerformanceResponse {
    pub prompt_id: String,
    pub metrics: Vec<VersionPerformanceMetrics>,
}

// ============================================================================
// Helper Functions
// ============================================================================

fn generate_id() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros();

    let random = (rand::random::<u64>() as u128) << 64;
    timestamp ^ random
}

fn current_timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

fn parse_id(id_str: &str) -> Result<u128, String> {
    let id_str = id_str.trim_start_matches("0x");
    u128::from_str_radix(id_str, 16).map_err(|e| format!("Invalid ID: {}", e))
}

fn extract_variables(template: &str) -> Vec<String> {
    let mut variables = Vec::new();
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' {
            if let Some(&'{') = chars.peek() {
                chars.next(); // consume second '{'
                let mut var_name = String::new();

                while let Some(&c) = chars.peek() {
                    if c == '}' {
                        chars.next();
                        if let Some(&'}') = chars.peek() {
                            chars.next(); // consume second '}'
                            if !var_name.is_empty() && !variables.contains(&var_name) {
                                variables.push(var_name);
                            }
                            break;
                        }
                    } else {
                        var_name.push(c);
                        chars.next();
                    }
                }
            }
        }
    }

    variables
}

/// Compute line-by-line diff between two templates
fn compute_diff(template1: &str, template2: &str) -> Vec<DiffLine> {
    let lines1: Vec<&str> = template1.lines().collect();
    let lines2: Vec<&str> = template2.lines().collect();

    let mut diff = Vec::new();

    // Simple LCS-based diff (for production, use a proper diff library)
    let mut i = 0;
    let mut j = 0;

    while i < lines1.len() || j < lines2.len() {
        if i < lines1.len() && j < lines2.len() && lines1[i] == lines2[j] {
            diff.push(DiffLine {
                line_type: "unchanged".to_string(),
                content: lines1[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if i < lines1.len() && (j >= lines2.len() || !lines2.contains(&lines1[i])) {
            diff.push(DiffLine {
                line_type: "removed".to_string(),
                content: format!("- {}", lines1[i]),
            });
            i += 1;
        } else if j < lines2.len() {
            diff.push(DiffLine {
                line_type: "added".to_string(),
                content: format!("+ {}", lines2[j]),
            });
            j += 1;
        }
    }

    diff
}

fn render_template(template: &str, variables: &HashMap<String, String>) -> String {
    let mut result = template.to_string();

    for (key, value) in variables {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }

    result
}

fn prompt_to_response(prompt: &PromptTemplate) -> PromptResponse {
    PromptResponse {
        id: format!("0x{:x}", prompt.id),
        name: prompt.name.clone(),
        description: prompt.description.clone(),
        template: prompt.template.clone(),
        variables: prompt.variables.clone(),
        tags: prompt.tags.clone(),
        version: prompt.version,
        created_at: prompt.created_at,
        updated_at: prompt.updated_at,
        created_by: prompt.created_by.clone(),
    }
}

// ============================================================================
// API Handlers
// ============================================================================

/// POST /api/v1/prompts
/// Create a new prompt template
pub async fn create_prompt(
    State(state): State<AppState>,
    Json(req): Json<CreatePromptRequest>,
) -> Result<(StatusCode, Json<PromptResponse>), (StatusCode, String)> {
    let prompt_id = generate_id();
    let timestamp = current_timestamp_us();

    let mut prompt = PromptTemplate::new(
        prompt_id,
        req.name,
        req.template,
        "api-user".to_string(), // TODO: Get from auth context
        timestamp,
    );

    if let Some(desc) = req.description {
        prompt.description = desc;
    }
    prompt.tags = req.tags;
    prompt.metadata = req.metadata;

    // Convert API type to Core type for storage
    let core_prompt: CorePromptTemplate = prompt.clone().into();
    state
        .db
        .store_prompt_template(core_prompt)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(prompt_to_response(&prompt))))
}

/// GET /api/v1/prompts
/// List all prompt templates
pub async fn list_prompts(
    State(state): State<AppState>,
    Query(params): Query<ListPromptsQuery>,
) -> Result<Json<PromptListResponse>, (StatusCode, String)> {
    let core_prompts = state
        .db
        .list_prompt_templates()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Convert from Core to API types
    let mut prompts: Vec<PromptTemplate> = core_prompts.into_iter().map(|p| p.into()).collect();

    // Filter by tag if provided
    if let Some(tag) = params.tag {
        prompts.retain(|p| p.tags.contains(&tag));
    }

    // Filter by search term if provided
    if let Some(search) = params.search {
        let search_lower = search.to_lowercase();
        prompts.retain(|p| {
            p.name.to_lowercase().contains(&search_lower)
                || p.description.to_lowercase().contains(&search_lower)
        });
    }

    let total = prompts.len();
    let prompt_responses: Vec<PromptResponse> = prompts.iter().map(prompt_to_response).collect();

    Ok(Json(PromptListResponse {
        prompts: prompt_responses,
        total,
    }))
}

/// GET /api/v1/prompts/:id
/// Get a specific prompt template
pub async fn get_prompt(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PromptResponse>, (StatusCode, String)> {
    let prompt_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let core_prompt = state
        .db
        .get_prompt_template(prompt_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "Prompt template not found".to_string(),
            )
        })?;

    let prompt: PromptTemplate = core_prompt.into();
    Ok(Json(prompt_to_response(&prompt)))
}

/// PUT /api/v1/prompts/:id
/// Update a prompt template
pub async fn update_prompt(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePromptRequest>,
) -> Result<Json<PromptResponse>, (StatusCode, String)> {
    let prompt_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let timestamp = current_timestamp_us();

    state
        .db
        .update_prompt_template(prompt_id, |prompt| {
            if let Some(name) = req.name {
                prompt.name = name;
            }
            if let Some(description) = req.description {
                prompt.description = description;
            }
            if let Some(template) = req.template {
                prompt.template = template.clone();
                prompt.variables = extract_variables(&template);
                prompt.version += 1;
            }
            if let Some(tags) = req.tags {
                prompt.tags = tags;
            }
            prompt.updated_at = timestamp;
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Fetch updated prompt
    let core_prompt = state
        .db
        .get_prompt_template(prompt_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "Prompt template not found".to_string(),
            )
        })?;

    let prompt: PromptTemplate = core_prompt.into();
    Ok(Json(prompt_to_response(&prompt)))
}

/// POST /api/v1/prompts/:id/render
/// Render a prompt template with variables
pub async fn render_prompt(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<RenderPromptRequest>,
) -> Result<Json<RenderPromptResponse>, (StatusCode, String)> {
    let prompt_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let prompt = state
        .db
        .get_prompt_template(prompt_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "Prompt template not found".to_string(),
            )
        })?;

    let rendered = render_template(&prompt.template, &req.variables);

    Ok(Json(RenderPromptResponse {
        rendered,
        template_id: format!("0x{:x}", prompt_id),
    }))
}

/// GET /api/v1/prompts/:id/versions
/// Get version history for a prompt template
pub async fn get_prompt_versions(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PromptVersionHistoryResponse>, (StatusCode, String)> {
    let prompt_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Get current prompt to verify it exists
    let current_prompt = state
        .db
        .get_prompt_template(prompt_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "Prompt template not found".to_string(),
            )
        })?;

    // For now, return current version as history
    // In production, this would query a version history table
    let version_responses = vec![PromptVersionResponse {
        version: current_prompt.version,
        template: current_prompt.template.clone(),
        created_at: current_prompt.created_at,
        created_by: current_prompt.created_by.clone(),
        change_summary: "Current version".to_string(),
    }];

    Ok(Json(PromptVersionHistoryResponse {
        prompt_id: format!("0x{:x}", prompt_id),
        versions: version_responses,
        total: 1,
    }))
}

/// GET /api/v1/prompts/:id/diff/:v1/:v2
/// Compare two versions of a prompt template
pub async fn compare_prompt_versions(
    State(state): State<AppState>,
    Path((id, v1, v2)): Path<(String, u32, u32)>,
) -> Result<Json<PromptDiffResponse>, (StatusCode, String)> {
    let prompt_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Get current prompt (in production, query version history table)
    let current = state
        .db
        .get_prompt_template(prompt_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "Prompt template not found".to_string(),
            )
        })?;

    // For now, compare current with itself (stub)
    // In production, fetch specific versions from history table
    let template1 = current.template.clone();
    let template2 = current.template.clone();
    let diff_lines = compute_diff(&template1, &template2);

    Ok(Json(PromptDiffResponse {
        prompt_id: format!("0x{:x}", prompt_id),
        version1: v1,
        version2: v2,
        diff: diff_lines,
        template1,
        template2,
    }))
}

/// POST /api/v1/prompts/:id/rollback/:version
/// Rollback to a specific version of a prompt template
pub async fn rollback_prompt_version(
    State(state): State<AppState>,
    Path((id, _version)): Path<(String, u32)>,
) -> Result<Json<PromptResponse>, (StatusCode, String)> {
    let prompt_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Get current prompt (in production, fetch target version from history table)
    state
        .db
        .get_prompt_template(prompt_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "Prompt template not found".to_string(),
            )
        })?;

    let timestamp = current_timestamp_us();

    state
        .db
        .update_prompt_template(prompt_id, |prompt| {
            // In production: prompt.template = target_version.template.clone();
            prompt.variables = extract_variables(&prompt.template);
            prompt.version += 1;
            prompt.updated_at = timestamp;
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Get the updated prompt
    let core_prompt = state
        .db
        .get_prompt_template(prompt_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "Prompt template not found".to_string(),
            )
        })?;

    let prompt: PromptTemplate = core_prompt.into();
    Ok(Json(prompt_to_response(&prompt)))
}

/// GET /api/v1/prompts/:id/performance
/// Get performance metrics across versions
pub async fn get_prompt_performance(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PromptPerformanceResponse>, (StatusCode, String)> {
    let prompt_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Verify prompt exists
    state
        .db
        .get_prompt_template(prompt_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "Prompt template not found".to_string(),
            )
        })?;

    // In production, query evaluation results linked to this prompt
    // For now, return stub metrics
    let version_metrics = vec![VersionPerformanceMetrics {
        version: 1,
        avg_score: 0.85,
        eval_count: 100,
        success_rate: 0.95,
        avg_latency_ms: 250.0,
        avg_cost: 0.002,
    }];

    Ok(Json(PromptPerformanceResponse {
        prompt_id: format!("0x{:x}", prompt_id),
        metrics: version_metrics,
    }))
}

/// DELETE /api/v1/prompts/:id
/// Delete a prompt template
pub async fn delete_prompt(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, String)> {
    let prompt_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let deleted = state
        .db
        .delete_prompt_template(prompt_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(DeleteResponse {
            success: true,
            message: "Prompt template deleted successfully".to_string(),
        }))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            "Prompt template not found".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_variables() {
        let template = "Hello {{name}}, your age is {{age}}!";
        let vars = extract_variables(template);
        assert_eq!(vars, vec!["name", "age"]);

        let template2 = "No variables here";
        let vars2 = extract_variables(template2);
        assert_eq!(vars2.len(), 0);
    }

    #[test]
    fn test_render_template() {
        let template = "Hello {{name}}, your age is {{age}}!";
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Alice".to_string());
        vars.insert("age".to_string(), "30".to_string());

        let rendered = render_template(template, &vars);
        assert_eq!(rendered, "Hello Alice, your age is 30!");
    }

    #[test]
    fn test_parse_id() {
        assert_eq!(parse_id("0x123").unwrap(), 0x123);
        assert_eq!(parse_id("123").unwrap(), 0x123);
    }
}
