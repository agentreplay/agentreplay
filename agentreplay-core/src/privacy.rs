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

//! Privacy Tag Processing
//!
//! Sensitive content redaction via `<private>...</private>` tags.
//!
//! # Usage
//!
//! Users can mark sensitive content in their prompts using privacy tags:
//!
//! ```text
//! Please help me fix this bug.
//! <private>My API key is sk-abc123</private>
//! The error occurs when...
//! ```
//!
//! Content within `<private>` tags is stripped before storage, replaced with
//! `[REDACTED]`. This enables users to exclude sensitive information from memory.

use serde::{Deserialize, Serialize};

mod processor;

pub use processor::{
    MalformedKind, MalformedTag, PrivacyMetadata, PrivacyProcessorConfig, PrivacyTagProcessor,
    RedactedRegion,
};

/// Result of privacy tag processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyResult {
    /// The content with private sections redacted.
    pub redacted_content: String,
    /// Whether any private content was found and redacted.
    pub had_private_content: bool,
    /// Number of private sections that were redacted.
    pub redacted_count: usize,
    /// Whether the entire content was private (should skip observation).
    pub entirely_private: bool,
}

/// Strip private content from text, replacing with [REDACTED].
///
/// # Arguments
///
/// * `text` - The input text potentially containing `<private>` tags
///
/// # Returns
///
/// A tuple of (stripped_text, had_private_content)
///
/// # Example
///
/// ```
/// use agentreplay_core::privacy::strip_private_content;
///
/// let (stripped, had_private) = strip_private_content("Hello <private>secret</private> world");
/// assert_eq!(stripped, "Hello [REDACTED] world");
/// assert!(had_private);
/// ```
pub fn strip_private_content(text: &str) -> (String, bool) {
    let result = process_privacy_tags(text);
    (result.redacted_content, result.had_private_content)
}

/// Process privacy tags with full result information.
///
/// # Example
///
/// ```
/// use agentreplay_core::privacy::process_privacy_tags;
///
/// let result = process_privacy_tags("Key: <private>secret123</private>");
/// assert!(result.had_private_content);
/// assert_eq!(result.redacted_count, 1);
/// assert!(!result.entirely_private);
/// ```
pub fn process_privacy_tags(text: &str) -> PrivacyResult {
    let mut result = String::with_capacity(text.len());
    let mut had_private = false;
    let mut redacted_count = 0;
    let mut remaining = text;
    let mut public_content_len = 0;

    const OPEN_TAG: &str = "<private>";
    const CLOSE_TAG: &str = "</private>";

    while let Some(start) = remaining.find(OPEN_TAG) {
        // Add content before the private tag
        let before = &remaining[..start];
        public_content_len += before.trim().len();
        result.push_str(before);

        // Find the closing tag
        let after_open = &remaining[start + OPEN_TAG.len()..];
        match after_open.find(CLOSE_TAG) {
            Some(end) => {
                result.push_str("[REDACTED]");
                had_private = true;
                redacted_count += 1;
                remaining = &after_open[end + CLOSE_TAG.len()..];
            }
            None => {
                // No closing tag found, treat rest as private
                result.push_str("[REDACTED]");
                had_private = true;
                redacted_count += 1;
                remaining = "";
                break;
            }
        }
    }

    // Add any remaining content
    public_content_len += remaining.trim().len();
    result.push_str(remaining);

    // Determine if entirely private
    let entirely_private = had_private && public_content_len == 0;

    PrivacyResult {
        redacted_content: result,
        had_private_content: had_private,
        redacted_count,
        entirely_private,
    }
}

/// Check if text contains any private content without processing.
///
/// This is a fast check that doesn't allocate memory.
pub fn has_private_content(text: &str) -> bool {
    text.contains("<private>")
}

/// Check if text is entirely private (should skip observation generation).
pub fn is_entirely_private(text: &str) -> bool {
    process_privacy_tags(text).entirely_private
}

/// Validate privacy tag syntax.
///
/// Returns true if all `<private>` tags have matching `</private>` tags.
pub fn validate_privacy_tags(text: &str) -> bool {
    let open_count = text.matches("<private>").count();
    let close_count = text.matches("</private>").count();
    open_count == close_count
}

