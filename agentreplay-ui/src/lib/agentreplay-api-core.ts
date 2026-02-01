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

/**
 * Agentreplay API Client
 * 
 * This adapter provides API access using HTTP calls to the backend server.
 * Set VITE_USE_TAURI=true to use Tauri IPC instead.
 * 
 * Gap #11 Fix: Request deduplication to prevent duplicate API calls during rapid navigation.
 */

import axios from 'axios';

const USE_TAURI = false; // Set to true to use Tauri

// Request deduplication map (Gap #11 fix)
// Maps request key -> Promise for in-flight requests
const inFlightRequests = new Map<string, Promise<any>>();

// Generate a cache key for deduplication
function getRequestKey(command: string, args?: any): string {
  return `${command}:${JSON.stringify(args || {})}`;
}

// Determine the API base URL based on environment
function getApiBaseUrl(): string {
  // In Tauri, window.__TAURI__ or window.__TAURI_INTERNALS__ is defined
  const isTauri = typeof window !== 'undefined' &&
    ('__TAURI__' in window || '__TAURI_INTERNALS__' in window);

  if (isTauri) {
    // Tauri app: connect directly to embedded server
    return 'http://127.0.0.1:47100';
  }

  // Development with Vite proxy (port 5173)
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return ''; // Use Vite proxy
  }

  // Fallback: direct connection to server
  return 'http://127.0.0.1:47100';
}

const API_BASE_URL = getApiBaseUrl();

// Export base URL for direct fetch usage where needed
export { API_BASE_URL };

// Dynamic import for Tauri (only when needed)
let tauriInvoke: any = null;
if (USE_TAURI) {
  import('@tauri-apps/api/core').then(module => {
    tauriInvoke = module.invoke;
  }).catch(() => {
    console.warn('Tauri API not available, falling back to HTTP');
  });
}

// HTTP API helper
async function httpRequest<T>(method: string, endpoint: string, data?: any): Promise<T> {
  try {
    const config: any = {
      method,
      url: `${API_BASE_URL}${endpoint}`,
      headers: {
        'Content-Type': 'application/json',
      },
    };

    if (data) {
      if (method === 'GET') {
        config.params = data;
      } else {
        config.data = data;
      }
    }

    const response = await axios(config);
    return response.data;
  } catch (error: any) {
    console.error(`API request failed: ${method} ${endpoint}`, error);
    throw error;
  }
}

// Commands that should be deduplicated (read-only operations)
const DEDUP_COMMANDS = new Set([
  'health_check',
  'get_db_stats',
  'list_projects',
  'get_project_metrics',
  'list_traces',
  'get_trace',
  'get_trace_tree',
  'get_trace_observations',
  'get_timeseries',
  'get_costs',
  'list_agents',
  'list_eval_datasets',
  'get_eval_dataset',
  'list_eval_runs',
  'get_eval_run',
  'get_evaluation_history',
  'list_experiments',
  'get_experiment',
  'list_prompts',
  'get_prompt',
]);

// Unified invoke function that uses either Tauri or HTTP
// Gap #11 Fix: Deduplicate in-flight requests for read operations
async function invoke<T>(command: string, args?: any): Promise<T> {
  // Check if this command should be deduplicated
  const shouldDedup = DEDUP_COMMANDS.has(command);
  const requestKey = shouldDedup ? getRequestKey(command, args) : null;

  // Return existing in-flight request if available
  if (requestKey && inFlightRequests.has(requestKey)) {
    return inFlightRequests.get(requestKey) as Promise<T>;
  }

  // Create the request promise
  const requestPromise = (async () => {
    try {
      if (USE_TAURI && tauriInvoke) {
        return await tauriInvoke(command, args);
      }

      // Map Tauri commands to HTTP endpoints
      const { method, endpoint, data } = commandToHttpRequest(command, args);
      return await httpRequest<T>(method, endpoint, data);
    } finally {
      // Remove from in-flight map when done
      if (requestKey) {
        inFlightRequests.delete(requestKey);
      }
    }
  })();

  // Store in-flight request for deduplication
  if (requestKey) {
    inFlightRequests.set(requestKey, requestPromise);
  }

  return requestPromise;
}

