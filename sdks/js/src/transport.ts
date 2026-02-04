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
 * Transport layer for batching, retrying, and sending spans.
 *
 * Features:
 * - Batch queue with size and time-based flushing
 * - Exponential backoff with jitter
 * - Bounded queue with backpressure
 * - Graceful shutdown
 */

import type { SpanInput, TransportConfig } from './types';
import { getConfig, getConfigOrNull } from './config';

/**
 * Transport statistics
 */
export interface TransportStats {
  /** Total spans queued */
  totalQueued: number;
  /** Total spans sent successfully */
  totalSent: number;
  /** Total spans dropped (queue overflow) */
  totalDropped: number;
  /** Current queue size */
  queueSize: number;
  /** Last successful send timestamp */
  lastSendTime: number | null;
  /** Last error message */
  lastError: string | null;
  /** Number of retries */
  retryCount: number;
}

/**
 * Batch transport for sending spans
 */
class BatchTransport {
  private queue: SpanInput[] = [];
  private flushTimer: ReturnType<typeof setTimeout> | null = null;
  private isShuttingDown = false;
  private isFlushing = false;
  private stats: TransportStats = {
    totalQueued: 0,
    totalSent: 0,
    totalDropped: 0,
    queueSize: 0,
    lastSendTime: null,
    lastError: null,
    retryCount: 0,
  };

  constructor() {
    // Setup graceful shutdown handlers in Node.js
    if (typeof process !== 'undefined') {
      const shutdown = () => {
        this.shutdown().catch(console.error);
      };

      process.on('beforeExit', shutdown);
      process.on('SIGTERM', shutdown);
      process.on('SIGINT', shutdown);
    }
  }

  /**
   * Get transport configuration
   */
  private getTransportConfig(): TransportConfig {
    const config = getConfigOrNull();
    return config?.transport ?? {
      mode: 'batch',
      batchSize: 100,
      flushIntervalMs: 5000,
      maxQueueSize: 10000,
      maxRetries: 3,
      retryDelayMs: 1000,
      compression: false,
    };
  }

  /**
   * Enqueue a span for sending
   */
  enqueue(span: SpanInput): void {
    if (this.isShuttingDown) {
      return;
    }

    const config = this.getTransportConfig();

    // Check queue limit
    if (this.queue.length >= config.maxQueueSize) {
      // Drop oldest spans (FIFO drop policy)
      const dropped = this.queue.shift();
      if (dropped) {
        this.stats.totalDropped++;
        const sdkConfig = getConfigOrNull();
        if (sdkConfig?.debug) {
          console.warn('[Agentreplay] Queue full, dropping oldest span');
        }
      }
    }

    this.queue.push(span);
    this.stats.totalQueued++;
    this.stats.queueSize = this.queue.length;

    // Console mode - print instead of send
    if (config.mode === 'console') {
      console.log('[Agentreplay] Span:', JSON.stringify(span, null, 2));
      this.queue.pop();
      this.stats.queueSize = this.queue.length;
      return;
    }

    // Check if we should flush immediately
    if (this.queue.length >= config.batchSize) {
      this.flushNow();
    } else {
      this.scheduleFlush();
    }
  }

  /**
   * Schedule a flush after the configured interval
   */
  private scheduleFlush(): void {
    if (this.flushTimer) return;

    const config = this.getTransportConfig();
    this.flushTimer = setTimeout(() => {
      this.flushTimer = null;
      this.flushNow();
    }, config.flushIntervalMs);
  }

  /**
   * Trigger an immediate flush
   */
  private flushNow(): void {
    if (this.isFlushing || this.queue.length === 0) return;

    this.isFlushing = true;
    this.doFlush()
      .catch((err) => {
        const sdkConfig = getConfigOrNull();
        if (sdkConfig?.debug) {
          console.error('[Agentreplay] Flush error:', err);
        }
        this.stats.lastError = err instanceof Error ? err.message : String(err);
      })
      .finally(() => {
        this.isFlushing = false;
      });
  }

  /**
   * Perform the actual flush with retries
   */
  private async doFlush(): Promise<void> {
    if (this.queue.length === 0) return;

    const config = this.getTransportConfig();
    const sdkConfig = getConfigOrNull();

    // Take batch from queue
    const batch = this.queue.splice(0, config.batchSize);
    this.stats.queueSize = this.queue.length;

    if (sdkConfig?.debug) {
      console.log(`[Agentreplay] Flushing ${batch.length} spans`);
    }

    // Retry with exponential backoff
    let lastError: Error | null = null;
    for (let attempt = 0; attempt <= config.maxRetries; attempt++) {
      try {
        await this.sendBatch(batch);
        this.stats.totalSent += batch.length;
        this.stats.lastSendTime = Date.now();
        this.stats.lastError = null;

        if (sdkConfig?.debug) {
          console.log(`[Agentreplay] Sent ${batch.length} spans successfully`);
        }
        return;
      } catch (err) {
        lastError = err instanceof Error ? err : new Error(String(err));
        this.stats.retryCount++;

        if (attempt < config.maxRetries) {
          // Exponential backoff with jitter
          const delay = config.retryDelayMs * Math.pow(2, attempt) * (0.5 + Math.random() * 0.5);
          if (sdkConfig?.debug) {
            console.warn(`[Agentreplay] Retry ${attempt + 1}/${config.maxRetries} in ${Math.round(delay)}ms`);
          }
          await sleep(delay);
        }
      }
    }

    // All retries failed - put spans back in queue
    if (lastError) {
      this.stats.lastError = lastError.message;
      // Put back at front of queue
      this.queue.unshift(...batch);
      this.stats.queueSize = this.queue.length;
      throw lastError;
    }
  }

