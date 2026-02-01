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

// Langfuse API client for Agentreplay UI
import axios from 'axios';

const LANGFUSE_BASE_URL = process.env.NEXT_PUBLIC_LANGFUSE_URL || 'http://localhost:47100';

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
  duration: number;
  inputTokens: number;
  outputTokens: number;
  cost: number;
  model: string;
  status: 'success' | 'error';
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

export interface TraceDetails extends Trace {
  spans: Span[];
  input?: string;
  output?: string;
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

class LangfuseClient {
  private baseURL: string;
  private publicKey?: string;
  private secretKey?: string;
  private circuitBreaker: CircuitBreaker;

  constructor(baseURL: string = LANGFUSE_BASE_URL) {
    this.baseURL = baseURL;
    this.publicKey = process.env.NEXT_PUBLIC_LANGFUSE_PUBLIC_KEY;
    this.secretKey = process.env.LANGFUSE_SECRET_KEY;
    this.circuitBreaker = new CircuitBreaker();
  }

  private getHeaders() {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    if (this.publicKey && this.secretKey) {
      headers['X-Langfuse-Public-Key'] = this.publicKey;
      headers['X-Langfuse-Secret-Key'] = this.secretKey;
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
  } = {}): Promise<{ data: Trace[]; total: number }> {
    try {
      return await this.circuitBreaker.execute(async () => {
        return await fetchWithRetry(async () => {
          const response = await axios.get(`${this.baseURL}/api/public/traces`, {
            params: {
              limit: params.limit || 20,
              page: params.page || 1,
              fromTimestamp: params.startTime,
              toTimestamp: params.endTime,
              userId: params.userId,
              status: params.status,
            },
            headers: this.getHeaders(),
          });
          return response.data;
        });
      });
    } catch (error) {
      console.error('Error fetching traces:', error);
      // Return mock data for development
      return this.getMockTraces(params.limit || 20);
    }
  }

  async getTrace(traceId: string): Promise<TraceDetails> {
    try {
      return await this.circuitBreaker.execute(async () => {
        return await fetchWithRetry(async () => {
          const response = await axios.get(`${this.baseURL}/api/public/traces/${traceId}`, {
            headers: this.getHeaders(),
          });
          return response.data;
        });
      });
    } catch (error) {
      console.error('Error fetching trace details:', error);
      throw error;
    }
  }

  async getObservations(traceId: string): Promise<Span[]> {
    try {
      return await this.circuitBreaker.execute(async () => {
        return await fetchWithRetry(async () => {
          const response = await axios.get(`${this.baseURL}/api/public/traces/${traceId}/observations`, {
            headers: this.getHeaders(),
          });
          return response.data;
        });
      });
    } catch (error) {
      console.error('Error fetching observations:', error);
      return [];
    }
  }

  async getMetrics(params: {
    startTime?: number;
    endTime?: number;
    granularity?: 'hour' | 'day' | 'week';
  } = {}): Promise<Metrics> {
    try {
      return await this.circuitBreaker.execute(async () => {
        return await fetchWithRetry(async () => {
          const response = await axios.get(`${this.baseURL}/api/public/metrics`, {
            params: {
              fromTimestamp: params.startTime,
              toTimestamp: params.endTime,
              granularity: params.granularity || 'hour',
            },
            headers: this.getHeaders(),
          });
          return response.data;
        });
      });
    } catch (error) {
      console.error('Error fetching metrics:', error);
      // Return mock metrics for development
      return this.getMockMetrics();
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
}

export const langfuseClient = new LangfuseClient();
