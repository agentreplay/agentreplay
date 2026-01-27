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
 * API client with exponential backoff retry logic
 * 
 * CRITICAL FIX: Prevents retry storms during backend recovery by implementing
 * proper exponential backoff with jitter for all API calls.
 */

import axios, { AxiosError, AxiosRequestConfig, AxiosResponse } from 'axios';

// ============================================================================
// Configuration
// ============================================================================

/** Default retry configuration */
export interface RetryConfig {
  /** Maximum number of retry attempts (default: 3) */
  maxRetries: number;
  /** Base delay in ms for exponential backoff (default: 1000) */
  baseDelay: number;
  /** Maximum delay in ms (default: 30000) */
  maxDelay: number;
  /** Jitter factor (0-1) to randomize delays (default: 0.2) */
  jitterFactor: number;
  /** HTTP status codes that should trigger retry (default: [429, 500, 502, 503, 504]) */
  retryableStatuses: number[];
}

const DEFAULT_RETRY_CONFIG: RetryConfig = {
  maxRetries: 3,
  baseDelay: 1000,
  maxDelay: 30000,
  jitterFactor: 0.2,
  retryableStatuses: [429, 500, 502, 503, 504],
};

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Calculate delay with exponential backoff and jitter
 */
function calculateDelay(
  attempt: number,
  baseDelay: number,
  maxDelay: number,
  jitterFactor: number,
  retryAfterHeader?: string
): number {
  // If server specifies Retry-After, respect it
  if (retryAfterHeader) {
    const retryAfterMs = parseInt(retryAfterHeader, 10) * 1000;
    if (!isNaN(retryAfterMs)) {
      return Math.min(retryAfterMs, maxDelay);
    }
  }

  // Exponential backoff: baseDelay * 2^attempt
  const exponentialDelay = baseDelay * Math.pow(2, attempt);
  
  // Apply max cap
  const cappedDelay = Math.min(exponentialDelay, maxDelay);
  
  // Add jitter to prevent thundering herd
  const jitter = cappedDelay * jitterFactor * Math.random();
  
  return Math.floor(cappedDelay + jitter);
}

/**
 * Check if error is retryable
 */
function isRetryable(error: AxiosError, retryableStatuses: number[]): boolean {
  // Network errors are retryable
  if (!error.response) {
    return true;
  }

  // Check if status code is in retryable list
  return retryableStatuses.includes(error.response.status);
}

/**
 * Sleep for specified milliseconds
 */
function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

// ============================================================================
// API Client with Retry
// ============================================================================

/**
 * Make an API request with exponential backoff retry
 * 
 * @param config - Axios request configuration
 * @param retryConfig - Retry configuration (optional)
 * @returns Promise with the response
 * @throws Last error after all retries exhausted
 * 
 * @example
 * ```typescript
 * // Simple GET request
 * const response = await apiRequest({ url: '/api/v1/projects' });
 * 
 * // POST with custom retry config
 * const response = await apiRequest(
 *   { method: 'POST', url: '/api/v1/traces', data: { ... } },
 *   { maxRetries: 5 }
 * );
 * ```
 */
export async function apiRequest<T = any>(
  config: AxiosRequestConfig,
  retryConfig: Partial<RetryConfig> = {}
): Promise<AxiosResponse<T>> {
  const finalConfig: RetryConfig = { ...DEFAULT_RETRY_CONFIG, ...retryConfig };
  let lastError: AxiosError | null = null;

  for (let attempt = 0; attempt <= finalConfig.maxRetries; attempt++) {
    try {
      // Make the request
      const response = await axios(config);
      return response;
    } catch (error) {
      const axiosError = error as AxiosError;
      lastError = axiosError;

      // Check if we should retry
      if (attempt < finalConfig.maxRetries && isRetryable(axiosError, finalConfig.retryableStatuses)) {
        // Get Retry-After header if present
        const retryAfterHeader = axiosError.response?.headers?.['retry-after'];
        
        // Calculate delay
        const delay = calculateDelay(
          attempt,
          finalConfig.baseDelay,
          finalConfig.maxDelay,
          finalConfig.jitterFactor,
          retryAfterHeader
        );

        console.warn(
          `API request failed (attempt ${attempt + 1}/${finalConfig.maxRetries + 1}), ` +
          `retrying after ${delay}ms...`,
          { url: config.url, status: axiosError.response?.status }
        );

        await sleep(delay);
        continue;
      }

      // Non-retryable error or max retries reached
      throw axiosError;
    }
  }

  // Should never reach here, but TypeScript needs this
  throw lastError || new Error('Unknown error during API request');
}

