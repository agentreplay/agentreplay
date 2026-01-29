/**
 * Agentreplay API client
 */

import type { AgentreplayConfig } from "./config.js";
import type { MemoryTracePayload } from "./types.js";
import type { PluginLogger } from "./plugin-types.js";

export interface TracePayload {
  tenant_id: number;
  project_id: number;
  agent_id: number;
  session_id: number;
  span_type: number;
  parent_edge_id?: string;
  token_count?: number;
  confidence?: number;
  metadata?: Record<string, unknown>;
  payload?: Record<string, unknown>;
}

export interface GenAITracePayload {
  tenant_id: number;
  project_id: number;
  agent_id: number;
  session_id: number;
  model: string;
  input_messages?: Array<{ role: string; content: string }>;
  output?: { role: string; content: string };
  input_usage?: number;
  output_usage?: number;
  total_usage?: number;
  finish_reason?: string;
  duration_ms?: number;
  parent_edge_id?: string;
  metadata?: Record<string, unknown>;
}

export interface ToolTracePayload {
  tenant_id: number;
  project_id: number;
  agent_id: number;
  session_id: number;
  tool_name: string;
  tool_input?: unknown;
  tool_output?: unknown;
  duration_ms?: number;
  parent_edge_id?: string;
  metadata?: Record<string, unknown>;
}

/**
 * Send a generic trace to Agentreplay
 */
export async function sendTrace(
  config: AgentreplayConfig,
  logger: PluginLogger,
  payload: TracePayload,
): Promise<string | null> {
  try {
    const response = await fetch(`${config.url}/api/v1/traces`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      logger.warn(`Agentreplay trace failed: ${response.status} ${response.statusText}`);
      return null;
    }

    const result = (await response.json()) as { edge_id?: string };
    return result.edge_id || null;
  } catch (err) {
    logger.debug?.(`Agentreplay connection error: ${err instanceof Error ? err.message : String(err)}`);
    return null;
  }
}

/**
 * Send GenAI-specific trace (LLM calls)
 */
export async function sendGenAITrace(
  config: AgentreplayConfig,
  logger: PluginLogger,
  payload: GenAITracePayload,
): Promise<string | null> {
  try {
    const response = await fetch(`${config.url}/api/v1/traces/genai`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      logger.warn(`Agentreplay GenAI trace failed: ${response.status}`);
      return null;
    }

    const result = (await response.json()) as { edge_id?: string };
    return result.edge_id || null;
  } catch (err) {
    logger.debug?.(`Agentreplay GenAI error: ${err instanceof Error ? err.message : String(err)}`);
    return null;
  }
}

/**
 * Send tool call trace
 */
export async function sendToolTrace(
  config: AgentreplayConfig,
  logger: PluginLogger,
  payload: ToolTracePayload,
): Promise<string | null> {
  try {
    const response = await fetch(`${config.url}/api/v1/traces/tool`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      logger.warn(`Agentreplay tool trace failed: ${response.status}`);
      return null;
    }

    const result = (await response.json()) as { edge_id?: string };
    return result.edge_id || null;
  } catch (err) {
    logger.debug?.(`Agentreplay tool error: ${err instanceof Error ? err.message : String(err)}`);
    return null;
  }
}

/**
 * Send memory operation trace
 */
export async function sendMemoryTrace(
  config: AgentreplayConfig,
  logger: PluginLogger,
  payload: MemoryTracePayload,
): Promise<string | null> {
  try {
    const response = await fetch(`${config.url}/api/v1/traces/memory`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      // Fall back to generic trace endpoint if memory endpoint doesn't exist
      if (response.status === 404) {
        return sendTrace(config, logger, {
          tenant_id: payload.tenant_id,
          project_id: payload.project_id,
          agent_id: payload.agent_id,
          session_id: payload.session_id,
          span_type: 8, // Retrieval span type for memory operations
          parent_edge_id: payload.parent_edge_id,
          metadata: {
            operation: payload.operation,
            memory_id: payload.memory_id,
            text: payload.text?.substring(0, 200), // Truncate for metadata
            category: payload.category,
            importance: payload.importance,
            result_count: payload.result_count,
            score: payload.score,
            duration_ms: payload.duration_ms,
            ...payload.metadata,
          },
        });
      }
      logger.warn(`Agentreplay memory trace failed: ${response.status}`);
      return null;
    }

    const result = (await response.json()) as { edge_id?: string };
    return result.edge_id || null;
  } catch (err) {
    logger.debug?.(`Agentreplay memory error: ${err instanceof Error ? err.message : String(err)}`);
    return null;
  }
}

/**
 * Generate a unique session ID from session key
 */
export function hashSessionKey(sessionKey: string): number {
  let hash = 0;
  for (let i = 0; i < sessionKey.length; i++) {
    const char = sessionKey.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash = hash & hash;
  }
  return Math.abs(hash);
}

/**
 * Generate a unique agent ID from agent identifier
 */
export function hashAgentId(agentId: string): number {
  let hash = 5381;
  for (let i = 0; i < agentId.length; i++) {
    hash = ((hash << 5) + hash) + agentId.charCodeAt(i);
  }
  return Math.abs(hash) % 1000000;
}
