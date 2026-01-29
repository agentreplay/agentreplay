/**
 * Span types matching Agentreplay SDK
 */
export enum SpanType {
  Root = 0,
  Planning = 1,
  Reasoning = 2,
  ToolCall = 3,
  ToolResponse = 4,
  Synthesis = 5,
  Response = 6,
  Error = 7,
  Retrieval = 8,
  Embedding = 9,
  HttpCall = 10,
  Database = 11,
  Function = 12,
  Reranking = 13,
  Parsing = 14,
  Generation = 15,
  Custom = 255,
}

export interface TraceContext {
  sessionKey: string;
  parentEdgeId?: string;
  startTime: number;
  model?: string;
  provider?: string;
}

/**
 * Memory operation types for tracking
 */
export type MemoryOperation = "store" | "recall" | "search" | "delete" | "update";

export interface MemoryTracePayload {
  tenant_id: number;
  project_id: number;
  agent_id: number;
  session_id: number;
  operation: MemoryOperation;
  memory_id?: string;
  text?: string;
  category?: string;
  importance?: number;
  result_count?: number;
  score?: number;
  duration_ms?: number;
  parent_edge_id?: string;
  metadata?: Record<string, unknown>;
}