// Map Tauri commands to HTTP API requests
function commandToHttpRequest(command: string, args?: any): { method: string; endpoint: string; data?: any } {
  switch (command) {
    case 'health_check':
      return { method: 'GET', endpoint: '/api/v1/health' };

    case 'get_db_stats':
      return { method: 'GET', endpoint: '/api/v1/stats' };

    case 'list_projects':
      return { method: 'GET', endpoint: '/api/v1/projects' };

    case 'create_project':
      return { method: 'POST', endpoint: '/api/v1/projects', data: args };

    case 'delete_project':
      return { method: 'DELETE', endpoint: `/api/v1/projects/${args?.projectId}` };

    case 'get_project_metrics':
      return { method: 'GET', endpoint: `/api/v1/projects/${args?.projectId}/metrics` };

    case 'list_traces':
      return { method: 'GET', endpoint: '/api/v1/traces', data: args?.params || args };

    case 'get_trace':
      return {
        method: 'GET',
        endpoint: `/api/v1/traces/${args?.traceId}`,
        data: { tenant_id: args?.tenant_id || 1, project_id: args?.project_id || 0 }
      };

    case 'get_trace_tree':
      return {
        method: 'GET',
        endpoint: `/api/v1/traces/${args?.traceId}/tree`,
        data: { tenant_id: args?.tenant_id || 1, project_id: args?.project_id || 0 }
      };

    case 'get_trace_observations':
      return {
        method: 'GET',
        endpoint: `/api/v1/traces/${args?.traceId}/observations`,
        data: { tenant_id: args?.tenant_id || 1, project_id: args?.project_id || 0 }
      };

    case 'search_traces':
      return { method: 'POST', endpoint: '/api/v1/search', data: args?.params };

    case 'delete_trace':
      return { method: 'DELETE', endpoint: `/api/v1/traces/${args?.traceId}` };

    case 'get_timeseries':
      return { method: 'GET', endpoint: '/api/v1/analytics/timeseries', data: args?.params };

    case 'get_costs':
      return { method: 'GET', endpoint: '/api/v1/analytics/costs', data: args };

    case 'list_agents':
      return { method: 'GET', endpoint: '/api/v1/agents' };

    case 'register_agent':
      return { method: 'POST', endpoint: '/api/v1/agents', data: args?.request };

    case 'list_eval_datasets':
      return { method: 'GET', endpoint: '/api/v1/evals/datasets' };

    case 'get_eval_dataset':
      return { method: 'GET', endpoint: `/api/v1/evals/datasets/${args?.datasetId}` };

    case 'create_eval_dataset':
      return { method: 'POST', endpoint: '/api/v1/evals/datasets', data: args };

    case 'delete_eval_dataset':
      return { method: 'DELETE', endpoint: `/api/v1/evals/datasets/${args?.datasetId}` };

    case 'add_eval_examples':
      return { method: 'POST', endpoint: `/api/v1/evals/datasets/${args?.datasetId}/examples`, data: { examples: args?.examples } };

    case 'delete_eval_example':
      return { method: 'DELETE', endpoint: `/api/v1/evals/datasets/${args?.datasetId}/examples/${args?.exampleId}` };

    case 'list_eval_runs':
      return { method: 'GET', endpoint: '/api/v1/evals/runs' };

    case 'get_eval_run':
      return { method: 'GET', endpoint: `/api/v1/evals/runs/${args?.runId}` };

    case 'create_eval_run':
      return { method: 'POST', endpoint: '/api/v1/evals/runs', data: args };

    case 'delete_eval_run':
      return { method: 'DELETE', endpoint: `/api/v1/evals/runs/${args?.runId}` };

    case 'add_run_result':
      return { method: 'POST', endpoint: `/api/v1/evals/runs/${args?.runId}/results`, data: args };

    case 'complete_eval_run':
      return { method: 'POST', endpoint: `/api/v1/evals/runs/${args?.runId}/complete`, data: { status: args?.status } };

    case 'compare_eval_runs':
      return { method: 'POST', endpoint: '/api/v1/evals/compare', data: args };

    // Evaluation Execution
    case 'run_geval':
      return { method: 'POST', endpoint: '/api/v1/evals/geval', data: args };

    case 'run_ragas':
      return { method: 'POST', endpoint: '/api/v1/evals/ragas', data: args };

    case 'get_evaluation_history':
      return { method: 'GET', endpoint: `/api/v1/evals/trace/${args?.traceId}/history` };

    // Experiments
    case 'list_experiments':
      return { method: 'GET', endpoint: '/api/v1/experiments', data: args?.status ? { status: args.status } : undefined };

    case 'get_experiment':
      return { method: 'GET', endpoint: `/api/v1/experiments/${args?.experimentId}` };

    case 'create_experiment':
      return { method: 'POST', endpoint: '/api/v1/experiments', data: args };

    case 'update_experiment':
      const { experimentId: updateExpId, ...updateData } = args || {};
      return { method: 'PUT', endpoint: `/api/v1/experiments/${updateExpId}`, data: updateData };

    case 'start_experiment':
      return { method: 'POST', endpoint: `/api/v1/experiments/${args?.experimentId}/start`, data: { traffic_split: args?.traffic_split } };

    case 'stop_experiment':
      return { method: 'POST', endpoint: `/api/v1/experiments/${args?.experimentId}/stop` };

    case 'get_experiment_stats':
      return { method: 'GET', endpoint: `/api/v1/experiments/${args?.experimentId}/stats` };

    case 'record_experiment_result':
      const { experimentId: recordExpId, ...resultData } = args || {};
      return { method: 'POST', endpoint: `/api/v1/experiments/${recordExpId}/results`, data: resultData };

    // Prompts
    case 'list_prompts':
      return { method: 'GET', endpoint: '/api/v1/prompts', data: args };

    case 'get_prompt':
      return { method: 'GET', endpoint: `/api/v1/prompts/${args?.promptId}` };

    case 'create_prompt':
      return { method: 'POST', endpoint: '/api/v1/prompts', data: args };

    case 'update_prompt':
      const { promptId: updatePromptId, ...promptData } = args || {};
      return { method: 'PUT', endpoint: `/api/v1/prompts/${updatePromptId}`, data: promptData };

    case 'delete_prompt':
      return { method: 'DELETE', endpoint: `/api/v1/prompts/${args?.promptId}` };

    case 'render_prompt':
      return { method: 'POST', endpoint: `/api/v1/prompts/${args?.promptId}/render`, data: { variables: args?.variables } };

    case 'get_prompt_version_history':
      return { method: 'GET', endpoint: `/api/v1/prompts/${args?.promptId}/versions` };

    case 'get_prompt_diff':
      return { method: 'GET', endpoint: `/api/v1/prompts/${args?.promptId}/diff`, data: { version1: args?.version1, version2: args?.version2 } };

    // Memory/RAG
    case 'ingest_memory':
      return { method: 'POST', endpoint: '/api/v1/memory/ingest', data: args };

    case 'retrieve_memory':
      return { method: 'POST', endpoint: '/api/v1/memory/retrieve', data: args };

    case 'list_collections':
      return { method: 'GET', endpoint: '/api/v1/memory/collections' };

    case 'get_collection':
      return { method: 'GET', endpoint: `/api/v1/memory/collections/${args?.collection}` };

    case 'delete_collection':
      return { method: 'DELETE', endpoint: `/api/v1/memory/collections/${args?.collection}` };

    case 'create_backup':
      return { method: 'POST', endpoint: '/api/v1/admin/backup' };

    case 'list_backups':
      return { method: 'GET', endpoint: '/api/v1/admin/backups' };

    case 'restore_backup':
      return { method: 'POST', endpoint: `/api/v1/admin/backups/${args?.backupId}/restore` };

    case 'check_for_updates':
      return { method: 'GET', endpoint: '/api/v1/system/updates' };

    case 'reset_all_data':
      return { method: 'DELETE', endpoint: '/api/v1/admin/reset' };

    case 'get_insights':
      return { method: 'GET', endpoint: `/api/v1/insights?project_id=${args?.project_id || 0}&window_seconds=${args?.window_seconds || 3600}&limit=${args?.limit || 50}` };

    case 'get_insights_summary':
      return { method: 'GET', endpoint: `/api/v1/insights/summary?project_id=${args?.project_id || 0}` };

    // ========== Evaluation Pipeline (5-Phase) ==========
    case 'eval_pipeline_collect':
      return { method: 'POST', endpoint: '/api/v1/evals/pipeline/collect', data: args };

    case 'eval_pipeline_process':
      return { method: 'POST', endpoint: '/api/v1/evals/pipeline/process', data: args };

    case 'eval_pipeline_annotate':
      return { method: 'POST', endpoint: '/api/v1/evals/pipeline/annotate', data: args };

    case 'eval_pipeline_golden':
      return { method: 'POST', endpoint: '/api/v1/evals/pipeline/golden', data: args };

    case 'eval_pipeline_evaluate':
      return { method: 'POST', endpoint: '/api/v1/evals/pipeline/evaluate', data: args };

    case 'eval_pipeline_recommendations':
      return { method: 'GET', endpoint: '/api/v1/evals/pipeline/recommendations', data: args };

    case 'eval_pipeline_metric_definitions':
      return { method: 'GET', endpoint: '/api/v1/evals/pipeline/metrics/definitions' };

    case 'eval_pipeline_history':
      return { method: 'GET', endpoint: '/api/v1/evals/pipeline/history', data: args };

    // Dataset Flywheel endpoints
    case 'get_dataset_candidates':
      return {
        method: 'GET',
        endpoint: `/api/v1/evals/flywheel/candidates?positive_threshold=${args?.positive_threshold || 0.9}&negative_threshold=${args?.negative_threshold || 0.3}&limit=${args?.limit || 100}`
      };

    case 'export_finetuning_dataset':
      return {
        method: 'POST',
        endpoint: '/api/v1/evals/flywheel/export',
        data: args
      };

    // ========== Tool Registry ==========
    case 'list_tools':
      return { method: 'GET', endpoint: '/api/v1/tools' };

    case 'get_tool':
      return { method: 'GET', endpoint: `/api/v1/tools/${args?.toolId}` };

    case 'register_tool':
      return { method: 'POST', endpoint: '/api/v1/tools', data: args };

    case 'update_tool':
      const { toolId: updateToolId, ...toolData } = args || {};
      return { method: 'PUT', endpoint: `/api/v1/tools/${updateToolId}`, data: toolData };

    case 'unregister_tool':
      return { method: 'DELETE', endpoint: `/api/v1/tools/${args?.toolId}` };

    case 'execute_tool':
      return { method: 'POST', endpoint: `/api/v1/tools/${args?.toolId}/execute`, data: args?.input };

    case 'get_tool_executions':
      return { method: 'GET', endpoint: `/api/v1/tools/${args?.toolId}/executions`, data: { limit: args?.limit || 50 } };

    case 'list_mcp_servers':
      return { method: 'GET', endpoint: '/api/v1/tools/mcp/servers' };

    case 'sync_mcp_server':
      return { method: 'POST', endpoint: `/api/v1/tools/mcp/servers/${args?.serverId}/sync` };

    case 'connect_mcp_server':
      return { method: 'POST', endpoint: '/api/v1/tools/mcp/servers', data: args };

    case 'disconnect_mcp_server':
      return { method: 'DELETE', endpoint: `/api/v1/tools/mcp/servers/${args?.serverId}` };

    // ========== Git-like Response Versioning ==========
    case 'git_commit':
    case 'version_commit':
      // Reference-based versioning - send trace_id reference
      return {
        method: 'POST',
        endpoint: '/api/v1/git/commit',
        data: {
          trace_id: args?.trace_id,
          span_id: args?.span_id,
          message: args?.message,
          model: args?.model,
          branch: args?.branch || args?.model,
          metadata: args?.metadata || {}
        }
      };

    case 'git_log':
    case 'version_log':
      return { method: 'GET', endpoint: '/api/v1/git/log', data: { branch: args?.branch, limit: args?.limit || 50 } };

    case 'git_show':
    case 'version_show':
      return { method: 'GET', endpoint: `/api/v1/git/show/${args?.commitId || args?.versionId}` };

    case 'git_diff':
    case 'version_diff':
      return { method: 'POST', endpoint: '/api/v1/git/diff', data: { old_ref: args?.from, new_ref: args?.to } };

    case 'git_list_branches':
    case 'version_branches':
      return { method: 'GET', endpoint: '/api/v1/git/branches' };

    case 'git_create_branch':
    case 'version_create_branch':
      return { method: 'POST', endpoint: '/api/v1/git/branches', data: args };

    case 'git_delete_branch':
    case 'version_delete_branch':
      return { method: 'DELETE', endpoint: `/api/v1/git/branches/${args?.branchName}` };

    case 'git_checkout':
    case 'version_checkout':
      return { method: 'POST', endpoint: '/api/v1/git/checkout', data: { ref: args?.ref } };

    case 'git_list_tags':
    case 'version_tags':
      return { method: 'GET', endpoint: '/api/v1/git/tags' };

    case 'git_create_tag':
    case 'version_create_tag':
      return { method: 'POST', endpoint: '/api/v1/git/tags', data: args };

    case 'git_delete_tag':
    case 'version_delete_tag':
      return { method: 'DELETE', endpoint: `/api/v1/git/tags/${args?.tagName}` };

    case 'git_stats':
    case 'version_stats':
      return { method: 'GET', endpoint: '/api/v1/git/stats' };

    default:
      return { method: 'POST', endpoint: `/api/v1/${command}`, data: args };
  }
}

