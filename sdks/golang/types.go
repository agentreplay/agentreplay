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

// Package flowtrace provides a high-performance client for the Flowtrace
// observability platform, designed for LLM agents and AI applications.
package flowtrace

import (
	"encoding/json"
	"time"
)

// SpanType represents the type of agent execution span.
type SpanType int

const (
	// SpanTypeRoot is the root/top-level span
	SpanTypeRoot SpanType = 0
	// SpanTypePlanning represents planning phase
	SpanTypePlanning SpanType = 1
	// SpanTypeReasoning represents reasoning/thinking phase
	SpanTypeReasoning SpanType = 2
	// SpanTypeToolCall represents a tool/function call
	SpanTypeToolCall SpanType = 3
	// SpanTypeToolResponse represents tool response
	SpanTypeToolResponse SpanType = 4
	// SpanTypeSynthesis represents result synthesis
	SpanTypeSynthesis SpanType = 5
	// SpanTypeResponse represents final response
	SpanTypeResponse SpanType = 6
	// SpanTypeError represents error state
	SpanTypeError SpanType = 7
	// SpanTypeRetrieval represents vector DB retrieval
	SpanTypeRetrieval SpanType = 8
	// SpanTypeEmbedding represents text embedding
	SpanTypeEmbedding SpanType = 9
	// SpanTypeHttpCall represents HTTP API call
	SpanTypeHttpCall SpanType = 10
	// SpanTypeDatabase represents database query
	SpanTypeDatabase SpanType = 11
	// SpanTypeFunction represents generic function
	SpanTypeFunction SpanType = 12
	// SpanTypeReranking represents result reranking
	SpanTypeReranking SpanType = 13
	// SpanTypeParsing represents document parsing
	SpanTypeParsing SpanType = 14
	// SpanTypeGeneration represents content generation
	SpanTypeGeneration SpanType = 15
	// SpanTypeCustom represents custom types (use values >= 16)
	SpanTypeCustom SpanType = 255
)

// String returns the string representation of SpanType.
func (s SpanType) String() string {
	names := map[SpanType]string{
		SpanTypeRoot:         "Root",
		SpanTypePlanning:     "Planning",
		SpanTypeReasoning:    "Reasoning",
		SpanTypeToolCall:     "ToolCall",
		SpanTypeToolResponse: "ToolResponse",
		SpanTypeSynthesis:    "Synthesis",
		SpanTypeResponse:     "Response",
		SpanTypeError:        "Error",
		SpanTypeRetrieval:    "Retrieval",
		SpanTypeEmbedding:    "Embedding",
		SpanTypeHttpCall:     "HttpCall",
		SpanTypeDatabase:     "Database",
		SpanTypeFunction:     "Function",
		SpanTypeReranking:    "Reranking",
		SpanTypeParsing:      "Parsing",
		SpanTypeGeneration:   "Generation",
		SpanTypeCustom:       "Custom",
	}
	if name, ok := names[s]; ok {
		return name
	}
	return "Unknown"
}

// SensitivityFlags represents sensitivity flags for PII and redaction control.
type SensitivityFlags uint8

const (
	// SensitivityNone indicates no special sensitivity
	SensitivityNone SensitivityFlags = 0
	// SensitivityPII indicates personally identifiable information
	SensitivityPII SensitivityFlags = 1 << 0
	// SensitivitySecret indicates secrets/credentials
	SensitivitySecret SensitivityFlags = 1 << 1
	// SensitivityInternal indicates internal-only data
	SensitivityInternal SensitivityFlags = 1 << 2
	// SensitivityNoEmbed indicates never embed in vector index
	SensitivityNoEmbed SensitivityFlags = 1 << 3
)

// Environment represents deployment environment.
type Environment string

const (
	EnvironmentDevelopment Environment = "development"
	EnvironmentStaging     Environment = "staging"
	EnvironmentProduction  Environment = "production"
)

// TraceResult contains the result of creating a trace.
type TraceResult struct {
	EdgeID    string   `json:"edge_id"`
	TenantID  int64    `json:"tenant_id"`
	AgentID   int64    `json:"agent_id"`
	SessionID int64    `json:"session_id"`
	SpanType  SpanType `json:"span_type"`
}

// GenAITraceResult contains the result of creating a GenAI trace.
type GenAITraceResult struct {
	EdgeID    string `json:"edge_id"`
	TenantID  int64  `json:"tenant_id"`
	AgentID   int64  `json:"agent_id"`
	SessionID int64  `json:"session_id"`
	Model     string `json:"model,omitempty"`
}

// ToolTraceResult contains the result of creating a tool trace.
type ToolTraceResult struct {
	EdgeID    string `json:"edge_id"`
	TenantID  int64  `json:"tenant_id"`
	AgentID   int64  `json:"agent_id"`
	SessionID int64  `json:"session_id"`
	ToolName  string `json:"tool_name"`
}

