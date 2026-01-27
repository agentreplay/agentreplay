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

package flowtrace

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"math/rand"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"sync/atomic"
	"time"
)

// Client is the Flowtrace client for Go applications.
type Client struct {
	url            string
	tenantID       int64
	projectID      int64
	agentID        int64
	timeout        time.Duration
	httpClient     *http.Client
	sessionCounter int64
}

// ClientOption is a function that configures a Client.
type ClientOption func(*Client)

// WithProjectID sets the project ID.
func WithProjectID(projectID int64) ClientOption {
	return func(c *Client) {
		c.projectID = projectID
	}
}

// WithAgentID sets the default agent ID.
func WithAgentID(agentID int64) ClientOption {
	return func(c *Client) {
		c.agentID = agentID
	}
}

// WithTimeout sets the request timeout.
func WithTimeout(timeout time.Duration) ClientOption {
	return func(c *Client) {
		c.timeout = timeout
		c.httpClient.Timeout = timeout
	}
}

// WithHTTPClient sets a custom HTTP client.
func WithHTTPClient(httpClient *http.Client) ClientOption {
	return func(c *Client) {
		c.httpClient = httpClient
	}
}

// NewClient creates a new Flowtrace client.
//
// Example:
//
//	client := vizu.NewClient(
//	    "http://localhost:8080",
//	    1, // tenantID
//	    vizu.WithProjectID(0),
//	    vizu.WithAgentID(1),
//	)
func NewClient(baseURL string, tenantID int64, opts ...ClientOption) *Client {
	c := &Client{
		url:       strings.TrimSuffix(baseURL, "/"),
		tenantID:  tenantID,
		projectID: 0,
		agentID:   1,
		timeout:   30 * time.Second,
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
			Transport: &http.Transport{
				MaxIdleConns:        100,
				MaxIdleConnsPerHost: 50,
				IdleConnTimeout:     30 * time.Second,
			},
		},
	}

	for _, opt := range opts {
		opt(c)
	}

	return c
}

// generateEdgeID generates a unique edge ID.
func generateEdgeID() string {
	timestamp := time.Now().UnixMilli()
	randomBits := rand.Intn(0xFFFF)
	edgeID := (timestamp << 16) | int64(randomBits)
	return fmt.Sprintf("%x", edgeID)
}

// nextSessionID returns the next session ID.
func (c *Client) nextSessionID() int64 {
	return atomic.AddInt64(&c.sessionCounter, 1)
}

// request makes an HTTP request to the Flowtrace server.
func (c *Client) request(ctx context.Context, method, path string, body interface{}, params map[string]string) ([]byte, error) {
	reqURL := c.url + path

	if len(params) > 0 {
		values := url.Values{}
		for k, v := range params {
			values.Set(k, v)
		}
		reqURL += "?" + values.Encode()
	}

	var bodyReader io.Reader
	if body != nil {
		bodyBytes, err := json.Marshal(body)
		if err != nil {
			return nil, fmt.Errorf("failed to marshal request body: %w", err)
		}
		bodyReader = bytes.NewReader(bodyBytes)
	}

	req, err := http.NewRequestWithContext(ctx, method, reqURL, bodyReader)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Tenant-ID", strconv.FormatInt(c.tenantID, 10))

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response body: %w", err)
	}

	if resp.StatusCode >= 400 {
		return nil, fmt.Errorf("Flowtrace API error (%d): %s", resp.StatusCode, string(respBody))
	}

	return respBody, nil
}

