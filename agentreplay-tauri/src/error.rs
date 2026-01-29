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

//! Structured error types for Tauri IPC commands
//!
//! This module provides rich error types that include error codes, categories,
//! and recovery suggestions for better user experience in desktop applications.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Error categories for grouping related errors
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// Database-related errors (read/write failures, corruption)
    Database,
    /// Configuration errors (invalid settings, missing config)
    Configuration,
    /// Network errors (server unreachable, timeout)
    Network,
    /// Validation errors (invalid input, out of range)
    Validation,
    /// Resource errors (file not found, permission denied)
    Resource,
    /// Internal errors (unexpected state, panic recovery)
    Internal,
    /// Authentication/authorization errors
    Auth,
    /// Plugin-related errors
    Plugin,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorCategory::Database => write!(f, "Database Error"),
            ErrorCategory::Configuration => write!(f, "Configuration Error"),
            ErrorCategory::Network => write!(f, "Network Error"),
            ErrorCategory::Validation => write!(f, "Validation Error"),
            ErrorCategory::Resource => write!(f, "Resource Error"),
            ErrorCategory::Internal => write!(f, "Internal Error"),
            ErrorCategory::Auth => write!(f, "Authentication Error"),
            ErrorCategory::Plugin => write!(f, "Plugin Error"),
        }
    }
}

/// Unique error codes for programmatic error handling in the UI
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    // Database errors (1xxx)
    DatabaseConnectionFailed = 1001,
    DatabaseQueryFailed = 1002,
    DatabaseWriteFailed = 1003,
    DatabaseCorrupted = 1004,
    DatabaseLocked = 1005,

    // Configuration errors (2xxx)
    ConfigLoadFailed = 2001,
    ConfigSaveFailed = 2002,
    ConfigInvalid = 2003,
    ConfigMissing = 2004,

    // Network errors (3xxx)
    NetworkUnreachable = 3001,
    NetworkTimeout = 3002,
    NetworkConnectionRefused = 3003,
    ServerError = 3004,

    // Validation errors (4xxx)
    ValidationFailed = 4001,
    InvalidInput = 4002,
    OutOfRange = 4003,
    MissingRequired = 4004,
    InvalidFormat = 4005,

    // Resource errors (5xxx)
    FileNotFound = 5001,
    PermissionDenied = 5002,
    ResourceBusy = 5003,
    DiskFull = 5004,

    // Internal errors (6xxx)
    InternalError = 6001,
    UnexpectedState = 6002,
    NotImplemented = 6003,
    Cancelled = 6004,

    // Auth errors (7xxx)
    Unauthorized = 7001,
    Forbidden = 7002,
    TokenExpired = 7003,

    // Plugin errors (8xxx)
    PluginLoadFailed = 8001,
    PluginExecutionFailed = 8002,
    PluginNotFound = 8003,
}

impl ErrorCode {
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }
}

/// Structured error type for Tauri commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandError {
    /// Unique error code for programmatic handling
    pub code: ErrorCode,
    /// Error category for grouping
    pub category: ErrorCategory,
    /// Human-readable error message
    pub message: String,
    /// Detailed error description (for logs/debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// Suggested recovery actions for the user
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub recovery_suggestions: Vec<String>,
    /// Whether this error is recoverable
    pub recoverable: bool,
    /// Whether user should retry the operation
    pub retryable: bool,
}

impl CommandError {
    /// Create a new CommandError
    pub fn new(code: ErrorCode, category: ErrorCategory, message: impl Into<String>) -> Self {
        Self {
            code,
            category,
            message: message.into(),
            details: None,
            recovery_suggestions: Vec::new(),
            recoverable: true,
            retryable: false,
        }
    }