/// Privacy-aware text processor for observations.
pub struct PrivacyProcessor {
    /// Replacement text for redacted content.
    replacement: String,
}

impl Default for PrivacyProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivacyProcessor {
    /// Create a new privacy processor with default settings.
    pub fn new() -> Self {
        Self {
            replacement: "[REDACTED]".to_string(),
        }
    }

    /// Create a processor with custom replacement text.
    pub fn with_replacement(replacement: impl Into<String>) -> Self {
        Self {
            replacement: replacement.into(),
        }
    }

    /// Process text, redacting private content.
    pub fn process(&self, text: &str) -> PrivacyResult {
        let mut result = String::with_capacity(text.len());
        let mut had_private = false;
        let mut redacted_count = 0;
        let mut remaining = text;
        let mut public_content_len = 0;

        const OPEN_TAG: &str = "<private>";
        const CLOSE_TAG: &str = "</private>";

        while let Some(start) = remaining.find(OPEN_TAG) {
            let before = &remaining[..start];
            public_content_len += before.trim().len();
            result.push_str(before);

            let after_open = &remaining[start + OPEN_TAG.len()..];
            match after_open.find(CLOSE_TAG) {
                Some(end) => {
                    result.push_str(&self.replacement);
                    had_private = true;
                    redacted_count += 1;
                    remaining = &after_open[end + CLOSE_TAG.len()..];
                }
                None => {
                    result.push_str(&self.replacement);
                    had_private = true;
                    redacted_count += 1;
                    remaining = "";
                    break;
                }
            }
        }

        public_content_len += remaining.trim().len();
        result.push_str(remaining);

        let entirely_private = had_private && public_content_len == 0;

        PrivacyResult {
            redacted_content: result,
            had_private_content: had_private,
            redacted_count,
            entirely_private,
        }
    }

    /// Process and return only the redacted content.
    pub fn redact(&self, text: &str) -> String {
        self.process(text).redacted_content
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_private_content() {
        let (stripped, had_private) = strip_private_content("Hello world");
        assert_eq!(stripped, "Hello world");
        assert!(!had_private);
    }

    #[test]
    fn test_single_private_section() {
        let (stripped, had_private) =
            strip_private_content("Key: <private>secret123</private> done");
        assert_eq!(stripped, "Key: [REDACTED] done");
        assert!(had_private);
    }

    #[test]
    fn test_multiple_private_sections() {
        let (stripped, had_private) = strip_private_content(
            "User: <private>john</private> Key: <private>abc</private>",
        );
        assert_eq!(stripped, "User: [REDACTED] Key: [REDACTED]");
        assert!(had_private);
    }

    #[test]
    fn test_nested_content() {
        let result = process_privacy_tags("<private>all secret</private>");
        assert!(result.entirely_private);
        assert_eq!(result.redacted_count, 1);
    }

    #[test]
    fn test_unclosed_tag() {
        let (stripped, had_private) = strip_private_content("Before <private>secret");
        assert_eq!(stripped, "Before [REDACTED]");
        assert!(had_private);
    }

    #[test]
    fn test_has_private_content() {
        assert!(has_private_content("Hello <private>secret</private>"));
        assert!(!has_private_content("Hello world"));
    }

    #[test]
    fn test_validate_tags() {
        assert!(validate_privacy_tags("Hello <private>x</private> world"));
        assert!(validate_privacy_tags("No tags here"));
        assert!(!validate_privacy_tags("Missing close <private>x"));
    }

    #[test]
    fn test_custom_replacement() {
        let processor = PrivacyProcessor::with_replacement("***");
        let result = processor.process("<private>secret</private>");
        assert_eq!(result.redacted_content, "***");
    }

    #[test]
    fn test_privacy_result_fields() {
        let result = process_privacy_tags("A <private>x</private> B <private>y</private> C");
        assert!(result.had_private_content);
        assert_eq!(result.redacted_count, 2);
        assert!(!result.entirely_private);
        assert_eq!(result.redacted_content, "A [REDACTED] B [REDACTED] C");
    }
}