// CreateTrace creates a new trace span.
func (c *Client) CreateTrace(ctx context.Context, opts CreateTraceOptions) (*TraceResult, error) {
	edgeID := generateEdgeID()
	sessionID := opts.SessionID
	if sessionID == 0 {
		sessionID = c.nextSessionID()
	}
	startTimeUs := nowMicroseconds()

	attributes := map[string]string{
		"tenant_id":   strconv.FormatInt(c.tenantID, 10),
		"project_id":  strconv.FormatInt(c.projectID, 10),
		"agent_id":    strconv.FormatInt(opts.AgentID, 10),
		"session_id":  strconv.FormatInt(sessionID, 10),
		"span_type":   strconv.Itoa(int(opts.SpanType)),
		"token_count": "0",
		"duration_us": "0",
	}

	// Add metadata
	if opts.Metadata != nil {
		for k, v := range opts.Metadata {
			if k == "name" {
				continue
			}
			switch val := v.(type) {
			case string:
				attributes[k] = val
			default:
				attributes[k] = toJSON(v)
			}
		}
	}

	name := fmt.Sprintf("span_%d", opts.AgentID)
	if opts.Metadata != nil {
		if n, ok := opts.Metadata["name"].(string); ok {
			name = n
		}
	}

	var parentSpanID *string
	if opts.ParentID != "" {
		parentSpanID = &opts.ParentID
	}

	span := SpanInput{
		SpanID:       edgeID,
		TraceID:      strconv.FormatInt(sessionID, 10),
		ParentSpanID: parentSpanID,
		Name:         name,
		StartTime:    startTimeUs,
		EndTime:      &startTimeUs,
		Attributes:   attributes,
	}

	_, err := c.request(ctx, "POST", "/api/v1/traces", map[string]interface{}{"spans": []SpanInput{span}}, nil)
	if err != nil {
		return nil, err
	}

	return &TraceResult{
		EdgeID:    edgeID,
		TenantID:  c.tenantID,
		AgentID:   opts.AgentID,
		SessionID: sessionID,
		SpanType:  opts.SpanType,
	}, nil
}

// CreateGenAITrace creates a GenAI trace with OpenTelemetry semantic conventions.
func (c *Client) CreateGenAITrace(ctx context.Context, opts CreateGenAITraceOptions) (*GenAITraceResult, error) {
	edgeID := generateEdgeID()
	sessionID := opts.SessionID
	if sessionID == 0 {
		sessionID = c.nextSessionID()
	}
	startTimeUs := nowMicroseconds()
	operationName := opts.OperationName
	if operationName == "" {
		operationName = "chat"
	}

	attributes := map[string]string{
		"tenant_id":             strconv.FormatInt(c.tenantID, 10),
		"project_id":            strconv.FormatInt(c.projectID, 10),
		"agent_id":              strconv.FormatInt(opts.AgentID, 10),
		"session_id":            strconv.FormatInt(sessionID, 10),
		"span_type":             "0",
		"gen_ai.operation.name": operationName,
	}

	// Auto-detect system from model name
	system := opts.System
	if system == "" && opts.Model != "" {
		modelLower := strings.ToLower(opts.Model)
		switch {
		case strings.Contains(modelLower, "gpt") || strings.Contains(modelLower, "openai"):
			system = "openai"
		case strings.Contains(modelLower, "claude") || strings.Contains(modelLower, "anthropic"):
			system = "anthropic"
		case strings.Contains(modelLower, "llama") || strings.Contains(modelLower, "meta"):
			system = "meta"
		case strings.Contains(modelLower, "gemini") || strings.Contains(modelLower, "palm"):
			system = "google"
		default:
			system = "unknown"
		}
	}

	if system != "" {
		attributes["gen_ai.system"] = system
	}

	if opts.Model != "" {
		attributes["gen_ai.request.model"] = opts.Model
		attributes["gen_ai.response.model"] = opts.Model
	}

	// Model parameters
	if opts.ModelParameters != nil {
		for k, v := range opts.ModelParameters {
			attributes["gen_ai.request."+k] = fmt.Sprintf("%v", v)
		}
	}

	// Token usage
	if opts.InputUsage != nil {
		attributes["gen_ai.usage.prompt_tokens"] = strconv.Itoa(*opts.InputUsage)
		attributes["gen_ai.usage.input_tokens"] = strconv.Itoa(*opts.InputUsage)
	}
	if opts.OutputUsage != nil {
		attributes["gen_ai.usage.completion_tokens"] = strconv.Itoa(*opts.OutputUsage)
		attributes["gen_ai.usage.output_tokens"] = strconv.Itoa(*opts.OutputUsage)
	}
	if opts.TotalUsage != nil {
		attributes["gen_ai.usage.total_tokens"] = strconv.Itoa(*opts.TotalUsage)
		attributes["token_count"] = strconv.Itoa(*opts.TotalUsage)
	}

	if opts.FinishReason != "" {
		attributes["gen_ai.response.finish_reasons"] = toJSON([]string{opts.FinishReason})
	}

	// Input messages
	if len(opts.InputMessages) > 0 {
		attributes["gen_ai.prompt.messages"] = toJSON(opts.InputMessages)
	}

	// Output
	if opts.Output != nil {
		attributes["gen_ai.completion.message"] = toJSON(opts.Output)
	}

	// Additional metadata
	if opts.Metadata != nil {
		for k, v := range opts.Metadata {
			if _, exists := attributes[k]; !exists {
				switch val := v.(type) {
				case string:
					attributes["metadata."+k] = val
				default:
					attributes["metadata."+k] = toJSON(v)
				}
			}
		}
	}

	var parentSpanID *string
	if opts.ParentID != "" {
		parentSpanID = &opts.ParentID
	}

	model := opts.Model
	if model == "" {
		model = "unknown"
	}

	span := SpanInput{
		SpanID:       edgeID,
		TraceID:      strconv.FormatInt(sessionID, 10),
		ParentSpanID: parentSpanID,
		Name:         fmt.Sprintf("%s-%s", operationName, model),
		StartTime:    startTimeUs,
		EndTime:      &startTimeUs,
		Attributes:   attributes,
	}

	_, err := c.request(ctx, "POST", "/api/v1/traces", map[string]interface{}{"spans": []SpanInput{span}}, nil)
	if err != nil {
		return nil, err
	}

	return &GenAITraceResult{
		EdgeID:    edgeID,
		TenantID:  c.tenantID,
		AgentID:   opts.AgentID,
		SessionID: sessionID,
		Model:     opts.Model,
	}, nil
}

