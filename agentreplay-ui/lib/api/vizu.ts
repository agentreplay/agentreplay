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
 * Agentreplay API Client for Tauri Desktop
 * 
 * This adapter makes the advanced UI (from app/) work with Tauri commands.
 * It provides the same API interface as the web version but uses Tauri IPC.
 */

import { invoke } from '@tauri-apps/api/core';

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
  agent_id: string;
  started_at: number;
  ended_at?: number;
  duration_ms?: number;
  status: string;
  cost?: number;
  tokens?: number;
  error?: string;
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
  dataset_id: string;
  name: string;
  created_at: number;
  results: Array<{
    example_id: string;
    actual_output: any;
    passed: boolean;
    score?: number;
    metadata?: Record<string, any>;
  }>;
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

// ============================================================================
// Agentreplay Client Class
// ============================================================================

class AgentreplayClient {
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
  }) {
    return invoke<{ traces: TraceMetadata[]; total: number }>('list_traces', { params });
  }

  async getTrace(traceId: string): Promise<TraceDetails> {
    return invoke<TraceDetails>('get_trace', { traceId });
  }

  async searchTraces(query: string, limit?: number) {
    return invoke<{ traces: TraceMetadata[]; total: number }>('search_traces', {
      params: { query, limit },
    });
  }

  async deleteTrace(traceId: string) {
    return invoke<{ success: boolean }>('delete_trace', { traceId });
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

  async getCosts(startTime?: number, endTime?: number): Promise<CostBreakdown> {
    return invoke<CostBreakdown>('get_costs', { startTime, endTime });
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

  async addExamples(datasetId: string, examples: EvalExample[]) {
    return invoke<{ success: boolean }>('add_eval_examples', { datasetId, examples });
  }

  async listEvalRuns() {
    return invoke<{ runs: EvalRun[] }>('list_eval_runs');
  }

  async getEvalRun(runId: string): Promise<EvalRun> {
    return invoke<EvalRun>('get_eval_run', { runId });
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
    return invoke<{ projects: Array<{ name: string; trace_count: number }> }>('list_projects');
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
