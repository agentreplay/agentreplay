/**
 * Copyright 2025 Sushanth (https://github.com/sushanthpy)
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/**
 * Agentreplay Client
 *
 * High-performance client for the Agentreplay observability platform.
 */

import {
  SpanType,
  SensitivityFlags,
  AgentreplayClientOptions,
  QueryFilter,
  QueryResponse,
  TraceView,
  SpanInput,
  IngestResponse,
  TraceTreeNode,
  GenAIAttributes,
  FeedbackValue,
  AgentFlowEdge,
} from './types';

/**
 * Generate a unique edge ID based on timestamp and random bits.
 */
function generateEdgeId(): string {
  const timestamp = Date.now();
  const randomBits = Math.floor(Math.random() * 0xffff);
  const edgeId = BigInt(timestamp) << 16n | BigInt(randomBits);
  return edgeId.toString(16);
}

/**
 * Get current timestamp in microseconds.
 */
function nowMicroseconds(): number {
  return Math.floor(Date.now() * 1000);
}

/**
 * AgentreplayClient - Main client for interacting with Agentreplay.
 *
 * @example
 * ```typescript
 * const client = new AgentreplayClient({
 *   url: 'http://localhost:8080',
 *   tenantId: 1,
 *   projectId: 0
 * });
 *
 * // Create a trace
 * const trace = await client.createTrace({
 *   agentId: 1,
 *   sessionId: 123,
 *   spanType: SpanType.Root,
 *   metadata: { name: 'my-agent' }
 * });
 *
 * // Track LLM call with GenAI attributes
 * await client.createGenAITrace({
 *   agentId: 1,
 *   sessionId: 123,
 *   model: 'gpt-4o',
 *   inputMessages: [{ role: 'user', content: 'Hello!' }],
 *   output: { role: 'assistant', content: 'Hi there!' },
 *   inputUsage: 10,
 *   outputUsage: 8
 * });
 * ```
 */
export class AgentreplayClient {
  private readonly url: string;
  private readonly tenantId: number;
  private readonly projectId: number;
  private readonly agentId: number;
  private readonly timeout: number;
  private readonly headers: Record<string, string>;
  private readonly fetchFn: typeof fetch;
  private sessionCounter = 0;

  constructor(options: AgentreplayClientOptions) {
    this.url = options.url.replace(/\/$/, '');
    this.tenantId = options.tenantId;
    this.projectId = options.projectId ?? 0;
    this.agentId = options.agentId ?? 1;
    this.timeout = options.timeout ?? 30000;
    this.fetchFn = options.fetch ?? fetch;
    this.headers = {
      'Content-Type': 'application/json',
      'X-Tenant-ID': String(this.tenantId),
      ...(options.headers ?? {}),
    };
  }

  /**
   * Make an HTTP request to the Agentreplay server.
   */
  private async request<T>(
    method: string,
    path: string,
    body?: unknown,
    params?: Record<string, string | number | boolean | undefined>
  ): Promise<T> {
    let url = `${this.url}${path}`;

    if (params) {
      const searchParams = new URLSearchParams();
      for (const [key, value] of Object.entries(params)) {
        if (value !== undefined) {
          searchParams.append(key, String(value));
        }
      }
      const queryString = searchParams.toString();
      if (queryString) {
        url += `?${queryString}`;
      }
    }

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeout);

