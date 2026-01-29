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

// Agentreplay API client for UI
import axios from 'axios';

const AGENTREPLAY_BASE_URL = process.env.NEXT_PUBLIC_AGENTREPLAY_URL || 'http://localhost:9600';
// Alias for backwards compatibility
const CHRONOLAKE_BASE_URL = AGENTREPLAY_BASE_URL;

// Retry configuration
interface RetryConfig {
  maxRetries: number;
  initialDelay: number;
  maxDelay: number;
  backoffMultiplier: number;
  retryableStatusCodes: Set<number>;
}

const DEFAULT_RETRY_CONFIG: RetryConfig = {
  maxRetries: 3,
  initialDelay: 1000, // 1 second
  maxDelay: 10000, // 10 seconds
  backoffMultiplier: 2,
  retryableStatusCodes: new Set([408, 429, 500, 502, 503, 504]),
};

// Circuit breaker state
enum CircuitState {
  CLOSED = 'CLOSED',
  OPEN = 'OPEN',
  HALF_OPEN = 'HALF_OPEN',
}

class CircuitBreaker {
  private state: CircuitState = CircuitState.CLOSED;
  private failureCount: number = 0;
  private lastFailureTime: number = 0;
  private readonly failureThreshold: number = 5;
  private readonly recoveryTimeout: number = 60000; // 1 minute

  async execute<T>(fn: () => Promise<T>): Promise<T> {
    // Check if circuit is open
    if (this.state === CircuitState.OPEN) {
      const now = Date.now();
      if (now - this.lastFailureTime >= this.recoveryTimeout) {
        this.state = CircuitState.HALF_OPEN;
        this.failureCount = 0;
      } else {
        throw new Error('Circuit breaker is OPEN - service unavailable');
      }
    }

    try {
      const result = await fn();
      
      // Success - reset if half-open or keep closed
      if (this.state === CircuitState.HALF_OPEN) {
        this.state = CircuitState.CLOSED;
        this.failureCount = 0;
      }
      
      return result;
    } catch (error) {
      this.failureCount++;
      this.lastFailureTime = Date.now();
      
      // Open circuit if threshold exceeded
      if (this.failureCount >= this.failureThreshold) {
        this.state = CircuitState.OPEN;
      }
      
      throw error;
    }
  }

  getState(): CircuitState {
    return this.state;
  }
}

// Exponential backoff with jitter
async function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function calculateBackoff(attempt: number, config: RetryConfig): number {
  const exponentialDelay = config.initialDelay * Math.pow(config.backoffMultiplier, attempt);
  const cappedDelay = Math.min(exponentialDelay, config.maxDelay);
  // Add jitter (±25%)
  const jitter = cappedDelay * 0.25 * (Math.random() - 0.5);
  return Math.floor(cappedDelay + jitter);
}

// Retry wrapper with exponential backoff
async function fetchWithRetry<T>(
  fn: () => Promise<T>,
  config: Partial<RetryConfig> = {}
): Promise<T> {
  const retryConfig = { ...DEFAULT_RETRY_CONFIG, ...config };
  let lastError: Error;

  for (let attempt = 0; attempt <= retryConfig.maxRetries; attempt++) {
    try {
      return await fn();
    } catch (error: any) {
      lastError = error;
      
      // Don't retry if not a retryable error
      const statusCode = error.response?.status;
      const isRetryable = statusCode && retryConfig.retryableStatusCodes.has(statusCode);
      
      if (!isRetryable || attempt === retryConfig.maxRetries) {
        throw error;
      }
      
      // Wait before retrying
      const backoffDelay = calculateBackoff(attempt, retryConfig);
      console.warn(`Request failed (attempt ${attempt + 1}/${retryConfig.maxRetries + 1}), retrying in ${backoffDelay}ms...`, {
        statusCode,
        error: error.message,
      });
      
      await sleep(backoffDelay);
    }
  }

  throw lastError!;
}