// ============================================================================
// Type Definitions (matching app/ UI expectations)
// ============================================================================

export interface Agent {
  agent_id: string;
  name: string;
  type: string;
  registered_at: number;
  last_seen?: number;
}

export interface AgentRegistrationRequest {
  name: string;
  type: string;
  metadata?: Record<string, any>;
}

export interface TraceMetadata {
  trace_id: string;
  span_id: string;
  parent_span_id?: string;
  tenant_id: number;
  project_id: number;
  agent_id: number;
  agent_name: string;
  session_id: number;
  span_type: string;
  environment: string;
  timestamp_us: number;
  duration_us: number;
  token_count: number;
  sensitivity_flags: number;
  metadata?: Record<string, any>; // OTEL attributes and custom metadata

  // Computed fields for UI compatibility
  started_at?: number;
  ended_at?: number;
  duration_ms?: number;
  status?: string;
  cost?: number;
  tokens?: number;
  error?: string;
  display_name?: string;
  provider?: string;
  model?: string;
  input_tokens?: number;
  output_tokens?: number;
  operation?: string;
  operation_name?: string;
  input_preview?: string;
  output_preview?: string;
}

// Server-side search response format
export interface TraceSearchView {
  edge_id: string;
  timestamp_us: number;
  operation: string;
  span_type: string;
  duration_ms: number;
  tokens: number;
  cost: number;
  status: string;
  model?: string;
  agent_id: number;
  session_id: number;
}

export interface TraceDetails {
  trace_id: string;
  agent_id: string;
  started_at: number;
  ended_at?: number;
  spans: Span[];
  metadata?: Record<string, any>;
}

export interface Span {
  span_id: string;
  parent_id?: string;
  name: string;
  span_type: string;
  started_at: number;
  ended_at?: number;
  duration_ms?: number;
  attributes?: Record<string, any>;
  input?: any;
  output?: any;
  error?: string;
  cost?: number;
  tokens?: number;
}

export interface DatabaseStats {
  total_traces: number;
  total_edges: number;
  total_spans: number;
  storage_size_bytes: number;
  index_size_bytes: number;
  storage_path?: string;
}

export interface TimeseriesDataPoint {
  timestamp: number;
  request_count: number;
  avg_duration: number;
  total_cost: number;
  error_count: number;
  total_tokens?: number;
}

/**
 * Sharded timeseries point with DDSketch percentiles and HyperLogLog cardinality.
 * Provides accurate latency percentiles (P50/P95/P99) via streaming DDSketch algorithm
 * and unique session/agent counts via HyperLogLog probabilistic counting.
 */
export interface ShardedTimeseriesPoint {
  timestamp: number;
  request_count: number;
  error_count: number;
  total_tokens: number;
  avg_duration_ms: number;
  /** Minimum latency in milliseconds */
  min_duration_ms?: number;
  /** Maximum latency in milliseconds */
  max_duration_ms?: number;
  /** P50 latency from DDSketch (median) */
  p50_duration_ms?: number;
  /** P95 latency from DDSketch */
  p95_duration_ms?: number;
  /** P99 latency from DDSketch (tail latency) */
  p99_duration_ms?: number;
  /** Unique sessions estimated via HyperLogLog (~0.81% standard error) */
  unique_sessions?: number;
  /** Unique agents estimated via HyperLogLog (~0.81% standard error) */
  unique_agents?: number;
}

export interface CostBreakdown {
  total_cost: number;
  by_model: Array<{
    model: string;
    cost: number;
    tokens: number;
    requests: number;
  }>;
  by_user: Array<{
    user: string;
    cost: number;
    requests: number;
  }>;
}

export interface EvalDataset {
  dataset_id: string;
  name: string;
  description?: string;
  created_at: number;
  examples: EvalExample[];
}

export interface EvalExample {
  example_id: string;
  input: any;
  expected_output: any;
  metadata?: Record<string, any>;
}

export interface EvalRun {
  run_id: string;
  id?: string; // Sometimes comes as 'id' from server
  dataset_id: string;
  name: string;
  agent_id: string;
  model: string;
  status: 'running' | 'completed' | 'failed' | 'stopped';
  started_at: number;
  completed_at?: number;
  created_at?: number; // Alias for started_at
  config: Record<string, string>;
  results: Array<{
    test_case_id: string;
    trace_id?: string;
    passed: boolean;
    error?: string;
    eval_metrics: Record<string, number>;
    timestamp_us: number;
  }>;
}

export interface CreateEvalRunRequest {
  dataset_id: string;
  name: string;
  agent_id?: string;
  model?: string;
  config?: Record<string, string>;
}

export interface CreateEvalRunResponse {
  run_id: string;
  dataset_id: string;
  name: string;
  status: string;
}

export interface AddRunResultRequest {
  test_case_id: string;
  trace_id?: string;
  passed: boolean;
  error?: string;
  eval_metrics?: Record<string, number>;
}

export interface CompleteEvalRunResponse {
  success: boolean;
  run_id: string;
  status: string;
  passed_count: number;
  failed_count: number;
  pass_rate: number;
}

// ============================================================================
// Statistical Comparison Types
// ============================================================================

export interface CompareEvalRunsRequest {
  baseline_run_id: string;
  treatment_run_id: string;
  metric_direction?: Record<string, boolean>; // true = higher is better
}

export interface RunStatsResponse {
  mean: number;
  std_dev: number;
  n: number;
  p50: number;
  p95: number;
}

export interface MetricComparisonResponse {
  metric_name: string;
  baseline: RunStatsResponse;
  treatment: RunStatsResponse;
  difference: number;
  percent_change: number;
  t_statistic: number;
  degrees_of_freedom: number;
  p_value: number;
  cohens_d: number;
  effect_size: 'negligible' | 'small' | 'medium' | 'large';
  is_significant: boolean;
  winner: 'baseline' | 'treatment' | 'tie';
  higher_is_better: boolean;
}

