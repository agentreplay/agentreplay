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
 * Fetch wrapper for automatic HTTP tracing.
 */

import { withSpan, type ActiveSpan } from '../traceable';
import type { SpanKind } from '../types';

/**
 * Wrapper options for fetch
 */
export interface WrapFetchOptions {
  /** Whether to record request body (default: false for privacy) */
  recordRequestBody?: boolean;
  /** Whether to record response body (default: false for privacy) */
  recordResponseBody?: boolean;
  /** Whether to record headers (default: false) */
  recordHeaders?: boolean;
  /** URL patterns to exclude from tracing */
  excludeUrls?: Array<string | RegExp>;
  /** Custom metadata to add to all spans */
  metadata?: Record<string, unknown>;
}

/**
 * Check if URL matches any exclude pattern
 */
function isExcluded(url: string, patterns: Array<string | RegExp>): boolean {
  for (const pattern of patterns) {
    if (typeof pattern === 'string') {
      if (url.includes(pattern)) return true;
    } else {
      if (pattern.test(url)) return true;
    }
  }
  return false;
}

/**
 * Wrap the global fetch function for automatic HTTP tracing.
 *
 * @example
 * ```typescript
 * import { init, wrapFetch } from '@agentreplay/sdk';
 *
 * init();
 *
 * const tracedFetch = wrapFetch(fetch, {
 *   excludeUrls: ['/health', /\.css$/]
 * });
 *
 * // All calls are now automatically traced
 * const response = await tracedFetch('https://api.example.com/data');
 * ```
 */
export function wrapFetch(
  fetchFn: typeof fetch,
  options: WrapFetchOptions = {}
): typeof fetch {
  const excludeUrls = options.excludeUrls ?? [];

  return async function tracedFetch(
    input: string | URL | Request,
    init?: RequestInit
  ): Promise<Response> {
    // Get URL string
    let url: string;
    let method = 'GET';

    if (typeof input === 'string') {
      url = input;
    } else if (input instanceof URL) {
      url = input.toString();
    } else {
      url = input.url;
      method = input.method ?? 'GET';
    }

    if (init?.method) {
      method = init.method;
    }

    // Check exclusions
    if (isExcluded(url, excludeUrls)) {
      return fetchFn(input, init);
    }

    // Parse URL for span name
    let hostname = 'unknown';
    let pathname = '/';
    try {
      const parsed = new URL(url);
      hostname = parsed.hostname;
      pathname = parsed.pathname;
    } catch {
      // Invalid URL, use as-is
    }

    const spanName = `${method} ${hostname}${pathname}`;

    return withSpan(
      spanName,
      {
        kind: 'tool' as SpanKind,
        metadata: {
          ...options.metadata,
          'http.request.method': method,
          'http.url': url,
          'server.address': hostname,
        },
      },
      async (span: ActiveSpan) => {
        // Record request details
        span.attributes['http.request.method'] = method;
        span.attributes['url.full'] = url;

        if (options.recordHeaders && init?.headers) {
          span.attributes['http.request.headers'] = JSON.stringify(init.headers);
        }

        if (options.recordRequestBody && init?.body) {
          try {
            const bodyStr = typeof init.body === 'string'
              ? init.body
              : JSON.stringify(init.body);
            span.attributes['http.request.body'] = bodyStr.slice(0, 10000); // Limit size
          } catch {
            // Can't serialize body
          }
        }

        const startTime = Date.now();

        try {
          const response = await fetchFn(input, init);

          // Record response details
          span.attributes['http.response.status_code'] = String(response.status);
          span.attributes['http.response.status_text'] = response.statusText;
          span.attributes['http.duration_ms'] = String(Date.now() - startTime);

          if (options.recordHeaders) {
            const headers: Record<string, string> = {};
            response.headers.forEach((value, key) => {
              headers[key] = value;
            });
            span.attributes['http.response.headers'] = JSON.stringify(headers);
          }

          // Mark error for non-2xx responses
          if (!response.ok) {
            span.attributes['error.type'] = 'HttpError';
            span.attributes['error.message'] = `HTTP ${response.status}: ${response.statusText}`;
          }

          return response;
        } catch (error) {
          span.setError(error);
          throw error;
        }
      }
    );
  };
}

/**
 * Install global fetch tracing.
 * Replaces globalThis.fetch with a traced version.
 *
 * @example
 * ```typescript
 * import { init, installFetchTracing } from '@agentreplay/sdk';
 *
 * init();
 * installFetchTracing({ excludeUrls: ['/health'] });
 *
 * // All fetch calls are now traced
 * const response = await fetch('https://api.example.com/data');
 * ```
 */
export function installFetchTracing(options: WrapFetchOptions = {}): void {
  if (typeof globalThis.fetch === 'function') {
    globalThis.fetch = wrapFetch(globalThis.fetch, options);
  }
}
