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

//! Input validation for API requests
//!
//! Provides comprehensive validation to protect against malicious or malformed input

use crate::api::query::ApiError;
use std::time::{SystemTime, UNIX_EPOCH};

/// Timestamp validation bounds
/// Minimum valid timestamp (January 1, 2020 in microseconds since epoch)
const MIN_VALID_TIMESTAMP: u64 = 1_577_836_800_000_000;
/// Maximum valid timestamp (December 31, 2099 in microseconds since epoch)
const MAX_VALID_TIMESTAMP: u64 = 4_102_444_800_000_000;

/// Maximum spans allowed per batch ingestion request
/// Increased to 10,000 for high-throughput ingestion scenarios
pub const MAX_SPANS_PER_BATCH: usize = 10_000;

/// Maximum total size of all attributes in a single span
pub const MAX_ATTRIBUTE_SIZE_BYTES: usize = 1_048_576; // 1 MB

/// Maximum size for a single attribute value
pub const MAX_SINGLE_ATTRIBUTE_SIZE: usize = 65_536; // 64 KB

/// Maximum number of attributes per span
pub const MAX_ATTRIBUTES_COUNT: usize = 100;

/// Maximum timestamp drift allowed (5 minutes into the future)
pub const MAX_TIMESTAMP_DRIFT_SECONDS: i64 = 300;

/// Maximum span name length
pub const MAX_SPAN_NAME_LENGTH: usize = 256;

/// Validate batch size
pub fn validate_batch_size(count: usize) -> Result<(), ApiError> {
    if count == 0 {
        return Err(ApiError::BadRequest(
            "Empty span batch - at least one span is required".to_string(),
        ));
    }

    if count > MAX_SPANS_PER_BATCH {
        return Err(ApiError::BadRequest(format!(
            "Batch size exceeds limit: {} spans (maximum allowed: {}). \
             Consider splitting into smaller batches.",
            count, MAX_SPANS_PER_BATCH
        )));
    }

    Ok(())
}

/// Validate span ID format (must be valid hex string)
pub fn validate_span_id(span_id: &str) -> Result<u128, ApiError> {
    // Remove 0x prefix if present
    let hex_str = span_id.strip_prefix("0x").unwrap_or(span_id);

    // Validate it's valid hex
    if hex_str.is_empty() {
        return Err(ApiError::BadRequest("Span ID cannot be empty".to_string()));
    }

    if hex_str.len() > 32 {
        return Err(ApiError::BadRequest(format!(
            "Span ID too long: {} characters (max 32 hex digits for 128-bit ID)",
            hex_str.len()
        )));
    }

    // Parse as u128
    u128::from_str_radix(hex_str, 16).map_err(|e| {
        ApiError::BadRequest(format!(
            "Invalid span ID '{}': not a valid hexadecimal string. Error: {}. \
             Example valid ID: 0x1a2b3c4d or 1a2b3c4d",
            span_id, e
        ))
    })
}

/// Validate timestamp is within reasonable bounds
pub fn validate_timestamp(timestamp_us: u64, field_name: &str) -> Result<(), ApiError> {
    // Check minimum timestamp (not too far in the past)
    if timestamp_us < MIN_VALID_TIMESTAMP {
        return Err(ApiError::BadRequest(format!(
            "Invalid {}: {} is too far in the past (before 2020-01-01). \
             Valid range: {} to {}. Check if you're using milliseconds instead of microseconds.",
            field_name, timestamp_us, MIN_VALID_TIMESTAMP, MAX_VALID_TIMESTAMP
        )));
    }

    // Check maximum timestamp (not too far in the future)
    if timestamp_us > MAX_VALID_TIMESTAMP {
        return Err(ApiError::BadRequest(format!(
            "Invalid {}: {} is too far in the future (after 2099-12-31). \
             Valid range: {} to {}",
            field_name, timestamp_us, MIN_VALID_TIMESTAMP, MAX_VALID_TIMESTAMP
        )));
    }

    // Check if timestamp is too far in the future (within MAX_TIMESTAMP_DRIFT_SECONDS)
    let now_us = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);

    let drift_us = MAX_TIMESTAMP_DRIFT_SECONDS as u64 * 1_000_000;
    if timestamp_us > now_us + drift_us {
        return Err(ApiError::BadRequest(format!(
            "Invalid {}: {} is more than {} seconds in the future. \
             Current server time: {}. Check system clock synchronization.",
            field_name, timestamp_us, MAX_TIMESTAMP_DRIFT_SECONDS, now_us
        )));
    }

    Ok(())
}

