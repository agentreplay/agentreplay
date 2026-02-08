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

//! Natural Language Query Parser
//!
//! Parses natural language queries into structured query plans for multi-index
//! execution. Supports template matching, intent classification, and entity extraction.
//!
//! ## Example Queries
//!
//! - "Find traces where the agent failed after tool call"
//! - "Show me slow LLM responses over 5 seconds"
//! - "Traces with error handling patterns"
//! - "Sessions where user asked about refunds"
//! - "Compare error rates between yesterday and today"

use crate::semantic::{QueryFilters, SemanticQuery, TimeRange};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed query from natural language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedQuery {
    /// Query intent
    pub intent: QueryIntent,

    /// Temporal constraints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temporal: Option<TemporalConstraint>,

    /// Numeric filters (latency, token count, etc.)
    #[serde(default)]
    pub numeric_filters: Vec<NumericFilter>,

    /// Semantic filters (free text search)
    #[serde(default)]
    pub semantic_filters: Vec<SemanticFilter>,

    /// Structural filters (span type, error state, etc.)
    #[serde(default)]
    pub structural_filters: Vec<StructuralFilter>,

    /// Aggregation (for analyze intent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregation: Option<Aggregation>,

    /// Original query text
    pub original_query: String,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
}

/// Query intent classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryIntent {
    /// Find specific traces
    Search,
    /// Aggregate and summarize
    Analyze,
    /// Compare two sets
    Compare,
    /// Root cause analysis
    Explain,
    /// List/browse traces
    List,
}

/// Temporal constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TemporalConstraint {
    /// Relative time (e.g., "last hour")
    Relative { amount: u64, unit: TimeUnit },
    /// Absolute time range
    Absolute { start: String, end: String },
    /// Named period (e.g., "yesterday", "this week")
    Named { period: String },
}

impl TemporalConstraint {
    /// Convert to TimeRange
    pub fn to_time_range(&self) -> Option<TimeRange> {
        match self {
            TemporalConstraint::Relative { amount, unit } => {
                let micros = match unit {
                    TimeUnit::Seconds => amount * 1_000_000,
                    TimeUnit::Minutes => amount * 60 * 1_000_000,
                    TimeUnit::Hours => amount * 3600 * 1_000_000,
                    TimeUnit::Days => amount * 86400 * 1_000_000,
                    TimeUnit::Weeks => amount * 7 * 86400 * 1_000_000,
                };
                Some(TimeRange {
                    start_us: now_micros() - micros,
                    end_us: now_micros(),
                })
            }
            TemporalConstraint::Named { period } => match period.to_lowercase().as_str() {
                "today" => Some(TimeRange::last_hours(24)),
                "yesterday" => {
                    let now = now_micros();
                    let day = 86400 * 1_000_000;
                    Some(TimeRange {
                        start_us: now - 2 * day,
                        end_us: now - day,
                    })
                }
                "this week" | "week" => Some(TimeRange::last_hours(7 * 24)),
                "this month" | "month" => Some(TimeRange::last_hours(30 * 24)),
                _ => None,
            },
            TemporalConstraint::Absolute { .. } => {
                // Would need proper date parsing
                None
            }
        }
    }
}

fn now_micros() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

/// Time unit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeUnit {
    Seconds,
    Minutes,
    Hours,
    Days,
    Weeks,
}

/// Numeric filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumericFilter {
    /// Field to filter on
    pub field: String,
    /// Comparison operator
    pub operator: ComparisonOp,
    /// Value to compare against
    pub value: f64,
}

/// Comparison operator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOp {
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Equal,
    NotEqual,
}

/// Semantic filter (text similarity)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFilter {
    /// Search text
    pub text: String,
    /// Minimum similarity threshold
    pub min_similarity: f32,
}

impl SemanticFilter {
    pub fn new(text: &str, min_similarity: f32) -> Self {
        Self {
            text: text.to_string(),
            min_similarity,
        }
    }
}

/// Structural filter
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StructuralFilter {
    /// Filter by trace ID
    TraceId { id: String },
    /// Filter by span type
    SpanType { span_type: String },
    /// Filter by agent ID
    AgentId { id: u64 },
    /// Filter by session ID
    SessionId { id: u64 },
    /// Filter by error state
    HasError { has_error: bool },
    /// Filter by project ID
    ProjectId { id: u16 },
}