// CreateToolTrace creates a tool call trace.
func (c *Client) CreateToolTrace(ctx context.Context, opts CreateToolTraceOptions) (*ToolTraceResult, error) {
	edgeID := generateEdgeID()
	sessionID := opts.SessionID
	if sessionID == 0 {
		sessionID = c.nextSessionID()
	}
	startTimeUs := nowMicroseconds()

	attributes := map[string]string{
		"tenant_id":        strconv.FormatInt(c.tenantID, 10),
		"project_id":       strconv.FormatInt(c.projectID, 10),
		"agent_id":         strconv.FormatInt(opts.AgentID, 10),
		"session_id":       strconv.FormatInt(sessionID, 10),
		"span_type":        "3", // TOOL_CALL
		"gen_ai.tool.name": opts.ToolName,
	}

	if opts.ToolDescription != "" {
		attributes["gen_ai.tool.description"] = opts.ToolDescription
	}
	if opts.ToolInput != nil {
		attributes["gen_ai.tool.call.input"] = toJSON(opts.ToolInput)
	}
	if opts.ToolOutput != nil {
		attributes["gen_ai.tool.call.output"] = toJSON(opts.ToolOutput)
	}

	// Additional metadata
	if opts.Metadata != nil {
		for k, v := range opts.Metadata {
			if _, exists := attributes[k]; !exists {
				switch val := v.(type) {
				case string:
					attributes["metadata."+k] = val
				default:
					attributes["metadata."+k] = toJSON(v)
				}
			}
		}
	}

	var parentSpanID *string
	if opts.ParentID != "" {
		parentSpanID = &opts.ParentID
	}

	span := SpanInput{
		SpanID:       edgeID,
		TraceID:      strconv.FormatInt(sessionID, 10),
		ParentSpanID: parentSpanID,
		Name:         "tool-" + opts.ToolName,
		StartTime:    startTimeUs,
		EndTime:      &startTimeUs,
		Attributes:   attributes,
	}

	_, err := c.request(ctx, "POST", "/api/v1/traces", map[string]interface{}{"spans": []SpanInput{span}}, nil)
	if err != nil {
		return nil, err
	}

	return &ToolTraceResult{
		EdgeID:    edgeID,
		TenantID:  c.tenantID,
		AgentID:   opts.AgentID,
		SessionID: sessionID,
		ToolName:  opts.ToolName,
	}, nil
}

