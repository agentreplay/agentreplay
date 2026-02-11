/**
 * HTTP Transport — Stateless POST /mcp
 *
 * Fire-and-forget per request. Each request is an independent HTTP POST.
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

export class HttpTransport implements McpTransport {
  readonly type = 'http' as const;
  private _info: ConnectionInfo;
  private _handlers: Set<TransportEventHandler> = new Set();
  private _abortController: AbortController | null = null;

  constructor() {
    this._info = createDefaultConnectionInfo('http');
  }

  get connectionInfo(): ConnectionInfo {
    return { ...this._info };
  }

  async connect(endpoint: string): Promise<void> {
    this._info.endpoint = endpoint;
    this._info.state = 'connecting';
    this._emit({ type: 'state-change', data: 'connecting', timestamp: Date.now() });

    try {
      // Verify connectivity with a health check
      const healthUrl = endpoint.replace(/\/mcp\/?$/, '/mcp/health');
      try {
        const resp = await fetch(healthUrl, { method: 'GET', signal: AbortSignal.timeout(5000) });
        if (resp.ok) {
          this._info.state = 'connected';
          this._info.connectedAt = Date.now();
          this._emit({ type: 'state-change', data: 'connected', timestamp: Date.now() });
          return;
        }
      } catch {
        // Health endpoint not available, try POST to the main endpoint
      }

      // Fallback: try a simple ping to the main endpoint
      try {
        const resp = await fetch(endpoint, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ jsonrpc: '2.0', method: 'ping', id: 0 }),
          signal: AbortSignal.timeout(5000),
        });
        if (resp.ok || resp.status < 500) {
          this._info.state = 'connected';
          this._info.connectedAt = Date.now();
          this._emit({ type: 'state-change', data: 'connected', timestamp: Date.now() });
          return;
        }
      } catch {
        // Main endpoint also unreachable
      }

      throw new Error('Server unreachable');
    } catch (err) {
      this._info.state = 'error';
      this._emit({ type: 'error', data: err, timestamp: Date.now() });
      throw err;
    }
  }

  async disconnect(): Promise<void> {
    this._abortController?.abort();
    this._abortController = null;
    this._info.state = 'disconnected';
    this._emit({ type: 'state-change', data: 'disconnected', timestamp: Date.now() });
  }

  async send(request: JsonRpcRequest): Promise<JsonRpcResponse> {
    const body = JSON.stringify(request);
    const bytesSent = new Blob([body]).size;
    this._info.metrics.bytesSent += bytesSent;
    this._info.metrics.requestCount++;

    this._abortController = new AbortController();
    const start = performance.now();

    try {
      const resp = await fetch(this._info.endpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Accept: 'application/json',
        },
        body,
        signal: this._abortController.signal,
      });

      const text = await resp.text();
      const elapsed = performance.now() - start;

      this._info.metrics.latencyMs = elapsed;
      this._info.metrics.emaLatencyMs = updateEma(this._info.metrics.emaLatencyMs, elapsed);
      this._info.metrics.bytesReceived += new Blob([text]).size;
      this._info.lastActivityAt = Date.now();

      this._emit({ type: 'metrics', data: { ...this._info.metrics }, timestamp: Date.now() });

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
    const bytesSent = new Blob([body]).size;
    this._info.metrics.bytesSent += bytesSent;
    this._info.metrics.requestCount += requests.length;

    this._abortController = new AbortController();
    const start = performance.now();

    try {
      const resp = await fetch(this._info.endpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Accept: 'application/json',
        },
        body,
        signal: this._abortController.signal,
      });

      const text = await resp.text();
      const elapsed = performance.now() - start;

      this._info.metrics.latencyMs = elapsed;
      this._info.metrics.emaLatencyMs = updateEma(this._info.metrics.emaLatencyMs, elapsed);
      this._info.metrics.bytesReceived += new Blob([text]).size;
      this._info.lastActivityAt = Date.now();

      if (!text || text.trim() === '') {
        return [];
      }
      try {
        return JSON.parse(text) as JsonRpcResponse[];
      } catch {
        return [{ jsonrpc: '2.0' as const, error: { code: -32700, message: 'Parse error' }, id: null }];
      }
    } catch (err) {
      this._info.metrics.errorCount++;
      this._emit({ type: 'error', data: err, timestamp: Date.now() });
      throw err;
    }
  }

  on(handler: TransportEventHandler): () => void {
    this._handlers.add(handler);
    return () => this._handlers.delete(handler);
  }

  isConnected(): boolean {
    return this._info.state === 'connected';
  }

  private _emit(event: TransportEvent) {
    this._handlers.forEach((h) => h(event));
  }
}
