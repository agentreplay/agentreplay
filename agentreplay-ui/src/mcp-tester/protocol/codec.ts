/**
 * Task 1 — JSON-RPC 2.0 Protocol Engine
 *
 * Typed JSON-RPC 2.0 Request/Response Codec with full spec fidelity.
 * Implements a finite state machine with 3 states per message lifecycle:
 *   Constructing → Serialized → Parsed
 *
 * Validation is O(1) per field. Batch encoding/decoding is O(n).
 * The JsonRpcId union type maps to a tagged discriminant with O(1) pattern matching.
 */

// ─── JSON-RPC 2.0 Types ───────────────────────────────────────────────────────

export type JsonRpcId = string | number | null;

export interface JsonRpcRequest {
  jsonrpc: '2.0';
  method: string;
  params?: Record<string, unknown> | unknown[];
  id: JsonRpcId;
}

export interface JsonRpcNotification {
  jsonrpc: '2.0';
  method: string;
  params?: Record<string, unknown> | unknown[];
}

export interface JsonRpcError {
  code: number;
  message: string;
  data?: unknown;
}

export interface JsonRpcResponse {
  jsonrpc: '2.0';
  result?: unknown;
  error?: JsonRpcError;
  id: JsonRpcId;
}

export type JsonRpcMessage = JsonRpcRequest | JsonRpcNotification | JsonRpcResponse;
export type JsonRpcBatchRequest = JsonRpcRequest[];
export type JsonRpcBatchResponse = JsonRpcResponse[];

// ─── Error Code Registry ───────────────────────────────────────────────────────

export const JSON_RPC_ERROR_CODES: Record<number, { label: string; description: string }> = {
  [-32700]: { label: 'Parse Error', description: 'Invalid JSON was received by the server.' },
  [-32600]: { label: 'Invalid Request', description: 'The JSON sent is not a valid Request object.' },
  [-32601]: { label: 'Method Not Found', description: 'The method does not exist or is not available.' },
  [-32602]: { label: 'Invalid Params', description: 'Invalid method parameter(s).' },
  [-32603]: { label: 'Internal Error', description: 'Internal JSON-RPC error.' },
  // Server-defined error codes (-32000 to -32099)
  [-32000]: { label: 'Server Error', description: 'Generic server error.' },
  [-32001]: { label: 'Not Initialized', description: 'Server has not been initialized.' },
  [-32002]: { label: 'Tool Execution Error', description: 'Tool execution failed.' },
};

export function getErrorLabel(code: number): string {
  return JSON_RPC_ERROR_CODES[code]?.label ?? `Server Error (${code})`;
}

export function getErrorDescription(code: number): string {
  return JSON_RPC_ERROR_CODES[code]?.description ?? 'An unknown error occurred.';
}

// ─── Message Lifecycle States ──────────────────────────────────────────────────

export type MessageState = 'constructing' | 'serialized' | 'parsed';

export interface TrackedMessage<T extends JsonRpcMessage = JsonRpcMessage> {
  state: MessageState;
  message: T;
  raw?: string;
  validationErrors: string[];
}

// ─── ID Generation ─────────────────────────────────────────────────────────────

let _nextId = 1;

export function nextId(): number {
  return _nextId++;
}

export function resetIdCounter(start = 1): void {
  _nextId = start;
}

// ─── Validation ────────────────────────────────────────────────────────────────

export function validateId(id: unknown): id is JsonRpcId {
  return id === null || typeof id === 'string' || typeof id === 'number';
}

function validateJsonRpcVersion(v: unknown): v is '2.0' {
  return v === '2.0';
}

export interface ValidationResult {
  valid: boolean;
  errors: string[];
}

export function validateRequest(obj: unknown): ValidationResult {
  const errors: string[] = [];
  if (typeof obj !== 'object' || obj === null) {
    return { valid: false, errors: ['Message must be a non-null object.'] };
  }
  const msg = obj as Record<string, unknown>;

  if (!validateJsonRpcVersion(msg.jsonrpc)) {
    errors.push('Missing or invalid "jsonrpc" field. Must be "2.0".');
  }
  if (typeof msg.method !== 'string' || msg.method.length === 0) {
    errors.push('Missing or invalid "method" field. Must be a non-empty string.');
  }
  if (msg.params !== undefined && typeof msg.params !== 'object') {
    errors.push('"params" must be an object or array if present.');
  }
  if ('id' in msg && !validateId(msg.id)) {
    errors.push('"id" must be a string, number, or null.');
  }

  return { valid: errors.length === 0, errors };
}