/**
 * Convenience wrapper for GET requests with retry
 */
export async function apiGet<T = any>(
  url: string,
  config?: AxiosRequestConfig,
  retryConfig?: Partial<RetryConfig>
): Promise<AxiosResponse<T>> {
  return apiRequest<T>({ ...config, method: 'GET', url }, retryConfig);
}

/**
 * Convenience wrapper for POST requests with retry
 */
export async function apiPost<T = any>(
  url: string,
  data?: any,
  config?: AxiosRequestConfig,
  retryConfig?: Partial<RetryConfig>
): Promise<AxiosResponse<T>> {
  return apiRequest<T>({ ...config, method: 'POST', url, data }, retryConfig);
}

/**
 * Convenience wrapper for PUT requests with retry
 */
export async function apiPut<T = any>(
  url: string,
  data?: any,
  config?: AxiosRequestConfig,
  retryConfig?: Partial<RetryConfig>
): Promise<AxiosResponse<T>> {
  return apiRequest<T>({ ...config, method: 'PUT', url, data }, retryConfig);
}

/**
 * Convenience wrapper for DELETE requests with retry
 */
export async function apiDelete<T = any>(
  url: string,
  config?: AxiosRequestConfig,
  retryConfig?: Partial<RetryConfig>
): Promise<AxiosResponse<T>> {
  return apiRequest<T>({ ...config, method: 'DELETE', url }, retryConfig);
}

// ============================================================================
// Fetch API wrapper with retry (for code that uses fetch directly)
// ============================================================================

/**
 * Fetch wrapper with exponential backoff retry
 * 
 * @param url - URL to fetch
 * @param init - Fetch init options
 * @param retryConfig - Retry configuration (optional)
 * @returns Promise with the Response
 * 
 * @example
 * ```typescript
 * const response = await fetchWithRetry('/api/v1/health');
 * const data = await response.json();
 * ```
 */
export async function fetchWithRetry(
  url: string,
  init?: RequestInit,
  retryConfig: Partial<RetryConfig> = {}
): Promise<Response> {
  const finalConfig: RetryConfig = { ...DEFAULT_RETRY_CONFIG, ...retryConfig };
  let lastError: Error | null = null;

  for (let attempt = 0; attempt <= finalConfig.maxRetries; attempt++) {
    try {
      const response = await fetch(url, init);

      // Check if response is an error that should be retried
      if (!response.ok && finalConfig.retryableStatuses.includes(response.status)) {
        if (attempt < finalConfig.maxRetries) {
          const retryAfterHeader = response.headers.get('Retry-After') || undefined;
          const delay = calculateDelay(
            attempt,
            finalConfig.baseDelay,
            finalConfig.maxDelay,
            finalConfig.jitterFactor,
            retryAfterHeader
          );

          console.warn(
            `Fetch failed (attempt ${attempt + 1}/${finalConfig.maxRetries + 1}), ` +
            `status: ${response.status}, retrying after ${delay}ms...`,
            { url }
          );

          await sleep(delay);
          continue;
        }
      }

      return response;
    } catch (error) {
      lastError = error as Error;

      // Network errors are retryable
      if (attempt < finalConfig.maxRetries) {
        const delay = calculateDelay(
          attempt,
          finalConfig.baseDelay,
          finalConfig.maxDelay,
          finalConfig.jitterFactor
        );

        console.warn(
          `Fetch network error (attempt ${attempt + 1}/${finalConfig.maxRetries + 1}), ` +
          `retrying after ${delay}ms...`,
          { url, error: (error as Error).message }
        );

        await sleep(delay);
        continue;
      }

      throw error;
    }
  }

  throw lastError || new Error('Unknown error during fetch');
}

// Export default configuration for customization
export { DEFAULT_RETRY_CONFIG };