// SWR fetcher with retry support
export async function fetcher<T>(url: string): Promise<T> {
  return fetchWithRetry(async () => {
    const response = await fetch(url);
    if (!response.ok) {
      const error: any = new Error('API request failed');
      error.response = { status: response.status };
      throw error;
    }
    return response.json();
  });
}

export interface Trace {
  id: string;
  timestamp: number;
  name: string;
  userId?: string;
  sessionId?: string;
  session_id?: string; // Backend returns session_id
  agent_id?: string; // Backend returns agent_id
  duration: number;
  inputTokens: number;
  outputTokens: number;
  cost: number;
  model: string;
  status: 'success' | 'error';
  environment?: string; // "development", "staging", "production", "test"
  tenant_id?: number; // Tenant identifier
  project_id?: number; // Project identifier
  agentId?: number; // Agent identifier
  agentName?: string; // Human-readable agent name
  metadata?: Record<string, any>; // Backend returns metadata field with OpenTelemetry attributes
  attributes?: Record<string, any>; // Alias for metadata
  evaluations?: {
    hallucination?: number;
    relevance?: number;
    groundedness?: number;
    toxicity?: number;
  };
}

export interface Metrics {
  totalRequests: number;
  totalCost: number;
  avgLatency: number;
  errorRate: number;
  avgTokensPerRequest: number;
  activeAgents?: number;
}

export interface Agent {
  agent_id: number;
  name: string;
  namespace?: string;
  version?: string;
  description?: string;
  created_at: number;
  updated_at: number;
  metadata?: Record<string, string>;
}

export interface AgentRegistrationRequest {
  agent_id: number;
  name: string;
  namespace?: string;
  version?: string;
  description?: string;
  metadata?: Record<string, string>;
}

// Saved Views types
export interface SavedView {
  id: string;
  name: string;
  description?: string;
  filters?: {
    timeRange?: { start: number; end: number };
    environment?: string;
    agentId?: number;
    userId?: string;
    status?: string;
  };
  columns?: string[];
  tags?: string[];
  createdAt: number;
  updatedAt: number;
}

export interface CreateViewRequest {
  name: string;
  description?: string;
  filters?: SavedView['filters'];
  columns?: string[];
  tags?: string[];
}

export interface UpdateViewRequest {
  name?: string;
  description?: string;
  filters?: SavedView['filters'];
  columns?: string[];
  tags?: string[];
}

// Backup types
export interface BackupMetadata {
  backup_id: string;
  backup_path: string;
  source_path: string;
  created_at: number;
  file_count: number;
  total_size: number;
  checksum: string;
}

// Retention types
export interface RetentionPolicy {
  environment: string;
  retention_days: number;
  enabled: boolean;
}

export interface RetentionStats {
  records_scanned: number;
  records_deleted: number;
  bytes_freed: number;
  duration_ms: number;
  environments_processed: string[];
}

export interface DatabaseStats {
  total_records: number;
  total_size_bytes: number;
  environments: Record<string, { count: number; size_bytes: number }>;
  oldest_record_timestamp?: number;
  newest_record_timestamp?: number;
}

// Eval Dataset types
export interface EvalDataset {
  id: string;
  name: string;
  description?: string;
  examples: EvalExample[];
  metadata?: Record<string, any>;
  created_at: number;
  updated_at: number;
}

export interface EvalExample {
  id: string;
  input: string;
  expected_output?: string;
  metadata?: Record<string, any>;
}

export interface CreateDatasetRequest {
  name: string;
  description?: string;
  examples?: EvalExample[];
  metadata?: Record<string, any>;
}

export interface UpdateDatasetRequest {
  name?: string;
  description?: string;
  metadata?: Record<string, any>;
}

// Eval Run types
export interface EvalRun {
  id: string;
  name: string;
  dataset_id: string;
  model: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  results: EvalResult[];
  summary?: {
    total: number;
    passed: number;
    failed: number;
    avg_score?: number;
  };
  metadata?: Record<string, any>;
  created_at: number;
  updated_at: number;
  completed_at?: number;
}