// TraceView represents a trace as returned by the API.
type TraceView struct {
	EdgeID      string                 `json:"edge_id"`
	TenantID    int64                  `json:"tenant_id"`
	ProjectID   int64                  `json:"project_id"`
	AgentID     int64                  `json:"agent_id"`
	AgentName   string                 `json:"agent_name,omitempty"`
	SessionID   int64                  `json:"session_id"`
	SpanType    string                 `json:"span_type"`
	TimestampUs int64                  `json:"timestamp_us"`
	DurationUs  int64                  `json:"duration_us"`
	TokenCount  int                    `json:"token_count"`
	Confidence  float64                `json:"confidence"`
	Environment string                 `json:"environment"`
	HasPayload  bool                   `json:"has_payload"`
	Metadata    map[string]interface{} `json:"metadata,omitempty"`
}

// QueryResponse represents the response from query operations.
type QueryResponse struct {
	Traces []TraceView `json:"traces"`
	Total  int         `json:"total"`
	Limit  int         `json:"limit"`
	Offset int         `json:"offset"`
}

// QueryFilter contains filters for querying traces.
type QueryFilter struct {
	TenantID       int64       `json:"tenant_id,omitempty"`
	ProjectID      *int64      `json:"project_id,omitempty"`
	AgentID        *int64      `json:"agent_id,omitempty"`
	SessionID      *int64      `json:"session_id,omitempty"`
	SpanType       *SpanType   `json:"span_type,omitempty"`
	MinConfidence  *float64    `json:"min_confidence,omitempty"`
	ExcludePII     bool        `json:"exclude_pii,omitempty"`
	ExcludeSecrets bool        `json:"exclude_secrets,omitempty"`
	Environment    Environment `json:"environment,omitempty"`
	Limit          int         `json:"limit,omitempty"`
	Offset         int         `json:"offset,omitempty"`
}

// SpanInput represents a span for ingestion.
type SpanInput struct {
	SpanID       string            `json:"span_id"`
	TraceID      string            `json:"trace_id"`
	ParentSpanID *string           `json:"parent_span_id,omitempty"`
	Name         string            `json:"name"`
	StartTime    int64             `json:"start_time"`
	EndTime      *int64            `json:"end_time,omitempty"`
	Attributes   map[string]string `json:"attributes"`
}

// IngestResponse represents the response from batch ingestion.
type IngestResponse struct {
	Accepted int      `json:"accepted"`
	Rejected int      `json:"rejected"`
	Errors   []string `json:"errors"`
}

// TraceTreeNode represents a node in the trace hierarchy.
type TraceTreeNode struct {
	EdgeID     string                 `json:"edge_id"`
	SpanType   string                 `json:"span_type"`
	DurationUs int64                  `json:"duration_us"`
	Children   []TraceTreeNode        `json:"children"`
	Metadata   map[string]interface{} `json:"metadata,omitempty"`
}

// TraceTreeResponse represents the response from getting a trace tree.
type TraceTreeResponse struct {
	Root TraceTreeNode `json:"root"`
}

// FeedbackResponse represents the response from submitting feedback.
type FeedbackResponse struct {
	Success bool   `json:"success"`
	Message string `json:"message"`
}

// DatasetResponse represents the response from adding to a dataset.
type DatasetResponse struct {
	Success     bool   `json:"success"`
	DatasetName string `json:"dataset_name"`
}

// HealthResponse represents the response from health check.
type HealthResponse struct {
	Status  string `json:"status"`
	Version string `json:"version,omitempty"`
}

// Message represents a chat message.
type Message struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

// CreateTraceOptions contains options for creating a trace.
type CreateTraceOptions struct {
	AgentID   int64
	SessionID int64
	SpanType  SpanType
	ParentID  string
	Metadata  map[string]interface{}
}

// CreateGenAITraceOptions contains options for creating a GenAI trace.
type CreateGenAITraceOptions struct {
	AgentID         int64
	SessionID       int64
	InputMessages   []Message
	Output          *Message
	Model           string
	ModelParameters map[string]interface{}
	InputUsage      *int
	OutputUsage     *int
	TotalUsage      *int
	ParentID        string
	Metadata        map[string]interface{}
	OperationName   string
	FinishReason    string
	System          string
}

// CreateToolTraceOptions contains options for creating a tool trace.
type CreateToolTraceOptions struct {
	AgentID         int64
	SessionID       int64
	ToolName        string
	ToolInput       map[string]interface{}
	ToolOutput      map[string]interface{}
	ToolDescription string
	ParentID        string
	Metadata        map[string]interface{}
}

// UpdateTraceOptions contains options for updating a trace.
type UpdateTraceOptions struct {
	EdgeID     string
	SessionID  int64
	TokenCount *int
	DurationUs *int64
	DurationMs *int64
	Payload    map[string]interface{}
}

// nowMicroseconds returns current timestamp in microseconds.
func nowMicroseconds() int64 {
	return time.Now().UnixMicro()
}

// toJSON converts a value to JSON string.
func toJSON(v interface{}) string {
	b, err := json.Marshal(v)
	if err != nil {
		return "{}"
	}
	return string(b)
}
