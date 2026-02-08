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

//! Additional tests for edge validation and security fixes

use crate::edge::*;

#[test]
fn test_confidence_validation() {
    let mut edge = AgentFlowEdge::new(1, 0, 1, 1, SpanType::Root, 0);

    // Valid confidence values
    assert!(edge.set_confidence(0.0).is_ok());
    assert!(edge.set_confidence(0.5).is_ok());
    assert!(edge.set_confidence(1.0).is_ok());

    // Invalid confidence values
    assert!(edge.set_confidence(-0.1).is_err());
    assert!(edge.set_confidence(1.1).is_err());
    assert!(edge.set_confidence(f32::NAN).is_err());
    assert!(edge.set_confidence(f32::INFINITY).is_err());
}

#[test]
fn test_sampling_rate_validation() {
    let mut edge = AgentFlowEdge::new(1, 0, 1, 1, SpanType::Root, 0);

    // Valid sampling rates
    assert!(edge.set_sampling_rate(0.0).is_ok());
    assert!(edge.set_sampling_rate(0.01).is_ok());
    assert!(edge.set_sampling_rate(1.0).is_ok());

    // Invalid sampling rates
    assert!(edge.set_sampling_rate(-0.1).is_err());
    assert!(edge.set_sampling_rate(1.01).is_err());
    assert!(edge.set_sampling_rate(f32::NAN).is_err());
}

#[test]
fn test_edge_validate() {
    let edge = AgentFlowEdge::new(1, 0, 1, 1, SpanType::Root, 0);
    assert!(edge.validate().is_ok());

    // Create invalid edge manually
    let mut invalid_edge = edge;
    invalid_edge.confidence = 2.0; // Invalid!
    invalid_edge.checksum = invalid_edge.compute_checksum();
    assert!(invalid_edge.validate().is_err());
}

#[test]
fn test_timestamp_no_panic() {
    // This should not panic even if time goes backwards
    let (wall1, logical1) = AgentFlowEdge::now_us();
    let (wall2, logical2) = AgentFlowEdge::now_us();

    // Logical clock should be monotonically increasing (never regresses)
    assert!(logical2 >= logical1, "Logical clock must be monotonic");

    // Wall clock should be reasonable (> 2020)
    assert!(
        wall1 > 1_000_000_000_000_000,
        "Wall clock timestamp should be > 2020"
    );
    assert!(
        wall2 > 1_000_000_000_000_000,
        "Wall clock timestamp should be > 2020"
    );
}

#[test]
fn test_edge_id_uniqueness_under_load() {
    use std::collections::HashSet;
    use std::thread;

    let mut handles = vec![];
    let iterations = 100;

    // Create edges from multiple threads
    for _ in 0..4 {
        let handle = thread::spawn(move || {
            let mut ids = Vec::new();
            for _ in 0..iterations {
                let edge = AgentFlowEdge::new(1, 0, 1, 1, SpanType::Root, 0);
                ids.push(edge.edge_id);
            }
            ids
        });
        handles.push(handle);
    }

    // Collect all IDs
    let mut all_ids = HashSet::new();
    for handle in handles {
        let ids = handle.join().unwrap();
        for id in ids {
            assert!(all_ids.insert(id), "Duplicate ID found: {}", id);
        }
    }

    // Should have 4 * iterations unique IDs
    assert_eq!(all_ids.len(), 4 * iterations);
}

#[test]
fn test_checksum_update_after_modification() {
    let mut edge = AgentFlowEdge::new(1, 0, 1, 1, SpanType::Root, 0);
    let original_checksum = edge.checksum;

    // Modify edge
    edge.set_confidence(0.5).unwrap();

    // Checksum should be updated
    assert_ne!(edge.checksum, original_checksum);
    assert!(edge.verify_checksum());
}

#[test]
fn test_roundtrip_serialization() {
    // Create an edge with various field values
    let mut edge = AgentFlowEdge::new(1, 0, 12345, 67890, SpanType::ToolCall, 99999);
    edge.set_confidence(0.75).unwrap();
    edge.set_sampling_rate(0.5).unwrap();
    edge.token_count = 42;
    edge.duration_us = 1234567;
    edge.compression_type = 1;
    edge.has_payload = 1;
    edge.flags = 0xFF;
    edge.checksum = edge.compute_checksum();

    // Serialize to bytes
    let bytes = edge.to_bytes();

    // Deserialize back
    let deserialized = AgentFlowEdge::from_bytes(&bytes).unwrap();

    // Verify all fields match exactly
    assert_eq!(deserialized.edge_id, edge.edge_id);
    assert_eq!(deserialized.causal_parent, edge.causal_parent);
    assert_eq!(deserialized.timestamp_us, edge.timestamp_us);
    assert_eq!(deserialized.logical_clock, edge.logical_clock);
    assert_eq!(deserialized.tenant_id, edge.tenant_id);
    assert_eq!(deserialized.project_id, edge.project_id);
    assert_eq!(deserialized.schema_version, edge.schema_version);
    assert_eq!(deserialized.sensitivity_flags, edge.sensitivity_flags);
    assert_eq!(deserialized.agent_id, edge.agent_id);
    assert_eq!(deserialized.session_id, edge.session_id);
    assert_eq!(deserialized.span_type, edge.span_type);
    assert_eq!(deserialized.parent_count, edge.parent_count);
    assert_eq!(deserialized.confidence, edge.confidence);
    assert_eq!(deserialized.token_count, edge.token_count);
    assert_eq!(deserialized.duration_us, edge.duration_us);
    assert_eq!(deserialized.sampling_rate, edge.sampling_rate);
    assert_eq!(deserialized.compression_type, edge.compression_type);
    assert_eq!(deserialized.has_payload, edge.has_payload);
    assert_eq!(deserialized.flags, edge.flags);
    assert_eq!(deserialized.checksum, edge.checksum);

    // Verify checksum is still valid
    assert!(deserialized.verify_checksum());

    // Verify bit-identical round-trip
    let bytes2 = deserialized.to_bytes();
    assert_eq!(bytes, bytes2);
}