export interface EvalResult {
  example_id: string;
  actual_output: string;
  score: number;
  passed: boolean;
  metrics?: {
    hallucination?: number;
    relevance?: number;
    groundedness?: number;
    toxicity?: number;
  };
  error?: string;
  latency_ms?: number;
}

export interface CreateRunRequest {
  name: string;
  dataset_id: string;
  model: string;
  metadata?: Record<string, any>;
}

export interface UpdateRunRequest {
  name?: string;
  status?: EvalRun['status'];
  metadata?: Record<string, any>;
}

export interface TraceDetails extends Trace {
  spans: Span[];
  input?: string;
  output?: string;
  metadata?: Record<string, any>; // Full OTEL attributes from payload
  attributes?: Record<string, any>; // Alias for metadata
}

export interface Span {
  id: string;
  name: string;
  startTime: number;
  endTime: number;
  duration: number;
  attributes?: Record<string, any>;
  children?: Span[];
}

class ChronoLakeClient {
  private baseURL: string;
  private apiKey?: string;
  private circuitBreaker: CircuitBreaker;
  private tenantId?: string;
  private projectId?: string;

  constructor(baseURL: string = CHRONOLAKE_BASE_URL) {
    this.baseURL = baseURL;
    this.apiKey = process.env.NEXT_PUBLIC_AGENTREPLAY_API_KEY;
    this.circuitBreaker = new CircuitBreaker();
    
    // Load tenant/project from localStorage if available
    if (typeof window !== 'undefined') {
      this.tenantId = localStorage.getItem('agentreplay_tenant_id') || undefined;
      this.projectId = localStorage.getItem('agentreplay_project_id') || undefined;
    }
  }

  setTenant(tenantId: string) {
    this.tenantId = tenantId;
    if (typeof window !== 'undefined') {
      localStorage.setItem('agentreplay_tenant_id', tenantId);
    }
  }

  setProject(projectId: string) {
    this.projectId = projectId;
    if (typeof window !== 'undefined') {
      localStorage.setItem('agentreplay_project_id', projectId);
    }
  }

  private getHeaders() {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    if (this.apiKey) {
      headers['X-Agentreplay-API-Key'] = this.apiKey;
    }

    if (this.tenantId) {
      headers['X-Tenant-ID'] = this.tenantId;
    }

    if (this.projectId) {
      headers['X-Project-ID'] = this.projectId;
    }

    return headers;
  }

  async getTraces(params: {
    limit?: number;
    page?: number;
    startTime?: number;
    endTime?: number;
    userId?: string;
    status?: string;
    environment?: string; // NEW: "development", "staging", "production", "test"
    agentId?: number; // NEW: Filter by specific agent
  } = {}): Promise<{ data: Trace[]; total: number }> {
    try {
      return await this.circuitBreaker.execute(async () => {
        return await fetchWithRetry(async () => {
          // Map frontend parameters to backend API contract
          const apiParams: any = {
            limit: params.limit || 20,
            offset: params.page ? (params.page - 1) * (params.limit || 20) : 0,
          };
          
          // Map timestamp parameters (convert ms to microseconds if provided)
          if (params.startTime) {
            apiParams.start_ts = params.startTime * 1000; // ms → μs
          }
          if (params.endTime) {
            apiParams.end_ts = params.endTime * 1000; // ms → μs
          }
          
          // Map userId to session_id (backend uses session_id)
          if (params.userId) {
            // Try to parse as number for session_id, otherwise skip
            const sessionId = parseInt(params.userId, 10);
            if (!isNaN(sessionId)) {
              apiParams.session_id = sessionId;
            }
          }

          // NEW: Environment filter
          if (params.environment) {
            apiParams.environment = params.environment;
          }

          // NEW: Agent ID filter
          if (params.agentId) {
            apiParams.agent_id = params.agentId;
          }
          
          // Note: status filtering can be added via environment/agent filters later (Task 5)
          
          const response = await axios.get(`${this.baseURL}/api/v1/traces`, {
            params: apiParams,
            headers: this.getHeaders(),
          });
          return response.data;
        });
      });
    } catch (error) {
      console.error('Error fetching traces:', error);
      // Return mock data only in development
      if (process.env.NODE_ENV === 'development') {
        console.warn('Falling back to mock data in development mode');
        return this.getMockTraces(params.limit || 20);
      }
      throw error;
    }
  }

