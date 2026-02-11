/**
 * Task 2 — Multi-Transport Abstraction Layer
 *
 * Unified Transport Interface using the Strategy Pattern.
 * O(1) transport dispatch via interface lookup.
 */

import type { JsonRpcRequest, JsonRpcResponse } from '../protocol/codec';

// ─── Transport Types ───────────────────────────────────────────────────────────

export type TransportType = 'http' | 'websocket' | 'sse' | 'stdio';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'error' | 'reconnecting';

export interface TransportMetrics {
  latencyMs: number;
  bytesSent: number;
  bytesReceived: number;
  requestCount: number;
  errorCount: number;
  /** Exponential moving average of latency */
  emaLatencyMs: number;
}

export interface ConnectionInfo {
  state: ConnectionState;
  transport: TransportType;
  endpoint: string;
  clientId?: string;
  connectedAt?: number;
  lastActivityAt?: number;
  metrics: TransportMetrics;
}

export interface TransportEvent {
  type: 'state-change' | 'message' | 'error' | 'metrics';
  data: unknown;
  timestamp: number;
}

export type TransportEventHandler = (event: TransportEvent) => void;

// ─── Transport Interface ───────────────────────────────────────────────────────

export interface McpTransport {
  readonly type: TransportType;
  readonly connectionInfo: ConnectionInfo;

  /** Connect to the MCP server */
  connect(endpoint: string): Promise<void>;

  /** Disconnect from the MCP server */
  disconnect(): Promise<void>;

  /** Send a JSON-RPC request and receive a response */
  send(request: JsonRpcRequest): Promise<JsonRpcResponse>;

  /** Send a batch of requests */
  sendBatch(requests: JsonRpcRequest[]): Promise<JsonRpcResponse[]>;

  /** Subscribe to transport events */
  on(handler: TransportEventHandler): () => void;

  /** Check if the transport is connected */
  isConnected(): boolean;
}

// ─── Exponential Backoff with Jitter ───────────────────────────────────────────

export interface BackoffConfig {
  baseMs: number;
  capMs: number;
  jitterMs: number;
}

export const DEFAULT_BACKOFF: BackoffConfig = {
  baseMs: 100,
  capMs: 30000,
  jitterMs: 500,
};

/**
 * delay = min(base × 2^attempt, cap) + random(0, jitter)
 * Converges to steady-state in O(log(cap/base)) attempts.
 */
export function computeBackoff(attempt: number, config: BackoffConfig = DEFAULT_BACKOFF): number {
  const exponential = Math.min(config.baseMs * Math.pow(2, attempt), config.capMs);
  const jitter = Math.random() * config.jitterMs;
  return exponential + jitter;
}

// ─── EMA Latency Tracker ───────────────────────────────────────────────────────

const EMA_ALPHA = 0.3;

export function updateEma(current: number, sample: number): number {
  return EMA_ALPHA * sample + (1 - EMA_ALPHA) * current;
}

// ─── Default Metrics ───────────────────────────────────────────────────────────

export function createDefaultMetrics(): TransportMetrics {
  return {
    latencyMs: 0,
    bytesSent: 0,
    bytesReceived: 0,
    requestCount: 0,
    errorCount: 0,
    emaLatencyMs: 0,
  };
}

export function createDefaultConnectionInfo(type: TransportType): ConnectionInfo {
  return {
    state: 'disconnected',
    transport: type,
    endpoint: '',
    metrics: createDefaultMetrics(),
  };
}