export interface ComparisonSummaryResponse {
  total_metrics: number;
  significant_improvements: number;
  significant_regressions: number;
  no_significant_change: number;
}

export interface RecommendationResponse {
  action: 'deploy_treatment' | 'keep_baseline' | 'need_more_data' | 'inconclusive';
  confidence: number;
  explanation: string;
}

export interface CompareEvalRunsResponse {
  baseline_run_id: string;
  treatment_run_id: string;
  baseline_run_name: string;
  treatment_run_name: string;
  metrics: MetricComparisonResponse[];
  summary: ComparisonSummaryResponse;
  recommendation: RecommendationResponse;
}

// ============================================================================
// Evaluation Execution Types
// ============================================================================

export interface GEvalRequest {
  trace_id: string;
  criteria: string[];
  weights?: Record<string, number>;
  model?: string;
}

export interface RagasRequest {
  trace_id: string;
  question: string;
  answer: string;
  context: string[];
  ground_truth?: string;
  model?: string;
}

export interface EvaluationResult {
  trace_id: string;
  evaluator: string;
  score: number;
  details: Record<string, number>;
  /** Per-metric explanations from the LLM judge */
  detail_explanations?: Record<string, string>;
  evaluation_time_ms: number;
  model_used: string;
  /** Overall explanation of the evaluation */
  explanation?: string;
  /** Confidence level (0-1) */
  confidence?: number;
  /** Whether evaluation passed threshold */
  passed?: boolean;
  /** Estimated cost in USD */
  cost_usd?: number;
}

// ============================================================================
// Experiment Types
// ============================================================================

export interface ExperimentVariant {
  id: string;
  name: string;
  description: string;
  config: Record<string, any>;
}

export interface ExperimentResponse {
  id: string;
  name: string;
  description: string;
  variants: ExperimentVariant[];
  status: 'draft' | 'running' | 'paused' | 'completed' | 'stopped';
  traffic_split: Record<string, number>;
  metrics: string[];
  start_time?: number;
  end_time?: number;
  created_at: number;
  updated_at: number;
}

export interface ExperimentListResponse {
  experiments: ExperimentResponse[];
  total: number;
}

export interface CreateExperimentRequest {
  name: string;
  description: string;
  variants: Array<{
    name: string;
    description: string;
    config: Record<string, any>;
  }>;
  metrics?: string[];
}

export interface UpdateExperimentRequest {
  name?: string;
  description?: string;
  traffic_split?: Record<string, number>;
}

export interface RecordResultRequest {
  variant_id: string;
  trace_id: string;
  metrics: Record<string, number>;
}

export interface VariantStats {
  variant_id: string;
  variant_name: string;
  sample_count: number;
  metrics: Record<string, MetricStats>;
}

export interface MetricStats {
  mean: number;
  std_dev: number;
  min: number;
  max: number;
  count: number;
}

export interface ExperimentStatsResponse {
  experiment_id: string;
  variant_stats: Record<string, VariantStats>;
  winner?: string;
  confidence?: number;
}

// ============================================================================
// Prompt Types
// ============================================================================

export interface PromptResponse {
  id: string;
  name: string;
  description: string;
  template: string;
  variables: string[];
  tags: string[];
  version: number;
  created_at: number;
  updated_at: number;
  created_by: string;
  metadata?: Record<string, any>;
}

export interface PromptListResponse {
  prompts: PromptResponse[];
  total: number;
}

export interface CreatePromptRequest {
  name: string;
  description?: string;
  template: string;
  tags?: string[];
  metadata?: Record<string, any>;
}

export interface UpdatePromptRequest {
  name?: string;
  description?: string;
  template?: string;
  tags?: string[];
  metadata?: Record<string, any>;
}

export interface PromptVersionHistoryResponse {
  prompt_id: string;
  versions: PromptVersionResponse[];
  total: number;
}

export interface PromptVersionResponse {
  version: number;
  template: string;
  created_at: number;
  created_by: string;
  change_summary: string;
}

export interface PromptDiffResponse {
  prompt_id: string;
  version1: number;
  version2: number;
  diff: DiffLine[];
  template1: string;
  template2: string;
}

export interface DiffLine {
  line_type: 'added' | 'removed' | 'unchanged';
  content: string;
}

// ============================================================================
// Memory/RAG Types
// ============================================================================

export interface IngestMemoryRequest {
  collection: string;
  content: string;
  metadata?: Record<string, string>;
}

export interface RetrieveMemoryRequest {
  collection: string;
  query: string;
  k: number;
}

export interface MemoryRetrievalResponse {
  results: MemoryResult[];
  collection: string;
  query_time_ms: number;
}

export interface MemoryResult {
  id: string;
  content: string;
  score: number;
  metadata: Record<string, string>;
}

export interface MemoryCollection {
  name: string;
  document_count: number;
  created_at: number;
  updated_at: number;
}

// ============================================================================
// Insights Types
// ============================================================================

export interface InsightView {
  id: string;
  insight_type: string;
  severity: 'info' | 'low' | 'medium' | 'high' | 'critical';
  confidence: number;
  summary: string;
  description: string;
  related_trace_ids: string[];
  metadata: Record<string, any>;
  generated_at: number;
  suggestions: string[];
}

export interface InsightsResponse {
  insights: InsightView[];
  total_count: number;
  window_seconds: number;
  generated_at: number;
}

export interface InsightsSummary {
  total_insights: number;
  critical_count: number;
  high_count: number;
  by_severity: Record<string, number>;
  by_type: Record<string, number>;
  health_score: number;
  top_insights: InsightView[];
}

export interface InsightsQuery {
  project_id: number;
  window_seconds?: number;
  min_severity?: string;
  insight_type?: string;
  limit?: number;
}

export interface BackupMetadata {
  backup_id: string;
  created_at: number;
  size_bytes: number;
  path: string;
}

export interface RetentionPolicy {
  environment: string;
  retention_days: number;
}

export interface RetentionStats {
  total_traces: number;
  traces_to_delete: number;
  storage_saved_bytes: number;
}

export interface ProjectMetricsResponse {
  latency_ms: {
    p50: number;
    p80: number;
    p90: number;
    p95: number;
    p99: number;
  };
  tokens: {
    p50: number;
    p80: number;
    p90: number;
  };
  cost_usd: {
    avg: number;
    total: number;
  };
}

// ============================================================================
// Evaluation Pipeline Types (5-Phase Comprehensive Metrics)
// ============================================================================

export type MetricCategory = 'operational' | 'quality' | 'agent' | 'user_experience' | 'safety';
export type MetricPriority = 'critical' | 'high' | 'medium' | 'low';
export type MetricType = 'percentage' | 'duration' | 'count' | 'currency' | 'score';
export type MetricStatus = 'good' | 'warning' | 'critical' | 'unknown';
export type HealthStatus = 'healthy' | 'warning' | 'critical' | 'unknown';

// Phase 1: Collect
export interface CollectTracesRequest {
  project_id?: number;
  start_time?: number;
  end_time?: number;
  status_filter?: 'success' | 'error' | 'all';
  min_duration_ms?: number;
  max_duration_ms?: number;
  search_query?: string;
  limit?: number;
  include_metadata?: boolean;
}

export interface CollectedTrace {
  trace_id: string;
  timestamp_us: number;
  duration_ms?: number;
  status: string;
  span_count: number;
  token_count?: number;
  cost_usd?: number;
  model?: string;
  input_preview?: string;
  output_preview?: string;
  metadata: Record<string, any>;
}

export interface CollectionSummary {
  success_count: number;
  error_count: number;
  avg_duration_ms: number;
  total_tokens: number;
  total_cost_usd: number;
  date_range: [number, number];
}

export interface CollectTracesResponse {
  traces: CollectedTrace[];
  total_count: number;
  filtered_count: number;
  summary: CollectionSummary;
}

