/**
 * Stdio Transport — Length-prefixed 4-byte big-endian framing
 *
 * For browser contexts, this provides a simulated interface.
 * The actual stdio transport is used in CLI/desktop contexts.
 * Encoding/decoding uses u32::from_be_bytes (4-byte header + payload).
 * Read complexity: O(n) where n = payload size.
 */

import type { JsonRpcRequest, JsonRpcResponse } from '../protocol/codec';
import {
  type McpTransport,
  type TransportEventHandler,
  type TransportEvent,
  type ConnectionInfo,
  createDefaultConnectionInfo,
} from './interface';

export class StdioTransport implements McpTransport {
  readonly type = 'stdio' as const;
  private _info: ConnectionInfo;
  private _handlers: Set<TransportEventHandler> = new Set();
  private _connected = false;

  constructor() {
    this._info = createDefaultConnectionInfo('stdio');
  }

  get connectionInfo(): ConnectionInfo {
    return { ...this._info };
  }

  /**
   * Encode a message with 4-byte big-endian length prefix.
   * Used in actual stdio transport (CLI/desktop).
   */
  static encodeFrame(payload: string): Uint8Array {
    const encoder = new TextEncoder();
    const data = encoder.encode(payload);
    const frame = new Uint8Array(4 + data.length);
    const view = new DataView(frame.buffer);
    view.setUint32(0, data.length, false); // big-endian
    frame.set(data, 4);
    return frame;
  }

  /**
   * Decode a length-prefixed frame.
   * Returns the payload string and number of bytes consumed.
   */
  static decodeFrame(buffer: Uint8Array): { payload: string; bytesConsumed: number } | null {
    if (buffer.length < 4) return null;
    const view = new DataView(buffer.buffer, buffer.byteOffset);
    const length = view.getUint32(0, false); // big-endian
    if (buffer.length < 4 + length) return null;
    const decoder = new TextDecoder();
    const payload = decoder.decode(buffer.slice(4, 4 + length));
    return { payload, bytesConsumed: 4 + length };
  }

  async connect(endpoint: string): Promise<void> {
    this._info.endpoint = endpoint;
    this._info.state = 'connected';
    this._info.connectedAt = Date.now();
    this._connected = true;
    this._emit({ type: 'state-change', data: 'connected', timestamp: Date.now() });
  }

  async disconnect(): Promise<void> {
    this._connected = false;
    this._info.state = 'disconnected';
    this._emit({ type: 'state-change', data: 'disconnected', timestamp: Date.now() });
  }

  async send(request: JsonRpcRequest): Promise<JsonRpcResponse> {
    // In browser context, stdio is simulated via HTTP POST
    // In Tauri/desktop context, this would use IPC
    const body = JSON.stringify(request);
    this._info.metrics.bytesSent += new Blob([body]).size;
    this._info.metrics.requestCount++;

    const start = performance.now();

    try {
      const resp = await fetch(this._info.endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body,
      });

      const text = await resp.text();
      const elapsed = performance.now() - start;
      this._info.metrics.latencyMs = elapsed;
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
    return Promise.all(requests.map((r) => this.send(r)));
  }

  on(handler: TransportEventHandler): () => void {
    this._handlers.add(handler);
    return () => this._handlers.delete(handler);
  }

  isConnected(): boolean {
    return this._connected;
  }

  private _emit(event: TransportEvent) {
    this._handlers.forEach((h) => h(event));
  }
}