  async getTrace(traceId: string): Promise<TraceDetails> {
    try {
      return await this.circuitBreaker.execute(async () => {
        return await fetchWithRetry(async () => {
          // Build query params with tenant_id and project_id
          const params = new URLSearchParams();
          if (this.tenantId) params.append('tenant_id', this.tenantId);
          if (this.projectId) params.append('project_id', this.projectId);
          
          const url = `${this.baseURL}/api/v1/traces/${traceId}${params.toString() ? `?${params.toString()}` : ''}`;
          const response = await axios.get(url, {
            headers: this.getHeaders(),
          });
          
          // Log for debugging
          if (process.env.NODE_ENV === 'development') {
            console.log('[Agentreplay] Fetched trace:', response.data.span_id, 'metadata keys:', Object.keys(response.data.metadata || {}));
          }
          
          return response.data;
        });
      });
    } catch (error) {
      console.error('Error fetching trace details:', error);
      if (process.env.NODE_ENV === 'development') {
        console.warn('Trace detail fetch failed in development mode');
      }
      throw error;
    }
  }

  async getObservations(traceId: string): Promise<Span[]> {
    try {
      return await this.circuitBreaker.execute(async () => {
        return await fetchWithRetry(async () => {
          // Use the rich /traces/:id/observations endpoint (returns full span tree with attributes)
          const response = await axios.get(`${this.baseURL}/api/v1/traces/${traceId}/observations`, {
            headers: this.getHeaders(),
          });
          return response.data;
        });
      });
    } catch (error) {
      console.error('Error fetching observations:', error);
      if (process.env.NODE_ENV === 'development') {
        console.warn('Observations fetch failed, returning empty array');
      }
      return [];
    }
  }

  async submitFeedback(traceId: string, feedback: 1 | -1): Promise<void> {
    try {
      await this.circuitBreaker.execute(async () => {
        return await fetchWithRetry(async () => {
          await axios.post(
            `${this.baseURL}/api/v1/traces/${traceId}/feedback`,
            { feedback },
            { headers: this.getHeaders() }
          );
        });
      });
    } catch (error) {
      console.error('Error submitting feedback:', error);
      throw error;
    }
  }

  async addToDataset(
    datasetName: string,
    data: {
      trace_id: string;
      input?: any;
      output?: any;
      expected_output?: any;
    }
  ): Promise<void> {
    try {
      await this.circuitBreaker.execute(async () => {
        return await fetchWithRetry(async () => {
          await axios.post(
            `${this.baseURL}/api/v1/datasets/${encodeURIComponent(datasetName)}/add`,
            data,
            { headers: this.getHeaders() }
          );
        });
      });
    } catch (error) {
      console.error('Error adding to dataset:', error);
      throw error;
    }
  }

  async getMetrics(params: {
    startTime?: number;
    endTime?: number;
    granularity?: 'hour' | 'day' | 'week';
    environment?: string; // NEW: Filter by environment
    agentId?: number; // NEW: Filter by agent
  } = {}): Promise<Metrics> {
    try {
      return await this.circuitBreaker.execute(async () => {
        return await fetchWithRetry(async () => {
          // Map frontend parameters to backend API contract
          const apiParams: any = {
            granularity: params.granularity || 'hour',
          };
          
          // Map timestamp parameters (convert ms to microseconds if provided)
          if (params.startTime) {
            apiParams.start_ts = params.startTime * 1000; // ms → μs
          }
          if (params.endTime) {
            apiParams.end_ts = params.endTime * 1000; // ms → μs
          }

          // NEW: Environment filter
          if (params.environment) {
            apiParams.environment = params.environment;
          }

          // NEW: Agent ID filter
          if (params.agentId) {
            apiParams.agent_id = params.agentId;
          }
          
          const response = await axios.get(`${this.baseURL}/api/v1/metrics/timeseries`, {
            params: apiParams,
            headers: this.getHeaders(),
          });
          return response.data;
        });
      });
    } catch (error) {
      console.error('Error fetching metrics:', error);
      // Return mock metrics only in development
      if (process.env.NODE_ENV === 'development') {
        console.warn('Falling back to mock metrics in development mode');
        return this.getMockMetrics();
      }
      throw error;
    }
  }

