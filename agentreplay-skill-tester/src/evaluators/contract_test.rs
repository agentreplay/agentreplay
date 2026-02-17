// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Contract test evaluator for tool adapters
//!
//! Validates tool inputs/outputs against JSON Schema definitions.
//! Computes violation rates with Wilson confidence intervals.
//!
//! Time complexity: O(d·f) per call where d=schema depth, f=fields.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A tool contract specifying expected schemas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContract {
    pub tool_name: String,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
}

/// Violation type classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationType {
    /// Missing required field
    MissingRequired(String),
    /// Wrong type for a field
    TypeMismatch { field: String, expected: String, actual: String },
    /// Value out of allowed range
    RangeViolation { field: String, detail: String },
    /// Pattern mismatch
    PatternMismatch { field: String, pattern: String },
    /// Extra fields not in schema
    ExtraField(String),
}

/// Result of a contract validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractValidationResult {
    pub tool_name: String,
    pub total_calls: usize,
    pub violations: Vec<ContractViolation>,
    pub violation_rate: f64,
    /// Wilson confidence interval (lower, upper) at 95%
    pub violation_rate_ci: (f64, f64),
}

/// A single contract violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractViolation {
    pub tool_name: String,
    pub call_index: usize,
    pub direction: String, // "input" or "output"
    pub violation_type: String,
    pub message: String,
    pub field_path: Option<String>,
}

/// Contract test evaluator
pub struct ContractTestEvaluator {
    contracts: Vec<ToolContract>,
}

impl ContractTestEvaluator {
    pub fn new(contracts: Vec<ToolContract>) -> Self {
        Self { contracts }
    }

    /// Validate a tool call against its contract
    pub fn validate_call(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        output: Option<&serde_json::Value>,
        call_index: usize,
    ) -> Vec<ContractViolation> {
        let mut violations = Vec::new();

        let contract = match self.contracts.iter().find(|c| c.tool_name == tool_name) {
            Some(c) => c,
            None => return violations, // No contract defined — pass
        };

        // Validate input against input_schema
        if let Some(schema) = &contract.input_schema {
            let input_violations = self.validate_against_schema(
                input,
                schema,
                tool_name,
                "input",
                call_index,
                "",
            );
            violations.extend(input_violations);
        }

        // Validate output against output_schema
        if let (Some(schema), Some(output_val)) = (&contract.output_schema, output) {
            let output_violations = self.validate_against_schema(
                output_val,
                schema,
                tool_name,
                "output",
                call_index,
                "",
            );
            violations.extend(output_violations);
        }

        violations
    }

    /// Simplified JSON Schema validation
    /// In production, use the `jsonschema` crate for full spec compliance
    fn validate_against_schema(
        &self,
        value: &serde_json::Value,
        schema: &serde_json::Value,
        tool_name: &str,
        direction: &str,
        call_index: usize,
        path: &str,
    ) -> Vec<ContractViolation> {
        let mut violations = Vec::new();

        // Check type
        if let Some(expected_type) = schema.get("type").and_then(|t| t.as_str()) {
            let actual_type = match value {
                serde_json::Value::Object(_) => "object",
                serde_json::Value::Array(_) => "array",
                serde_json::Value::String(_) => "string",
                serde_json::Value::Number(n) => {
                    if n.is_i64() { "integer" } else { "number" }
                }
                serde_json::Value::Bool(_) => "boolean",
                serde_json::Value::Null => "null",
            };

            // Allow integer where number is expected
            let type_matches = actual_type == expected_type
                || (expected_type == "number" && actual_type == "integer");

            if !type_matches {
                violations.push(ContractViolation {
                    tool_name: tool_name.to_string(),
                    call_index,
                    direction: direction.to_string(),
                    violation_type: "type_mismatch".to_string(),
                    message: format!(
                        "Expected type '{}', got '{}'",
                        expected_type, actual_type
                    ),
                    field_path: Some(if path.is_empty() { "$".to_string() } else { path.to_string() }),
                });
                return violations; // Type mismatch — skip deeper checks
            }
        }

        // Check required fields for objects
        if let (Some(required), Some(obj)) = (
            schema.get("required").and_then(|r| r.as_array()),
            value.as_object(),
        ) {
            for req_field in required {
                if let Some(field_name) = req_field.as_str() {
                    if !obj.contains_key(field_name) {
                        violations.push(ContractViolation {
                            tool_name: tool_name.to_string(),
                            call_index,
                            direction: direction.to_string(),
                            violation_type: "missing_required".to_string(),
                            message: format!("Missing required field '{}'", field_name),
                            field_path: Some(format!("{}.{}", if path.is_empty() { "$" } else { path }, field_name)),
                        });
                    }
                }
            }
        }

        // Check properties recursively
        if let (Some(properties), Some(obj)) = (
            schema.get("properties").and_then(|p| p.as_object()),
            value.as_object(),
        ) {
            for (prop_name, prop_schema) in properties {
                if let Some(prop_value) = obj.get(prop_name) {
                    let prop_path = format!(
                        "{}.{}",
                        if path.is_empty() { "$" } else { path },
                        prop_name
                    );
                    let sub_violations = self.validate_against_schema(
                        prop_value,
                        prop_schema,
                        tool_name,
                        direction,
                        call_index,
                        &prop_path,
                    );
                    violations.extend(sub_violations);
                }
            }
        }

        // Check enum constraints
        if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
            if !enum_values.contains(value) {
                violations.push(ContractViolation {
                    tool_name: tool_name.to_string(),
                    call_index,
                    direction: direction.to_string(),
                    violation_type: "enum_violation".to_string(),
                    message: format!(
                        "Value {:?} not in allowed values {:?}",
                        value, enum_values
                    ),
                    field_path: Some(if path.is_empty() { "$".to_string() } else { path.to_string() }),
                });
            }
        }

