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
 * Flowtrace SDK Types
 *
 * Core type definitions for the Flowtrace observability platform.
 */

/**
 * Agent execution span types - represents the type of operation being traced.
 */
export enum SpanType {
  /** Root span - top-level agent execution */
  Root = 0,
  /** Planning phase */
  Planning = 1,
  /** Reasoning/thinking phase */
  Reasoning = 2,
  /** Tool/function call */
  ToolCall = 3,
  /** Tool response */
  ToolResponse = 4,
  /** Result synthesis */
  Synthesis = 5,
  /** Final response */
  Response = 6,
  /** Error state */
  Error = 7,
  /** Vector DB retrieval */
  Retrieval = 8,
  /** Text embedding */
  Embedding = 9,
  /** HTTP API call */
  HttpCall = 10,
  /** Database query */
  Database = 11,
  /** Generic function */
  Function = 12,
  /** Result reranking */
  Reranking = 13,
  /** Document parsing */
  Parsing = 14,
  /** Content generation */
  Generation = 15,
  /** Custom type (use values >= 16) */
  Custom = 255,
}

/**
 * Sensitivity flags for PII and redaction control.
 */
export enum SensitivityFlags {
  /** No special sensitivity */
  None = 0,
  /** Contains personally identifiable information */
  PII = 1 << 0,
  /** Contains secrets/credentials */
  Secret = 1 << 1,
  /** Internal-only data */
  Internal = 1 << 2,
  /** Never embed in vector index */
  NoEmbed = 1 << 3,
}

/**
 * Environment type for deployment context.
 */
export type Environment = 'development' | 'staging' | 'production';

/**
 * AgentFlowEdge - represents one step in agent execution.
 * This is the fundamental unit of data in Flowtrace.
 */
export interface AgentFlowEdge {
  /** Unique edge identifier */
  edgeId: string;
  /** Parent edge ID (empty string for root) */
  causalParent: string;
  /** Timestamp in microseconds since epoch */
  timestampUs: number;
  /** Lamport logical clock */
  logicalClock: number;
  /** Tenant identifier */
  tenantId: number;
  /** Project identifier within tenant */
  projectId: number;
  /** AFF schema version */
  schemaVersion: number;
  /** Sensitivity/privacy flags */
  sensitivityFlags: SensitivityFlags;
  /** Agent identifier */
  agentId: number;
  /** Session/conversation identifier */
  sessionId: number;
  /** Type of agent execution span */
  spanType: SpanType;
  /** Number of parents (>1 for DAG fan-in) */
  parentCount: number;
  /** Confidence score (0.0 - 1.0) */
  confidence: number;
  /** Number of tokens used */
  tokenCount: number;
  /** Duration in microseconds */
  durationUs: number;
  /** Sampling rate (0.0 - 1.0) */
  samplingRate: number;
  /** Compression type (0=None, 1=LZ4, 2=ZSTD) */
  compressionType: number;
  /** Whether payload data exists */
  hasPayload: boolean;
  /** General purpose flags */
  flags: number;
  /** BLAKE3 checksum for integrity */
  checksum: number;
  /** Optional metadata/payload */
  metadata?: Record<string, unknown>;
}

/**
 * Query filter for retrieving traces.
 */
export interface QueryFilter {
  /** Tenant identifier (required) */
  tenantId: number;
  /** Project identifier */
  projectId?: number;
  /** Agent identifier */
  agentId?: number;
  /** Session identifier */
  sessionId?: number;
  /** Span type filter */
  spanType?: SpanType;
  /** Minimum confidence threshold */
  minConfidence?: number;
  /** Exclude traces with PII */
  excludePii?: boolean;
  /** Exclude traces with secrets */
  excludeSecrets?: boolean;
  /** Environment filter */
  environment?: Environment;
  /** Maximum results */
  limit?: number;
  /** Offset for pagination */
  offset?: number;
}

/**
 * Response from query operations.
 */
export interface QueryResponse {
  /** List of matching traces */
  traces: TraceView[];
  /** Total count of matching traces */
  total: number;
  /** Requested limit */
  limit: number;
  /** Current offset */
  offset: number;
}

