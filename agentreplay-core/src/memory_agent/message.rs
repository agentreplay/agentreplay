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

//! Message types for memory agent conversations.

use serde::{Deserialize, Serialize};

/// Role of a message participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message (instructions).
    System,
    /// User message (input).
    User,
    /// Assistant message (LLM output).
    Assistant,
}

impl MessageRole {
    /// Get the role as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        }
    }
}

/// A single message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender.
    pub role: MessageRole,
    /// Content of the message.
    pub content: String,
    /// Optional name identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Timestamp when the message was created (microseconds since epoch).
    pub timestamp_us: u64,
    /// Estimated token count for this message.
    #[serde(default)]
    pub token_estimate: usize,
}

impl Message {
    /// Create a new system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            name: None,
            timestamp_us: current_timestamp_us(),
            token_estimate: 0,
        }
    }

    /// Create a new user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            name: None,
            timestamp_us: current_timestamp_us(),
            token_estimate: 0,
        }
    }

    /// Create a new assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            name: None,
            timestamp_us: current_timestamp_us(),
            token_estimate: 0,
        }
    }

    /// Set the name for this message.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the token estimate for this message.
    pub fn with_token_estimate(mut self, tokens: usize) -> Self {
        self.token_estimate = tokens;
        self
    }

    /// Estimate token count based on character count.
    /// Rough estimate: ~4 characters per token for English text.
    pub fn estimate_tokens(&mut self) {
        self.token_estimate = (self.content.len() + 3) / 4;
    }
}

/// Conversation history for the memory agent.
///
/// Maintains an append-only conversation history with sliding window
/// summarization when the token budget is exceeded.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversationHistory {
    /// Messages in the conversation.
    messages: Vec<Message>,
    /// Total token count estimate.
    total_tokens: usize,
    /// Token budget for the conversation.
    token_budget: usize,
}

impl ConversationHistory {
    /// Create a new conversation history with the given token budget.
    pub fn new(token_budget: usize) -> Self {
        Self {
            messages: Vec::new(),
            total_tokens: 0,
            token_budget,
        }
    }

    /// Add a message to the history.
    pub fn add(&mut self, mut message: Message) {
        if message.token_estimate == 0 {
            message.estimate_tokens();
        }
        self.total_tokens += message.token_estimate;
        self.messages.push(message);
    }

    /// Add a system message.
    pub fn add_system(&mut self, content: impl Into<String>) {
        self.add(Message::system(content));
    }

    /// Add a user message.
    pub fn add_user(&mut self, content: impl Into<String>) {
        self.add(Message::user(content));
    }

    /// Add an assistant message.
    pub fn add_assistant(&mut self, content: impl Into<String>) {
        self.add(Message::assistant(content));
    }

    /// Get all messages.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get the number of messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if the history is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get the total token estimate.
    pub fn total_tokens(&self) -> usize {
        self.total_tokens
    }

    /// Check if the history exceeds the token budget.
    pub fn exceeds_budget(&self) -> bool {
        self.total_tokens > self.token_budget
    }

    /// Get the last N messages.
    pub fn last_n(&self, n: usize) -> &[Message] {
        if n >= self.messages.len() {
            &self.messages
        } else {
            &self.messages[self.messages.len() - n..]
        }
    }

    /// Summarize older messages to fit within token budget.
    /// Returns the summary text if summarization was performed.
    pub fn summarize_if_needed(&mut self, keep_recent: usize) -> Option<String> {
        if !self.exceeds_budget() || self.messages.len() <= keep_recent {
            return None;
        }

        // Keep system messages and recent messages
        let (system_msgs, other_msgs): (Vec<_>, Vec<_>) = self
            .messages
            .iter()
            .enumerate()
            .partition(|(_, m)| m.role == MessageRole::System);

        // Determine how many messages to summarize
        let to_summarize = other_msgs.len().saturating_sub(keep_recent);
        if to_summarize == 0 {
            return None;
        }

        // Build summary of older messages
        let mut summary_parts = Vec::new();
        for (idx, _) in other_msgs.iter().take(to_summarize) {
            let msg = &self.messages[*idx];
            let role = msg.role.as_str();
            let content_preview = if msg.content.len() > 100 {
                format!("{}...", &msg.content[..100])
            } else {
                msg.content.clone()
            };
            summary_parts.push(format!("[{role}]: {content_preview}"));
        }

        let summary = format!(
            "[Previous {} messages summarized]\n{}",
            to_summarize,
            summary_parts.join("\n")
        );

        // Rebuild messages with summary
        let mut new_messages = Vec::new();

        // Keep system messages
        for (idx, _) in system_msgs {
            new_messages.push(self.messages[idx].clone());
        }

        // Add summary as a system message
        new_messages.push(Message::system(&summary));

        // Add recent messages
        for (idx, _) in other_msgs.iter().skip(to_summarize) {
            new_messages.push(self.messages[*idx].clone());
        }

        // Recalculate tokens
        self.total_tokens = new_messages.iter().map(|m| m.token_estimate).sum();
        self.messages = new_messages;

        Some(summary)
    }

    /// Clear all messages except system messages.
    pub fn clear_non_system(&mut self) {
        self.messages.retain(|m| m.role == MessageRole::System);
        self.total_tokens = self.messages.iter().map(|m| m.token_estimate).sum();
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.total_tokens = 0;
    }
}

fn current_timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::user("Hello").with_name("test_user");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "Hello");
        assert_eq!(msg.name, Some("test_user".to_string()));
    }

    #[test]
    fn test_conversation_history() {
        let mut history = ConversationHistory::new(1000);
        history.add_system("You are a memory agent.");
        history.add_user("Process this event.");
        history.add_assistant("Observation generated.");

        assert_eq!(history.len(), 3);
        assert!(!history.is_empty());
    }

    #[test]
    fn test_token_estimation() {
        let mut msg = Message::user("Hello, world!");
        msg.estimate_tokens();
        assert!(msg.token_estimate > 0);
    }

    #[test]
    fn test_last_n() {
        let mut history = ConversationHistory::new(1000);
        for i in 0..10 {
            history.add_user(format!("Message {}", i));
        }

        let last_3 = history.last_n(3);
        assert_eq!(last_3.len(), 3);
        assert!(last_3[0].content.contains("Message 7"));
    }
}
