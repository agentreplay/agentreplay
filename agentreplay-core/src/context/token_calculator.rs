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

//! Token calculation utilities.
//!
//! Provides token estimation compatible with GPT-4/cl100k_base tokenizer.

/// Token calculator for estimating text token counts.
pub struct TokenCalculator {
    /// Average characters per token (approximately 4 for English text).
    chars_per_token: f64,
}

impl Default for TokenCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCalculator {
    /// Create a new token calculator with default settings.
    pub fn new() -> Self {
        Self {
            chars_per_token: 4.0,
        }
    }

    /// Create a calculator with custom chars per token ratio.
    pub fn with_ratio(chars_per_token: f64) -> Self {
        Self { chars_per_token }
    }

    /// Estimate the number of tokens in a string.
    pub fn estimate(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        ((text.len() as f64) / self.chars_per_token).ceil() as usize
    }

    /// Estimate tokens for multiple strings.
    pub fn estimate_all(&self, texts: &[&str]) -> usize {
        texts.iter().map(|t| self.estimate(t)).sum()
    }

    /// Check if text fits within a token budget.
    pub fn fits_budget(&self, text: &str, budget: usize) -> bool {
        self.estimate(text) <= budget
    }

    /// Truncate text to fit within a token budget.
    pub fn truncate_to_budget(&self, text: &str, budget: usize) -> String {
        let estimated = self.estimate(text);
        if estimated <= budget {
            return text.to_string();
        }

        // Approximate character limit
        let char_limit = (budget as f64 * self.chars_per_token) as usize;
        if char_limit >= text.len() {
            return text.to_string();
        }

        // Find a clean break point (word boundary)
        let truncated = &text[..char_limit.min(text.len())];
        if let Some(last_space) = truncated.rfind(' ') {
            format!("{}...", &truncated[..last_space])
        } else {
            format!("{}...", truncated)
        }
    }

    /// Calculate remaining budget after text.
    pub fn remaining_budget(&self, text: &str, total_budget: usize) -> usize {
        total_budget.saturating_sub(self.estimate(text))
    }
}

/// Estimate tokens using a simple heuristic.
/// ~4 characters per token for English text.
pub fn estimate_tokens(text: &str) -> usize {
    TokenCalculator::new().estimate(text)
}

/// Truncate text to approximately fit a token budget.
pub fn truncate_to_tokens(text: &str, max_tokens: usize) -> String {
    TokenCalculator::new().truncate_to_budget(text, max_tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_estimation() {
        let calc = TokenCalculator::new();

        assert_eq!(calc.estimate(""), 0);
        assert!(calc.estimate("Hello") > 0);

        // ~4 chars per token
        let long_text = "a".repeat(100);
        let tokens = calc.estimate(&long_text);
        assert!(tokens >= 20 && tokens <= 30);
    }

    #[test]
    fn test_fits_budget() {
        let calc = TokenCalculator::new();
        assert!(calc.fits_budget("Hello", 10));
        assert!(!calc.fits_budget("a".repeat(1000).as_str(), 10));
    }

    #[test]
    fn test_truncate() {
        let calc = TokenCalculator::new();
        let long_text = "Hello world this is a very long text that needs truncating";

        let truncated = calc.truncate_to_budget(long_text, 5);
        assert!(truncated.len() < long_text.len());
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_remaining_budget() {
        let calc = TokenCalculator::new();
        let remaining = calc.remaining_budget("Hello", 100);
        assert!(remaining > 90);
    }
}