// Phase 2: Process
export interface CategorizationConfig {
  by_model: boolean;
  by_status: boolean;
  by_latency_bucket: boolean;
  by_cost_bucket: boolean;
  custom_tags?: string[];
}

export interface SamplingConfig {
  strategy: 'random' | 'stratified' | 'recent' | 'diverse';
  sample_size: number;
  seed?: number;
}

export interface ProcessTracesRequest {
  trace_ids: string[];
  categorization?: CategorizationConfig;
  sampling?: SamplingConfig;
}

export interface CategoryStats {
  count: number;
  avg_duration_ms: number;
  avg_tokens: number;
  avg_cost_usd: number;
  error_rate: number;
  trace_ids: string[];
}

export interface ProcessingStats {
  total_traces: number;
  categorized_traces: number;
  uncategorized_traces: number;
  processing_time_ms: number;
}

export interface ProcessTracesResponse {
  processed_count: number;
  categories: Record<string, CategoryStats>;
  sampled_trace_ids?: string[];
  processing_stats: ProcessingStats;
}

// Phase 3: Annotate
export interface CreateAnnotationRequest {
  trace_id: string;
  annotation_type: 'label' | 'score' | 'feedback' | 'golden';
  value: any;
  annotator?: string;
  confidence?: number;
  metadata?: Record<string, any>;
}

export interface Annotation {
  id: string;
  trace_id: string;
  annotation_type: string;
  value: any;
  annotator: string;
  confidence: number;
  created_at: number;
  metadata: Record<string, any>;
}

export interface GoldenTestCase {
  input: string;
  expected_output: string;
  context?: string[];
  metadata?: Record<string, string>;
  source_trace_id?: string;
}

export interface AddGoldenTestCasesRequest {
  dataset_name: string;
  test_cases: GoldenTestCase[];
}

export interface AddGoldenTestCasesResponse {
  dataset_id: string;
  added_count: number;
  total_count: number;
}

// Phase 4: Evaluate
export interface MetricDefinition {
  id: string;
  name: string;
  description: string;
  category: MetricCategory;
  metric_type: MetricType;
  priority: MetricPriority;
  target?: number;
  target_description?: string;
  unit?: string;
}

export interface MetricValue {
  metric_id: string;
  value: number;
  target?: number;
  trend?: number;
  status: MetricStatus;
  samples: number;
}

export interface MetricAlert {
  metric_id: string;
  severity: string;
  message: string;
  current_value: number;
  threshold: number;
}

export interface BaselineComparison {
  baseline_run_id: string;
  improvements: string[];
  regressions: string[];
  unchanged: string[];
}

export interface RunEvaluationRequest {
  trace_ids: string[];
  metrics: string[];
  categories?: MetricCategory[];
  compare_with_baseline?: string;
  llm_judge_model?: string;
}

export interface EvaluationResults {
  run_id: string;
  timestamp: number;
  trace_count: number;
  metrics: Record<string, MetricValue>;
  category_scores: Record<string, number>;
  overall_health: HealthStatus;
  alerts: MetricAlert[];
  comparison?: BaselineComparison;
}

// Phase 5: Iterate - Recommendations
export interface Recommendation {
  id: string;
  priority: string;
  category: string;
  title: string;
  description: string;
  impact: string;
  effort: string;
  actions: string[];
}

export interface RecommendationsSummary {
  total_recommendations: number;
  critical_count: number;
  high_count: number;
  medium_count: number;
  low_count: number;
  estimated_impact: string;
}

export interface RecommendationsResponse {
  recommendations: Recommendation[];
  summary: RecommendationsSummary;
}

// Eval History
export interface EvalHistoryEntry {
  run_id: string;
  timestamp: number;
  trace_count: number;
  overall_health: HealthStatus;
  category_scores: Record<string, number>;
  alert_count: number;
}

export interface EvalHistoryResponse {
  entries: EvalHistoryEntry[];
  total: number;
}

// ============================================================================
// Tool Registry Types
// ============================================================================

export interface ToolInfo {
  id: string;
  name: string;
  version: string;
  kind: 'mcp' | 'rest' | 'native' | 'mock';
  status: 'active' | 'inactive' | 'error';
  description: string;
  input_schema?: Record<string, any>;
  output_schema?: Record<string, any>;
  last_executed?: number;
  execution_count: number;
  avg_latency_ms: number;
  success_rate: number;
  rate_limit?: {
    max_requests: number;
    window_seconds: number;
  };
  mcp_server_id?: string;
  created_at: number;
  updated_at: number;
}

export interface RegisterToolRequest {
  name: string;
  version?: string;
  kind: 'mcp' | 'rest' | 'native' | 'mock';
  description?: string;
  input_schema?: Record<string, any>;
  output_schema?: Record<string, any>;
  rate_limit?: {
    max_requests: number;
    window_seconds: number;
  };
  endpoint_url?: string;
}

export interface ToolExecutionResult {
  success: boolean;
  output?: any;
  error?: string;
  latency_ms: number;
  executed_at: number;
}

export interface ToolExecution {
  id: string;
  tool_id: string;
  input: Record<string, any>;
  output?: any;
  success: boolean;
  error?: string;
  latency_ms: number;
  executed_at: number;
  trace_id?: string;
}

export interface McpServer {
  id: string;
  name: string;
  uri: string;
  status: 'connected' | 'disconnected' | 'error';
  tool_count: number;
  tools: Array<{ name: string; description?: string }>;
  last_synced?: number;
  error?: string;
}

// ============================================================================
// Git-like Response Versioning Types (Reference-based)
// ============================================================================

export interface GitCommitRequest {
  message: string;
  trace_id: string;  // Reference to existing trace
  span_id?: string;  // Optional span within trace
  author?: string;
  branch?: string;
  model?: string;
  metadata?: {
    experiment?: string;
    variant?: string;
    latency_ms?: number;
    cost?: number;
  };
  parent?: string;
}

export interface GitCommitResponse {
  version_id: string;
  short_id: string;
  branch: string;
  trace_id: string;
  timestamp: number;
}

export interface GitLogEntry {
  id: string;
  short_id: string;
  message: string;
  author: string;
  timestamp: number;
  branch: string;
  trace_id: string;
  span_id?: string;
  model?: string;
  parent?: string;
  metadata: Record<string, any>;
}

export interface GitCommitDetail {
  version_id: string;
  trace_id: string;
  span_id?: string;
  branch: string;
  message: string;
  author: string;
  model?: string;
  timestamp: number;
  metadata: Record<string, any>;
  // These will be fetched from the trace storage
  prompt?: string;
  response?: string;
}

export interface GitDiffResult {
  from: string;
  to: string;
  changes: GitChange[];
  stats: {
    additions: number;
    deletions: number;
    files_changed: number;
  };
  semantic_similarity?: number;
}

export interface GitChange {
  type: 'Added' | 'Removed' | 'Modified' | 'Unchanged';
  path: string;
  content?: string;
  line_number?: number;
  old_content?: string;
  new_content?: string;
}

export interface GitBranch {
  name: string;
  commit: string;
  is_head: boolean;
  updated_at: number;
}

export interface GitTag {
  name: string;
  commit: string;
  message?: string;
  created_at: number;
}

export interface GitRepoStats {
  total_commits: number;
  total_branches: number;
  total_tags: number;
  total_objects: number;
  storage_bytes: number;
}

// ============================================================================
// Agentreplay Client Class
// ============================================================================

class AgentreplayClient {
  /**
   * Delete a trace by ID (permanently)
   * 
   * @param traceId - The ID of the trace to delete
   * @param projectId - Optional project ID for context
   */
  async deleteTrace(traceId: string, projectId: number = 0): Promise<void> {
    await invoke('delete_trace', { traceId, trace_id: traceId });
  }


  // ========== Health & Stats ==========

  async healthCheck() {
    return invoke<{ status: string; database_path: string }>('health_check');
  }