/**
 * TraceView - API response format for traces.
 */
export interface TraceView {
  /** Edge identifier (hex format) */
  edgeId: string;
  /** Tenant identifier */
  tenantId: number;
  /** Project identifier */
  projectId: number;
  /** Agent identifier */
  agentId: number;
  /** Agent name (resolved from registry) */
  agentName?: string;
  /** Session identifier */
  sessionId: number;
  /** Span type name */
  spanType: string;
  /** Timestamp in microseconds */
  timestampUs: number;
  /** Duration in microseconds */
  durationUs: number;
  /** Token count */
  tokenCount: number;
  /** Confidence score */
  confidence: number;
  /** Environment */
  environment: string;
  /** Whether payload exists */
  hasPayload: boolean;
  /** Payload metadata if loaded */
  metadata?: Record<string, unknown>;
}

/**
 * OpenTelemetry GenAI attributes for LLM tracking.
 */
export interface GenAIAttributes {
  /** Provider system (openai, anthropic, etc.) */
  'gen_ai.system'?: string;
  /** Requested model */
  'gen_ai.request.model'?: string;
  /** Response model */
  'gen_ai.response.model'?: string;
  /** Temperature setting */
  'gen_ai.request.temperature'?: number;
  /** Max tokens setting */
  'gen_ai.request.max_tokens'?: number;
  /** Top P setting */
  'gen_ai.request.top_p'?: number;
  /** Input/prompt tokens */
  'gen_ai.usage.input_tokens'?: number;
  /** Output/completion tokens */
  'gen_ai.usage.output_tokens'?: number;
  /** Total tokens */
  'gen_ai.usage.total_tokens'?: number;
  /** Prompt text */
  'gen_ai.prompt'?: string;
  /** Completion text */
  'gen_ai.completion'?: string;
  /** Finish reason */
  'gen_ai.response.finish_reason'?: string;
  /** Response ID */
  'gen_ai.response.id'?: string;
  /** Operation name (chat, completion, embedding) */
  'gen_ai.operation.name'?: string;
  /** Additional custom attributes */
  [key: string]: unknown;
}

/**
 * Span input for trace ingestion.
 */
export interface SpanInput {
  /** Span identifier */
  spanId: string;
  /** Trace/session identifier */
  traceId: string;
  /** Parent span identifier */
  parentSpanId?: string | null;
  /** Span name */
  name: string;
  /** Start time in microseconds */
  startTime: number;
  /** End time in microseconds */
  endTime?: number | null;
  /** Span attributes */
  attributes: Record<string, string>;
}

/**
 * Batch ingestion response.
 */
export interface IngestResponse {
  /** Number of spans accepted */
  accepted: number;
  /** Number of spans rejected */
  rejected: number;
  /** Error messages */
  errors: string[];
}

/**
 * Trace tree node for hierarchical view.
 */
export interface TraceTreeNode {
  /** Edge identifier */
  edgeId: string;
  /** Span type */
  spanType: string;
  /** Duration in microseconds */
  durationUs: number;
  /** Child nodes */
  children: TraceTreeNode[];
  /** Metadata if loaded */
  metadata?: Record<string, unknown>;
}

/**
 * Client configuration options.
 */
export interface FlowtraceClientOptions {
  /** Base URL of Flowtrace server */
  url: string;
  /** Tenant identifier */
  tenantId: number;
  /** Project identifier (default: 0) */
  projectId?: number;
  /** Default agent identifier (default: 1) */
  agentId?: number;
  /** Request timeout in milliseconds (default: 30000) */
  timeout?: number;
  /** Custom fetch implementation */
  fetch?: typeof fetch;
  /** Additional headers */
  headers?: Record<string, string>;
}

/**
 * Feedback value for user satisfaction signals.
 */
export type FeedbackValue = -1 | 0 | 1;

/**
 * Metrics time series data point.
 */
export interface MetricsDataPoint {
  /** Timestamp */
  timestamp: number;
  /** Value */
  value: number;
}

/**
 * Time series metrics response.
 */
export interface MetricsResponse {
  /** Metric name */
  metric: string;
  /** Data points */
  data: MetricsDataPoint[];
}
