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
 * Host functions provided by Agentreplay to plugins.
 */

import type {
  TraceContext,
  TraceId,
  Embedding,
  LogLevel,
  HttpResponse,
} from './types';

/**
 * Host provides access to Agentreplay runtime functions.
 */
export const Host = {
  /**
   * Log a message.
   */
  log(level: LogLevel, message: string): void {
    const levelNum =
      level === 'trace'
        ? 0
        : level === 'debug'
          ? 1
          : level === 'info'
            ? 2
            : level === 'warn'
              ? 3
              : 4;
    (globalThis as any).__agentreplay_log?.(levelNum, message);
  },

  /**
   * Log at trace level.
   */
  trace(message: string): void {
    Host.log('trace', message);
  },

  /**
   * Log at debug level.
   */
  debug(message: string): void {
    Host.log('debug', message);
  },

  /**
   * Log at info level.
   */
  info(message: string): void {
    Host.log('info', message);
  },

  /**
   * Log at warn level.
   */
  warn(message: string): void {
    Host.log('warn', message);
  },

  /**
   * Log at error level.
   */
  error(message: string): void {
    Host.log('error', message);
  },

  /**
   * Get plugin configuration.
   */
  getConfig<T = Record<string, unknown>>(): T {
    const configJson = (globalThis as any).__agentreplay_get_config?.() ?? '{}';
    return JSON.parse(configJson);
  },

  /**
   * Get a specific configuration value.
   */
  getConfigValue(key: string): string | null {
    return (globalThis as any).__agentreplay_get_config_value?.(key) ?? null;
  },

  /**
   * Query traces from the database.
   * Requires trace-read capability.
   */
  queryTraces(filterJson: string, limit: number = 100): TraceContext[] {
    const resultJson =
      (globalThis as any).__agentreplay_query_traces?.(filterJson, limit) ?? '[]';
    return JSON.parse(resultJson);
  },

  /**
   * Get a single trace by ID.
   * Requires trace-read capability.
   */
  getTrace(traceId: TraceId): TraceContext | null {
    const idStr =
      traceId.high.toString(16).padStart(16, '0') +
      traceId.low.toString(16).padStart(16, '0');
    const resultJson = (globalThis as any).__agentreplay_get_trace?.(idStr);
    return resultJson ? JSON.parse(resultJson) : null;
  },

  /**
   * Make an HTTP request.
   * Requires network capability.
   */
  async httpRequest(
    method: string,
    url: string,
    headers?: Record<string, string>,
    body?: Uint8Array
  ): Promise<HttpResponse> {
    const result = await (globalThis as any).__agentreplay_http_request?.(
      method,
      url,
      JSON.stringify(headers ?? {}),
      body ?? new Uint8Array()
    );
    const parsed = JSON.parse(result);
    return {
      status: parsed.status,
      headers: new Map(Object.entries(parsed.headers)),
      body: new Uint8Array(parsed.body),
    };
  },

  /**
   * Generate text embedding.
   * Requires embedding capability.
   */
  embedText(text: string): Embedding {
    const resultJson =
      (globalThis as any).__agentreplay_embed_text?.(text) ?? '[]';
    return new Float32Array(JSON.parse(resultJson));
  },

  /**
   * Batch embed multiple texts.
   * Requires embedding capability.
   */
  embedBatch(texts: string[]): Embedding[] {
    const resultJson =
      (globalThis as any).__agentreplay_embed_batch?.(JSON.stringify(texts)) ??
      '[]';
    return JSON.parse(resultJson).map(
      (arr: number[]) => new Float32Array(arr)
    );
  },

  /**
   * Get environment variable.
   * Requires env-vars capability.
   */
  getEnv(name: string): string | null {
    return (globalThis as any).__agentreplay_get_env?.(name) ?? null;
  },
};
