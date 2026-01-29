# Agentreplay Rust SDK

High-performance observability SDK for LLM agents and AI applications.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
agentreplay-client = "0.1"
tokio = { version = "1.0", features = ["rt-multi-thread", "macros"] }
```

## Quick Start

```rust
use agentreplay_client::{AgentreplayClient, ClientConfig, SpanType, CreateTraceOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let config = ClientConfig::new("http://localhost:8080", 1)
        .with_project_id(0)
        .with_agent_id(1);

    let client = AgentreplayClient::new(config);

    // Create a basic trace
    let trace = client.create_trace(CreateTraceOptions {
        agent_id: 1,
        session_id: Some(123),
        span_type: SpanType::Root,
        metadata: Some([("name".into(), "my-agent".into())].into_iter().collect()),
        ..Default::default()
    }).await?;

    println!("Created trace: {}", trace.edge_id);
    Ok(())
}
```

## Tracking LLM Calls

The SDK supports OpenTelemetry GenAI semantic conventions:

```rust
use agentreplay_client::{AgentreplayClient, ClientConfig, CreateGenAITraceOptions, Message};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = AgentreplayClient::new(ClientConfig::new("http://localhost:8080", 1));

    let trace = client.create_genai_trace(CreateGenAITraceOptions {
        agent_id: 1,
        session_id: Some(123),
        model: Some("gpt-4o".into()),
        input_messages: vec![
            Message::system("You are a helpful assistant."),
            Message::user("What is the capital of France?"),
        ],
        output: Some(Message::assistant("The capital of France is Paris.")),
        model_parameters: Some({
            let mut params = HashMap::new();
            params.insert("temperature".into(), serde_json::json!(0.7));
            params.insert("max_tokens".into(), serde_json::json!(1000));
            params
        }),
        input_usage: Some(25),
        output_usage: Some(12),
        total_usage: Some(37),
        finish_reason: Some("stop".into()),
        ..Default::default()
    }).await?;

    println!("Created GenAI trace: {}", trace.edge_id);
    Ok(())
}
```

## Tracking Tool Calls

```rust
use agentreplay_client::{AgentreplayClient, ClientConfig, CreateToolTraceOptions};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = AgentreplayClient::new(ClientConfig::new("http://localhost:8080", 1));

    let trace = client.create_tool_trace(CreateToolTraceOptions {
        agent_id: 1,
        session_id: Some(123),
        tool_name: "web_search".into(),
        tool_input: Some({
            let mut input = HashMap::new();
            input.insert("query".into(), serde_json::json!("weather in Paris"));
            input
        }),
        tool_output: Some({
            let mut output = HashMap::new();
            output.insert("results".into(), serde_json::json!(["sunny", "20°C"]));
            output
        }),
        tool_description: Some("Search the web for information".into()),
        parent_id: Some("parent_edge_id".into()),
        ..Default::default()
    }).await?;

    println!("Created tool trace: {}", trace.edge_id);
    Ok(())
}
```

## Querying Traces

```rust
use agentreplay_client::{AgentreplayClient, ClientConfig, QueryFilter};
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = AgentreplayClient::new(ClientConfig::new("http://localhost:8080", 1));

    // Query traces with filters
    let results = client.query_traces(Some(&QueryFilter {
        session_id: Some(123),
        limit: Some(100),
        ..Default::default()
    })).await?;

    // Query within a time range
    let now = Utc::now().timestamp_micros();
    let hour_ago = now - 3600_000_000;

    let range_results = client.query_temporal_range(hour_ago, now, Some(&QueryFilter {
        agent_id: Some(1),
        ..Default::default()
    })).await?;

    // Get a specific trace with payload
    let trace = client.get_trace("abc123").await?;

    // Get trace hierarchy
    let tree = client.get_trace_tree("abc123").await?;

    Ok(())
}
```

## User Feedback

```rust
use agentreplay_client::{AgentreplayClient, ClientConfig};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = AgentreplayClient::new(ClientConfig::new("http://localhost:8080", 1));

    // Submit feedback
    client.submit_feedback("trace_id", 1).await?;  // thumbs up
    client.submit_feedback("trace_id", -1).await?; // thumbs down

    // Add to evaluation dataset
    let mut input = HashMap::new();
    input.insert("prompt".into(), serde_json::json!("Hello"));
    let mut output = HashMap::new();
    output.insert("response".into(), serde_json::json!("..."));

    client.add_to_dataset("trace_id", "bad_responses", Some(&input), Some(&output)).await?;

    Ok(())
}
```

## Span Types

```rust
use agentreplay_client::SpanType;

SpanType::Root         // 0 - Root span
SpanType::Planning     // 1 - Planning phase
SpanType::Reasoning    // 2 - Reasoning/thinking
SpanType::ToolCall     // 3 - Tool/function call
SpanType::ToolResponse // 4 - Tool response
SpanType::Synthesis    // 5 - Result synthesis
SpanType::Response     // 6 - Final response
SpanType::Error        // 7 - Error state
SpanType::Retrieval    // 8 - Vector DB retrieval
SpanType::Embedding    // 9 - Text embedding
SpanType::HttpCall     // 10 - HTTP API call
SpanType::Database     // 11 - Database query
SpanType::Function     // 12 - Generic function
SpanType::Reranking    // 13 - Result reranking
SpanType::Parsing      // 14 - Document parsing
SpanType::Generation   // 15 - Content generation
SpanType::Custom       // 255 - Custom types
```

## Configuration Options

```rust
use agentreplay_client::ClientConfig;
use std::time::Duration;

let config = ClientConfig::new("http://localhost:8080", 1)
    .with_project_id(0)           // Project ID
    .with_agent_id(1)             // Default agent ID
    .with_timeout(Duration::from_secs(30)); // Request timeout
```

## Error Handling

```rust
use agentreplay_client::{AgentreplayClient, ClientConfig, AgentreplayError, CreateTraceOptions, SpanType};

#[tokio::main]
async fn main() {
    let client = AgentreplayClient::new(ClientConfig::new("http://localhost:8080", 1));

    match client.create_trace(CreateTraceOptions {
        agent_id: 1,
        span_type: SpanType::Root,
        ..Default::default()
    }).await {
        Ok(trace) => println!("Created: {}", trace.edge_id),
        Err(AgentreplayError::ApiError { status, message }) => {
            eprintln!("API error ({}): {}", status, message);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Message Helper Methods

```rust
use agentreplay_client::Message;

// Using helper constructors
let system_msg = Message::system("You are a helpful assistant.");
let user_msg = Message::user("What is the capital of France?");
let assistant_msg = Message::assistant("The capital of France is Paris.");

// Using new() for custom roles
let custom_msg = Message::new("function", "function result here");
```

## License

MIT