// UpdateTrace updates a trace with completion information.
func (c *Client) UpdateTrace(ctx context.Context, opts UpdateTraceOptions) error {
	endTimeUs := nowMicroseconds()
	var durationUs int64 = 1000

	if opts.DurationUs != nil {
		durationUs = *opts.DurationUs
	} else if opts.DurationMs != nil {
		durationUs = *opts.DurationMs * 1000
	}

	startTimeUs := endTimeUs - durationUs

	tokenCount := 0
	if opts.TokenCount != nil {
		tokenCount = *opts.TokenCount
	}

	attributes := map[string]string{
		"tenant_id":   strconv.FormatInt(c.tenantID, 10),
		"project_id":  strconv.FormatInt(c.projectID, 10),
		"agent_id":    strconv.FormatInt(c.agentID, 10),
		"session_id":  strconv.FormatInt(opts.SessionID, 10),
		"span_type":   "6", // RESPONSE
		"token_count": strconv.Itoa(tokenCount),
		"duration_us": strconv.FormatInt(durationUs, 10),
	}

	if opts.Payload != nil {
		for k, v := range opts.Payload {
			switch val := v.(type) {
			case string:
				attributes["payload."+k] = val
			default:
				attributes["payload."+k] = toJSON(v)
			}
		}
	}

	span := SpanInput{
		SpanID:       opts.EdgeID + "_complete",
		TraceID:      strconv.FormatInt(opts.SessionID, 10),
		ParentSpanID: &opts.EdgeID,
		Name:         "RESPONSE",
		StartTime:    startTimeUs,
		EndTime:      &endTimeUs,
		Attributes:   attributes,
	}

	_, err := c.request(ctx, "POST", "/api/v1/traces", map[string]interface{}{"spans": []SpanInput{span}}, nil)
	return err
}

// IngestBatch ingests multiple spans in a batch.
func (c *Client) IngestBatch(ctx context.Context, spans []SpanInput) (*IngestResponse, error) {
	respBody, err := c.request(ctx, "POST", "/api/v1/traces", map[string]interface{}{"spans": spans}, nil)
	if err != nil {
		return nil, err
	}

	var resp IngestResponse
	if err := json.Unmarshal(respBody, &resp); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &resp, nil
}

// QueryTraces queries traces with optional filters.
func (c *Client) QueryTraces(ctx context.Context, filter *QueryFilter) (*QueryResponse, error) {
	params := make(map[string]string)

	if filter != nil {
		if filter.ProjectID != nil {
			params["project_id"] = strconv.FormatInt(*filter.ProjectID, 10)
		}
		if filter.AgentID != nil {
			params["agent_id"] = strconv.FormatInt(*filter.AgentID, 10)
		}
		if filter.SessionID != nil {
			params["session_id"] = strconv.FormatInt(*filter.SessionID, 10)
		}
		if filter.Environment != "" {
			params["environment"] = string(filter.Environment)
		}
		if filter.ExcludePII {
			params["exclude_pii"] = "true"
		}
		if filter.ExcludeSecrets {
			params["exclude_secrets"] = "true"
		}
		if filter.Limit > 0 {
			params["limit"] = strconv.Itoa(filter.Limit)
		}
		if filter.Offset > 0 {
			params["offset"] = strconv.Itoa(filter.Offset)
		}
	}

	respBody, err := c.request(ctx, "GET", "/api/v1/traces", nil, params)
	if err != nil {
		return nil, err
	}

	var resp QueryResponse
	if err := json.Unmarshal(respBody, &resp); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &resp, nil
}

