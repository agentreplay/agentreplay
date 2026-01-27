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

// Package main demonstrates basic usage of the Vizu Go SDK.
package main

import (
	"context"
	"fmt"
	"log"
	"time"

	vizu "github.com/sushanthpy/vizu/sdks/golang"
)

func main() {
	// Initialize client
	client := vizu.NewClient(
		"http://localhost:8080",
		1, // tenantID
		vizu.WithProjectID(0),
		vizu.WithAgentID(1),
	)
	defer client.Close()

	ctx := context.Background()
	sessionID := time.Now().UnixMilli()

	fmt.Println("Vizu Go SDK Example")
	fmt.Println()

	// 1. Create a root trace
	fmt.Println("1. Creating root trace...")
	rootTrace, err := client.CreateTrace(ctx, vizu.CreateTraceOptions{
		AgentID:   1,
		SessionID: sessionID,
		SpanType:  vizu.SpanTypeRoot,
		Metadata:  map[string]interface{}{"name": "example-agent"},
	})
	if err != nil {
		log.Printf("   Warning: %v\n", err)
	} else {
		fmt.Printf("   Created: %s\n\n", rootTrace.EdgeID)
	}

	// 2. Create a planning span (child of root)
	fmt.Println("2. Creating planning span...")
	planningTrace, err := client.CreateTrace(ctx, vizu.CreateTraceOptions{
		AgentID:   1,
		SessionID: sessionID,
		SpanType:  vizu.SpanTypePlanning,
		ParentID:  rootTrace.EdgeID,
		Metadata:  map[string]interface{}{"step": "analyze_request"},
	})
	if err != nil {
		log.Printf("   Warning: %v\n", err)
	} else {
		fmt.Printf("   Created: %s\n\n", planningTrace.EdgeID)
	}

	// 3. Track an LLM call with GenAI attributes
	fmt.Println("3. Creating GenAI trace (LLM call)...")
	inputUsage := 25
	outputUsage := 12
	totalUsage := 37

	llmTrace, err := client.CreateGenAITrace(ctx, vizu.CreateGenAITraceOptions{
		AgentID:   1,
		SessionID: sessionID,
		Model:     "gpt-4o",
		InputMessages: []vizu.Message{
			{Role: "system", Content: "You are a helpful assistant."},
			{Role: "user", Content: "What is the capital of France?"},
		},
		Output: &vizu.Message{
			Role:    "assistant",
			Content: "The capital of France is Paris.",
		},
		ModelParameters: map[string]interface{}{
			"temperature": 0.7,
			"max_tokens":  1000,
		},
		InputUsage:   &inputUsage,
		OutputUsage:  &outputUsage,
		TotalUsage:   &totalUsage,
		ParentID:     planningTrace.EdgeID,
		FinishReason: "stop",
	})
	if err != nil {
		log.Printf("   Warning: %v\n", err)
	} else {
		fmt.Printf("   Created: %s\n", llmTrace.EdgeID)
		fmt.Printf("   Model: %s\n\n", llmTrace.Model)
	}

	// 4. Track a tool call
	fmt.Println("4. Creating tool trace...")
	toolTrace, err := client.CreateToolTrace(ctx, vizu.CreateToolTraceOptions{
		AgentID:   1,
		SessionID: sessionID,
		ToolName:  "web_search",
		ToolInput: map[string]interface{}{
			"query": "Paris population 2024",
		},
		ToolOutput: map[string]interface{}{
			"result": "Paris has a population of approximately 2.1 million",
		},
		ToolDescription: "Search the web for information",
		ParentID:        llmTrace.EdgeID,
	})
	if err != nil {
		log.Printf("   Warning: %v\n", err)
	} else {
		fmt.Printf("   Created: %s\n", toolTrace.EdgeID)
		fmt.Printf("   Tool: %s\n\n", toolTrace.ToolName)
	}

	// 5. Create a response span
	fmt.Println("5. Creating response span...")
	responseTrace, err := client.CreateTrace(ctx, vizu.CreateTraceOptions{
		AgentID:   1,
		SessionID: sessionID,
		SpanType:  vizu.SpanTypeResponse,
		ParentID:  rootTrace.EdgeID,
		Metadata: map[string]interface{}{
			"final_answer": "The capital of France is Paris, with a population of about 2.1 million.",
		},
	})
	if err != nil {
		log.Printf("   Warning: %v\n", err)
	} else {
		fmt.Printf("   Created: %s\n\n", responseTrace.EdgeID)
	}

	// 6. Update trace with final metrics
	fmt.Println("6. Updating trace with completion info...")
	tokenCount := 50
	durationMs := int64(1500)
	err = client.UpdateTrace(ctx, vizu.UpdateTraceOptions{
		EdgeID:     rootTrace.EdgeID,
		SessionID:  sessionID,
		TokenCount: &tokenCount,
		DurationMs: &durationMs,
	})
	if err != nil {
		log.Printf("   Warning: %v\n", err)
	} else {
		fmt.Println("   Updated successfully")
	}

	// 7. Query traces
	fmt.Println("7. Querying traces for session...")
	traces, err := client.FilterBySession(ctx, sessionID)
	if err != nil {
		log.Printf("   Warning: %v\n", err)
	} else {
		fmt.Printf("   Found %d traces in session %d\n\n", len(traces), sessionID)
	}

	// 8. Submit feedback
	fmt.Println("8. Submitting user feedback...")
	_, err = client.SubmitFeedback(ctx, rootTrace.EdgeID, 1) // thumbs up
	if err != nil {
		fmt.Println("   Feedback endpoint not available (expected in demo)")
	} else {
		fmt.Println("   Feedback submitted (thumbs up)")
	}

	// 9. Health check
	fmt.Println("9. Checking server health...")
	health, err := client.Health(ctx)
	if err != nil {
		fmt.Println("   Server not running (expected in demo)")
	} else {
		fmt.Printf("   Status: %s\n\n", health.Status)
	}

	fmt.Println("Example complete!")
}
