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
 * Context propagation for automatic span nesting.
 *
 * Uses AsyncLocalStorage in Node.js for correct span hierarchy
 * under concurrent requests.
 */

import type { Span } from './types';

/**
 * Context data stored in AsyncLocalStorage
 */
export interface SpanContext {
  /** Current active span */
  span: Span | null;
  /** Trace ID (session ID) */
  traceId: string;
  /** User ID for the current context */
  userId?: string;
  /** Session ID */
  sessionId?: number;
  /** Custom tags */
  tags?: Record<string, string>;
  /** Custom metadata */
  metadata?: Record<string, unknown>;
}

/**
 * Context manager interface
 */
interface ContextManager {
  getContext(): SpanContext | undefined;
  setContext(ctx: SpanContext): void;
  runWithContext<T>(ctx: SpanContext, fn: () => T): T;
}

/**
 * AsyncLocalStorage-based context manager for Node.js
 */
class AsyncLocalStorageContextManager implements ContextManager {
  private storage: any; // AsyncLocalStorage<SpanContext>

  constructor() {
    // Dynamic import to avoid issues in non-Node environments
    try {
      const { AsyncLocalStorage } = require('async_hooks');
      this.storage = new AsyncLocalStorage();
    } catch {
      // Fallback for environments without async_hooks
      this.storage = null;
    }
  }

  getContext(): SpanContext | undefined {
    if (!this.storage) return undefined;
    return this.storage.getStore();
  }

  setContext(ctx: SpanContext): void {
    // AsyncLocalStorage doesn't support setContext directly
    // Use runWithContext instead
  }

  runWithContext<T>(ctx: SpanContext, fn: () => T): T {
    if (!this.storage) {
      return fn();
    }
    return this.storage.run(ctx, fn);
  }
}

/**
 * Simple stack-based context manager for browsers/edge
 */
class StackContextManager implements ContextManager {
  private stack: SpanContext[] = [];

  getContext(): SpanContext | undefined {
    return this.stack[this.stack.length - 1];
  }

  setContext(ctx: SpanContext): void {
    if (this.stack.length > 0) {
      this.stack[this.stack.length - 1] = ctx;
    } else {
      this.stack.push(ctx);
    }
  }

  runWithContext<T>(ctx: SpanContext, fn: () => T): T {
    this.stack.push(ctx);
    try {
      return fn();
    } finally {
      this.stack.pop();
    }
  }
}

/**
 * Global context manager instance
 */
let contextManager: ContextManager;

// Initialize based on environment
if (typeof process !== 'undefined' && process.versions?.node) {
  contextManager = new AsyncLocalStorageContextManager();
} else {
  contextManager = new StackContextManager();
}

/**
 * Get the current span context.
 */
export function getCurrentContext(): SpanContext | undefined {
  return contextManager.getContext();
}

/**
 * Get the current active span.
 */
export function getCurrentSpan(): Span | null {
  const ctx = getCurrentContext();
  return ctx?.span ?? null;
}

/**
 * Get the current trace ID.
 */
export function getCurrentTraceId(): string | undefined {
  return getCurrentContext()?.traceId;
}

/**
 * Run a function with the given context.
 * Spans created inside will automatically be children of the context's span.
 *
 * @example
 * ```typescript
 * await withContext({ traceId: '123', span: rootSpan }, async () => {
 *   // Spans created here will be children of rootSpan
 *   await traceable(async () => { ... })();
 * });
 * ```
 */
export function withContext<T>(ctx: SpanContext, fn: () => T): T {
  return contextManager.runWithContext(ctx, fn);
}

/**
 * Run a function with a span as the current context.
 * Convenience wrapper around withContext.
 */
export function withSpanContext<T>(span: Span, traceId: string, fn: () => T): T {
  return withContext({ span, traceId }, fn);
}

/**
 * Set global context values (userId, sessionId, tags).
 * These are attached to all spans created after this call.
 *
 * @example
 * ```typescript
 * setGlobalContext({
 *   userId: 'user_123',
 *   sessionId: 456,
 *   tags: { tier: 'premium' }
 * });
 * ```
 */
let globalContextValues: Partial<SpanContext> = {};

export function setGlobalContext(values: {
  userId?: string;
  sessionId?: number;
  tags?: Record<string, string>;
  metadata?: Record<string, unknown>;
}): void {
  globalContextValues = {
    ...globalContextValues,
    ...values,
    tags: { ...globalContextValues.tags, ...values.tags },
    metadata: { ...globalContextValues.metadata, ...values.metadata },
  };
}

/**
 * Get global context values.
 */
export function getGlobalContext(): Partial<SpanContext> {
  return globalContextValues;
}

/**
 * Clear global context values.
 */
export function clearGlobalContext(): void {
  globalContextValues = {};
}

/**
 * Create a merged context with current + global values.
 */
export function getMergedContext(overrides?: Partial<SpanContext>): SpanContext {
  const current = getCurrentContext();
  return {
    span: overrides?.span ?? current?.span ?? null,
    traceId: overrides?.traceId ?? current?.traceId ?? generateTraceId(),
    userId: overrides?.userId ?? current?.userId ?? globalContextValues.userId,
    sessionId: overrides?.sessionId ?? current?.sessionId ?? globalContextValues.sessionId,
    tags: { ...globalContextValues.tags, ...current?.tags, ...overrides?.tags },
    metadata: { ...globalContextValues.metadata, ...current?.metadata, ...overrides?.metadata },
  };
}

/**
 * Generate a unique trace ID.
 */
function generateTraceId(): string {
  const timestamp = Date.now();
  const random = Math.floor(Math.random() * 0xffffff);
  return `${timestamp.toString(16)}-${random.toString(16)}`;
}

/**
 * Bind a function to the current context.
 * Useful for callbacks and event handlers.
 *
 * @example
 * ```typescript
 * const boundHandler = bindContext(myHandler);
 * eventEmitter.on('data', boundHandler);
 * ```
 */
export function bindContext<T extends (...args: any[]) => any>(fn: T): T {
  const ctx = getCurrentContext();
  if (!ctx) return fn;

  return ((...args: Parameters<T>) => {
    return withContext(ctx, () => fn(...args));
  }) as T;
}