/// Aggregation specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aggregation {
    /// Aggregation function
    pub function: AggregationFunction,
    /// Field to aggregate
    pub field: Option<String>,
    /// Group by field
    pub group_by: Option<String>,
}

/// Aggregation function
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationFunction {
    Count,
    Sum,
    Average,
    Min,
    Max,
    Percentile,
}

/// Natural language query parser
pub struct NLQueryParser {
    /// Intent keywords
    intent_keywords: HashMap<QueryIntent, Vec<&'static str>>,
    /// Time patterns
    time_patterns: Vec<TimePattern>,
    /// Numeric patterns
    numeric_patterns: Vec<NumericPattern>,
    /// Query templates
    templates: Vec<QueryTemplate>,
}

struct TimePattern {
    regex: regex::Regex,
    extract: fn(&regex::Captures) -> Option<TemporalConstraint>,
}

#[allow(dead_code)]
struct NumericPattern {
    regex: regex::Regex,
    field: &'static str,
    extract: fn(&regex::Captures) -> Option<NumericFilter>,
}

#[allow(dead_code)]
struct QueryTemplate {
    pattern: regex::Regex,
    intent: QueryIntent,
    build: fn(&regex::Captures, &str) -> ParsedQuery,
}

impl NLQueryParser {
    /// Create a new parser
    pub fn new() -> Self {
        let mut intent_keywords = HashMap::new();

        intent_keywords.insert(
            QueryIntent::Search,
            vec!["find", "search", "show", "get", "where", "with"],
        );
        intent_keywords.insert(
            QueryIntent::Analyze,
            vec![
                "analyze",
                "summarize",
                "aggregate",
                "count",
                "average",
                "how many",
            ],
        );
        intent_keywords.insert(
            QueryIntent::Compare,
            vec!["compare", "versus", "vs", "difference", "between"],
        );
        intent_keywords.insert(
            QueryIntent::Explain,
            vec!["why", "explain", "root cause", "reason", "what caused"],
        );
        intent_keywords.insert(QueryIntent::List, vec!["list", "all", "every", "browse"]);

        Self {
            intent_keywords,
            time_patterns: Self::build_time_patterns(),
            numeric_patterns: Self::build_numeric_patterns(),
            templates: Self::build_templates(),
        }
    }

    fn build_time_patterns() -> Vec<TimePattern> {
        vec![
            // "last N hours/minutes/days"
            TimePattern {
                regex: regex::Regex::new(r"(?i)last\s+(\d+)\s+(hour|minute|day|week)s?").unwrap(),
                extract: |caps| {
                    let amount: u64 = caps.get(1)?.as_str().parse().ok()?;
                    let unit = match caps.get(2)?.as_str().to_lowercase().as_str() {
                        "hour" => TimeUnit::Hours,
                        "minute" => TimeUnit::Minutes,
                        "day" => TimeUnit::Days,
                        "week" => TimeUnit::Weeks,
                        _ => return None,
                    };
                    Some(TemporalConstraint::Relative { amount, unit })
                },
            },
            // "past hour/day/week"
            TimePattern {
                regex: regex::Regex::new(r"(?i)past\s+(hour|day|week)").unwrap(),
                extract: |caps| {
                    let unit = match caps.get(1)?.as_str().to_lowercase().as_str() {
                        "hour" => TimeUnit::Hours,
                        "day" => TimeUnit::Days,
                        "week" => TimeUnit::Weeks,
                        _ => return None,
                    };
                    Some(TemporalConstraint::Relative { amount: 1, unit })
                },
            },
            // "today", "yesterday", "this week"
            TimePattern {
                regex: regex::Regex::new(r"(?i)(today|yesterday|this week|this month)").unwrap(),
                extract: |caps| {
                    Some(TemporalConstraint::Named {
                        period: caps.get(1)?.as_str().to_lowercase(),
                    })
                },
            },
        ]
    }