/// Validate timestamp range (start <= end)
pub fn validate_timestamp_range(start_time: u64, end_time: Option<u64>) -> Result<(), ApiError> {
    validate_timestamp(start_time, "start_time")?;

    if let Some(end) = end_time {
        validate_timestamp(end, "end_time")?;

        if end < start_time {
            return Err(ApiError::BadRequest(format!(
                "Invalid timestamp range: end_time ({}) is before start_time ({}). \
                 Span duration would be negative.",
                end, start_time
            )));
        }

        // Check for suspiciously long spans (> 24 hours)
        let duration_us = end - start_time;
        let one_day_us = 24 * 60 * 60 * 1_000_000u64;
        if duration_us > one_day_us {
            // Warning only, not an error
            tracing::warn!(
                "Span duration > 24 hours: {} microseconds ({:.2} hours). \
                 This may indicate incorrect timestamps.",
                duration_us,
                duration_us as f64 / 3_600_000_000.0
            );
        }
    }

    Ok(())
}

/// Validate attributes size
pub fn validate_attributes_size(
    attributes: &std::collections::HashMap<String, String>,
) -> Result<(), ApiError> {
    if attributes.len() > MAX_ATTRIBUTES_COUNT {
        return Err(ApiError::BadRequest(format!(
            "Too many attributes: {} (maximum allowed: {}). \
             Consider consolidating or removing unnecessary attributes.",
            attributes.len(),
            MAX_ATTRIBUTES_COUNT
        )));
    }

    let mut total_size = 0;

    for (key, value) in attributes {
        // Validate key length
        if key.is_empty() {
            return Err(ApiError::BadRequest(
                "Attribute key cannot be empty".to_string(),
            ));
        }

        if key.len() > 128 {
            return Err(ApiError::BadRequest(format!(
                "Attribute key too long: '{}' ({} bytes, max 128)",
                key,
                key.len()
            )));
        }

        // Validate value size
        let value_size = value.len();
        if value_size > MAX_SINGLE_ATTRIBUTE_SIZE {
            return Err(ApiError::BadRequest(format!(
                "Attribute '{}' value too large: {} bytes (maximum: {} bytes). \
                 Consider storing large payloads separately.",
                key, value_size, MAX_SINGLE_ATTRIBUTE_SIZE
            )));
        }

        total_size += key.len() + value_size;
    }

    if total_size > MAX_ATTRIBUTE_SIZE_BYTES {
        return Err(ApiError::BadRequest(format!(
            "Total attributes size too large: {} bytes (maximum: {} bytes). \
             Consider reducing attribute count or sizes.",
            total_size, MAX_ATTRIBUTE_SIZE_BYTES
        )));
    }

    Ok(())
}

/// Validate span name
pub fn validate_span_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() {
        return Err(ApiError::BadRequest(
            "Span name cannot be empty".to_string(),
        ));
    }

    if name.len() > MAX_SPAN_NAME_LENGTH {
        return Err(ApiError::BadRequest(format!(
            "Span name too long: {} characters (maximum: {})",
            name.len(),
            MAX_SPAN_NAME_LENGTH
        )));
    }

    // Check for control characters or invalid UTF-8
    if name
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(ApiError::BadRequest(
            "Span name contains invalid control characters".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_batch_size() {
        assert!(validate_batch_size(0).is_err());
        assert!(validate_batch_size(1).is_ok());
        assert!(validate_batch_size(MAX_SPANS_PER_BATCH).is_ok());
        assert!(validate_batch_size(MAX_SPANS_PER_BATCH + 1).is_err());
    }

    #[test]
    fn test_validate_span_id() {
        assert!(validate_span_id("0x123abc").is_ok());
        assert!(validate_span_id("123abc").is_ok());
        assert!(validate_span_id("").is_err());
        assert!(validate_span_id("not_hex").is_err());
    }

    #[test]
    fn test_validate_timestamp() {
        let now_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        // Valid current timestamp
        assert!(validate_timestamp(now_us, "test").is_ok());

        // Too old
        assert!(validate_timestamp(1000, "test").is_err());

        // Too far in future
        assert!(validate_timestamp(MAX_VALID_TIMESTAMP + 1, "test").is_err());
    }

    #[test]
    fn test_validate_span_name() {
        assert!(validate_span_name("valid_span").is_ok());
        assert!(validate_span_name("").is_err());
        assert!(validate_span_name(&"x".repeat(MAX_SPAN_NAME_LENGTH + 1)).is_err());
    }
}
