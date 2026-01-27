# Flowtrace Go SDK

High-performance observability SDK for LLM agents and AI applications.

## Installation

```bash
go get github.com/flowtrace/flowtrace-go
```

## Quick Start

```go
package main

import (
    "context"
    "fmt"
    "log"

    flowtrace "github.com/flowtrace/flowtrace-go"
)

func main() {
    // Create client
    client := flowtrace.NewClient(
        "http://localhost:8080",
        1, // tenantID
        flowtrace.WithProjectID(0),
        flowtrace.WithAgentID(1),
    )
    defer client.Close()

    ctx := context.Background()

    // Create a basic trace
    trace, err := client.CreateTrace(ctx, flowtrace.CreateTraceOptions{
        AgentID:   1,
        SessionID: 123,
        SpanType:  flowtrace.SpanTypeRoot,
        Metadata:  map[string]interface{}{"name": "my-agent"},
    })
    if err != nil {
        log.Fatal(err)
    }

    fmt.Printf("Created trace: %s\n", trace.EdgeID)
}
```

## Tracking LLM Calls

The SDK supports OpenTelemetry GenAI semantic conventions:

```go
inputUsage := 25
outputUsage := 12
totalUsage := 37

llmTrace, err := client.CreateGenAITrace(ctx, flowtrace.CreateGenAITraceOptions{
    AgentID:   1,
    SessionID: 123,
    Model:     "gpt-4o",
    InputMessages: []flowtrace.Message{
        {Role: "system", Content: "You are a helpful assistant."},
        {Role: "user", Content: "What is the capital of France?"},
    },
    Output: &flowtrace.Message{
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
    FinishReason: "stop",
})
```

## Tracking Tool Calls

```go
toolTrace, err := client.CreateToolTrace(ctx, flowtrace.CreateToolTraceOptions{
    AgentID:   1,
    SessionID: 123,
    ToolName:  "web_search",
    ToolInput: map[string]interface{}{
        "query": "weather in Paris",
    },
    ToolOutput: map[string]interface{}{
        "results": []string{"sunny", "20°C"},
    },
    ToolDescription: "Search the web for information",
    ParentID:        llmTrace.EdgeID, // Link to parent
})
```

## Querying Traces

```go
// Query traces with filters
sessionID := int64(123)
results, err := client.QueryTraces(ctx, &flowtrace.QueryFilter{
    SessionID: &sessionID,
    Limit:     100,
})

// Query within a time range
now := time.Now().UnixMicro()
hourAgo := now - 3600_000_000

rangeResults, err := client.QueryTemporalRange(ctx, hourAgo, now, &flowtrace.QueryFilter{
    AgentID: &agentID,
})

// Get a specific trace with payload
trace, err := client.GetTrace(ctx, "abc123")

// Get trace hierarchy
tree, err := client.GetTraceTree(ctx, "abc123")
```

## User Feedback

```go
// Submit feedback
_, err := client.SubmitFeedback(ctx, trace.EdgeID, 1)  // thumbs up
_, err = client.SubmitFeedback(ctx, trace.EdgeID, -1)  // thumbs down

// Add to evaluation dataset
_, err = client.AddToDataset(ctx, trace.EdgeID, "bad_responses",
    map[string]interface{}{"prompt": "Hello"},
    map[string]interface{}{"response": "..."},
)
```

## Span Types

```go
flowtrace.SpanTypeRoot         // 0 - Root span
flowtrace.SpanTypePlanning     // 1 - Planning phase
flowtrace.SpanTypeReasoning    // 2 - Reasoning/thinking
flowtrace.SpanTypeToolCall     // 3 - Tool/function call
flowtrace.SpanTypeToolResponse // 4 - Tool response
flowtrace.SpanTypeSynthesis    // 5 - Result synthesis
flowtrace.SpanTypeResponse     // 6 - Final response
flowtrace.SpanTypeError        // 7 - Error state
flowtrace.SpanTypeRetrieval    // 8 - Vector DB retrieval
flowtrace.SpanTypeEmbedding    // 9 - Text embedding
flowtrace.SpanTypeHttpCall     // 10 - HTTP API call
flowtrace.SpanTypeDatabase     // 11 - Database query
flowtrace.SpanTypeFunction     // 12 - Generic function
flowtrace.SpanTypeReranking    // 13 - Result reranking
flowtrace.SpanTypeParsing      // 14 - Document parsing
flowtrace.SpanTypeGeneration   // 15 - Content generation
flowtrace.SpanTypeCustom       // 255 - Custom types
```

## Configuration Options

```go
client := flowtrace.NewClient(
    "http://localhost:8080",
    tenantID,
    flowtrace.WithProjectID(0),         // Project ID
    flowtrace.WithAgentID(1),           // Default agent ID
    flowtrace.WithTimeout(30*time.Second), // Request timeout
    flowtrace.WithHTTPClient(customClient), // Custom HTTP client
)
```

## Error Handling

```go
trace, err := client.CreateTrace(ctx, opts)
if err != nil {
    // Handle error
    log.Printf("Failed to create trace: %v", err)
    return
}
```

## Framework Integrations

### With OpenAI Go SDK

```go
import (
    "github.com/sashabaranov/go-openai"
    flowtrace "github.com/flowtrace/flowtrace-go"
)

func chat(client *openai.Client, ft *flowtrace.Client, messages []openai.ChatCompletionMessage) (string, error) {
    resp, err := client.CreateChatCompletion(ctx, openai.ChatCompletionRequest{
        Model:       openai.GPT4o,
        Messages:    messages,
        Temperature: 0.7,
    })
    if err != nil {
        return "", err
    }

    // Track the call
    inputTokens := resp.Usage.PromptTokens
    outputTokens := resp.Usage.CompletionTokens
    totalTokens := resp.Usage.TotalTokens

    _, _ = ft.CreateGenAITrace(ctx, flowtrace.CreateGenAITraceOptions{
        AgentID:   1,
        SessionID: time.Now().UnixMilli(),
        Model:     resp.Model,
        InputMessages: convertMessages(messages),
        Output: &flowtrace.Message{
            Role:    "assistant",
            Content: resp.Choices[0].Message.Content,
        },
        InputUsage:   &inputTokens,
        OutputUsage:  &outputTokens,
        TotalUsage:   &totalTokens,
        FinishReason: string(resp.Choices[0].FinishReason),
    })

    return resp.Choices[0].Message.Content, nil
}
```

## License

MIT