// QueryTemporalRange queries traces within a time range.
func (c *Client) QueryTemporalRange(ctx context.Context, startUs, endUs int64, filter *QueryFilter) (*QueryResponse, error) {
	params := map[string]string{
		"start_ts": strconv.FormatInt(startUs, 10),
		"end_ts":   strconv.FormatInt(endUs, 10),
	}

	if filter != nil {
		if filter.SessionID != nil {
			params["session_id"] = strconv.FormatInt(*filter.SessionID, 10)
		}
		if filter.AgentID != nil {
			params["agent_id"] = strconv.FormatInt(*filter.AgentID, 10)
		}
		if filter.Environment != "" {
			params["environment"] = string(filter.Environment)
		}
		if filter.ExcludePII {
			params["exclude_pii"] = "true"
		}
		if filter.Limit > 0 {
			params["limit"] = strconv.Itoa(filter.Limit)
		}
		if filter.Offset > 0 {
			params["offset"] = strconv.Itoa(filter.Offset)
		}
	}

	respBody, err := c.request(ctx, "GET", "/api/v1/traces", nil, params)
	if err != nil {
		return nil, err
	}

	var resp QueryResponse
	if err := json.Unmarshal(respBody, &resp); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &resp, nil
}

// GetTrace gets a specific trace by ID.
func (c *Client) GetTrace(ctx context.Context, traceID string) (*TraceView, error) {
	respBody, err := c.request(ctx, "GET", "/api/v1/traces/"+traceID, nil, nil)
	if err != nil {
		return nil, err
	}

	var resp TraceView
	if err := json.Unmarshal(respBody, &resp); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &resp, nil
}

// GetTraceTree gets the hierarchical trace tree.
func (c *Client) GetTraceTree(ctx context.Context, traceID string) (*TraceTreeResponse, error) {
	respBody, err := c.request(ctx, "GET", "/api/v1/traces/"+traceID+"/tree", nil, nil)
	if err != nil {
		return nil, err
	}

	var resp TraceTreeResponse
	if err := json.Unmarshal(respBody, &resp); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &resp, nil
}

// FilterBySession gets all traces in a session.
func (c *Client) FilterBySession(ctx context.Context, sessionID int64) ([]TraceView, error) {
	resp, err := c.QueryTraces(ctx, &QueryFilter{SessionID: &sessionID})
	if err != nil {
		return nil, err
	}
	return resp.Traces, nil
}

// SubmitFeedback submits user feedback for a trace.
func (c *Client) SubmitFeedback(ctx context.Context, traceID string, feedback int) (*FeedbackResponse, error) {
	if feedback < -1 || feedback > 1 {
		return nil, fmt.Errorf("feedback must be -1, 0, or 1")
	}

	respBody, err := c.request(ctx, "POST", "/api/v1/traces/"+traceID+"/feedback", map[string]int{"feedback": feedback}, nil)
	if err != nil {
		return nil, err
	}

	var resp FeedbackResponse
	if err := json.Unmarshal(respBody, &resp); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &resp, nil
}

// AddToDataset adds a trace to an evaluation dataset.
func (c *Client) AddToDataset(ctx context.Context, traceID, datasetName string, inputData, outputData map[string]interface{}) (*DatasetResponse, error) {
	payload := map[string]interface{}{
		"trace_id": traceID,
	}
	if inputData != nil {
		payload["input"] = inputData
	}
	if outputData != nil {
		payload["output"] = outputData
	}

	respBody, err := c.request(ctx, "POST", "/api/v1/datasets/"+datasetName+"/add", payload, nil)
	if err != nil {
		return nil, err
	}

	var resp DatasetResponse
	if err := json.Unmarshal(respBody, &resp); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &resp, nil
}

// Health checks server health.
func (c *Client) Health(ctx context.Context) (*HealthResponse, error) {
	respBody, err := c.request(ctx, "GET", "/api/v1/health", nil, nil)
	if err != nil {
		return nil, err
	}

	var resp HealthResponse
	if err := json.Unmarshal(respBody, &resp); err != nil {
		return nil, fmt.Errorf("failed to unmarshal response: %w", err)
	}

	return &resp, nil
}

// Close closes the client and releases resources.
func (c *Client) Close() {
	c.httpClient.CloseIdleConnections()
}
