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

//! OpenTelemetry Event support for GenAI traces
//!
//! Events are span-level records used for structured logging of prompts, completions,
//! and other temporal data points within a span's lifecycle.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Span Event per OpenTelemetry specification
///
/// Events are used to log structured data associated with a span, especially
/// for GenAI use cases where prompts and completions should be stored as
/// events rather than attributes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpanEvent {
    /// Event name (e.g., "gen_ai.content.prompt", "gen_ai.content.completion")
    pub name: String,

    /// Timestamp in microseconds since Unix epoch
    pub timestamp_us: u64,

    /// Event attributes as key-value pairs
    pub attributes: HashMap<String, String>,
}

impl SpanEvent {
    /// Create a new span event
    pub fn new(name: String, timestamp_us: u64) -> Self {
        Self {
            name,
            timestamp_us,
            attributes: HashMap::new(),
        }
    }

    /// Create a GenAI prompt event
    pub fn gen_ai_prompt(timestamp_us: u64, content: String, role: String, index: u32) -> Self {
        let mut event = Self::new("gen_ai.content.prompt".to_string(), timestamp_us);
        event
            .attributes
            .insert("gen_ai.prompt".to_string(), content);
        event
            .attributes
            .insert("gen_ai.content.role".to_string(), role);
        event
            .attributes
            .insert("gen_ai.content.index".to_string(), index.to_string());
        event
    }

    /// Create a GenAI completion event
    pub fn gen_ai_completion(
        timestamp_us: u64,
        content: String,
        role: String,
        finish_reason: Option<String>,
    ) -> Self {
        let mut event = Self::new("gen_ai.content.completion".to_string(), timestamp_us);
        event
            .attributes
            .insert("gen_ai.completion".to_string(), content);
        event
            .attributes
            .insert("gen_ai.content.role".to_string(), role);
        if let Some(reason) = finish_reason {
            event
                .attributes
                .insert("gen_ai.completion.finish_reason".to_string(), reason);
        }
        event
    }

    /// Get event content (prompt or completion text)
    pub fn get_content(&self) -> Option<&str> {
        self.attributes
            .get("gen_ai.prompt")
            .or_else(|| self.attributes.get("gen_ai.completion"))
            .map(|s| s.as_str())
    }

    /// Get role (user, assistant, system, tool)
    pub fn get_role(&self) -> Option<&str> {
        self.attributes
            .get("gen_ai.content.role")
            .map(|s| s.as_str())
    }

    /// Check if this is a prompt event
    pub fn is_prompt(&self) -> bool {
        self.name == "gen_ai.content.prompt"
    }

    /// Check if this is a completion event
    pub fn is_completion(&self) -> bool {
        self.name == "gen_ai.content.completion"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_prompt_event() {
        let event = SpanEvent::gen_ai_prompt(
            1000000,
            "What is the weather?".to_string(),
            "user".to_string(),
            0,
        );

        assert_eq!(event.name, "gen_ai.content.prompt");
        assert_eq!(event.get_content(), Some("What is the weather?"));
        assert_eq!(event.get_role(), Some("user"));
        assert!(event.is_prompt());
        assert!(!event.is_completion());
    }

    #[test]
    fn test_create_completion_event() {
        let event = SpanEvent::gen_ai_completion(
            2000000,
            "It's sunny!".to_string(),
            "assistant".to_string(),
            Some("stop".to_string()),
        );

        assert_eq!(event.name, "gen_ai.content.completion");
        assert_eq!(event.get_content(), Some("It's sunny!"));
        assert_eq!(event.get_role(), Some("assistant"));
        assert!(!event.is_prompt());
        assert!(event.is_completion());
    }
}
