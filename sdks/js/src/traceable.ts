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
 * Traceable wrapper for automatic span creation.
 *
 * The core ergonomic API for tracing any function.
 */

import type { Span, SpanKind, SpanInput } from './types';
import { SpanType } from './types';
import { getConfig, getConfigOrNull } from './config';
import { getCurrentContext, withContext, getMergedContext } from './context';
import { enqueueSpan } from './transport';
import { shouldSample } from './sampling';
import { redactPayload } from './privacy';

/**
 * Options for traceable wrapper
 */
export interface TraceableOptions {
  /** Span name */
  name?: string;
  /** Span kind (llm, tool, retriever, etc.) */
  kind?: SpanKind;
  /** Input data to record */
  input?: Record<string, unknown>;
  /** Custom metadata */
  metadata?: Record<string, unknown>;
  /** Custom tags */
  tags?: Record<string, string>;
  /** Override session ID */
  sessionId?: number;
  /** Override agent ID */
  agentId?: number;
}

/**
 * Options for withSpan
 */
export interface WithSpanOptions extends TraceableOptions {
  /** Record input automatically from function args */
  recordInput?: boolean;
  /** Record output automatically */
  recordOutput?: boolean;
}

/**
 * Active span interface with methods to add data
 */
export interface ActiveSpan extends Span {
  /** Set output data */
  setOutput(output: Record<string, unknown>): void;
  /** Set error */
  setError(error: Error | unknown): void;
  /** Add an event */
  addEvent(name: string, data?: Record<string, unknown>): void;
  /** Set a tag */
  setTag(key: string, value: string): void;
  /** Set token usage */
  setTokenUsage(usage: { prompt?: number; completion?: number; total?: number }): void;
  /** End the span */
  end(data?: { output?: Record<string, unknown>; error?: Error | unknown }): void;
}

/**
 * Generate a unique span ID
 */
function generateSpanId(): string {
  const timestamp = Date.now();
  const randomBits = Math.floor(Math.random() * 0xffff);
  const spanId = BigInt(timestamp) << 16n | BigInt(randomBits);
  return spanId.toString(16);
}

/**
 * Get current timestamp in microseconds
 */
function nowMicroseconds(): number {
  return Math.floor(Date.now() * 1000);
}

/**
 * Convert SpanKind to SpanType
 */
function kindToSpanType(kind?: SpanKind): SpanType {
  switch (kind) {
    case 'llm':
      return SpanType.Generation;
    case 'tool':
      return SpanType.ToolCall;
    case 'retriever':
      return SpanType.Retrieval;
    case 'embedding':
      return SpanType.Embedding;
    case 'chain':
      return SpanType.Function;
    case 'guardrail':
      return SpanType.Function;
    case 'cache':
      return SpanType.Function;
    case 'request':
      return SpanType.Root;
    default:
      return SpanType.Function;
  }
}

/**
 * Create a span object
 */
function createSpanObject(
  spanId: string,
  name: string,
  options: WithSpanOptions,
  parentSpanId: string | null,
  traceId: string,
  startTime: number
): ActiveSpan {
  const config = getConfigOrNull();
  const attributes: Record<string, string> = {
    tenant_id: String(config?.tenantId ?? 1),
    project_id: String(config?.projectId ?? 0),
    agent_id: String(options.agentId ?? config?.agentId ?? 1),
    session_id: String(options.sessionId ?? traceId),
    span_type: String(kindToSpanType(options.kind)),
  };

  // Add input
  if (options.input) {
    const redacted = redactPayload(options.input);
    attributes['input'] = JSON.stringify(redacted);
  }

  // Add metadata
  if (options.metadata) {
    for (const [key, value] of Object.entries(options.metadata)) {
      attributes[`metadata.${key}`] = typeof value === 'object' ? JSON.stringify(value) : String(value);
    }
  }

  // Add tags
  if (options.tags) {
    for (const [key, value] of Object.entries(options.tags)) {
      attributes[`tag.${key}`] = value;
    }
  }

  let output: Record<string, unknown> | undefined;
  let error: Error | unknown | undefined;
  const events: Array<{ name: string; time: number; data?: Record<string, unknown> }> = [];
  let ended = false;

  const span: ActiveSpan = {
    spanId,
    traceId,
    parentSpanId,
    name,
    kind: options.kind ?? 'chain',
    startTime,
    attributes,

    setOutput(data: Record<string, unknown>) {
      output = data;
      const redacted = redactPayload(data);
      attributes['output'] = JSON.stringify(redacted);
    },

    setError(err: Error | unknown) {
      error = err;
      if (err instanceof Error) {
        attributes['error.type'] = err.name;
        attributes['error.message'] = err.message;
        if (err.stack) {
          attributes['error.stack'] = err.stack;
        }
      } else {
        attributes['error.message'] = String(err);
      }
    },

    addEvent(eventName: string, data?: Record<string, unknown>) {
      events.push({ name: eventName, time: nowMicroseconds(), data });
    },

    setTag(key: string, value: string) {
      attributes[`tag.${key}`] = value;
    },

    setTokenUsage(usage: { prompt?: number; completion?: number; total?: number }) {
      if (usage.prompt !== undefined) {
        attributes['gen_ai.usage.prompt_tokens'] = String(usage.prompt);
      }
      if (usage.completion !== undefined) {
        attributes['gen_ai.usage.completion_tokens'] = String(usage.completion);
      }
      if (usage.total !== undefined) {
        attributes['gen_ai.usage.total_tokens'] = String(usage.total);
        attributes['token_count'] = String(usage.total);
      }
    },

    end(data?: { output?: Record<string, unknown>; error?: Error | unknown }) {
      if (ended) return;
      ended = true;

      if (data?.output) {
        span.setOutput(data.output);
      }
      if (data?.error) {
        span.setError(data.error);
      }

      const endTime = nowMicroseconds();
      attributes['duration_us'] = String(endTime - startTime);

      // Add events
      if (events.length > 0) {
        attributes['events'] = JSON.stringify(events);
      }

      // Create span input for transport
      const spanInput: SpanInput = {
        spanId,
        traceId,
        parentSpanId,
        name,
        startTime,
        endTime,
        attributes,
      };

      // Enqueue for sending
      enqueueSpan(spanInput);
    },
  };

  return span;
}