    fn build_numeric_patterns() -> Vec<NumericPattern> {
        vec![
            // "over N seconds" (latency)
            NumericPattern {
                regex: regex::Regex::new(r"(?i)over\s+(\d+(?:\.\d+)?)\s*(?:s|seconds?)").unwrap(),
                field: "duration_ms",
                extract: |caps| {
                    let value: f64 = caps.get(1)?.as_str().parse().ok()?;
                    Some(NumericFilter {
                        field: "duration_ms".to_string(),
                        operator: ComparisonOp::GreaterThan,
                        value: value * 1000.0, // Convert to ms
                    })
                },
            },
            // "more than N tokens"
            NumericPattern {
                regex: regex::Regex::new(r"(?i)more\s+than\s+(\d+)\s*tokens?").unwrap(),
                field: "token_count",
                extract: |caps| {
                    let value: f64 = caps.get(1)?.as_str().parse().ok()?;
                    Some(NumericFilter {
                        field: "token_count".to_string(),
                        operator: ComparisonOp::GreaterThan,
                        value,
                    })
                },
            },
            // "less than N ms"
            NumericPattern {
                regex: regex::Regex::new(r"(?i)less\s+than\s+(\d+)\s*ms").unwrap(),
                field: "duration_ms",
                extract: |caps| {
                    let value: f64 = caps.get(1)?.as_str().parse().ok()?;
                    Some(NumericFilter {
                        field: "duration_ms".to_string(),
                        operator: ComparisonOp::LessThan,
                        value,
                    })
                },
            },
        ]
    }

    fn build_templates() -> Vec<QueryTemplate> {
        vec![
            // "Show me errors from last hour"
            QueryTemplate {
                pattern: regex::Regex::new(
                    r"(?i)(show|find|get)\s+(?:me\s+)?errors?\s+from\s+(.+)",
                )
                .unwrap(),
                intent: QueryIntent::Search,
                build: |caps, original| {
                    let time_str = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                    let mut query = ParsedQuery {
                        intent: QueryIntent::Search,
                        temporal: None,
                        numeric_filters: vec![],
                        semantic_filters: vec![SemanticFilter::new("error failure exception", 0.6)],
                        structural_filters: vec![StructuralFilter::HasError { has_error: true }],
                        aggregation: None,
                        original_query: original.to_string(),
                        confidence: 0.9,
                    };

                    // Parse time
                    if time_str.contains("hour") {
                        if let Ok(re) = regex::Regex::new(r"(\d+)") {
                            if let Some(caps) = re.captures(time_str) {
                                if let Ok(hours) = caps.get(1).unwrap().as_str().parse::<u64>() {
                                    query.temporal = Some(TemporalConstraint::Relative {
                                        amount: hours,
                                        unit: TimeUnit::Hours,
                                    });
                                }
                            }
                        }
                    }

                    query
                },
            },
            // "Why did trace X fail"
            QueryTemplate {
                pattern: regex::Regex::new(
                    r"(?i)why\s+did\s+(?:trace\s+)?(.+?)\s+(fail|error|crash)",
                )
                .unwrap(),
                intent: QueryIntent::Explain,
                build: |caps, original| {
                    let trace_ref = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    ParsedQuery {
                        intent: QueryIntent::Explain,
                        temporal: None,
                        numeric_filters: vec![],
                        semantic_filters: vec![],
                        structural_filters: vec![
                            StructuralFilter::TraceId {
                                id: trace_ref.to_string(),
                            },
                            StructuralFilter::HasError { has_error: true },
                        ],
                        aggregation: None,
                        original_query: original.to_string(),
                        confidence: 0.85,
                    }
                },
            },
            // "slow LLM responses"
            QueryTemplate {
                pattern: regex::Regex::new(
                    r"(?i)slow\s+(?:llm\s+)?responses?\s+(?:over\s+)?(\d+)\s*(?:s|seconds?)",
                )
                .unwrap(),
                intent: QueryIntent::Search,
                build: |caps, original| {
                    let seconds: f64 = caps
                        .get(1)
                        .and_then(|m| m.as_str().parse().ok())
                        .unwrap_or(5.0);
                    ParsedQuery {
                        intent: QueryIntent::Search,
                        temporal: None,
                        numeric_filters: vec![NumericFilter {
                            field: "duration_ms".to_string(),
                            operator: ComparisonOp::GreaterThan,
                            value: seconds * 1000.0,
                        }],
                        semantic_filters: vec![SemanticFilter::new("LLM response generation", 0.5)],
                        structural_filters: vec![],
                        aggregation: None,
                        original_query: original.to_string(),
                        confidence: 0.8,
                    }
                },
            },
        ]
    }

