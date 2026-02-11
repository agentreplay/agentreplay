/**
 * Transport Factory — O(1) strategy dispatch via type lookup.
 */

import type { McpTransport, TransportType } from './interface';
import { HttpTransport } from './http';
import { WebSocketTransport } from './ws';
import { SseTransport } from './sse';
import { StdioTransport } from './stdio';

export function createTransport(type: TransportType): McpTransport {
  switch (type) {
    case 'http':
      return new HttpTransport();
    case 'websocket':
      return new WebSocketTransport();
    case 'sse':
      return new SseTransport();
    case 'stdio':
      return new StdioTransport();
  }
}

export { HttpTransport } from './http';
export { WebSocketTransport } from './ws';
export { SseTransport } from './sse';
export { StdioTransport } from './stdio';
export type { McpTransport, TransportType, ConnectionState, ConnectionInfo, TransportMetrics } from './interface';
