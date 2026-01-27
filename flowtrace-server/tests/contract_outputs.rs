use jsonschema::JSONSchema;
use serde_json::json;

use flowtrace_core::{
    ContentPartV1, EnvironmentStateV2, EvalTraceV1, MessageV1, OutcomeV1, OutcomeV2,
    SideEffectV2, SpanSummaryV1, TraceRefV1, TraceStatsV1, EVAL_TRACE_SCHEMA_VERSION_V1,
};

#[test]
fn eval_trace_v1_schema_contract() {
    let schema_str = include_str!("../schemas/eval_trace_v1.schema.json");
    let schema_json: serde_json::Value = serde_json::from_str(schema_str).unwrap();
    let compiled = JSONSchema::compile(&schema_json).unwrap();

    let mut eval_trace = EvalTraceV1::new("0x1".to_string(), 1);
    eval_trace.schema_version = EVAL_TRACE_SCHEMA_VERSION_V1.to_string();
    eval_trace.spans.push(SpanSummaryV1 {
        span_id: "0x10".to_string(),
        parent_span_id: None,
        span_type: "Root".to_string(),
        timestamp_us: 1,
        duration_us: 10,
        status: "completed".to_string(),
        attributes: None,
    });

    let message = MessageV1 {
        role: "assistant".to_string(),
        content: vec![ContentPartV1::Text {
            text: "Hello".to_string(),
        }],
        name: None,
        tool_call_id: None,
        metadata: Default::default(),
    };

    eval_trace.outcome = OutcomeV1 {
        status: "completed".to_string(),
        error: None,
        messages: vec![message],
        output_text: Some("Hello".to_string()),
        metadata: Default::default(),
    };

    eval_trace.trace_ref = Some(TraceRefV1 {
        schema_version: EVAL_TRACE_SCHEMA_VERSION_V1.to_string(),
        trace_id: eval_trace.trace_id.clone(),
        export_uri: Some("/api/v1/traces/0x1/detailed".to_string()),
        hash: Some("deadbeef".to_string()),
    });

    eval_trace.transcript = vec![
        flowtrace_core::TranscriptEventV1::Message {
            id: "m1".to_string(),
            role: "assistant".to_string(),
            content: vec![ContentPartV1::Text {
                text: "Hello".to_string(),
            }],
            timestamp_us: 2,
            span_id: None,
            metadata: Default::default(),
        },
        flowtrace_core::TranscriptEventV1::ToolCall {
            id: "tc1".to_string(),
            name: "search".to_string(),
            arguments: Some("{\"q\":\"hello\"}".to_string()),
            timestamp_us: 3,
            span_id: None,
            metadata: Default::default(),
        },
        flowtrace_core::TranscriptEventV1::ToolResult {
            id: "tr1".to_string(),
            tool_call_id: "tc1".to_string(),
            content: Some("ok".to_string()),
            timestamp_us: 4,
            span_id: None,
            metadata: Default::default(),
        },
        flowtrace_core::TranscriptEventV1::SpanStart {
            span_id: "s1".to_string(),
            parent_span_id: None,
            span_type: "Root".to_string(),
            timestamp_us: 1,
        },
        flowtrace_core::TranscriptEventV1::SpanEnd {
            span_id: "s1".to_string(),
            timestamp_us: 5,
            duration_us: 4,
            status: "completed".to_string(),
        },
    ];

    eval_trace.outcome_v2 = Some(OutcomeV2 {
        status: "completed".to_string(),
        error: None,
        messages: eval_trace.outcome.messages.clone(),
        output_text: eval_trace.outcome.output_text.clone(),
        metadata: Default::default(),
        state_before: Some(EnvironmentStateV2 {
            snapshot_id: Some("state_before".to_string()),
            state_hash: Some("abc123".to_string()),
            files: Default::default(),
            databases: Default::default(),
            custom: Default::default(),
        }),
        state_after: Some(EnvironmentStateV2 {
            snapshot_id: Some("state_after".to_string()),
            state_hash: Some("def456".to_string()),
            files: Default::default(),
            databases: Default::default(),
            custom: Default::default(),
        }),
        side_effects: vec![SideEffectV2 {
            effect_type: "file_write".to_string(),
            target: "/tmp/output.txt".to_string(),
            payload: Some(serde_json::json!({"bytes": 12})),
            timestamp_us: Some(123),
        }],
    });

    eval_trace.stats = TraceStatsV1 {
        total_tokens: 10,
        input_tokens: 4,
        output_tokens: 6,
        cost_usd: None,
        latency_ms: Some(12.5),
    };

    let payload = serde_json::to_value(&eval_trace).unwrap();
    let result = compiled.validate(&payload);
    assert!(result.is_ok(), "EvalTraceV1 schema validation failed");
}

#[test]
fn run_detail_v1_schema_contract() {
    let schema_str = include_str!("../schemas/run_detail_v1.schema.json");
    let schema_json: serde_json::Value = serde_json::from_str(schema_str).unwrap();
    let compiled = JSONSchema::compile(&schema_json).unwrap();

    let payload = json!({
        "id": "0x1",
        "dataset_id": "0x2",
        "name": "run",
        "agent_id": "agent-1",
        "model": "gpt-4",
        "schema_version": "eval_run_v1",
        "status": "completed",
        "started_at": 1,
        "completed_at": 2,
        "results": [
            {
                "test_case_id": "0x10",
                "trial_id": 1,
                "seed": 42,
                "trace_id": "0x20",
                "trace_ref": {
                    "schema_version": "eval_trace_v1",
                    "trace_id": "0x20",
                    "export_uri": "/api/v1/traces/0x20/detailed",
                    "hash": null
                },
                "eval_trace": null,
                "trace_summary": {
                    "status": "completed",
                    "final_output_text": "Hello",
                    "tool_call_count": 1,
                    "state_diff_hash": "abc123"
                },
                "eval_metrics": {"accuracy": 0.9},
                "grader_results": [],
                "overall": null,
                "passed": true,
                "error": null,
                "timestamp_us": 3,
                "cost_usd": 0.01,
                "latency_ms": 120
            }
        ],
        "task_aggregates": [
            {
                "test_case_id": 16,
                "trials": 1,
                "passed": 1,
                "pass_rate": 1.0,
                "pass_at_k_estimated": {"1": 1.0},
                "pass_at_k_empirical": {"1": 1.0},
                "pass_all_k_estimated": {"1": 1.0},
                "pass_all_k_empirical": {"1": 1.0},
                "mean_cost_usd": 0.01,
                "p50_latency_ms": 120,
                "p95_latency_ms": 120,
                "pass_rate_ci": {"method": "wilson", "lower": 1.0, "upper": 1.0}
            }
        ],
        "aggregated_metrics": {"accuracy": 0.9},
        "passed_count": 1,
        "failed_count": 0,
        "pass_rate": 1.0,
        "config": {}
    });

    let result = compiled.validate(&payload);
    assert!(result.is_ok(), "RunDetailResponse schema validation failed");
}
