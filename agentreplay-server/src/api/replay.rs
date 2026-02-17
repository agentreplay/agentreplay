// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use blake3::Hasher;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::api::query::{find_edge_by_id_or_session, ApiError, AppState};
use crate::auth::AuthContext;

#[derive(Debug, Deserialize)]
pub struct ReplayQuery {
    #[serde(default)]
    pub include_payload: bool,
    pub max_events: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ForkReplayRequest {
    pub fork_edge_id: String,
    pub alternate_tool_response: serde_json::Value,
    pub max_events: Option<usize>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ReplayEvent {
    pub step: usize,
    pub edge_id: String,
    pub parent_edge_id: Option<String>,
    pub timestamp_us: u64,
    pub logical_clock: u32,
    pub session_id: u64,
    pub agent_id: u64,
    pub span_type: String,
    pub duration_us: u32,
    pub is_tool_call: bool,
    pub tool_name: Option<String>,
    pub tool_arguments: Option<serde_json::Value>,
    pub tool_response: Option<serde_json::Value>,
    pub synthetic: bool,
    pub change_note: Option<String>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ReplayResponse {
    pub trace_id: String,
    pub root_edge_id: String,
    pub total_events: usize,
    pub breakpoint_candidates: Vec<String>,
    pub outcome_signature: String,
    pub events: Vec<ReplayEvent>,
}

#[derive(Debug, Serialize)]
pub struct ReplayForkResponse {
    pub original: ReplayResponse,
    pub forked: ReplayResponse,
    pub trajectory_distance: usize,
    pub sensitivity_score: f64,
    pub affected_nodes: usize,
}

#[derive(Clone)]
struct ReplayBuild {
    root_edge_id: u128,
    events: Vec<ReplayEvent>,
    ordered_edge_ids: Vec<u128>,
    parent_map: HashMap<u128, u128>,
    labels: HashMap<u128, String>,
}

/// GET /api/v1/replay/:trace_id
/// Deterministic replay using HLC ordering over the execution DAG.
pub async fn get_trace_replay(
    State(state): State<AppState>,
    Path(trace_id): Path<String>,
    Query(query): Query<ReplayQuery>,
    auth: axum::Extension<AuthContext>,
) -> Result<Json<ReplayResponse>, ApiError> {
    let max_events = query.max_events.unwrap_or(10_000).clamp(1, 50_000);

    let response = generate_replay_response(
        &trace_id,
        &state,
        auth.tenant_id,
        query.include_payload,
        max_events,
    )
    .await?;

    Ok(Json(response))
}

/// POST /api/v1/replay/:trace_id/fork
/// Counterfactual replay by forking at a tool-call edge and injecting an alternate response.
pub async fn fork_trace_replay(
    State(state): State<AppState>,
    Path(trace_id): Path<String>,
    auth: axum::Extension<AuthContext>,
    Json(req): Json<ForkReplayRequest>,
) -> Result<Json<ReplayForkResponse>, ApiError> {
    let max_events = req.max_events.unwrap_or(10_000).clamp(1, 50_000);

    let response = generate_fork_replay_response(
        &trace_id,
        &state,
        auth.tenant_id,
        &req.fork_edge_id,
        req.alternate_tool_response,
        max_events,
    )
    .await?;

    Ok(Json(response))
}

/// Shared replay generator for HTTP + MCP callers.
pub async fn generate_replay_response(
    trace_id: &str,
    state: &AppState,
    tenant_id: u64,
    include_payload: bool,
    max_events: usize,
) -> Result<ReplayResponse, ApiError> {
    let trace_id_u128 = parse_trace_hex(trace_id)?;
    let built = build_replay(state, trace_id_u128, tenant_id, include_payload, max_events).await?;
    Ok(to_replay_response(trace_id, built))
}

/// Shared counterfactual replay generator for HTTP + MCP callers.
pub async fn generate_fork_replay_response(
    trace_id: &str,
    state: &AppState,
    tenant_id: u64,
    fork_edge_id_hex: &str,
    alternate_tool_response: serde_json::Value,
    max_events: usize,
) -> Result<ReplayForkResponse, ApiError> {
    let trace_id_u128 = parse_trace_hex(trace_id)?;
    let fork_edge_id = parse_trace_hex(fork_edge_id_hex)?;

    let original_build = build_replay(state, trace_id_u128, tenant_id, true, max_events).await?;
    if !original_build.labels.contains_key(&fork_edge_id) {
        return Err(ApiError::BadRequest(format!(
            "fork_edge_id {:#x} is not part of trace {}",
            fork_edge_id, trace_id
        )));
    }

    let children_map = build_children_map(&original_build.parent_map);
    let affected = collect_descendants(fork_edge_id, &children_map);

    let mut forked_events = original_build.events.clone();
    let mut forked_labels = original_build.labels.clone();

    for event in &mut forked_events {
        let edge_id = parse_trace_hex(&event.edge_id)?;
        if edge_id == fork_edge_id {
            event.tool_response = Some(alternate_tool_response.clone());
            event.synthetic = true;
            event.change_note = Some("Fork point: alternate tool response injected".to_string());
            forked_labels.insert(edge_id, label_for_event(event));
        }
    }

    // Deterministic downstream propagation: if a node is in the fork subtree, derive a
    // synthetic label from parent synthetic state + original node label.
    let order_index: HashMap<u128, usize> = original_build
        .ordered_edge_ids
        .iter()
        .enumerate()
        .map(|(idx, id)| (*id, idx))
        .collect();

    let mut affected_sorted: Vec<u128> = affected.iter().copied().collect();
    affected_sorted.sort_by_key(|id| order_index.get(id).copied().unwrap_or(usize::MAX));

    for edge_id in affected_sorted {
        if edge_id == fork_edge_id {
            continue;
        }

        let parent = original_build.parent_map.get(&edge_id).copied();
        let parent_label = parent
            .and_then(|p| forked_labels.get(&p))
            .cloned()
            .unwrap_or_else(|| "root".to_string());
        let original_label = original_build
            .labels
            .get(&edge_id)
            .cloned()
            .unwrap_or_default();

        let synthetic_label = short_hash_label(&format!("{}::{}", parent_label, original_label));
        forked_labels.insert(edge_id, synthetic_label.clone());

        if let Some(step_idx) = order_index.get(&edge_id).copied() {
            if let Some(event) = forked_events.get_mut(step_idx) {
                event.synthetic = true;
                event.change_note = Some("Counterfactual downstream state recomputed".to_string());
                if event.tool_response.is_none() {
                    event.tool_response = Some(serde_json::json!({
                        "counterfactual_signature": synthetic_label,
                    }));
                }
            }
        }
    }

    let original_tree = build_tree(&original_build.root_edge_id, &original_build.parent_map, &original_build.labels, &order_index);
    let forked_tree = build_tree(&original_build.root_edge_id, &original_build.parent_map, &forked_labels, &order_index);
    let distance = tree_edit_distance(&original_tree, &forked_tree);

    let original = to_replay_response(&trace_id, original_build.clone());
    let mut forked_build = original_build;
    forked_build.events = forked_events;
    forked_build.labels = forked_labels;
    let forked = to_replay_response(&trace_id, forked_build);

    let denom = original.total_events.max(forked.total_events).max(1) as f64;
    let sensitivity_score = distance as f64 / denom;

    Ok(ReplayForkResponse {
        original,
        forked,
        trajectory_distance: distance,
        sensitivity_score,
        affected_nodes: affected.len(),
    })
}

fn parse_trace_hex(input: &str) -> Result<u128, ApiError> {
    u128::from_str_radix(input.trim_start_matches("0x"), 16)
        .map_err(|_| ApiError::BadRequest(format!("Invalid hex ID: {}", input)))
}

async fn build_replay(
    state: &AppState,
    trace_id_u128: u128,
    tenant_id: u64,
    include_payload: bool,
    max_events: usize,
) -> Result<ReplayBuild, ApiError> {
    let root = find_edge_by_id_or_session(state, trace_id_u128, tenant_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Trace not found".to_string()))?;

    let db = if let Some(ref pm) = state.project_manager {
        pm.get_or_open_project(root.project_id)
            .unwrap_or_else(|_| state.db.clone())
    } else {
        state.db.clone()
    };

    let mut spans = db
        .get_descendants_with_depth_for_tenant(root.edge_id, tenant_id, 4096, max_events)
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .into_iter()
        .map(|(edge, _depth)| edge)
        .collect::<Vec<_>>();

    if spans.is_empty() {
        spans.push(root);
    }

    spans.sort_by_key(|e| (e.timestamp_us, e.logical_clock, e.edge_id));

    let mut events = Vec::with_capacity(spans.len());
    let mut ordered_edge_ids = Vec::with_capacity(spans.len());
    let mut parent_map = HashMap::new();
    let mut labels = HashMap::new();

    for (idx, edge) in spans.iter().enumerate() {
        ordered_edge_ids.push(edge.edge_id);
        if edge.causal_parent != 0 {
            parent_map.insert(edge.edge_id, edge.causal_parent);
        }

        let payload_value = if edge.has_payload > 0 {
            db.get_payload(edge.edge_id)
                .ok()
                .flatten()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        } else {
            None
        };

        let tool_name = payload_value
            .as_ref()
            .and_then(|v| v.get("gen_ai.tool.name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let tool_arguments = payload_value
            .as_ref()
            .and_then(|v| v.get("gen_ai.tool.call.arguments"))
            .cloned();
        let tool_response = payload_value
            .as_ref()
            .and_then(|v| v.get("gen_ai.tool.call.result"))
            .cloned();

        let event = ReplayEvent {
            step: idx,
            edge_id: format!("{:#x}", edge.edge_id),
            parent_edge_id: (edge.causal_parent != 0).then(|| format!("{:#x}", edge.causal_parent)),
            timestamp_us: edge.timestamp_us,
            logical_clock: edge.logical_clock,
            session_id: edge.session_id,
            agent_id: edge.agent_id,
            span_type: format!("{:?}", edge.get_span_type()).to_lowercase(),
            duration_us: edge.duration_us,
            is_tool_call: tool_name.is_some() || tool_response.is_some(),
            tool_name,
            tool_arguments,
            tool_response,
            synthetic: false,
            change_note: None,
            payload: include_payload.then_some(payload_value).flatten(),
        };

        labels.insert(edge.edge_id, label_for_event(&event));
        events.push(event);
    }

    Ok(ReplayBuild {
        root_edge_id: root.edge_id,
        events,
        ordered_edge_ids,
        parent_map,
        labels,
    })
}

fn to_replay_response(trace_id: &str, built: ReplayBuild) -> ReplayResponse {
    let breakpoint_candidates = built
        .events
        .iter()
        .filter(|e| e.is_tool_call)
        .map(|e| e.edge_id.clone())
        .collect::<Vec<_>>();

    let mut hasher = Hasher::new();
    for edge_id in &built.ordered_edge_ids {
        hasher.update(edge_id.to_be_bytes().as_ref());
        if let Some(label) = built.labels.get(edge_id) {
            hasher.update(label.as_bytes());
        }
    }

    ReplayResponse {
        trace_id: trace_id.to_string(),
        root_edge_id: format!("{:#x}", built.root_edge_id),
        total_events: built.events.len(),
        breakpoint_candidates,
        outcome_signature: hasher.finalize().to_hex().to_string(),
        events: built.events,
    }
}

fn label_for_event(event: &ReplayEvent) -> String {
    if let Some(resp) = &event.tool_response {
        return format!("{}:{}", event.span_type, resp);
    }
    format!("{}:{}:{}", event.span_type, event.agent_id, event.duration_us)
}

fn short_hash_label(input: &str) -> String {
    let mut hasher = Hasher::new();
    hasher.update(input.as_bytes());
    hasher.finalize().to_hex()[0..16].to_string()
}

fn build_children_map(parent_map: &HashMap<u128, u128>) -> HashMap<u128, Vec<u128>> {
    let mut children: HashMap<u128, Vec<u128>> = HashMap::new();
    for (child, parent) in parent_map {
        children.entry(*parent).or_default().push(*child);
    }
    children
}

fn collect_descendants(root: u128, children_map: &HashMap<u128, Vec<u128>>) -> HashSet<u128> {
    let mut out = HashSet::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if out.insert(node) {
            if let Some(children) = children_map.get(&node) {
                for child in children {
                    stack.push(*child);
                }
            }
        }
    }
    out
}

#[derive(Clone)]
struct OrderedTree {
    root: usize,
    labels: Vec<String>,
    children: Vec<Vec<usize>>,
    subtree_size: Vec<usize>,
}

fn build_tree(
    root_edge_id: &u128,
    parent_map: &HashMap<u128, u128>,
    labels_by_edge: &HashMap<u128, String>,
    order_index: &HashMap<u128, usize>,
) -> OrderedTree {
    let mut edge_ids = labels_by_edge.keys().copied().collect::<Vec<_>>();
    edge_ids.sort_by_key(|id| order_index.get(id).copied().unwrap_or(usize::MAX));

    let mut id_to_node = HashMap::new();
    for (idx, edge_id) in edge_ids.iter().enumerate() {
        id_to_node.insert(*edge_id, idx);
    }

    let mut children = vec![Vec::new(); edge_ids.len()];
    for (child, parent) in parent_map {
        if let (Some(&c), Some(&p)) = (id_to_node.get(child), id_to_node.get(parent)) {
            children[p].push(c);
        }
    }

    for list in &mut children {
        list.sort_unstable();
    }

    let labels = edge_ids
        .iter()
        .map(|id| labels_by_edge.get(id).cloned().unwrap_or_default())
        .collect::<Vec<_>>();

    let root = id_to_node.get(root_edge_id).copied().unwrap_or(0);
    let mut subtree_size = vec![1usize; edge_ids.len()];
    compute_subtree_size(root, &children, &mut subtree_size);

    OrderedTree {
        root,
        labels,
        children,
        subtree_size,
    }
}

fn compute_subtree_size(node: usize, children: &[Vec<usize>], out: &mut [usize]) -> usize {
    let mut total = 1usize;
    for &child in &children[node] {
        total += compute_subtree_size(child, children, out);
    }
    out[node] = total;
    total
}

fn tree_edit_distance(t1: &OrderedTree, t2: &OrderedTree) -> usize {
    fn dist(
        t1: &OrderedTree,
        t2: &OrderedTree,
        n1: Option<usize>,
        n2: Option<usize>,
        memo: &mut HashMap<(Option<usize>, Option<usize>), usize>,
    ) -> usize {
        if let Some(v) = memo.get(&(n1, n2)) {
            return *v;
        }

        let val = match (n1, n2) {
            (None, None) => 0,
            (Some(i), None) => t1.subtree_size[i],
            (None, Some(j)) => t2.subtree_size[j],
            (Some(i), Some(j)) => {
                let relabel = if t1.labels[i] == t2.labels[j] { 0 } else { 1 };
                let c1 = &t1.children[i];
                let c2 = &t2.children[j];
                let m = c1.len();
                let n = c2.len();
                let mut dp = vec![vec![0usize; n + 1]; m + 1];

                for a in 1..=m {
                    dp[a][0] = dp[a - 1][0] + t1.subtree_size[c1[a - 1]];
                }
                for b in 1..=n {
                    dp[0][b] = dp[0][b - 1] + t2.subtree_size[c2[b - 1]];
                }

                for a in 1..=m {
                    for b in 1..=n {
                        let del = dp[a - 1][b] + t1.subtree_size[c1[a - 1]];
                        let ins = dp[a][b - 1] + t2.subtree_size[c2[b - 1]];
                        let sub = dp[a - 1][b - 1]
                            + dist(t1, t2, Some(c1[a - 1]), Some(c2[b - 1]), memo);
                        dp[a][b] = del.min(ins).min(sub);
                    }
                }

                relabel + dp[m][n]
            }
        };

        memo.insert((n1, n2), val);
        val
    }

    let mut memo = HashMap::new();
    dist(t1, t2, Some(t1.root), Some(t2.root), &mut memo)
}
