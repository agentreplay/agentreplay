/**
 * SSE Transport — Server-push via GET /mcp/sse
 *
 * Receive-only for server events (init, keepalive, notifications).
 * Sends requests via companion HTTP POST (SSE is unidirectional).
 */

import type { JsonRpcRequest, JsonRpcResponse } from '../protocol/codec';
import {
  type McpTransport,
  type TransportEventHandler,
  type TransportEvent,
  type ConnectionInfo,
  createDefaultConnectionInfo,
  updateEma,
} from './interface';

export class SseTransport implements McpTransport {
  readonly type = 'sse' as const;
  private _info: ConnectionInfo;
  private _handlers: Set<TransportEventHandler> = new Set();
  private _eventSource: EventSource | null = null;
  private _postEndpoint = '';

  constructor() {
    this._info = createDefaultConnectionInfo('sse');
  }

  get connectionInfo(): ConnectionInfo {
    return { ...this._info };
  }

  async connect(endpoint: string): Promise<void> {
    this._info.endpoint = endpoint;
    this._postEndpoint = endpoint; // POST goes to /mcp
    const sseUrl = endpoint.replace(/\/mcp\/?$/, '/mcp/sse');

    this._info.state = 'connecting';
    this._emit({ type: 'state-change', data: 'connecting', timestamp: Date.now() });

    return new Promise<void>((resolve, reject) => {
      try {
        this._eventSource = new EventSource(sseUrl);

        this._eventSource.onopen = () => {
          this._info.state = 'connected';
          this._info.connectedAt = Date.now();
          this._emit({ type: 'state-change', data: 'connected', timestamp: Date.now() });
          resolve();
        };

        this._eventSource.onmessage = (event) => {
          this._info.lastActivityAt = Date.now();
          this._info.metrics.bytesReceived += new Blob([event.data]).size;
          try {
            const data = JSON.parse(event.data);
            this._emit({ type: 'message', data, timestamp: Date.now() });
          } catch {
            // Non-JSON SSE data (keepalive, etc.)
            this._emit({ type: 'message', data: event.data, timestamp: Date.now() });
          }
        };

        this._eventSource.onerror = () => {
          if (this._info.state === 'connecting') {
            this._info.state = 'error';
            this._emit({ type: 'error', data: 'SSE connection failed', timestamp: Date.now() });
            reject(new Error('SSE connection failed'));
          } else {
            this._info.state = 'reconnecting';
            this._emit({ type: 'state-change', data: 'reconnecting', timestamp: Date.now() });
          }
        };

        // Listen for specific SSE event types
        this._eventSource.addEventListener('init', (event) => {
          this._emit({ type: 'message', data: { type: 'init', payload: (event as MessageEvent).data }, timestamp: Date.now() });
        });

        this._eventSource.addEventListener('keepalive', () => {
          this._info.lastActivityAt = Date.now();
        });
      } catch (err) {
        this._info.state = 'error';
        reject(err);
      }
    });
  }

  async disconnect(): Promise<void> {
    this._eventSource?.close();
    this._eventSource = null;
    this._info.state = 'disconnected';
    this._emit({ type: 'state-change', data: 'disconnected', timestamp: Date.now() });
  }

  async send(request: JsonRpcRequest): Promise<JsonRpcResponse> {
    // SSE is receive-only; send via HTTP POST
    const body = JSON.stringify(request);
    this._info.metrics.bytesSent += new Blob([body]).size;
    this._info.metrics.requestCount++;

    const start = performance.now();

    try {
      const resp = await fetch(this._postEndpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Accept: 'application/json',
        },
        body,
      });

      const text = await resp.text();
      const elapsed = performance.now() - start;

      this._info.metrics.latencyMs = elapsed;
      this._info.metrics.emaLatencyMs = updateEma(this._info.metrics.emaLatencyMs, elapsed);
      this._info.metrics.bytesReceived += new Blob([text]).size;
      this._info.lastActivityAt = Date.now();

      if (!text || text.trim() === '') {
        return { jsonrpc: '2.0' as const, result: { acknowledged: true }, id: null };
      }
      try {
        return JSON.parse(text) as JsonRpcResponse;
      } catch {
        return { jsonrpc: '2.0' as const, error: { code: -32700, message: `Parse error: ${text.slice(0, 100)}` }, id: null };
      }
    } catch (err) {
      this._info.metrics.errorCount++;
      this._emit({ type: 'error', data: err, timestamp: Date.now() });
      throw err;
    }
  }

  async sendBatch(requests: JsonRpcRequest[]): Promise<JsonRpcResponse[]> {
    const body = JSON.stringify(requests);
    this._info.metrics.bytesSent += new Blob([body]).size;
    this._info.metrics.requestCount += requests.length;

    const start = performance.now();

    const resp = await fetch(this._postEndpoint, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
      body,
    });

    const text = await resp.text();
    const elapsed = performance.now() - start;
    this._info.metrics.latencyMs = elapsed;
    this._info.metrics.emaLatencyMs = updateEma(this._info.metrics.emaLatencyMs, elapsed);
    this._info.metrics.bytesReceived += new Blob([text]).size;

    if (!text || text.trim() === '') {
      return [];
    }
    try {
      return JSON.parse(text) as JsonRpcResponse[];
    } catch {
      return [{ jsonrpc: '2.0' as const, error: { code: -32700, message: 'Parse error' }, id: null }];
    }
  }

  on(handler: TransportEventHandler): () => void {
    this._handlers.add(handler);
    return () => this._handlers.delete(handler);
  }

  isConnected(): boolean {
    return this._eventSource !== null && this._eventSource.readyState === EventSource.OPEN;
  }

  private _emit(event: TransportEvent) {
    this._handlers.forEach((h) => h(event));
  }
}