  async getDatabaseStats(): Promise<DatabaseStats> {
    return invoke<DatabaseStats>('get_db_stats');
  }

  // ========== Traces ==========

  async listTraces(params?: {
    limit?: number;
    offset?: number;
    start_time?: number;
    end_time?: number;
    agent_id?: string;
    project_id?: number;
    session_id?: number;
    status?: string[];
    span_types?: string[];
    min_latency_ms?: number;
    max_latency_ms?: number;
    min_cost?: number;
    max_cost?: number;
    min_tokens?: number;
    max_tokens?: number;
    min_confidence?: number;
    max_confidence?: number;
    providers?: string[];
    models?: string[];
    routes?: string[];
    has_errors?: boolean;
    full_text_search?: string;
    sort_by?: string;
    sort_order?: string;
  }) {
    return invoke<{ traces: TraceMetadata[]; total: number }>('list_traces', { params });
  }

  async getTrace(traceId: string, tenantId?: number, projectId?: number): Promise<any> {
    return invoke<any>('get_trace', {
      traceId,
      tenant_id: tenantId || 1,
      project_id: projectId || 44819
    });
  }

  async getTraceTree(traceId: string, tenantId?: number, projectId?: number): Promise<any> {
    return invoke<any>('get_trace_tree', {
      traceId,
      tenant_id: tenantId || 1,
      project_id: projectId || 44819
    });
  }

  async getTraceObservations(traceId: string, tenantId?: number, projectId?: number): Promise<any[]> {
    return invoke<any[]>('get_trace_observations', {
      traceId,
      tenant_id: tenantId || 1,
      project_id: projectId || 44819
    });
  }

  async searchTraces(query: string, projectId: number, limit?: number, embeddingConfig?: {
    provider: string;
    model: string;
    apiKey?: string | null;
    baseUrl?: string | null;
    enabled: boolean;
  }): Promise<{ traces: TraceMetadata[]; total: number }> {
    const response = await invoke<{ results?: TraceSearchView[]; traces?: TraceMetadata[]; count?: number; total?: number }>('search_traces', {
      params: {
        query,
        project_id: projectId,
        limit,
        embedding_config: embeddingConfig ? {
          provider: embeddingConfig.provider,
          model: embeddingConfig.model,
          api_key: embeddingConfig.apiKey || null,
          base_url: embeddingConfig.baseUrl || null,
          enabled: embeddingConfig.enabled,
        } : undefined,
      },
    });

    // Handle both API response formats
    // Server returns: { results, count, query_interpretation }
    // Tauri returns: array of TraceMetadata directly or { traces, total }
    if (Array.isArray(response)) {
      return { traces: response as TraceMetadata[], total: (response as TraceMetadata[]).length };
    }

    if (response.results) {
      // Map TraceSearchView to TraceMetadata - include all display fields
      const traces: TraceMetadata[] = response.results.map((r: any) => ({
        trace_id: r.edge_id,
        span_id: r.edge_id,
        timestamp_us: r.timestamp_us,
        duration_us: Math.round(r.duration_ms * 1000),
        token_count: r.tokens,
        cost: r.cost,
        status: r.status,
        agent_id: r.agent_id,
        agent_name: `Agent ${r.agent_id}`,
        model: r.model,
        operation: r.operation,
        span_type: r.span_type,
        project_id: r.project_id || projectId,
        session_id: r.session_id,
        tenant_id: 1,
        environment: 'production',
        sensitivity_flags: 0,
        // Include display fields from search response
        input_preview: r.input_preview,
        output_preview: r.output_preview,
        metadata: r.metadata,
      }));
      return { traces, total: response.count || traces.length };
    }

    return { traces: response.traces || [], total: response.total || 0 };
  }



  // ========== Metrics & Analytics ==========

  async getTimeseries(params: {
    start_time: number;
    end_time: number;
    interval_seconds: number;
    metric: string;
  }): Promise<TimeseriesDataPoint[]> {
    return invoke<TimeseriesDataPoint[]>('get_timeseries', { params });
  }

  /**
   * Get sharded timeseries with DDSketch percentiles and HyperLogLog cardinality.
   * This provides accurate P50/P95/P99 latency percentiles via streaming DDSketch
   * and unique session/agent counts via HyperLogLog probabilistic counting.
   * 
   * Automatically selects granularity based on time range:
   * - Up to 6 hours: minute buckets
   * - Up to 7 days: hour buckets
   * - Longer: day buckets
   */
  async getShardedTimeseries(params: {
    start_ts: number;
    end_ts: number;
    project_id?: number;
  }): Promise<ShardedTimeseriesPoint[]> {
    return invoke<ShardedTimeseriesPoint[]>('query_sharded_timeseries', params);
  }

  async getCosts(startTime?: number, endTime?: number): Promise<CostBreakdown> {
    // TODO: Implement proper costs endpoint on backend
    // For now, calculate from traces
    try {
      const traces = await this.listTraces({
        start_time: startTime,
        end_time: endTime,
        limit: 10000,
      });

      const totalCost = traces.traces.reduce((sum, t) => sum + (t.cost || 0), 0);

      return {
        total_cost: totalCost,
        by_model: [],
        by_user: [],
      };
    } catch (error) {
      console.warn('Failed to calculate costs:', error);
      return {
        total_cost: 0,
        by_model: [],
        by_user: [],
      };
    }
  }

  // ========== Agents ==========

  async listAgents() {
    return invoke<{ agents: Agent[] }>('list_agents');
  }

  async registerAgent(request: AgentRegistrationRequest) {
    return invoke<{ agent_id: string }>('register_agent', { request });
  }

  // ========== Evals ==========

  async listDatasets() {
    return invoke<{ datasets: EvalDataset[] }>('list_eval_datasets');
  }

  async getDataset(datasetId: string): Promise<EvalDataset> {
    return invoke<EvalDataset>('get_eval_dataset', { datasetId });
  }

  async createDataset(name: string, description?: string) {
    return invoke<{ dataset_id: string }>('create_eval_dataset', { name, description });
  }

  async deleteDataset(datasetId: string) {
    return invoke<{ success: boolean }>('delete_eval_dataset', { datasetId });
  }

  async addExamples(datasetId: string, examples: EvalExample[]) {
    return invoke<{ success: boolean }>('add_eval_examples', { datasetId, examples });
  }

  async deleteExample(datasetId: string, exampleId: string) {
    return invoke<{ success: boolean }>('delete_eval_example', { datasetId, exampleId });
  }

  async listEvalRuns() {
    return invoke<{ runs: EvalRun[] }>('list_eval_runs');
  }

  async getEvalRun(runId: string): Promise<EvalRun> {
    return invoke<EvalRun>('get_eval_run', { runId });
  }

  async createEvalRun(request: CreateEvalRunRequest): Promise<CreateEvalRunResponse> {
    return invoke<CreateEvalRunResponse>('create_eval_run', request);
  }

  async deleteEvalRun(runId: string): Promise<{ success: boolean }> {
    return invoke<{ success: boolean }>('delete_eval_run', { runId });
  }

  async addRunResult(runId: string, result: AddRunResultRequest): Promise<{ success: boolean; total_results: number }> {
    return invoke<{ success: boolean; total_results: number }>('add_run_result', { runId, ...result });
  }

  async completeEvalRun(runId: string, status?: 'completed' | 'failed' | 'stopped'): Promise<CompleteEvalRunResponse> {
    return invoke<CompleteEvalRunResponse>('complete_eval_run', { runId, status: status || 'completed' });
  }

  // ========== Statistical Comparison ==========

  async compareEvalRuns(
    baselineRunId: string,
    treatmentRunId: string,
    metricDirection?: Record<string, boolean>
  ): Promise<CompareEvalRunsResponse> {
    return invoke<CompareEvalRunsResponse>('compare_eval_runs', {
      baseline_run_id: baselineRunId,
      treatment_run_id: treatmentRunId,
      metric_direction: metricDirection,
    });
  }