    try {
      const response = await this.fetchFn(url, {
        method,
        headers: this.headers,
        body: body ? JSON.stringify(body) : undefined,
        signal: controller.signal,
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`Agentreplay API error (${response.status}): ${errorText}`);
      }

      return await response.json() as T;
    } finally {
      clearTimeout(timeoutId);
    }
  }

  /**
   * Generate next session ID.
   */
  private nextSessionId(): number {
    return ++this.sessionCounter;
  }

  // ==================== Trace Creation ====================

  /**
   * Create a new trace span.
   *
   * @param options - Trace creation options
   * @returns Created trace information
   */
  async createTrace(options: {
    agentId: number;
    sessionId?: number;
    spanType: SpanType;
    parentId?: string;
    metadata?: Record<string, unknown>;
  }): Promise<{ edgeId: string; tenantId: number; agentId: number; sessionId: number; spanType: string }> {
    const edgeId = generateEdgeId();
    const sessionId = options.sessionId ?? this.nextSessionId();
    const startTimeUs = nowMicroseconds();

    const attributes: Record<string, string> = {
      tenant_id: String(this.tenantId),
      project_id: String(this.projectId),
      agent_id: String(options.agentId),
      session_id: String(sessionId),
      span_type: String(options.spanType),
      token_count: '0',
      duration_us: '0',
    };

    // Add metadata as attributes
    if (options.metadata) {
      for (const [key, value] of Object.entries(options.metadata)) {
        if (key === 'name') continue;
        attributes[key] = typeof value === 'object' ? JSON.stringify(value) : String(value);
      }
    }

    const span: SpanInput = {
      spanId: edgeId,
      traceId: String(sessionId),
      parentSpanId: options.parentId ?? null,
      name: options.metadata?.name as string ?? `span_${options.agentId}`,
      startTime: startTimeUs,
      endTime: startTimeUs,
      attributes,
    };

    await this.request<IngestResponse>('POST', '/api/v1/traces', { spans: [span] });

    return {
      edgeId,
      tenantId: this.tenantId,
      agentId: options.agentId,
      sessionId,
      spanType: SpanType[options.spanType] ?? String(options.spanType),
    };
  }

  /**
   * Create a GenAI trace with OpenTelemetry semantic conventions.
   *
   * @param options - GenAI trace options
   * @returns Created trace information
   */
  async createGenAITrace(options: {
    agentId: number;
    sessionId?: number;
    inputMessages?: Array<{ role: string; content: string }>;
    output?: { role: string; content: string } | string;
    model?: string;
    modelParameters?: Record<string, unknown>;
    inputUsage?: number;
    outputUsage?: number;
    totalUsage?: number;
    parentId?: string;
    metadata?: Record<string, unknown>;
    operationName?: string;
    finishReason?: string;
    system?: string;
  }): Promise<{ edgeId: string; tenantId: number; agentId: number; sessionId: number; model?: string }> {
    const edgeId = generateEdgeId();
    const sessionId = options.sessionId ?? this.nextSessionId();
    const startTimeUs = nowMicroseconds();
    const operationName = options.operationName ?? 'chat';

    const attributes: Record<string, string> = {
      tenant_id: String(this.tenantId),
      project_id: String(this.projectId),
      agent_id: String(options.agentId),
      session_id: String(sessionId),
      span_type: '0',
      'gen_ai.operation.name': operationName,
    };

    // Auto-detect system from model name
    let system = options.system;
    if (!system && options.model) {
      const modelLower = options.model.toLowerCase();
      if (modelLower.includes('gpt') || modelLower.includes('openai')) {
        system = 'openai';
      } else if (modelLower.includes('claude') || modelLower.includes('anthropic')) {
        system = 'anthropic';
      } else if (modelLower.includes('llama') || modelLower.includes('meta')) {
        system = 'meta';
      } else if (modelLower.includes('gemini') || modelLower.includes('palm')) {
        system = 'google';
      } else {
        system = 'unknown';
      }
    }

    if (system) {
      attributes['gen_ai.system'] = system;
    }

    if (options.model) {
      attributes['gen_ai.request.model'] = options.model;
      attributes['gen_ai.response.model'] = options.model;
    }

    // Model parameters
    if (options.modelParameters) {
      for (const [key, value] of Object.entries(options.modelParameters)) {
        attributes[`gen_ai.request.${key}`] = String(value);
      }
    }

    // Token usage
    if (options.inputUsage !== undefined) {
      attributes['gen_ai.usage.prompt_tokens'] = String(options.inputUsage);
      attributes['gen_ai.usage.input_tokens'] = String(options.inputUsage);
    }
    if (options.outputUsage !== undefined) {
      attributes['gen_ai.usage.completion_tokens'] = String(options.outputUsage);
      attributes['gen_ai.usage.output_tokens'] = String(options.outputUsage);
    }
    if (options.totalUsage !== undefined) {
      attributes['gen_ai.usage.total_tokens'] = String(options.totalUsage);
      attributes['token_count'] = String(options.totalUsage);
    }

    if (options.finishReason) {
      attributes['gen_ai.response.finish_reasons'] = JSON.stringify([options.finishReason]);
    }

    // Input messages
    if (options.inputMessages) {
      attributes['gen_ai.prompt.messages'] = JSON.stringify(options.inputMessages);
    }

    // Output
    if (options.output) {
      attributes['gen_ai.completion.message'] = JSON.stringify(options.output);
    }

    // Additional metadata
    if (options.metadata) {
      for (const [key, value] of Object.entries(options.metadata)) {
        if (!(key in attributes)) {
          attributes[`metadata.${key}`] = typeof value === 'object' ? JSON.stringify(value) : String(value);
        }
      }
    }

    const span: SpanInput = {
      spanId: edgeId,
      traceId: String(sessionId),
      parentSpanId: options.parentId ?? null,
      name: `${operationName}-${options.model ?? 'unknown'}`,
      startTime: startTimeUs,
      endTime: startTimeUs,
      attributes,
    };

    await this.request<IngestResponse>('POST', '/api/v1/traces', { spans: [span] });

    return {
      edgeId,
      tenantId: this.tenantId,
      agentId: options.agentId,
      sessionId,
      model: options.model,
    };
  }

  /**
   * Create a tool call trace.
   *
   * @param options - Tool trace options
   * @returns Created trace information
   */
  async createToolTrace(options: {
    agentId: number;
    sessionId?: number;
    toolName: string;
    toolInput?: Record<string, unknown>;
    toolOutput?: Record<string, unknown>;
    toolDescription?: string;
    parentId?: string;
    metadata?: Record<string, unknown>;
  }): Promise<{ edgeId: string; tenantId: number; agentId: number; sessionId: number; toolName: string }> {
    const edgeId = generateEdgeId();
    const sessionId = options.sessionId ?? this.nextSessionId();
    const startTimeUs = nowMicroseconds();

    const attributes: Record<string, string> = {
      tenant_id: String(this.tenantId),
      project_id: String(this.projectId),
      agent_id: String(options.agentId),
      session_id: String(sessionId),
      span_type: '3', // TOOL_CALL
      'gen_ai.tool.name': options.toolName,
    };

    if (options.toolDescription) {
      attributes['gen_ai.tool.description'] = options.toolDescription;
    }
    if (options.toolInput) {
      attributes['gen_ai.tool.call.input'] = JSON.stringify(options.toolInput);
    }
    if (options.toolOutput) {
      attributes['gen_ai.tool.call.output'] = JSON.stringify(options.toolOutput);
    }

    // Additional metadata
    if (options.metadata) {
      for (const [key, value] of Object.entries(options.metadata)) {
        if (!(key in attributes)) {
          attributes[`metadata.${key}`] = typeof value === 'object' ? JSON.stringify(value) : String(value);
        }
      }
    }

    const span: SpanInput = {
      spanId: edgeId,
      traceId: String(sessionId),
      parentSpanId: options.parentId ?? null,
      name: `tool-${options.toolName}`,
      startTime: startTimeUs,
      endTime: startTimeUs,
      attributes,
    };

    await this.request<IngestResponse>('POST', '/api/v1/traces', { spans: [span] });

    return {
      edgeId,
      tenantId: this.tenantId,
      agentId: options.agentId,
      sessionId,
      toolName: options.toolName,
    };
  }

  /**
   * Update a trace with completion information.
   *
   * @param options - Update options
   */
  async updateTrace(options: {
    edgeId: string;
    sessionId: number;
    tokenCount?: number;
    durationUs?: number;
    durationMs?: number;
    payload?: Record<string, unknown>;
  }): Promise<void> {
    const endTimeUs = nowMicroseconds();
    let durationUs = options.durationUs;
    if (!durationUs && options.durationMs) {
      durationUs = options.durationMs * 1000;
    }
    durationUs = durationUs ?? 1000;

    const startTimeUs = endTimeUs - durationUs;

    const attributes: Record<string, string> = {
      tenant_id: String(this.tenantId),
      project_id: String(this.projectId),
      agent_id: String(this.agentId),
      session_id: String(options.sessionId),
      span_type: '6', // RESPONSE
      token_count: String(options.tokenCount ?? 0),
      duration_us: String(durationUs),
    };

    if (options.payload) {
      for (const [key, value] of Object.entries(options.payload)) {
        attributes[`payload.${key}`] = typeof value === 'object' ? JSON.stringify(value) : String(value);
      }
    }

    const span: SpanInput = {
      spanId: `${options.edgeId}_complete`,
      traceId: String(options.sessionId),
      parentSpanId: options.edgeId,
      name: 'RESPONSE',
      startTime: startTimeUs,
      endTime: endTimeUs,
      attributes,
    };

    await this.request<IngestResponse>('POST', '/api/v1/traces', { spans: [span] });
  }

  /**
   * Ingest multiple spans in a batch.
   *
   * @param spans - Array of spans to ingest
   * @returns Ingestion response
   */
  async ingestBatch(spans: SpanInput[]): Promise<IngestResponse> {
    return this.request<IngestResponse>('POST', '/api/v1/traces', { spans });
  }

  // ==================== Query Operations ====================

  /**
   * Query traces with optional filters.
   *
   * @param filter - Query filters
   * @returns Query response with matching traces
   */
  async queryTraces(filter?: Partial<QueryFilter>): Promise<QueryResponse> {
    const params: Record<string, string | number | boolean | undefined> = {};

    if (filter) {
      if (filter.projectId !== undefined) params.project_id = filter.projectId;
      if (filter.agentId !== undefined) params.agent_id = filter.agentId;
      if (filter.sessionId !== undefined) params.session_id = filter.sessionId;
      if (filter.environment) params.environment = filter.environment;
      if (filter.excludePii !== undefined) params.exclude_pii = filter.excludePii;
      if (filter.excludeSecrets !== undefined) params.exclude_secrets = filter.excludeSecrets;
      if (filter.limit !== undefined) params.limit = filter.limit;
      if (filter.offset !== undefined) params.offset = filter.offset;
    }

    return this.request<QueryResponse>('GET', '/api/v1/traces', undefined, params);
  }

  /**
   * Query traces within a temporal range.
   *
   * @param startTimestampUs - Start timestamp in microseconds
   * @param endTimestampUs - End timestamp in microseconds
   * @param filter - Optional additional filters
   * @returns Query response
   */
  async queryTemporalRange(
    startTimestampUs: number,
    endTimestampUs: number,
    filter?: Partial<QueryFilter>
  ): Promise<QueryResponse> {
    const params: Record<string, string | number | boolean | undefined> = {
      start_ts: startTimestampUs,
      end_ts: endTimestampUs,
    };

    if (filter) {
      if (filter.sessionId !== undefined) params.session_id = filter.sessionId;
      if (filter.agentId !== undefined) params.agent_id = filter.agentId;
      if (filter.environment) params.environment = filter.environment;
      if (filter.excludePii !== undefined) params.exclude_pii = filter.excludePii;
      if (filter.limit !== undefined) params.limit = filter.limit;
      if (filter.offset !== undefined) params.offset = filter.offset;
    }

    return this.request<QueryResponse>('GET', '/api/v1/traces', undefined, params);
  }

  /**
   * Get a specific trace by ID.
   *
   * @param traceId - Trace/edge identifier
   * @returns Trace view with payload
   */
  async getTrace(traceId: string): Promise<TraceView> {
    return this.request<TraceView>('GET', `/api/v1/traces/${traceId}`);
  }

  /**
   * Get child spans of a trace.
   *
   * @param traceId - Parent trace identifier
   * @returns Child traces
   */
  async getChildren(traceId: string): Promise<{ parentId: string; children: TraceView[]; total: number }> {
    return this.request('GET', `/api/v1/traces/${traceId}/children`);
  }

  /**
   * Get hierarchical trace tree.
   *
   * @param traceId - Root trace identifier
   * @returns Tree structure
   */
  async getTraceTree(traceId: string): Promise<{ root: TraceTreeNode }> {
    return this.request('GET', `/api/v1/traces/${traceId}/tree`);
  }

  /**
   * Get all traces in a session.
   *
   * @param sessionId - Session identifier
   * @returns Traces in the session
   */
  async filterBySession(sessionId: number): Promise<TraceView[]> {
    const response = await this.queryTraces({ sessionId });
    return response.traces;
  }

  // ==================== Feedback & Datasets ====================

  /**
   * Submit user feedback for a trace.
   *
   * @param traceId - Trace identifier
   * @param feedback - Feedback value (-1, 0, or 1)
   */
  async submitFeedback(traceId: string, feedback: FeedbackValue): Promise<{ success: boolean; message: string }> {
    return this.request('POST', `/api/v1/traces/${traceId}/feedback`, { feedback });
  }

  /**
   * Add a trace to an evaluation dataset.
   *
   * @param traceId - Trace identifier
   * @param datasetName - Dataset name
   * @param options - Optional input/output data
   */
  async addToDataset(
    traceId: string,
    datasetName: string,
    options?: { inputData?: Record<string, unknown>; outputData?: Record<string, unknown> }
  ): Promise<{ success: boolean; datasetName: string }> {
    const payload: Record<string, unknown> = { trace_id: traceId };
    if (options?.inputData) payload.input = options.inputData;
    if (options?.outputData) payload.output = options.outputData;

    return this.request('POST', `/api/v1/datasets/${datasetName}/add`, payload);
  }

  // ==================== Health & Metrics ====================

  /**
   * Check server health.
   *
   * @returns Health status
   */
  async health(): Promise<{ status: string; version?: string }> {
    return this.request('GET', '/api/v1/health');
  }

  /**
   * Get time-series metrics.
   *
   * @param metric - Metric name
   * @param startTs - Start timestamp
   * @param endTs - End timestamp
   */
  async getMetrics(
    metric: string,
    startTs: number,
    endTs: number
  ): Promise<{ metric: string; data: Array<{ timestamp: number; value: number }> }> {
    return this.request('GET', '/api/v1/metrics/timeseries', undefined, {
      metric,
      start_ts: startTs,
      end_ts: endTs,
    });
  }
}

// Re-export types
export { SpanType, SensitivityFlags };
export type {
  AgentreplayClientOptions,
  QueryFilter,
  QueryResponse,
  TraceView,
  SpanInput,
  IngestResponse,
  TraceTreeNode,
  GenAIAttributes,
  FeedbackValue,
  AgentFlowEdge,
};