        // Check minimum/maximum for numbers
        if let Some(num) = value.as_f64() {
            if let Some(min) = schema.get("minimum").and_then(|m| m.as_f64()) {
                if num < min {
                    violations.push(ContractViolation {
                        tool_name: tool_name.to_string(),
                        call_index,
                        direction: direction.to_string(),
                        violation_type: "range_violation".to_string(),
                        message: format!("Value {} is below minimum {}", num, min),
                        field_path: Some(if path.is_empty() { "$".to_string() } else { path.to_string() }),
                    });
                }
            }
        }

        // Check minLength/maxLength for strings
        if let Some(s) = value.as_str() {
            if let Some(min_len) = schema.get("minLength").and_then(|m| m.as_u64()) {
                if (s.len() as u64) < min_len {
                    violations.push(ContractViolation {
                        tool_name: tool_name.to_string(),
                        call_index,
                        direction: direction.to_string(),
                        violation_type: "length_violation".to_string(),
                        message: format!("String length {} is below minimum {}", s.len(), min_len),
                        field_path: Some(if path.is_empty() { "$".to_string() } else { path.to_string() }),
                    });
                }
            }
            if let Some(max_len) = schema.get("maxLength").and_then(|m| m.as_u64()) {
                if (s.len() as u64) > max_len {
                    violations.push(ContractViolation {
                        tool_name: tool_name.to_string(),
                        call_index,
                        direction: direction.to_string(),
                        violation_type: "length_violation".to_string(),
                        message: format!("String length {} exceeds maximum {}", s.len(), max_len),
                        field_path: Some(if path.is_empty() { "$".to_string() } else { path.to_string() }),
                    });
                }
            }
        }

        violations
    }

    /// Compute Wilson CI for violation rate
    pub fn wilson_ci(violations: usize, total: usize, z: f64) -> (f64, f64) {
        if total == 0 {
            return (0.0, 1.0);
        }

        let n = total as f64;
        let p_hat = violations as f64 / n;
        let z2 = z * z;

        let denom = 1.0 + z2 / n;
        let center = p_hat + z2 / (2.0 * n);
        let margin = z * (p_hat * (1.0 - p_hat) / n + z2 / (4.0 * n * n)).sqrt();

        let lower = ((center - margin) / denom).max(0.0);
        let upper = ((center + margin) / denom).min(1.0);

        (lower, upper)
    }

    /// Evaluate all tool calls and produce summary results
    pub fn evaluate_batch(
        &self,
        calls: &[(String, serde_json::Value, Option<serde_json::Value>)],
    ) -> Vec<ContractValidationResult> {
        let mut results_map: HashMap<String, Vec<ContractViolation>> = HashMap::new();
        let mut counts: HashMap<String, usize> = HashMap::new();

        for (idx, (tool_name, input, output)) in calls.iter().enumerate() {
            *counts.entry(tool_name.clone()).or_insert(0) += 1;
            let violations = self.validate_call(tool_name, input, output.as_ref(), idx);
            results_map.entry(tool_name.clone()).or_default().extend(violations);
        }

        counts.iter().map(|(tool_name, &total)| {
            let violations = results_map.remove(tool_name).unwrap_or_default();
            let violation_count = violations.len();
            let violation_rate = if total > 0 {
                violation_count as f64 / total as f64
            } else {
                0.0
            };
            let ci = Self::wilson_ci(violation_count, total, 1.96);

            ContractValidationResult {
                tool_name: tool_name.clone(),
                total_calls: total,
                violations,
                violation_rate,
                violation_rate_ci: ci,
            }
        }).collect()
    }
}