  // ========== Evaluation Execution ==========

  async runGEval(request: GEvalRequest): Promise<EvaluationResult> {
    return invoke<EvaluationResult>('run_geval', request);
  }

  async runRagas(request: RagasRequest): Promise<EvaluationResult> {
    return invoke<EvaluationResult>('run_ragas', request);
  }

  async getEvaluationHistory(traceId: string): Promise<EvaluationResult[]> {
    return invoke<EvaluationResult[]>('get_evaluation_history', { traceId });
  }

  // ========== Experiments ==========

  async listExperiments(status?: string): Promise<ExperimentListResponse> {
    return invoke<ExperimentListResponse>('list_experiments', { status });
  }

  async getExperiment(experimentId: string): Promise<ExperimentResponse> {
    return invoke<ExperimentResponse>('get_experiment', { experimentId });
  }

  async createExperiment(request: CreateExperimentRequest): Promise<ExperimentResponse> {
    return invoke<ExperimentResponse>('create_experiment', request);
  }

  async updateExperiment(experimentId: string, request: UpdateExperimentRequest): Promise<ExperimentResponse> {
    return invoke<ExperimentResponse>('update_experiment', { experimentId, ...request });
  }

  async startExperiment(experimentId: string, trafficSplit: Record<string, number>): Promise<ExperimentResponse> {
    return invoke<ExperimentResponse>('start_experiment', { experimentId, traffic_split: trafficSplit });
  }

  async stopExperiment(experimentId: string): Promise<ExperimentResponse> {
    return invoke<ExperimentResponse>('stop_experiment', { experimentId });
  }

  async getExperimentStats(experimentId: string): Promise<ExperimentStatsResponse> {
    return invoke<ExperimentStatsResponse>('get_experiment_stats', { experimentId });
  }

  async recordExperimentResult(experimentId: string, request: RecordResultRequest): Promise<{ success: boolean }> {
    return invoke<{ success: boolean }>('record_experiment_result', { experimentId, ...request });
  }

  // ========== Prompts ==========

  async listPrompts(params?: { tag?: string; search?: string }): Promise<PromptListResponse> {
    return invoke<PromptListResponse>('list_prompts', params || {});
  }

  async getPrompt(promptId: string): Promise<PromptResponse> {
    return invoke<PromptResponse>('get_prompt', { promptId });
  }

  async createPrompt(request: CreatePromptRequest): Promise<PromptResponse> {
    return invoke<PromptResponse>('create_prompt', request);
  }

  async updatePrompt(promptId: string, request: UpdatePromptRequest): Promise<PromptResponse> {
    return invoke<PromptResponse>('update_prompt', { promptId, ...request });
  }

  async deletePrompt(promptId: string): Promise<{ success: boolean; message: string }> {
    return invoke<{ success: boolean; message: string }>('delete_prompt', { promptId });
  }

  async renderPrompt(promptId: string, variables: Record<string, string>): Promise<{ rendered: string }> {
    return invoke<{ rendered: string }>('render_prompt', { promptId, variables });
  }

  async getPromptVersionHistory(promptId: string): Promise<PromptVersionHistoryResponse> {
    return invoke<PromptVersionHistoryResponse>('get_prompt_version_history', { promptId });
  }

  async getPromptDiff(promptId: string, version1: number, version2: number): Promise<PromptDiffResponse> {
    return invoke<PromptDiffResponse>('get_prompt_diff', { promptId, version1, version2 });
  }

  // ========== Memory/RAG ==========

  async ingestMemory(request: IngestMemoryRequest): Promise<{ memory_id: string }> {
    return invoke<{ memory_id: string }>('ingest_memory', request);
  }

  async retrieveMemory(request: RetrieveMemoryRequest): Promise<MemoryRetrievalResponse> {
    return invoke<MemoryRetrievalResponse>('retrieve_memory', request);
  }

  async listCollections(): Promise<{ collections: MemoryCollection[] }> {
    return invoke<{ collections: MemoryCollection[] }>('list_collections');
  }

  async getCollection(collection: string): Promise<MemoryCollection> {
    return invoke<MemoryCollection>('get_collection', { collection });
  }

  async deleteCollection(collection: string): Promise<{ success: boolean }> {
    return invoke<{ success: boolean }>('delete_collection', { collection });
  }

  // ========== Backup & Retention ==========

  async createBackup() {
    return invoke<BackupMetadata>('create_backup');
  }

  async listBackups() {
    return invoke<{ backups: BackupMetadata[] }>('list_backups');
  }

  async restoreBackup(backupId: string) {
    return invoke<{ success: boolean }>('restore_backup', { backupId });
  }

  async getRetentionPolicies() {
    return invoke<{ policies: RetentionPolicy[] }>('get_retention_policies');
  }

  async setRetentionPolicy(environment: string, retentionDays: number) {
    return invoke<{ success: boolean }>('set_retention_policy', { environment, retentionDays });
  }

  async getRetentionStats() {
    return invoke<RetentionStats>('get_retention_stats');
  }

  async applyRetention() {
    return invoke<{ deleted_count: number }>('apply_retention');
  }

  // ========== Projects ==========

  async listProjects() {
    return invoke<{
      projects: Array<{
        project_id: number;
        name: string;
        description?: string;
        created_at: number;
        trace_count: number;
        favorite: boolean;
      }>;
      total: number;
    }>('list_projects');
  }

  async getProjectMetrics(projectId: number) {
    return invoke<ProjectMetricsResponse>('get_project_metrics', { projectId });
  }

  async createProject(name: string, description?: string) {
    return invoke<{ project_id: number }>('create_project', { name, description });
  }

  async deleteProject(projectId: number | string) {
    return invoke<{ success: boolean; message: string; deleted_count?: number }>('delete_project', { projectId: String(projectId) });
  }

  // ========== Dataset Flywheel ==========

  /**
   * Get candidates for fine-tuning dataset based on evaluation scores.
   * Implements τ_high and τ_low thresholds from the Dataset Flywheel.
   */
  async getDatasetCandidates(params?: {
    positive_threshold?: number;
    negative_threshold?: number;
    limit?: number;
    project_id?: number;
  }) {
    return invoke<{
      positive_candidates: Array<{
        trace_id: string;
        score: number;
        timestamp_us: number;
        has_payload: boolean;
      }>;
      negative_candidates: Array<{
        trace_id: string;
        score: number;
        timestamp_us: number;
        has_payload: boolean;
      }>;
      thresholds: {
        positive: number;
        negative: number;
      };
    }>('get_dataset_candidates', params || {});
  }

  /**
   * Export traces to a fine-tuning dataset in JSONL format.
   */
  async exportFinetuningDataset(params?: {
    positive_threshold?: number;
    negative_threshold?: number;
    max_examples?: number;
    include_scores?: boolean;
  }) {
    return invoke<{
      jsonl: string;
      positive_count: number;
      negative_count: number;
      total_examples: number;
    }>('export_finetuning_dataset', params || {});
  }

  // ========== Time-Travel Debugging ==========

  /**
   * Get a preview of fork state at a specific span.
   */
  async getForkPreview(spanId: string) {
    return invoke<{
      span_id: string;
      path_depth: number;
      session_id: number;
      timestamp_us: number;
      token_count: number;
      can_fork: boolean;
      message: string;
    }>('get_fork_preview', { span_id: spanId });
  }

