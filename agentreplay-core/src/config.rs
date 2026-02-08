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

//! Configuration for Agentreplay core behavior
//!
//! Provides configurable settings for timestamp validation, allowing
//! flexibility for testing, historical data import, and different deployment scenarios.

use serde::{Deserialize, Serialize};

/// Default minimum valid timestamp (January 1, 2020 in microseconds since epoch)
pub const DEFAULT_MIN_TIMESTAMP: u64 = 1_577_836_800_000_000;

/// Default maximum valid timestamp (December 31, 2099 in microseconds since epoch)
pub const DEFAULT_MAX_TIMESTAMP: u64 = 4_102_444_800_000_000;

/// Configuration for timestamp validation
///
/// Allows flexible timestamp bounds for different use cases:
/// - Production: Use defaults (2020-2099) to catch clock errors
/// - Testing: Disable validation or use simple bounds (0-u64::MAX)
/// - Historical: Set min_timestamp to 0 for old data import
/// - Future: Extend max_timestamp beyond 2099 as needed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampConfig {
    /// Minimum valid timestamp in microseconds since epoch.
    /// None = no lower bound (allows any timestamp >= 0)
    pub min_timestamp: Option<u64>,

    /// Maximum valid timestamp in microseconds since epoch.
    /// None = no upper bound (allows any timestamp <= u64::MAX)
    pub max_timestamp: Option<u64>,

    /// Whether to enforce timestamp validation.
    /// If false, all timestamps are accepted (useful for testing)
    pub enforce_validation: bool,
}

impl Default for TimestampConfig {
    fn default() -> Self {
        Self {
            min_timestamp: Some(DEFAULT_MIN_TIMESTAMP),
            max_timestamp: Some(DEFAULT_MAX_TIMESTAMP),
            enforce_validation: true,
        }
    }
}

impl TimestampConfig {
    /// Create a config with no timestamp bounds (accepts all timestamps)
    ///
    /// Useful for testing with simple timestamps like 1000, 2000, etc.
    pub fn unrestricted() -> Self {
        Self {
            min_timestamp: None,
            max_timestamp: None,
            enforce_validation: false,
        }
    }

    /// Create a config for historical data import
    ///
    /// Allows timestamps from Unix epoch (1970) onwards
    pub fn historical() -> Self {
        Self {
            min_timestamp: Some(0),
            max_timestamp: Some(DEFAULT_MAX_TIMESTAMP),
            enforce_validation: true,
        }
    }

    /// Create a config for production use with strict validation
    pub fn production() -> Self {
        Self::default()
    }

    /// Create a config with custom bounds
    pub fn custom(min: Option<u64>, max: Option<u64>) -> Self {
        Self {
            min_timestamp: min,
            max_timestamp: max,
            enforce_validation: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TimestampConfig::default();
        assert_eq!(config.min_timestamp, Some(DEFAULT_MIN_TIMESTAMP));
        assert_eq!(config.max_timestamp, Some(DEFAULT_MAX_TIMESTAMP));
        assert!(config.enforce_validation);
    }

    #[test]
    fn test_unrestricted_config() {
        let config = TimestampConfig::unrestricted();
        assert_eq!(config.min_timestamp, None);
        assert_eq!(config.max_timestamp, None);
        assert!(!config.enforce_validation);
    }

    #[test]
    fn test_historical_config() {
        let config = TimestampConfig::historical();
        assert_eq!(config.min_timestamp, Some(0));
        assert!(config.enforce_validation);
    }
}
