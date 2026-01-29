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
 * Core types for Agentreplay plugins.
 *
 * These types match the WIT interface definition.
 */

/**
 * Unique identifier for traces and spans (128-bit UUID).
 */
export interface TraceId {
  high: bigint;
  low: bigint;
}

/**
 * Create TraceId from UUID string.
 */
export function traceIdFromUuid(uuid: string): TraceId {
  const clean = uuid.replace(/-/g, '');
  if (clean.length !== 32) {
    throw new Error('Invalid UUID string');
  }
  return {
    high: BigInt('0x' + clean.slice(0, 16)),
    low: BigInt('0x' + clean.slice(16)),
  };
}

/**
 * Convert TraceId to UUID string.
 */
export function traceIdToUuid(id: TraceId): string {
  const high = id.high.toString(16).padStart(16, '0');
  const low = id.low.toString(16).padStart(16, '0');
  return high + low;
}

/**
 * Span types in a trace.
 */
export type SpanType =
  | 'llm-call'
  | 'tool-call'
  | 'retrieval'
  | 'agent-step'
  | 'embedding'
  | 'custom';

/**
 * A single span/edge in a trace.
 */
export interface Span {
  id: TraceId;
  parentId?: TraceId;
  spanType: SpanType;
  name: string;
  input?: string;
  output?: string;
  model?: string;
  timestampUs: bigint;
  durationUs?: bigint;
  tokenCount?: number;
  costUsd?: number;
  metadata: Map<string, string>;
}

/**
 * Complete trace context for evaluation.
 */
export interface TraceContext {
  traceId: TraceId;
  spans: Span[];
  input?: string;
  output?: string;
  metadata: Map<string, string>;
}

/**
 * Helper functions for TraceContext.
 */
export const TraceContextUtils = {
  /**
   * Get the root span.
   */
  rootSpan(trace: TraceContext): Span | undefined {
    return trace.spans.find((s) => s.parentId === undefined);
  },

  /**
   * Get all LLM call spans.
   */
  llmSpans(trace: TraceContext): Span[] {
    return trace.spans.filter((s) => s.spanType === 'llm-call');
  },

  /**
   * Get all tool call spans.
   */
  toolSpans(trace: TraceContext): Span[] {
    return trace.spans.filter((s) => s.spanType === 'tool-call');
  },

  /**
   * Calculate total duration.
   */
  totalDurationUs(trace: TraceContext): bigint {
    return trace.spans.reduce(
      (sum, s) => sum + (s.durationUs ?? 0n),
      0n
    );
  },

  /**
   * Calculate total tokens.
   */
  totalTokens(trace: TraceContext): number {
    return trace.spans.reduce((sum, s) => sum + (s.tokenCount ?? 0), 0);
  },

  /**
   * Calculate total cost.
   */
  totalCost(trace: TraceContext): number {
    return trace.spans.reduce((sum, s) => sum + (s.costUsd ?? 0), 0);
  },
};

/**
 * Metric value types.
 */
export type MetricValue = number | bigint | boolean | string;

/**
 * Evaluation result.
 */
export interface EvalResult {
  evaluatorId: string;
  passed: boolean;
  confidence: number;
  explanation?: string;
  metrics?: Map<string, MetricValue>;
  costUsd?: number;
  durationMs?: number;
}

/**
 * Create a passing result.
 */
export function evalPass(
  evaluatorId: string,
  confidence: number = 1.0,
  explanation?: string
): EvalResult {
  return {
    evaluatorId,
    passed: true,
    confidence,
    explanation,
  };
}

/**
 * Create a failing result.
 */
export function evalFail(
  evaluatorId: string,
  confidence: number = 1.0,
  explanation?: string
): EvalResult {
  return {
    evaluatorId,
    passed: false,
    confidence,
    explanation,
  };
}

/**
 * Plugin metadata.
 */
export interface PluginMetadata {
  id: string;
  name: string;
  version: string;
  description: string;
  author?: string;
  tags?: string[];
  costPerEval?: number;
}

/**
 * Embedding vector type.
 */
export type Embedding = Float32Array;

/**
 * Log levels.
 */
export type LogLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error';

/**
 * HTTP response from host.
 */
export interface HttpResponse {
  status: number;
  headers: Map<string, string>;
  body: Uint8Array;
}

/**
 * Helper functions for HttpResponse.
 */
export const HttpResponseUtils = {
  /**
   * Get body as string.
   */
  text(response: HttpResponse): string {
    return new TextDecoder().decode(response.body);
  },

  /**
   * Parse body as JSON.
   */
  json<T = unknown>(response: HttpResponse): T {
    return JSON.parse(HttpResponseUtils.text(response));
  },

  /**
   * Check if response is successful (2xx).
   */
  isSuccess(response: HttpResponse): boolean {
    return response.status >= 200 && response.status < 300;
  },
};