    /// Add detailed error information
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Add recovery suggestions
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.recovery_suggestions.push(suggestion.into());
        self
    }

    /// Mark error as non-recoverable
    pub fn non_recoverable(mut self) -> Self {
        self.recoverable = false;
        self
    }

    /// Mark error as retryable
    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    // =========================================================================
    // Convenience constructors for common errors
    // =========================================================================

    /// Database query failed
    pub fn database_query(err: impl fmt::Display) -> Self {
        Self::new(
            ErrorCode::DatabaseQueryFailed,
            ErrorCategory::Database,
            "Failed to query database",
        )
        .with_details(err.to_string())
        .with_suggestion("Check if the database file is accessible")
        .with_suggestion("Try restarting the application")
        .retryable()
    }

    /// Database write failed
    pub fn database_write(err: impl fmt::Display) -> Self {
        Self::new(
            ErrorCode::DatabaseWriteFailed,
            ErrorCategory::Database,
            "Failed to write to database",
        )
        .with_details(err.to_string())
        .with_suggestion("Check if you have write permissions")
        .with_suggestion("Ensure sufficient disk space is available")
        .retryable()
    }

    /// Configuration save failed
    pub fn config_save(err: impl fmt::Display) -> Self {
        Self::new(
            ErrorCode::ConfigSaveFailed,
            ErrorCategory::Configuration,
            "Failed to save configuration",
        )
        .with_details(err.to_string())
        .with_suggestion("Check file permissions")
        .retryable()
    }

    /// Invalid input validation
    pub fn invalid_input(field: &str, reason: &str) -> Self {
        Self::new(
            ErrorCode::InvalidInput,
            ErrorCategory::Validation,
            format!("Invalid value for '{}': {}", field, reason),
        )
        .with_suggestion(format!("Please provide a valid value for '{}'", field))
    }

    /// Required field missing
    pub fn missing_required(field: &str) -> Self {
        Self::new(
            ErrorCode::MissingRequired,
            ErrorCategory::Validation,
            format!("Required field '{}' is missing", field),
        )
        .with_suggestion(format!("Please provide a value for '{}'", field))
    }

    /// Value out of range
    pub fn out_of_range(field: &str, min: impl fmt::Display, max: impl fmt::Display) -> Self {
        Self::new(
            ErrorCode::OutOfRange,
            ErrorCategory::Validation,
            format!("Value for '{}' is out of range", field),
        )
        .with_details(format!("Must be between {} and {}", min, max))
        .with_suggestion(format!(
            "Please provide a value between {} and {}",
            min, max
        ))
    }

    /// Resource not found
    pub fn not_found(resource: &str) -> Self {
        Self::new(
            ErrorCode::FileNotFound,
            ErrorCategory::Resource,
            format!("{} not found", resource),
        )
        .with_suggestion("Verify the resource exists")
    }

    /// Internal error
    pub fn internal(err: impl fmt::Display) -> Self {
        Self::new(
            ErrorCode::InternalError,
            ErrorCategory::Internal,
            "An internal error occurred",
        )
        .with_details(err.to_string())
        .with_suggestion("Please try again or restart the application")
        .with_suggestion("If the problem persists, check the logs")
    }

    /// Network unreachable
    pub fn network_unreachable(target: &str) -> Self {
        Self::new(
            ErrorCode::NetworkUnreachable,
            ErrorCategory::Network,
            format!("Cannot reach {}", target),
        )
        .with_suggestion("Check your internet connection")
        .with_suggestion("Verify the server address is correct")
        .retryable()
    }

    /// Plugin error
    pub fn plugin_error(plugin_name: &str, err: impl fmt::Display) -> Self {
        Self::new(
            ErrorCode::PluginExecutionFailed,
            ErrorCategory::Plugin,
            format!("Plugin '{}' failed", plugin_name),
        )
        .with_details(err.to_string())
        .with_suggestion("Check if the plugin is installed correctly")
        .with_suggestion("Try reinstalling the plugin")
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.code.as_u32(), self.category, self.message)
    }
}

impl std::error::Error for CommandError {}

/// Result type alias for Tauri commands
pub type CommandResult<T> = Result<T, CommandError>;

/// Trait for converting errors to CommandError
pub trait IntoCommandError {
    fn into_command_error(self, category: ErrorCategory, code: ErrorCode) -> CommandError;
}

impl<E: fmt::Display> IntoCommandError for E {
    fn into_command_error(self, category: ErrorCategory, code: ErrorCode) -> CommandError {
        CommandError::new(code, category, self.to_string())
    }
}

/// Extension trait for Result to map errors to CommandError
pub trait CommandResultExt<T, E> {
    fn map_cmd_err(
        self,
        category: ErrorCategory,
        code: ErrorCode,
        message: &str,
    ) -> CommandResult<T>;
}

impl<T, E: fmt::Display> CommandResultExt<T, E> for Result<T, E> {
    fn map_cmd_err(
        self,
        category: ErrorCategory,
        code: ErrorCode,
        message: &str,
    ) -> CommandResult<T> {
        self.map_err(|e| {
            CommandError::new(code, category, message).with_details(e.to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_serialization() {
        let error = CommandError::database_query("connection timeout");
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"code\":\"DatabaseQueryFailed\"") || json.contains("1002"));
        assert!(json.contains("database"));
    }

    #[test]
    fn test_error_with_suggestions() {
        let error = CommandError::invalid_input("email", "invalid format")
            .with_suggestion("Use format: user@example.com");
        
        assert_eq!(error.recovery_suggestions.len(), 2);
        assert!(!error.retryable);
    }
}