    /// Parse a natural language query
    pub fn parse(&self, query: &str) -> ParsedQuery {
        let query = query.trim();

        // Try template matching first (fast path)
        for template in &self.templates {
            if let Some(caps) = template.pattern.captures(query) {
                return (template.build)(&caps, query);
            }
        }

        // Fall back to component-based parsing
        let intent = self.classify_intent(query);
        let temporal = self.extract_temporal(query);
        let numeric_filters = self.extract_numeric_filters(query);
        let structural_filters = self.extract_structural_filters(query);
        let aggregation = self.detect_aggregation(query, intent);

        // The remaining text becomes semantic search
        let semantic_filters = vec![SemanticFilter::new(query, 0.5)];

        ParsedQuery {
            intent,
            temporal,
            numeric_filters,
            semantic_filters,
            structural_filters,
            aggregation,
            original_query: query.to_string(),
            confidence: 0.6, // Lower confidence for fallback parsing
        }
    }

    /// Classify query intent
    fn classify_intent(&self, query: &str) -> QueryIntent {
        let query_lower = query.to_lowercase();

        // Define priority order - more specific intents first
        let priority_order = [
            QueryIntent::Explain, // Highest priority - specific action
            QueryIntent::Compare, // Specific action
            QueryIntent::Analyze, // Specific action
            QueryIntent::Search,  // General search
            QueryIntent::List,    // Most generic - fallback
        ];

        // Score each intent
        let mut best_intent = QueryIntent::Search;
        let mut best_score = 0;

        for intent in &priority_order {
            if let Some(keywords) = self.intent_keywords.get(intent) {
                let score: usize = keywords
                    .iter()
                    .filter(|kw| query_lower.contains(*kw))
                    .count();
                // Use > for strict priority (first high score wins due to priority order)
                if score > best_score {
                    best_score = score;
                    best_intent = *intent;
                }
            }
        }

        best_intent
    }

    /// Extract temporal constraints
    fn extract_temporal(&self, query: &str) -> Option<TemporalConstraint> {
        for pattern in &self.time_patterns {
            if let Some(caps) = pattern.regex.captures(query) {
                if let Some(constraint) = (pattern.extract)(&caps) {
                    return Some(constraint);
                }
            }
        }
        None
    }

    /// Extract numeric filters
    fn extract_numeric_filters(&self, query: &str) -> Vec<NumericFilter> {
        let mut filters = Vec::new();
        for pattern in &self.numeric_patterns {
            if let Some(caps) = pattern.regex.captures(query) {
                if let Some(filter) = (pattern.extract)(&caps) {
                    filters.push(filter);
                }
            }
        }
        filters
    }

    /// Extract structural filters
    fn extract_structural_filters(&self, query: &str) -> Vec<StructuralFilter> {
        let mut filters = Vec::new();
        let query_lower = query.to_lowercase();

        // Error detection
        if query_lower.contains("error")
            || query_lower.contains("fail")
            || query_lower.contains("crash")
        {
            filters.push(StructuralFilter::HasError { has_error: true });
        }

        // Span type detection
        if query_lower.contains("tool call") || query_lower.contains("tool_call") {
            filters.push(StructuralFilter::SpanType {
                span_type: "ToolCall".to_string(),
            });
        }
        if query_lower.contains("planning") {
            filters.push(StructuralFilter::SpanType {
                span_type: "Planning".to_string(),
            });
        }
        if query_lower.contains("reasoning") {
            filters.push(StructuralFilter::SpanType {
                span_type: "Reasoning".to_string(),
            });
        }

        filters
    }

    /// Detect aggregation needs
    fn detect_aggregation(&self, query: &str, intent: QueryIntent) -> Option<Aggregation> {
        if intent != QueryIntent::Analyze {
            return None;
        }

        let query_lower = query.to_lowercase();

        if query_lower.contains("count") || query_lower.contains("how many") {
            return Some(Aggregation {
                function: AggregationFunction::Count,
                field: None,
                group_by: None,
            });
        }

        if query_lower.contains("average") || query_lower.contains("avg") {
            return Some(Aggregation {
                function: AggregationFunction::Average,
                field: Some("duration_ms".to_string()),
                group_by: None,
            });
        }

        None
    }

