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

//! Span Type Mapping
//!
//! Maps custom SpanType enum to OTEL GenAI operations and vice versa.

use crate::genai_conventions::Operation;

/// Map custom SpanType to OTEL GenAI Operation
pub fn span_type_to_operation(span_type: u64) -> Operation {
    match span_type {
        0 => Operation::CreateAgent,     // Root
        1 => Operation::Chat,            // Planning
        2 => Operation::Chat,            // Reasoning
        3 => Operation::ExecuteTool,     // ToolCall
        4 => Operation::ExecuteTool,     // ToolResponse
        5 => Operation::Chat,            // Synthesis
        6 => Operation::InvokeAgent,     // Response
        7 => Operation::Chat,            // Error
        8 => Operation::Chat,            // Retrieval (could be custom)
        9 => Operation::Embeddings,      // Embedding
        10 => Operation::ExecuteTool,    // HttpCall
        11 => Operation::ExecuteTool,    // Database
        12 => Operation::ExecuteTool,    // Function
        13 => Operation::Chat,           // Reranking
        14 => Operation::Chat,           // Parsing
        15 => Operation::TextCompletion, // Generation
        _ => Operation::Chat,            // Custom/Default
    }
}

/// Map OTEL GenAI Operation to custom SpanType value
pub fn operation_to_span_type(operation: Operation) -> u64 {
    match operation {
        Operation::Chat => 1,            // Planning
        Operation::TextCompletion => 15, // Generation
        Operation::Embeddings => 9,      // Embedding
        Operation::CreateAgent => 0,     // Root
        Operation::InvokeAgent => 6,     // Response
        Operation::ExecuteTool => 3,     // ToolCall
    }
}

/// Infer operation from span name (fallback heuristic)
pub fn infer_operation_from_name(name: &str) -> Operation {
    let lower = name.to_lowercase();
    if lower.contains("chat") || lower.contains("llm") {
        Operation::Chat
    } else if lower.contains("tool") || lower.contains("function") {
        Operation::ExecuteTool
    } else if lower.contains("embed") {
        Operation::Embeddings
    } else if lower.contains("agent") {
        Operation::InvokeAgent
    } else if lower.contains("completion") {
        Operation::TextCompletion
    } else {
        Operation::Chat // Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_type_mapping() {
        assert_eq!(span_type_to_operation(1), Operation::Chat);
        assert_eq!(span_type_to_operation(3), Operation::ExecuteTool);
        assert_eq!(span_type_to_operation(9), Operation::Embeddings);
    }

    #[test]
    fn test_operation_mapping() {
        assert_eq!(operation_to_span_type(Operation::Chat), 1);
        assert_eq!(operation_to_span_type(Operation::ExecuteTool), 3);
        assert_eq!(operation_to_span_type(Operation::Embeddings), 9);
    }

    #[test]
    fn test_name_inference() {
        assert_eq!(
            infer_operation_from_name("chat completion"),
            Operation::Chat
        );
        assert_eq!(
            infer_operation_from_name("tool execution"),
            Operation::ExecuteTool
        );
        assert_eq!(
            infer_operation_from_name("embedding generation"),
            Operation::Embeddings
        );
    }
}