  // Mock data for development/testing
  private getMockTraces(limit: number): { data: Trace[]; total: number } {
    const traces = Array.from({ length: limit }, (_, i) => ({
      id: `trace-${i}`,
      timestamp: Date.now() - i * 60000,
      name: 'process_query',
      userId: `user-${Math.floor(Math.random() * 5)}`,
      sessionId: `session-${Math.floor(Math.random() * 10)}`,
      duration: 800 + Math.random() * 2000,
      inputTokens: 200 + Math.floor(Math.random() * 500),
      outputTokens: 100 + Math.floor(Math.random() * 400),
      cost: parseFloat((Math.random() * 0.05).toFixed(4)),
      model: Math.random() > 0.5 ? 'claude-sonnet-4.5' : 'claude-haiku-4',
      status: (Math.random() > 0.9 ? 'error' : 'success') as 'success' | 'error',
      evaluations: {
        hallucination: Math.random() * 0.1,
        relevance: 0.8 + Math.random() * 0.2,
        groundedness: 0.75 + Math.random() * 0.25,
        toxicity: Math.random() * 0.05,
      },
    }));

    return { data: traces, total: limit };
  }

  private getMockMetrics(): Metrics {
    return {
      totalRequests: 12450,
      totalCost: 245.67,
      avgLatency: 1234,
      errorRate: 0.023,
      avgTokensPerRequest: 650,
      activeAgents: 12,
    };
  }

  // Agent Registry API methods
  
