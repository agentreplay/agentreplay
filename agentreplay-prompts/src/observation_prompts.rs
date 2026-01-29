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

//! Observation Generation Prompts
//!
//! System and user prompts for memory agent LLM calls.
//!
//! # Prompt Types
//!
//! - **Init Prompt**: First turn to establish session context
//! - **Observation Prompt**: Generate observations from tool events
//! - **Summary Prompt**: End-of-session summary generation

use serde::{Deserialize, Serialize};

/// Prompt configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    /// Model to use for generation.
    pub model: String,
    /// Maximum tokens for response.
    pub max_tokens: usize,
    /// Temperature for generation.
    pub temperature: f32,
    /// Whether to use streaming.
    pub stream: bool,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 4096,
            temperature: 0.3,
            stream: true,
        }
    }
}

/// Build the initialization prompt for starting a memory session.
///
/// This establishes the system prompt and provides initial context
/// about the observation format and guidelines.
pub fn build_init_prompt(project_context: &str, session_context: &str) -> String {
    format!(
        r#"You are an expert observer of software development activities. Your task is to generate high-quality observations that capture key learnings, decisions, and discoveries from coding sessions.

## Project Context

{project_context}

## Session Context

{session_context}

## Your Role

As you receive tool events and interactions, generate observations that:
1. Capture the developer's intent and approach
2. Record key technical decisions and their rationale
3. Note important discoveries about the codebase
4. Identify patterns and connections between concepts

## Output Format

Generate observations in this XML format:

```xml
<observation>
  <type>[implementation|debugging|refactoring|testing|architecture|design|research|documentation|configuration|review|learning|planning]</type>
  <title>[Brief, descriptive title (3-7 words)]</title>
  <subtitle>[Optional: more specific context]</subtitle>
  <facts>
    <fact>[Concrete, verifiable fact learned]</fact>
    <fact>[Another fact...]</fact>
  </facts>
  <narrative>[2-3 sentences explaining the observation with context]</narrative>
  <concepts>
    <concept>[key-concept-1]</concept>
    <concept>[key-concept-2]</concept>
  </concepts>
  <files>
    <read>[path/to/file.rs]</read>
    <modified>[path/to/changed.rs]</modified>
  </files>
</observation>
```

## Guidelines

- Be specific and actionable, not vague
- Focus on insights that would help someone unfamiliar with this work
- Extract patterns and connections
- Omit private content marked with <private> tags
- Use lowercase-hyphenated concepts for indexing
- Include file paths when relevant"#,
    )
}

/// Build a prompt for generating an observation from tool events.
pub fn build_observation_prompt(
    tool_events: &[ToolEventInput],
    prior_context: &str,
    observation_count: usize,
) -> String {
    let events_text = tool_events
        .iter()
        .map(|e| format_tool_event(e))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    format!(
        r#"## Prior Context

{prior_context}

## New Tool Events

The following tool events occurred. Analyze them and generate an observation if there's something worth capturing.

{events_text}

## Instructions

Based on these events, generate a single observation that captures the most significant learning or activity. This will be observation #{observation_count} in this session.

If the events don't contain anything worth observing (e.g., trivial changes, repeated actions), respond with:

```xml
<no_observation reason="[brief explanation]" />
```

Otherwise, generate an observation following the XML format specified earlier."#,
        prior_context = prior_context,
        events_text = events_text,
        observation_count = observation_count + 1
    )
}

/// Build a prompt for generating an end-of-session summary.
pub fn build_summary_prompt(
    observations: &[ObservationSummaryInput],
    session_duration_ms: u64,
    user_prompts: &[String],
) -> String {
    let obs_text = observations
        .iter()
        .enumerate()
        .map(|(i, o)| format!("{}. **{}**: {}", i + 1, o.title, o.narrative))
        .collect::<Vec<_>>()
        .join("\n");

    let prompts_text = user_prompts
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{}. {}", i + 1, p))
        .collect::<Vec<_>>()
        .join("\n");

    let duration_min = session_duration_ms / 60_000;

    format!(
        r#"## Session Summary Request

Generate an end-of-session summary capturing the overall progress and learnings.

## Session Duration

{duration_min} minutes

## User Prompts/Requests

{prompts_text}

## Observations Generated

{obs_text}

## Output Format

Generate a summary in this XML format:

```xml
<session_summary>
  <request>[Original request or goal the user was trying to accomplish]</request>
  <investigated>[Key areas explored or researched]</investigated>
  <learned>[Most important learnings from this session]</learned>
  <completed>[What was successfully accomplished]</completed>
  <next_steps>
    <step>[Suggested follow-up action 1]</step>
    <step>[Suggested follow-up action 2]</step>
  </next_steps>
</session_summary>
```

Focus on actionable insights and clear progress tracking."#,
        duration_min = duration_min,
        prompts_text = prompts_text,
        obs_text = obs_text
    )
}

