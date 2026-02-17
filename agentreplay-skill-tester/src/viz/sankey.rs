// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Sankey flow data exporter for tool call visualization
//!
//! Extracts (source, target, count) flow tuples from batches of traces.
//! Bigram frequency count over tool-call subsequences.
//!
//! Time complexity: O(Σ mᵢ) = O(M) for M total tool calls across N traces
//! Thrash detection: bidirectional flow F[i][j] > 0 ∧ F[j][i] > 0 ∧ sum > threshold

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A flow link in the Sankey diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SankeyLink {
    pub source: String,
    pub target: String,
    pub value: usize,
}

/// A node in the Sankey diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SankeyNode {
    pub id: String,
    pub label: String,
    pub call_count: usize,
}

/// Tool thrash detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolThrash {
    pub tool_a: String,
    pub tool_b: String,
    pub a_to_b_count: usize,
    pub b_to_a_count: usize,
    pub total_cycles: usize,
    pub message: String,
}

/// Complete Sankey data for visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SankeyData {
    pub nodes: Vec<SankeyNode>,
    pub links: Vec<SankeyLink>,
    pub thrash_detections: Vec<ToolThrash>,
    pub total_tool_calls: usize,
    pub unique_tools: usize,
    pub redundant_calls: usize,
}

/// Sankey flow data exporter
pub struct SankeyExporter {
    /// Minimum flow count to include in output
    min_flow_count: usize,
    /// Thrash detection threshold
    thrash_threshold: usize,
}

impl SankeyExporter {
    pub fn new() -> Self {
        Self {
            min_flow_count: 1,
            thrash_threshold: 2,
        }
    }

    pub fn with_min_flow(mut self, min: usize) -> Self {
        self.min_flow_count = min;
        self
    }

    pub fn with_thrash_threshold(mut self, threshold: usize) -> Self {
        self.thrash_threshold = threshold;
        self
    }

    /// Export Sankey data from a sequence of tool call traces
    ///
    /// Each inner Vec<String> is the ordered sequence of tool calls for one trace.
    pub fn export(&self, traces: &[Vec<String>]) -> SankeyData {
        // Build bigram flow counts
        let mut flow_counts: HashMap<(String, String), usize> = HashMap::new();
        let mut tool_counts: HashMap<String, usize> = HashMap::new();
        let mut total_calls = 0;

        for trace in traces {
            // Add "Input" → first tool and last tool → "Output" flows
            if let Some(first) = trace.first() {
                *flow_counts.entry(("Input".to_string(), first.clone())).or_insert(0) += 1;
            }
            if let Some(last) = trace.last() {
                *flow_counts.entry((last.clone(), "Output".to_string())).or_insert(0) += 1;
            }

            // Count bigrams
            for window in trace.windows(2) {
                let source = &window[0];
                let target = &window[1];
                *flow_counts.entry((source.clone(), target.clone())).or_insert(0) += 1;
            }

            // Count tool usage
            for tool in trace {
                *tool_counts.entry(tool.clone()).or_insert(0) += 1;
                total_calls += 1;
            }
        }

        // Build nodes
        let mut nodes = vec![
            SankeyNode {
                id: "Input".to_string(),
                label: "User Input".to_string(),
                call_count: traces.len(),
            },
            SankeyNode {
                id: "Output".to_string(),
                label: "Response".to_string(),
                call_count: traces.len(),
            },
        ];

        for (tool, count) in &tool_counts {
            nodes.push(SankeyNode {
                id: tool.clone(),
                label: tool.clone(),
                call_count: *count,
            });
        }

        // Build links (filter by min_flow_count)
        let links: Vec<SankeyLink> = flow_counts.iter()
            .filter(|(_, &count)| count >= self.min_flow_count)
            .map(|((source, target), &count)| SankeyLink {
                source: source.clone(),
                target: target.clone(),
                value: count,
            })
            .collect();

        // Detect thrashing (bidirectional flows)
        let mut thrash_detections = Vec::new();
        let mut checked = std::collections::HashSet::new();

        for ((source, target), &count) in &flow_counts {
            if source == "Input" || source == "Output" || target == "Input" || target == "Output" {
                continue;
            }

            let pair_key = if source < target {
                (source.clone(), target.clone())
            } else {
                (target.clone(), source.clone())
            };

            if checked.contains(&pair_key) {
                continue;
            }
            checked.insert(pair_key.clone());

            let reverse_count = flow_counts.get(&(target.clone(), source.clone())).copied().unwrap_or(0);

            if count > 0 && reverse_count > 0 && (count + reverse_count) >= self.thrash_threshold {
                thrash_detections.push(ToolThrash {
                    tool_a: source.clone(),
                    tool_b: target.clone(),
                    a_to_b_count: count,
                    b_to_a_count: reverse_count,
                    total_cycles: count.min(reverse_count),
                    message: format!(
                        "⚠️ {} ←→ {} ({} cycles) — Agent oscillated between these calls",
                        source, target, count.min(reverse_count)
                    ),
                });
            }
        }

        // Count redundant calls (same tool called consecutively)
        let redundant = traces.iter()
            .flat_map(|t| t.windows(2))
            .filter(|w| w[0] == w[1])
            .count();

        SankeyData {
            nodes,
            links,
            thrash_detections,
            total_tool_calls: total_calls,
            unique_tools: tool_counts.len(),
            redundant_calls: redundant,
        }
    }
}

impl Default for SankeyExporter {
    fn default() -> Self {
        Self::new()
    }
}