  /**
   * Send a batch of spans to the server
   */
  private async sendBatch(spans: SpanInput[]): Promise<void> {
    const config = getConfigOrNull();
    if (!config) {
      throw new Error('SDK not initialized');
    }

    const url = `${config.baseUrl}/api/v1/traces`;
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      'X-Tenant-ID': String(config.tenantId),
      ...config.headers,
    };

    if (config.apiKey) {
      headers['Authorization'] = `Bearer ${config.apiKey}`;
    }

    const body = JSON.stringify({ spans });

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), config.timeout);

    try {
      const response = await config.fetch(url, {
        method: 'POST',
        headers,
        body,
        signal: controller.signal,
      });

      if (!response.ok) {
        const errorText = await response.text();

        // Handle rate limiting
        if (response.status === 429) {
          const retryAfter = response.headers.get('Retry-After');
          throw new Error(`Rate limited. Retry after: ${retryAfter ?? 'unknown'}`);
        }

        throw new Error(`API error (${response.status}): ${errorText}`);
      }
    } finally {
      clearTimeout(timeoutId);
    }
  }

  /**
   * Flush all pending spans.
   * Returns a promise that resolves when flush is complete.
   *
   * @param options - Flush options
   */
  async flush(options: { timeoutMs?: number } = {}): Promise<void> {
    const { timeoutMs = 30000 } = options;

    // Cancel scheduled flush
    if (this.flushTimer) {
      clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }

    if (this.queue.length === 0) {
      return;
    }

    const sdkConfig = getConfigOrNull();
    if (sdkConfig?.debug) {
      console.log(`[Agentreplay] Manual flush of ${this.queue.length} spans`);
    }

    // Flush with timeout
    const flushPromise = this.flushAll();
    const timeoutPromise = new Promise<void>((_, reject) => {
      setTimeout(() => reject(new Error('Flush timeout')), timeoutMs);
    });

    await Promise.race([flushPromise, timeoutPromise]);
  }

  /**
   * Flush all spans in the queue
   */
  private async flushAll(): Promise<void> {
    while (this.queue.length > 0) {
      await this.doFlush();
    }
  }

  /**
   * Shutdown the transport gracefully.
   * Flushes remaining spans and stops all timers.
   */
  async shutdown(): Promise<void> {
    if (this.isShuttingDown) return;
    this.isShuttingDown = true;

    const sdkConfig = getConfigOrNull();
    if (sdkConfig?.debug) {
      console.log('[Agentreplay] Shutting down transport...');
    }

    // Cancel scheduled flush
    if (this.flushTimer) {
      clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }

    // Flush remaining spans
    try {
      await this.flush({ timeoutMs: 5000 });
    } catch (err) {
      if (sdkConfig?.debug) {
        console.error('[Agentreplay] Error during shutdown flush:', err);
      }
    }

    if (sdkConfig?.debug) {
      console.log('[Agentreplay] Transport shutdown complete');
    }
  }

  /**
   * Get transport statistics
   */
  getStats(): TransportStats {
    return { ...this.stats, queueSize: this.queue.length };
  }

  /**
   * Check if transport is healthy (can send data)
   */
  isHealthy(): boolean {
    return !this.isShuttingDown && this.stats.lastError === null;
  }

  /**
   * Clear the queue (for testing)
   */
  clear(): void {
    this.queue = [];
    this.stats.queueSize = 0;
  }
}

/**
 * Sleep helper
 */
function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Global transport instance
 */
let globalTransport: BatchTransport | null = null;

/**
 * Get or create the global transport instance
 */
export function getTransport(): BatchTransport {
  if (!globalTransport) {
    globalTransport = new BatchTransport();
  }
  return globalTransport;
}

/**
 * Enqueue a span for sending
 */
export function enqueueSpan(span: SpanInput): void {
  getTransport().enqueue(span);
}

/**
 * Flush all pending spans.
 *
 * @example
 * ```typescript
 * // In serverless, call before function ends
 * await flush({ timeoutMs: 5000 });
 * ```
 */
export async function flush(options?: { timeoutMs?: number }): Promise<void> {
  return getTransport().flush(options);
}

/**
 * Shutdown the SDK transport.
 * Flushes remaining spans and cleans up resources.
 *
 * @example
 * ```typescript
 * // Before process exit
 * await shutdown();
 * ```
 */
export async function shutdown(): Promise<void> {
  if (globalTransport) {
    await globalTransport.shutdown();
    globalTransport = null;
  }
}

/**
 * Get transport statistics
 */
export function getTransportStats(): TransportStats {
  return getTransport().getStats();
}

/**
 * Reset transport (for testing)
 */
export function resetTransport(): void {
  if (globalTransport) {
    globalTransport.clear();
  }
  globalTransport = null;
}
