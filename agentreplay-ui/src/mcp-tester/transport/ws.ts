/**
 * WebSocket Transport — Bidirectional /mcp/ws
 *
 * Persistent connection with client-id tracking.
 * Reconnection uses exponential backoff with jitter:
 *   delay = min(base × 2^attempt, cap) + random(0, jitter)
 */

import type { JsonRpcRequest, JsonRpcResponse } from '../protocol/codec';
import {
  type McpTransport,
  type TransportEventHandler,
  type TransportEvent,
  type ConnectionInfo,
  type BackoffConfig,
  createDefaultConnectionInfo,
  computeBackoff,
  updateEma,
  DEFAULT_BACKOFF,
} from './interface';

interface PendingRequest {
  resolve: (resp: JsonRpcResponse) => void;
  reject: (err: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

export class WebSocketTransport implements McpTransport {
  readonly type = 'websocket' as const;
  private _info: ConnectionInfo;
  private _handlers: Set<TransportEventHandler> = new Set();
  private _ws: WebSocket | null = null;
  private _pending: Map<string | number, PendingRequest> = new Map();
  private _reconnectAttempt = 0;
  private _reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private _autoReconnect = true;
  private _backoffConfig: BackoffConfig;
  private _requestTimeoutMs = 30000;

  constructor(backoffConfig?: Partial<BackoffConfig>) {
    this._info = createDefaultConnectionInfo('websocket');
    this._backoffConfig = { ...DEFAULT_BACKOFF, ...backoffConfig };
  }

  get connectionInfo(): ConnectionInfo {
    return { ...this._info };
  }

  async connect(endpoint: string): Promise<void> {
    this._info.endpoint = endpoint;
    this._autoReconnect = true;

    return new Promise<void>((resolve, reject) => {
      this._info.state = 'connecting';
      this._emit({ type: 'state-change', data: 'connecting', timestamp: Date.now() });

      try {
        // Convert HTTP URL to WS URL
        const wsUrl = endpoint
          .replace(/^http:/, 'ws:')
          .replace(/^https:/, 'wss:')
          .replace(/\/mcp\/?$/, '/mcp/ws');

        this._ws = new WebSocket(wsUrl);

        this._ws.onopen = () => {
          this._info.state = 'connected';
          this._info.connectedAt = Date.now();
          this._reconnectAttempt = 0;
          this._emit({ type: 'state-change', data: 'connected', timestamp: Date.now() });
          resolve();
        };

        this._ws.onmessage = (event) => {
          this._info.lastActivityAt = Date.now();
          const raw = typeof event.data === 'string' ? event.data : '';
          this._info.metrics.bytesReceived += new Blob([raw]).size;

          try {
            const data = JSON.parse(raw);
            // Handle batch responses
            if (Array.isArray(data)) {
              data.forEach((resp: JsonRpcResponse) => this._resolveResponse(resp));
            } else {
              this._resolveResponse(data as JsonRpcResponse);
            }
            this._emit({ type: 'message', data, timestamp: Date.now() });
          } catch {
            this._emit({ type: 'error', data: `Failed to parse WS message: ${raw}`, timestamp: Date.now() });
          }
        };

        this._ws.onerror = (err) => {
          this._info.metrics.errorCount++;
          this._emit({ type: 'error', data: err, timestamp: Date.now() });
          if (this._info.state === 'connecting') {
            reject(new Error('WebSocket connection failed'));
          }
        };

        this._ws.onclose = () => {
          this._info.state = 'disconnected';
          this._emit({ type: 'state-change', data: 'disconnected', timestamp: Date.now() });
          // Reject all pending
          this._pending.forEach((p) => p.reject(new Error('WebSocket closed')));
          this._pending.clear();
          // Auto-reconnect
          if (this._autoReconnect) {
            this._scheduleReconnect();
          }
        };
      } catch (err) {
        this._info.state = 'error';
        reject(err);
      }
    });
  }

  async disconnect(): Promise<void> {
    this._autoReconnect = false;
    if (this._reconnectTimer) {
      clearTimeout(this._reconnectTimer);
      this._reconnectTimer = null;
    }
    if (this._ws) {
      this._ws.close(1000, 'Client disconnect');
      this._ws = null;
    }
    this._pending.forEach((p) => p.reject(new Error('Disconnected')));
    this._pending.clear();
    this._info.state = 'disconnected';
    this._emit({ type: 'state-change', data: 'disconnected', timestamp: Date.now() });
  }

  async send(request: JsonRpcRequest): Promise<JsonRpcResponse> {
    if (!this._ws || this._ws.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket is not connected');
    }

    const body = JSON.stringify(request);
    this._info.metrics.bytesSent += new Blob([body]).size;
    this._info.metrics.requestCount++;

    const start = performance.now();

    return new Promise<JsonRpcResponse>((resolve, reject) => {
      const idKey = request.id ?? '';
      const timer = setTimeout(() => {
        this._pending.delete(idKey);
        reject(new Error(`Request timed out after ${this._requestTimeoutMs}ms`));
      }, this._requestTimeoutMs);

      this._pending.set(idKey, {
        resolve: (resp) => {
          const elapsed = performance.now() - start;
          this._info.metrics.latencyMs = elapsed;
          this._info.metrics.emaLatencyMs = updateEma(this._info.metrics.emaLatencyMs, elapsed);
          this._emit({ type: 'metrics', data: { ...this._info.metrics }, timestamp: Date.now() });
          resolve(resp);
        },
        reject,
        timer,
      });

      this._ws!.send(body);
    });
  }

  async sendBatch(requests: JsonRpcRequest[]): Promise<JsonRpcResponse[]> {
    // For WS, send each individually and collect
    return Promise.all(requests.map((r) => this.send(r)));
  }

  on(handler: TransportEventHandler): () => void {
    this._handlers.add(handler);
    return () => this._handlers.delete(handler);
  }

  isConnected(): boolean {
    return this._ws !== null && this._ws.readyState === WebSocket.OPEN;
  }

  private _resolveResponse(resp: JsonRpcResponse) {
    const idKey = resp.id ?? '';
    const pending = this._pending.get(idKey);
    if (pending) {
      clearTimeout(pending.timer);
      this._pending.delete(idKey);
      pending.resolve(resp);
    }
  }

  private _scheduleReconnect() {
    const delay = computeBackoff(this._reconnectAttempt, this._backoffConfig);
    this._reconnectAttempt++;
    this._info.state = 'reconnecting';
    this._emit({ type: 'state-change', data: 'reconnecting', timestamp: Date.now() });

    this._reconnectTimer = setTimeout(async () => {
      try {
        await this.connect(this._info.endpoint);
      } catch {
        // Will trigger another reconnect via onclose
      }
    }, delay);
  }

  private _emit(event: TransportEvent) {
    this._handlers.forEach((h) => h(event));
  }
}