/**
 * Wrap a function with automatic tracing.
 *
 * @example
 * ```typescript
 * const result = await traceable(async () => {
 *   const response = await openai.chat.completions.create({...});
 *   return response.choices[0].message.content;
 * }, { name: 'chat_request', kind: 'llm' })();
 * ```
 */
export function traceable<TArgs extends any[], TResult>(
  fn: (...args: TArgs) => Promise<TResult> | TResult,
  options: TraceableOptions = {}
): (...args: TArgs) => Promise<TResult> {
  const name = options.name ?? fn.name ?? 'anonymous';

  return async (...args: TArgs): Promise<TResult> => {
    // Check sampling
    if (!shouldSample()) {
      return fn(...args);
    }

    // Get context
    const ctx = getMergedContext();
    const parentSpan = ctx.span;
    const traceId = ctx.traceId;

    // Create span
    const spanId = generateSpanId();
    const startTime = nowMicroseconds();

    const span = createSpanObject(
      spanId,
      name,
      {
        ...options,
        input: options.input ?? (args.length > 0 ? { args } : undefined),
        sessionId: options.sessionId ?? ctx.sessionId,
        tags: { ...ctx.tags, ...options.tags },
        metadata: { ...ctx.metadata, ...options.metadata },
      },
      parentSpan?.spanId ?? null,
      traceId,
      startTime
    );

    // Run function with span as context
    try {
      const result = await withContext(
        { ...ctx, span },
        () => fn(...args)
      );

      // Auto-record output if it's an object
      if (result !== null && typeof result === 'object') {
        span.setOutput(result as Record<string, unknown>);
      }

      span.end();
      return result;
    } catch (err) {
      span.setError(err);
      span.end();
      throw err;
    }
  };
}

/**
 * Execute a function within a span scope.
 *
 * @example
 * ```typescript
 * const context = await withSpan('retrieve_context', { kind: 'retriever' }, async (span) => {
 *   const docs = await vectorDb.search(query);
 *   span.setOutput({ count: docs.length });
 *   return docs;
 * });
 * ```
 */
export async function withSpan<T>(
  name: string,
  options: WithSpanOptions,
  fn: (span: ActiveSpan) => Promise<T> | T
): Promise<T> {
  // Check sampling
  if (!shouldSample()) {
    const noopSpan: ActiveSpan = {
      spanId: '',
      traceId: '',
      parentSpanId: null,
      name,
      kind: options.kind ?? 'chain',
      startTime: 0,
      attributes: {},
      setOutput: () => {},
      setError: () => {},
      addEvent: () => {},
      setTag: () => {},
      setTokenUsage: () => {},
      end: () => {},
    };
    return fn(noopSpan);
  }

  // Get context
  const ctx = getMergedContext();
  const parentSpan = ctx.span;
  const traceId = ctx.traceId;

  // Create span
  const spanId = generateSpanId();
  const startTime = nowMicroseconds();

  const span = createSpanObject(
    spanId,
    name,
    {
      ...options,
      sessionId: options.sessionId ?? ctx.sessionId,
      tags: { ...ctx.tags, ...options.tags },
      metadata: { ...ctx.metadata, ...options.metadata },
    },
    parentSpan?.spanId ?? null,
    traceId,
    startTime
  );

  // Run function with span as context
  try {
    const result = await withContext(
      { ...ctx, span },
      () => fn(span)
    );
    span.end();
    return result;
  } catch (err) {
    span.setError(err);
    span.end();
    throw err;
  }
}

/**
 * Start a manual span.
 * Must call span.end() when done.
 *
 * @example
 * ```typescript
 * const span = startSpan('db_lookup', { kind: 'tool' });
 * try {
 *   const rows = await query();
 *   span.end({ output: { count: rows.length } });
 * } catch (err) {
 *   span.end({ error: err });
 *   throw err;
 * }
 * ```
 */
export function startSpan(name: string, options: TraceableOptions = {}): ActiveSpan {
  // Get context
  const ctx = getMergedContext();
  const parentSpan = ctx.span;
  const traceId = ctx.traceId;

  // Create span
  const spanId = generateSpanId();
  const startTime = nowMicroseconds();

  return createSpanObject(
    spanId,
    name,
    {
      ...options,
      sessionId: options.sessionId ?? ctx.sessionId,
      tags: { ...ctx.tags, ...options.tags },
      metadata: { ...ctx.metadata, ...options.metadata },
    },
    parentSpan?.spanId ?? null,
    traceId,
    startTime
  );
}

/**
 * Capture an exception with context.
 *
 * @example
 * ```typescript
 * try {
 *   await riskyOperation();
 * } catch (err) {
 *   captureException(err, { operation: 'riskyOperation' });
 *   throw err;
 * }
 * ```
 */
export function captureException(
  error: Error | unknown,
  context?: Record<string, unknown>
): void {
  const span = startSpan('exception', {
    kind: 'chain',
    metadata: context,
  });
  span.setError(error);
  span.end();
}