  /**
   * Get all registered agents
   */
  async getAgents(): Promise<{ agents: Agent[]; total: number }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/agents`, {
          headers: this.getHeaders(),
        });
        return response.data;
      });
    });
  }

  /**
   * Get a specific agent by ID
   */
  async getAgent(agentId: number): Promise<Agent> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/agents/${agentId}`, {
          headers: this.getHeaders(),
        });
        return response.data;
      });
    });
  }

  /**
   * Register a new agent or update existing
   */
  async registerAgent(request: AgentRegistrationRequest): Promise<{ success: boolean; agent: Agent }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.post(
          `${this.baseURL}/api/v1/agents/register`,
          request,
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Update agent metadata
   */
  async updateAgent(
    agentId: number,
    updates: Partial<Omit<AgentRegistrationRequest, 'agent_id'>>
  ): Promise<{ success: boolean; agent: Agent }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.put(
          `${this.baseURL}/api/v1/agents/${agentId}`,
          updates,
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Delete an agent registration
   */
  async deleteAgent(agentId: number): Promise<void> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        await axios.delete(`${this.baseURL}/api/v1/agents/${agentId}`, {
          headers: this.getHeaders(),
        });
      });
    });
  }

  // Saved Views API methods

  /**
   * Get all saved views
   */
  async getSavedViews(): Promise<{ views: SavedView[] }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/views`, {
          headers: this.getHeaders(),
        });
        return response.data;
      });
    });
  }

  /**
   * Get a specific saved view by ID
   */
  async getSavedView(viewId: string): Promise<SavedView> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/views/${viewId}`, {
          headers: this.getHeaders(),
        });
        return response.data;
      });
    });
  }

  /**
   * Create a new saved view
   */
  async createSavedView(request: CreateViewRequest): Promise<{ success: boolean; view: SavedView }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.post(
          `${this.baseURL}/api/v1/views`,
          request,
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Update an existing saved view
   */
  async updateSavedView(
    viewId: string,
    updates: UpdateViewRequest
  ): Promise<{ success: boolean; view: SavedView }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.put(
          `${this.baseURL}/api/v1/views/${viewId}`,
          updates,
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Delete a saved view
   */
  async deleteSavedView(viewId: string): Promise<void> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        await axios.delete(`${this.baseURL}/api/v1/views/${viewId}`, {
          headers: this.getHeaders(),
        });
      });
    });
  }

  /**
   * Search saved views by tag or name
   */
  async searchSavedViews(query?: string, tag?: string): Promise<{ views: SavedView[] }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const params: any = {};
        if (query) params.query = query;
        if (tag) params.tag = tag;
        
        const response = await axios.get(`${this.baseURL}/api/v1/views/search`, {
          headers: this.getHeaders(),
          params,
        });
        return response.data;
      });
    });
  }

  // Backup & Restore API methods

  /**
   * Create a backup of the database
   */
  async createBackup(): Promise<{ backup_id: string; backup_path: string; created_at: number }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.post(
          `${this.baseURL}/api/v1/backup`,
          {},
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * List all available backups
   */
  async listBackups(): Promise<{ backups: BackupMetadata[] }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/backup`, {
          headers: this.getHeaders(),
        });
        return response.data;
      });
    });
  }

  /**
   * Restore database from a backup
   */
  async restoreBackup(backupId: string): Promise<{ success: boolean; message: string }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.post(
          `${this.baseURL}/api/v1/backup/restore`,
          { backup_id: backupId },
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Verify a backup's integrity
   */
  async verifyBackup(backupId: string): Promise<{ valid: boolean; message?: string }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.post(
          `${this.baseURL}/api/v1/backup/verify`,
          { backup_id: backupId },
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  // Retention Policy API methods

  /**
   * Get current retention configuration
   */
  async getRetentionConfig(): Promise<{ policies: RetentionPolicy[] }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/retention/config`, {
          headers: this.getHeaders(),
        });
        return response.data;
      });
    });
  }

  /**
   * Update retention policies
   */
  async updateRetentionConfig(policies: RetentionPolicy[]): Promise<{ success: boolean }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.post(
          `${this.baseURL}/api/v1/retention/config`,
          { policies },
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Trigger manual cleanup based on retention policies
   */
  async triggerRetentionCleanup(): Promise<RetentionStats> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.post(
          `${this.baseURL}/api/v1/retention/cleanup`,
          {},
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Get database statistics
   */
  async getDatabaseStats(): Promise<DatabaseStats> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/retention/stats`, {
          headers: this.getHeaders(),
        });
        return response.data;
      });
    });
  }

  // Eval Datasets API methods

  /**
   * Get all eval datasets
   */
  async getEvalDatasets(): Promise<{ datasets: EvalDataset[] }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/evals/datasets`, {
          headers: this.getHeaders(),
        });
        return response.data;
      });
    });
  }

  /**
   * Get a specific eval dataset by ID
   */
  async getEvalDataset(datasetId: string): Promise<EvalDataset> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/evals/datasets/${datasetId}`, {
          headers: this.getHeaders(),
        });
        return response.data;
      });
    });
  }

  /**
   * Create a new eval dataset
   */
  async createEvalDataset(request: CreateDatasetRequest): Promise<{ success: boolean; dataset: EvalDataset }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.post(
          `${this.baseURL}/api/v1/evals/datasets`,
          request,
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Update an eval dataset
   */
  async updateEvalDataset(
    datasetId: string,
    updates: UpdateDatasetRequest
  ): Promise<{ success: boolean; dataset: EvalDataset }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.put(
          `${this.baseURL}/api/v1/evals/datasets/${datasetId}`,
          updates,
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Delete an eval dataset
   */
  async deleteEvalDataset(datasetId: string): Promise<void> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        await axios.delete(`${this.baseURL}/api/v1/evals/datasets/${datasetId}`, {
          headers: this.getHeaders(),
        });
      });
    });
  }

  /**
   * Add an example to an eval dataset
   */
  async addEvalExample(
    datasetId: string,
    example: EvalExample
  ): Promise<{ success: boolean; dataset: EvalDataset }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.post(
          `${this.baseURL}/api/v1/evals/datasets/${datasetId}/examples`,
          example,
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  // Eval Runs API methods

  /**
   * Get all eval runs
   */
  async getEvalRuns(filters?: { datasetId?: string; model?: string }): Promise<{ runs: EvalRun[] }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/evals/runs`, {
          headers: this.getHeaders(),
          params: filters,
        });
        return response.data;
      });
    });
  }

  /**
   * Get a specific eval run by ID
   */
  async getEvalRun(runId: string): Promise<EvalRun> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/evals/runs/${runId}`, {
          headers: this.getHeaders(),
        });
        return response.data;
      });
    });
  }

  /**
   * Create a new eval run
   */
  async createEvalRun(request: CreateRunRequest): Promise<{ success: boolean; run: EvalRun }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.post(
          `${this.baseURL}/api/v1/evals/runs`,
          request,
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Update an eval run
   */
  async updateEvalRun(
    runId: string,
    updates: UpdateRunRequest
  ): Promise<{ success: boolean; run: EvalRun }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.put(
          `${this.baseURL}/api/v1/evals/runs/${runId}`,
          updates,
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Delete an eval run
   */
  async deleteEvalRun(runId: string): Promise<void> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        await axios.delete(`${this.baseURL}/api/v1/evals/runs/${runId}`, {
          headers: this.getHeaders(),
        });
      });
    });
  }

  /**
   * Record an eval result for a run
   */
  async recordEvalResult(
    runId: string,
    result: EvalResult
  ): Promise<{ success: boolean; run: EvalRun }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.post(
          `${this.baseURL}/api/v1/evals/runs/${runId}/results`,
          result,
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Get results for an eval run
   */
  async getEvalResults(runId: string): Promise<{ results: EvalResult[] }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/evals/runs/${runId}/results`, {
          headers: this.getHeaders(),
        });
        return response.data;
      });
    });
  }

  /**
   * Send chat completion request to LLM provider
   */
  async chatCompletion(
    provider: string,
    messages: Array<{ role: string; content: string }>,
    model?: string
  ): Promise<{
    content: string;
    provider: string;
    model: string;
    tokens_used?: number;
    duration_ms: number;
  }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.post(
          `${this.baseURL}/api/v1/chat/completions`,
          { provider, messages, model },
          { headers: this.getHeaders() }
        );
        return response.data;
      });
    });
  }

  /**
   * Stream chat completion from LLM provider
   */
  async *streamChatCompletion(
    provider: string,
    messages: Array<{ role: string; content: string }>,
    model?: string
  ): AsyncGenerator<string, void, unknown> {
    const response = await axios.post(
      `${this.baseURL}/api/v1/chat/stream`,
      { provider, messages, model },
      {
        headers: { ...this.getHeaders(), 'Accept': 'text/event-stream' },
        responseType: 'stream',
      }
    );

    const stream = response.data;
    for await (const chunk of stream) {
      const lines = chunk.toString().split('\n');
      for (const line of lines) {
        if (line.startsWith('data: ')) {
          yield line.slice(6);
        }
      }
    }
  }

  /**
   * List available LLM models and providers
   */
  async listLLMModels(): Promise<{
    providers: Array<{
      id: string;
      name: string;
      available: boolean;
      models: string[];
    }>;
  }> {
    return fetchWithRetry(async () => {
      return this.circuitBreaker.execute(async () => {
        const response = await axios.get(`${this.baseURL}/api/v1/chat/models`, {
          headers: this.getHeaders(),
        });
        return response.data;
      });
    });
  }
}

export const agentreplayClient = new ChronoLakeClient();
// Alias for backwards compatibility
export const AgentreplayClient = ChronoLakeClient;