  /**
   * Fork trace state at a specific span, reconstructing full conversation history.
   * Returns state ready for the Playground.
   */
  async forkTraceState(spanId: string) {
    return invoke<{
      messages: Array<{
        role: 'system' | 'user' | 'assistant' | 'tool';
        content: string;
        tool_call_id?: string;
        tool_calls?: any[];
      }>;
      span_path: Array<{
        span_id: string;
        parent_id?: string;
        name: string;
        span_type: string;
        timestamp_us: number;
        duration_us: number;
        input?: string;
        output?: string;
      }>;
      fork_point: {
        span_id: string;
        parent_id?: string;
        name: string;
        span_type: string;
        timestamp_us: number;
        duration_us: number;
        input?: string;
        output?: string;
      };
      total_tokens: number;
      system_prompt?: string;
      context_variables: Record<string, any>;
    }>('fork_trace_state', { span_id: spanId });
  }

  // ========== Insights ==========

  async getInsights(params: InsightsQuery) {
    return invoke<InsightsResponse>('get_insights', params);
  }

  async getInsightsSummary(projectId: number) {
    return invoke<InsightsSummary>('get_insights_summary', { project_id: projectId });
  }

  // ========== Updates ==========

  async checkForUpdates() {
    return invoke<{ available: boolean; current_version: string; latest_version?: string }>('check_for_updates');
  }

  // ========== Admin ==========

  async resetAllData() {
    // For full reset (including traces), use Tauri command directly
    // This will delete the database and restart the app
    const isTauri = typeof window !== 'undefined' &&
      ('__TAURI__' in window || '__TAURI_INTERNALS__' in window);

    if (isTauri) {
      try {
        // Import Tauri invoke dynamically
        const { invoke: tauriInvoke } = await import('@tauri-apps/api/core');
        // This will restart the app, so we won't get a response
        await tauriInvoke('reset_all_data');
        return { success: true };
      } catch (error) {
        console.error('Tauri reset failed:', error);
        // Fall back to HTTP endpoint
      }
    }

    // Fallback: HTTP endpoint (only clears projects, not traces)
    return invoke<{ success: boolean; message?: string }>('reset_all_data');
  }

  // ========== Tool Registry ==========

  async listTools() {
    return invoke<{ tools: ToolInfo[] }>('list_tools');
  }

  async getTool(toolId: string) {
    return invoke<ToolInfo>('get_tool', { toolId });
  }

  async registerTool(tool: RegisterToolRequest) {
    return invoke<ToolInfo>('register_tool', tool);
  }

  async updateTool(toolId: string, updates: Partial<RegisterToolRequest>) {
    return invoke<ToolInfo>('update_tool', { toolId, ...updates });
  }

  async unregisterTool(toolId: string) {
    return invoke<{ success: boolean }>('unregister_tool', { toolId });
  }

  async executeTool(toolId: string, input: Record<string, any>) {
    return invoke<ToolExecutionResult>('execute_tool', { toolId, input });
  }

  async getToolExecutions(toolId: string, limit?: number) {
    return invoke<{ executions: ToolExecution[] }>('get_tool_executions', { toolId, limit });
  }

  async listMcpServers() {
    return invoke<{ servers: McpServer[] }>('list_mcp_servers');
  }

  async syncMcpServer(serverId: string) {
    return invoke<{ tools: ToolInfo[] }>('sync_mcp_server', { serverId });
  }

  async connectMcpServer(uri: string, name?: string) {
    return invoke<McpServer>('connect_mcp_server', { uri, name });
  }

  async disconnectMcpServer(serverId: string) {
    return invoke<{ success: boolean }>('disconnect_mcp_server', { serverId });
  }

  // ========== Version Store (Reference-based) ==========

  async gitCommit(request: GitCommitRequest) {
    return invoke<GitCommitResponse>('version_commit', request);
  }

  async gitLog(branch?: string, limit?: number) {
    return invoke<{ versions: GitLogEntry[] }>('version_log', { branch, limit });
  }

  async gitShow(versionId: string) {
    return invoke<GitCommitDetail>('version_show', { versionId });
  }

  async gitDiff(from: string, to: string) {
    return invoke<GitDiffResult>('version_diff', { from, to });
  }

  async gitListBranches() {
    return invoke<{ branches: GitBranch[] }>('version_branches');
  }

  async gitCreateBranch(name: string, fromVersion?: string) {
    return invoke<GitBranch>('version_create_branch', { name, fromVersion });
  }

  async gitDeleteBranch(branchName: string) {
    return invoke<{ success: boolean }>('version_delete_branch', { branchName });
  }

  async gitCheckout(ref: string) {
    return invoke<{ success: boolean; head: string }>('version_checkout', { ref });
  }

  async gitListTags() {
    return invoke<{ tags: GitTag[] }>('version_tags');
  }

  async gitCreateTag(name: string, versionId: string, message?: string) {
    return invoke<GitTag>('version_create_tag', { name, versionId, message });
  }

  async gitDeleteTag(tagName: string) {
    return invoke<{ success: boolean }>('version_delete_tag', { tagName });
  }

  async gitStats() {
    return invoke<GitRepoStats>('version_stats');
  }

  // ========== Evaluation Pipeline (5-Phase) ==========

  /**
   * Phase 1: Collect traces with filtering
   */
  async evalPipelineCollect(request: CollectTracesRequest): Promise<CollectTracesResponse> {
    return invoke<CollectTracesResponse>('eval_pipeline_collect', request);
  }

  /**
   * Phase 2: Process and categorize traces
   */
  async evalPipelineProcess(request: ProcessTracesRequest): Promise<ProcessTracesResponse> {
    return invoke<ProcessTracesResponse>('eval_pipeline_process', request);
  }

  /**
   * Phase 3: Create annotation
   */
  async evalPipelineAnnotate(request: CreateAnnotationRequest): Promise<{ annotation: Annotation }> {
    return invoke<{ annotation: Annotation }>('eval_pipeline_annotate', request);
  }

  /**
   * Phase 3: Add golden test cases
   */
  async evalPipelineAddGolden(request: AddGoldenTestCasesRequest): Promise<AddGoldenTestCasesResponse> {
    return invoke<AddGoldenTestCasesResponse>('eval_pipeline_golden', request);
  }

  /**
   * Phase 4: Run comprehensive evaluation
   */
  async evalPipelineEvaluate(request: RunEvaluationRequest): Promise<EvaluationResults> {
    return invoke<EvaluationResults>('eval_pipeline_evaluate', request);
  }

  /**
   * Phase 5: Get recommendations
   */
  async evalPipelineRecommendations(params?: {
    run_id?: string;
    category?: MetricCategory;
    include_historical?: boolean;
  }): Promise<RecommendationsResponse> {
    return invoke<RecommendationsResponse>('eval_pipeline_recommendations', params);
  }

  /**
   * Get all metric definitions
   */
  async evalPipelineMetricDefinitions(): Promise<MetricDefinition[]> {
    return invoke<MetricDefinition[]>('eval_pipeline_metric_definitions');
  }

  /**
   * Get evaluation history
   */
  async evalPipelineHistory(params?: {
    limit?: number;
    category?: string;
  }): Promise<EvalHistoryResponse> {
    return invoke<EvalHistoryResponse>('eval_pipeline_history', params);
  }
  // Generic invoke for ad-hoc commands
  async invoke<T>(command: string, args?: any): Promise<T> {
    return invoke<T>(command, args);
  }
}

// ============================================================================
// Export singleton instance
// ============================================================================

export const agentreplayClient = new AgentreplayClient();

// SWR fetcher function (used by some components)
export const fetcher = async (url: string) => {
  // Extract the path and convert to appropriate Tauri command
  // This is a simple adapter - expand as needed
  if (url.includes('/costs')) {
    return agentreplayClient.getCosts();
  }
  if (url.includes('/timeseries')) {
    const params = new URL(url, 'http://localhost').searchParams;
    return agentreplayClient.getTimeseries({
      start_time: Number(params.get('start_time') || 0),
      end_time: Number(params.get('end_time') || Date.now()),
      interval_seconds: Number(params.get('interval') || 3600),
      metric: params.get('metric') || 'request_count',
    });
  }
  throw new Error(`Unsupported fetch URL: ${url}`);
};

// Types are already exported above with 'export interface'
