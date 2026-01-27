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

//! Integration test for timestamp configuration and monotonic clock features

use flowtrace_core::{validate_timestamp, AgentFlowEdge, SpanType, TimestampConfig};

#[test]
fn test_production_config_validation() {
    let config = TimestampConfig::production();

    // Current time (2024) should be valid
    let current = 1730800800000000u64;
    assert!(validate_timestamp(current, &config).is_ok());

    // Historical time (1970) should be rejected
    let historical = 100000000u64;
    assert!(validate_timestamp(historical, &config).is_err());

    // Future beyond 2099 should be rejected
    let far_future = 5000000000000000u64;
    assert!(validate_timestamp(far_future, &config).is_err());
}

#[test]
fn test_unrestricted_config() {
    let config = TimestampConfig::unrestricted();

    // Simple test timestamps should work
    assert!(validate_timestamp(1000, &config).is_ok());
    assert!(validate_timestamp(999999, &config).is_ok());

    // Historical timestamps should work
    assert!(validate_timestamp(100000000, &config).is_ok());

    // Future timestamps should work
    assert!(validate_timestamp(9000000000000000, &config).is_ok());
}

#[test]
fn test_historical_config() {
    let config = TimestampConfig::historical();

    // 1970 should be valid
    assert!(validate_timestamp(100000000, &config).is_ok());

    // 2024 should be valid
    assert!(validate_timestamp(1730800800000000, &config).is_ok());

    // Beyond 2099 should be rejected
    assert!(validate_timestamp(5000000000000000, &config).is_err());
}

#[test]
fn test_custom_config() {
    let config = TimestampConfig::custom(
        Some(1700000000000000), // Jan 2024
        Some(1800000000000000), // Aug 2027
    );

    // Within bounds should be valid
    assert!(validate_timestamp(1730800800000000, &config).is_ok());

    // Before min should be rejected
    assert!(validate_timestamp(1600000000000000, &config).is_err());

    // After max should be rejected
    assert!(validate_timestamp(2000000000000000, &config).is_err());
}

#[test]
fn test_monotonic_clock_ordering() {
    // Get multiple timestamps - logical clock should always increase
    let (_, logical1) = AgentFlowEdge::now_us();
    let (_, logical2) = AgentFlowEdge::now_us();
    let (_, logical3) = AgentFlowEdge::now_us();

    assert!(logical2 >= logical1, "Logical clock must be monotonic");
    assert!(logical3 >= logical2, "Logical clock must be monotonic");
}

#[test]
fn test_edge_creation_with_monotonic_clock() {
    let edge1 = AgentFlowEdge::new(1, 1, 1, 1, SpanType::Planning, 0);
    let edge2 = AgentFlowEdge::new(1, 1, 1, 1, SpanType::ToolCall, 0);

    // Logical clocks should be ordered (even if created in same microsecond)
    assert!(edge2.logical_clock >= edge1.logical_clock);

    // Wall clocks should be reasonable (> 2020)
    assert!(edge1.timestamp_us > 1_500_000_000_000_000);
    assert!(edge2.timestamp_us > 1_500_000_000_000_000);
}

#[test]
fn test_edge_validation_with_config() {
    let edge = AgentFlowEdge::new(1, 1, 1, 1, SpanType::Planning, 0);

    // Should validate with production config (default)
    assert!(edge.validate().is_ok());

    // Should also validate with unrestricted config
    assert!(edge
        .validate_with_config(&TimestampConfig::unrestricted())
        .is_ok());
}

#[test]
fn test_unrestricted_allows_simple_timestamps() {
    let config = TimestampConfig::unrestricted();

    // These simple timestamps are useful for testing
    assert!(validate_timestamp(1, &config).is_ok());
    assert!(validate_timestamp(100, &config).is_ok());
    assert!(validate_timestamp(1000, &config).is_ok());
    assert!(validate_timestamp(10000, &config).is_ok());
}
