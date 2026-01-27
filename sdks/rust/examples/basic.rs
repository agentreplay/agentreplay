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

//! Flowtrace SDK Basic Example
//!
//! Demonstrates core functionality of the Flowtrace Rust SDK.

use std::collections::HashMap;
use flowtrace_client::{
    ClientConfig, CreateGenAITraceOptions, CreateToolTraceOptions, CreateTraceOptions, Message,
    SpanType, UpdateTraceOptions, FlowtraceClient,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize client
    let config = ClientConfig::new("http://localhost:8080", 1)
        .with_project_id(0)
        .with_agent_id(1);

    let client = FlowtraceClient::new(config);

    let session_id = chrono::Utc::now().timestamp_millis();

    println!("Flowtrace Rust SDK Example\n");

    // 1. Create a root trace
    println!("1. Creating root trace...");
    let root_trace = match client
        .create_trace(CreateTraceOptions {
            agent_id: 1,
            session_id: Some(session_id),
            span_type: SpanType::Root,
            metadata: Some({
                let mut m = HashMap::new();
                m.insert("name".into(), serde_json::json!("example-agent"));
                m
            }),
            ..Default::default()
        })
        .await
    {
        Ok(t) => {
            println!("   Created: {}\n", t.edge_id);
            t
        }
        Err(e) => {
            println!("   Warning: {}\n", e);
            return Ok(());
        }
    };

    // 2. Create a planning span (child of root)
    println!("2. Creating planning span...");
    let planning_trace = match client
        .create_trace(CreateTraceOptions {
            agent_id: 1,
            session_id: Some(session_id),
            span_type: SpanType::Planning,
            parent_id: Some(root_trace.edge_id.clone()),
            metadata: Some({
                let mut m = HashMap::new();
                m.insert("step".into(), serde_json::json!("analyze_request"));
                m
            }),
            ..Default::default()
        })
        .await
    {
        Ok(t) => {
            println!("   Created: {}\n", t.edge_id);
            t
        }
        Err(e) => {
            println!("   Warning: {}\n", e);
            return Ok(());
        }
    };

    // 3. Track an LLM call with GenAI attributes
    println!("3. Creating GenAI trace (LLM call)...");
    let llm_trace = match client
        .create_genai_trace(CreateGenAITraceOptions {
            agent_id: 1,
            session_id: Some(session_id),
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
            parent_id: Some(planning_trace.edge_id.clone()),
            finish_reason: Some("stop".into()),
            ..Default::default()
        })
        .await
    {
        Ok(t) => {
            println!("   Created: {}", t.edge_id);
            println!("   Model: {}\n", t.model.as_deref().unwrap_or("unknown"));
            t
        }
        Err(e) => {
            println!("   Warning: {}\n", e);
            return Ok(());
        }
    };

    // 4. Track a tool call
    println!("4. Creating tool trace...");
    let tool_trace = match client
        .create_tool_trace(CreateToolTraceOptions {
            agent_id: 1,
            session_id: Some(session_id),
            tool_name: "web_search".into(),
            tool_input: Some({
                let mut input = HashMap::new();
                input.insert("query".into(), serde_json::json!("Paris population 2024"));
                input
            }),
            tool_output: Some({
                let mut output = HashMap::new();
                output.insert(
                    "result".into(),
                    serde_json::json!("Paris has a population of approximately 2.1 million"),
                );
                output
            }),
            tool_description: Some("Search the web for information".into()),
            parent_id: Some(llm_trace.edge_id.clone()),
            ..Default::default()
        })
        .await
    {
        Ok(t) => {
            println!("   Created: {}", t.edge_id);
            println!("   Tool: {}\n", t.tool_name);
            t
        }
        Err(e) => {
            println!("   Warning: {}\n", e);
            return Ok(());
        }
    };

    // 5. Create a response span
    println!("5. Creating response span...");
    match client
        .create_trace(CreateTraceOptions {
            agent_id: 1,
            session_id: Some(session_id),
            span_type: SpanType::Response,
            parent_id: Some(root_trace.edge_id.clone()),
            metadata: Some({
                let mut m = HashMap::new();
                m.insert(
                    "final_answer".into(),
                    serde_json::json!(
                        "The capital of France is Paris, with a population of about 2.1 million."
                    ),
                );
                m
            }),
            ..Default::default()
        })
        .await
    {
        Ok(t) => println!("   Created: {}\n", t.edge_id),
        Err(e) => println!("   Warning: {}\n", e),
    };

    // 6. Update trace with final metrics
    println!("6. Updating trace with completion info...");
    match client
        .update_trace(UpdateTraceOptions {
            edge_id: root_trace.edge_id.clone(),
            session_id,
            token_count: Some(50),
            duration_ms: Some(1500),
            ..Default::default()
        })
        .await
    {
        Ok(_) => println!("   Updated successfully\n"),
        Err(e) => println!("   Warning: {}\n", e),
    };

    // 7. Query traces
    println!("7. Querying traces for session...");
    match client.filter_by_session(session_id).await {
        Ok(traces) => println!(
            "   Found {} traces in session {}\n",
            traces.len(),
            session_id
        ),
        Err(e) => println!("   Warning: {}\n", e),
    };

    // 8. Submit feedback
    println!("8. Submitting user feedback...");
    match client.submit_feedback(&root_trace.edge_id, 1).await {
        Ok(_) => println!("   Feedback submitted (thumbs up)\n"),
        Err(_) => println!("   Feedback endpoint not available (expected in demo)\n"),
    };

    // 9. Health check
    println!("9. Checking server health...");
    match client.health().await {
        Ok(health) => println!("   Status: {}\n", health.status),
        Err(_) => println!("   Server not running (expected in demo)\n"),
    };

    println!("Example complete!");
    Ok(())
}