    /// Convert parsed query to SemanticQuery
    pub fn to_semantic_query(&self, parsed: &ParsedQuery) -> SemanticQuery {
        let mut filters = QueryFilters::default();

        // Apply temporal constraint
        if let Some(ref temporal) = parsed.temporal {
            filters.time_range = temporal.to_time_range();
        }

        // Apply structural filters
        for filter in &parsed.structural_filters {
            match filter {
                StructuralFilter::HasError { has_error } => {
                    filters.has_error = Some(*has_error);
                }
                StructuralFilter::SpanType { span_type } => {
                    let types = filters.span_types.get_or_insert_with(Vec::new);
                    types.push(span_type.clone());
                }
                StructuralFilter::AgentId { id } => {
                    let ids = filters.agent_ids.get_or_insert_with(Vec::new);
                    ids.push(*id);
                }
                StructuralFilter::SessionId { id } => {
                    let ids = filters.session_ids.get_or_insert_with(Vec::new);
                    ids.push(*id);
                }
                StructuralFilter::ProjectId { id } => {
                    filters.project_id = Some(*id);
                }
                _ => {}
            }
        }

        // Build query text from semantic filters
        let query_text = if parsed.semantic_filters.is_empty() {
            parsed.original_query.clone()
        } else {
            parsed
                .semantic_filters
                .iter()
                .map(|f| f.text.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };

        // Determine min_similarity from semantic filters
        let min_similarity = parsed
            .semantic_filters
            .first()
            .map(|f| f.min_similarity)
            .unwrap_or(0.5);

        SemanticQuery {
            query_text,
            limit: 10,
            min_similarity,
            filters,
            include_highlights: true,
            rerank: true,
        }
    }
}

impl Default for NLQueryParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_classification() {
        let parser = NLQueryParser::new();

        assert_eq!(
            parser.classify_intent("find all errors"),
            QueryIntent::Search
        );
        assert_eq!(
            parser.classify_intent("how many traces failed"),
            QueryIntent::Analyze
        );
        assert_eq!(
            parser.classify_intent("why did the agent fail"),
            QueryIntent::Explain
        );
        assert_eq!(
            parser.classify_intent("compare error rates"),
            QueryIntent::Compare
        );
    }

    #[test]
    fn test_temporal_extraction() {
        let parser = NLQueryParser::new();

        let constraint = parser.extract_temporal("show errors from last 24 hours");
        assert!(constraint.is_some());

        if let Some(TemporalConstraint::Relative { amount, unit }) = constraint {
            assert_eq!(amount, 24);
            assert_eq!(unit, TimeUnit::Hours);
        }
    }

    #[test]
    fn test_numeric_filter_extraction() {
        let parser = NLQueryParser::new();

        let filters = parser.extract_numeric_filters("slow responses over 5 seconds");
        assert!(!filters.is_empty());

        let filter = &filters[0];
        assert_eq!(filter.field, "duration_ms");
        assert_eq!(filter.operator, ComparisonOp::GreaterThan);
        assert_eq!(filter.value, 5000.0);
    }

    #[test]
    fn test_template_matching() {
        let parser = NLQueryParser::new();

        let parsed = parser.parse("show me errors from last hour");
        assert_eq!(parsed.intent, QueryIntent::Search);
        assert!(parsed.confidence > 0.8);
        assert!(!parsed.structural_filters.is_empty());
    }

    #[test]
    fn test_error_query_parsing() {
        let parser = NLQueryParser::new();

        let parsed = parser.parse("find traces where the agent failed");
        assert_eq!(parsed.intent, QueryIntent::Search);

        let has_error_filter = parsed
            .structural_filters
            .iter()
            .any(|f| matches!(f, StructuralFilter::HasError { has_error: true }));
        assert!(has_error_filter);
    }

    #[test]
    fn test_to_semantic_query() {
        let parser = NLQueryParser::new();

        let parsed = parser.parse("show errors from last 24 hours");
        let semantic = parser.to_semantic_query(&parsed);

        assert!(semantic.filters.has_error == Some(true));
        assert!(semantic.filters.time_range.is_some());
    }

    #[test]
    fn test_parsed_query_serialization() {
        let parsed = ParsedQuery {
            intent: QueryIntent::Search,
            temporal: Some(TemporalConstraint::Relative {
                amount: 1,
                unit: TimeUnit::Hours,
            }),
            numeric_filters: vec![],
            semantic_filters: vec![SemanticFilter::new("errors", 0.6)],
            structural_filters: vec![StructuralFilter::HasError { has_error: true }],
            aggregation: None,
            original_query: "find errors".to_string(),
            confidence: 0.8,
        };

        let json = serde_json::to_string(&parsed).unwrap();
        let restored: ParsedQuery = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.intent, QueryIntent::Search);
        assert_eq!(restored.confidence, 0.8);
    }
}