export function validateResponse(obj: unknown): ValidationResult {
  const errors: string[] = [];
  if (typeof obj !== 'object' || obj === null) {
    return { valid: false, errors: ['Response must be a non-null object.'] };
  }
  const msg = obj as Record<string, unknown>;

  if (!validateJsonRpcVersion(msg.jsonrpc)) {
    errors.push('Missing or invalid "jsonrpc" field. Must be "2.0".');
  }
  if (!('result' in msg) && !('error' in msg)) {
    errors.push('Response must contain either "result" or "error".');
  }
  if ('result' in msg && 'error' in msg) {
    errors.push('Response must not contain both "result" and "error".');
  }
  if ('id' in msg && !validateId(msg.id)) {
    errors.push('"id" must be a string, number, or null.');
  }
  if ('error' in msg && msg.error !== undefined) {
    const err = msg.error as Record<string, unknown>;
    if (typeof err.code !== 'number') errors.push('"error.code" must be a number.');
    if (typeof err.message !== 'string') errors.push('"error.message" must be a string.');
  }

  return { valid: errors.length === 0, errors };
}

// ─── Codec: Build & Parse ──────────────────────────────────────────────────────

export function buildRequest(
  method: string,
  params?: Record<string, unknown>,
  id?: JsonRpcId
): TrackedMessage<JsonRpcRequest> {
  const request: JsonRpcRequest = {
    jsonrpc: '2.0',
    method,
    id: id ?? nextId(),
  };
  if (params && Object.keys(params).length > 0) {
    request.params = params;
  }
  const validation = validateRequest(request);
  return {
    state: 'constructing',
    message: request,
    validationErrors: validation.errors,
  };
}

export function buildNotification(
  method: string,
  params?: Record<string, unknown>
): TrackedMessage<JsonRpcNotification> {
  const notification: JsonRpcNotification = {
    jsonrpc: '2.0',
    method,
  };
  if (params && Object.keys(params).length > 0) {
    notification.params = params;
  }
  return {
    state: 'constructing',
    message: notification,
    validationErrors: [],
  };
}

export function buildBatchRequest(
  requests: JsonRpcRequest[]
): TrackedMessage<JsonRpcRequest>[] {
  return requests.map((req) => {
    const validation = validateRequest(req);
    return {
      state: 'constructing' as MessageState,
      message: req,
      validationErrors: validation.errors,
    };
  });
}

export function serialize(msg: TrackedMessage): string {
  const raw = JSON.stringify(msg.message);
  msg.state = 'serialized';
  msg.raw = raw;
  return raw;
}

export function serializeBatch(messages: TrackedMessage<JsonRpcRequest>[]): string {
  const payload = messages.map((m) => {
    m.state = 'serialized';
    return m.message;
  });
  const raw = JSON.stringify(payload);
  messages.forEach((m) => (m.raw = raw));
  return raw;
}

export function parseResponse(raw: string): TrackedMessage<JsonRpcResponse> {
  try {
    const parsed = JSON.parse(raw);
    const validation = validateResponse(parsed);
    return {
      state: 'parsed',
      message: parsed as JsonRpcResponse,
      raw,
      validationErrors: validation.errors,
    };
  } catch {
    return {
      state: 'parsed',
      message: {
        jsonrpc: '2.0',
        error: { code: -32700, message: 'Parse error: Invalid JSON' },
        id: null,
      },
      raw,
      validationErrors: ['Failed to parse JSON response.'],
    };
  }
}

export function parseBatchResponse(raw: string): TrackedMessage<JsonRpcResponse>[] {
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      throw new Error('Batch response must be an array.');
    }
    return parsed.map((item: unknown) => {
      const validation = validateResponse(item);
      return {
        state: 'parsed' as MessageState,
        message: item as JsonRpcResponse,
        raw: JSON.stringify(item),
        validationErrors: validation.errors,
      };
    });
  } catch {
    return [
      {
        state: 'parsed',
        message: {
          jsonrpc: '2.0',
          error: { code: -32700, message: 'Parse error: Invalid batch JSON' },
          id: null,
        },
        raw,
        validationErrors: ['Failed to parse batch JSON response.'],
      },
    ];
  }
}

// ─── Helper: Check if a message is a notification (no id) ──────────────────────

export function isNotification(msg: JsonRpcMessage): msg is JsonRpcNotification {
  return !('id' in msg);
}

export function isRequest(msg: JsonRpcMessage): msg is JsonRpcRequest {
  return 'id' in msg && 'method' in msg;
}

export function isResponse(msg: JsonRpcMessage): msg is JsonRpcResponse {
  return 'id' in msg && ('result' in msg || 'error' in msg);
}

export function isErrorResponse(resp: JsonRpcResponse): boolean {
  return resp.error !== undefined && resp.error !== null;
}