/// Build a prompt for multi-turn continuation.
pub fn build_continuation_prompt(
    new_events: &[ToolEventInput],
    conversation_summary: &str,
) -> String {
    let events_text = new_events
        .iter()
        .map(|e| format_tool_event(e))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    format!(
        r#"## Conversation Context

{conversation_summary}

## New Events

{events_text}

## Instructions

Continue observing and generate observations for these new events. Maintain context from previous observations in this session."#,
        conversation_summary = conversation_summary,
        events_text = events_text
    )
}

/// Input structure for tool events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEventInput {
    /// Tool name.
    pub tool_name: String,
    /// Tool input (may be truncated).
    pub tool_input: String,
    /// Tool output (may be truncated).
    pub tool_output: Option<String>,
    /// Files involved.
    pub files: Vec<String>,
    /// Timestamp.
    pub timestamp: u64,
    /// Whether this was successful.
    pub success: bool,
}

/// Input structure for observation summaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationSummaryInput {
    /// Observation title.
    pub title: String,
    /// Observation narrative.
    pub narrative: String,
    /// Observation type.
    pub observation_type: String,
    /// Concepts extracted.
    pub concepts: Vec<String>,
}

fn format_tool_event(event: &ToolEventInput) -> String {
    let status = if event.success { "✓" } else { "✗" };
    let files = if event.files.is_empty() {
        String::new()
    } else {
        format!("\nFiles: {}", event.files.join(", "))
    };

    let output = event
        .tool_output
        .as_ref()
        .map(|o| format!("\nOutput: {}", truncate_string(o, 500)))
        .unwrap_or_default();

    format!(
        "[{status}] {tool_name}\nInput: {input}{files}{output}",
        status = status,
        tool_name = event.tool_name,
        input = truncate_string(&event.tool_input, 500),
        files = files,
        output = output
    )
}

fn truncate_string(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        let end = s
            .char_indices()
            .take_while(|(i, _)| *i <= max_len)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(max_len);
        &s[..end]
    }
}

/// Observation type vocabulary for prompts.
pub const OBSERVATION_TYPES: &[&str] = &[
    "implementation",
    "debugging",
    "refactoring",
    "testing",
    "architecture",
    "design",
    "research",
    "documentation",
    "configuration",
    "review",
    "learning",
    "planning",
];

/// Validate observation type.
pub fn validate_observation_type(t: &str) -> bool {
    OBSERVATION_TYPES.contains(&t.to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_init_prompt() {
        let prompt = build_init_prompt("Test project", "New session");
        assert!(prompt.contains("expert observer"));
        assert!(prompt.contains("Test project"));
        assert!(prompt.contains("<observation>"));
    }

    #[test]
    fn test_build_observation_prompt() {
        let events = vec![ToolEventInput {
            tool_name: "read_file".to_string(),
            tool_input: "src/main.rs".to_string(),
            tool_output: Some("fn main() {}".to_string()),
            files: vec!["src/main.rs".to_string()],
            timestamp: 12345,
            success: true,
        }];

        let prompt = build_observation_prompt(&events, "Prior context", 0);
        assert!(prompt.contains("read_file"));
        assert!(prompt.contains("observation #1"));
    }

    #[test]
    fn test_build_summary_prompt() {
        let observations = vec![ObservationSummaryInput {
            title: "Test obs".to_string(),
            narrative: "Test narrative".to_string(),
            observation_type: "implementation".to_string(),
            concepts: vec!["test".to_string()],
        }];

        let prompt = build_summary_prompt(&observations, 60000, &["Do something".to_string()]);
        assert!(prompt.contains("1 minutes"));
        assert!(prompt.contains("Test obs"));
    }

    #[test]
    fn test_validate_observation_type() {
        assert!(validate_observation_type("implementation"));
        assert!(validate_observation_type("DEBUGGING"));
        assert!(!validate_observation_type("invalid"));
    }
}
