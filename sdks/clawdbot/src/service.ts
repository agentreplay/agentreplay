/**
 * Agentreplay Service
 *
 * Manages the lifecycle of Agentreplay tracing and hooks into agent events.
 */

import type { PluginApi, PluginService } from "./plugin-types.js";

import { getAgentreplayConfig } from "../index.js";
import type { AgentreplayConfig } from "./config.js";
import { sendTrace, sendToolTrace, sendMemoryTrace, hashSessionKey, hashAgentId } from "./client.js";
import { SpanType, type TraceContext, type MemoryOperation } from "./types.js";

// Track active sessions -> trace contexts
const activeTraces = new Map<string, TraceContext>();
// Track tool call start times for duration calculation
const toolStartTimes = new Map<string, number>();

export function createAgentreplayService(api: PluginApi): PluginService {
  let config: AgentreplayConfig;

  return {
    id: "agentreplay",

    async start() {
      config = getAgentreplayConfig(api);

      if (!config.enabled) {
        api.logger.info("Agentreplay service disabled");
        return;
      }

      api.logger.info(
        `Agentreplay enabled: ${config.url} (tenant=${config.tenantId}, project=${config.projectId})`,
      );

      // =========================================================================
      // before_agent_start: Called before the agent starts processing
      // =========================================================================
      api.on("before_agent_start", async (event, ctx) => {
        const sessionKey = ctx.sessionKey || "unknown";
        const agentId = ctx.agentId || "clawdbot";
        const numericAgentId = hashAgentId(agentId);
        const numericSessionId = hashSessionKey(sessionKey);

        // Create a root trace for this agent run
        const edgeId = await sendTrace(config, api.logger, {
          tenant_id: config.tenantId,
          project_id: config.projectId,
          agent_id: numericAgentId,
          session_id: numericSessionId,
          span_type: SpanType.Root,
          metadata: {
            session_key: sessionKey,
            agent_id: agentId,
            workspace_dir: ctx.workspaceDir,
            message_provider: ctx.messageProvider,
            prompt_length: event.prompt?.length || 0,
          },
        });

        if (edgeId) {
          activeTraces.set(sessionKey, {
            sessionKey,
            parentEdgeId: edgeId,
            startTime: Date.now(),
          });
          api.logger.debug?.(`Started trace ${edgeId} for session ${sessionKey}`);
        }

        return undefined;
      });

      // =========================================================================
      // agent_end: Called after the agent finishes processing
      // =========================================================================
      api.on("agent_end", async (event, ctx) => {
        const sessionKey = ctx.sessionKey || "unknown";
        const traceCtx = activeTraces.get(sessionKey);

        if (!traceCtx) return;

        const agentId = ctx.agentId || "clawdbot";
        const numericAgentId = hashAgentId(agentId);
        const numericSessionId = hashSessionKey(sessionKey);
        const durationMs = Date.now() - traceCtx.startTime;

        // Send completion trace
        await sendTrace(config, api.logger, {
          tenant_id: config.tenantId,
          project_id: config.projectId,
          agent_id: numericAgentId,
          session_id: numericSessionId,
          span_type: event.success ? SpanType.Response : SpanType.Error,
          parent_edge_id: traceCtx.parentEdgeId,
          metadata: {
            session_key: sessionKey,
            success: event.success,
            error: event.error,
            duration_ms: durationMs,
            message_count: Array.isArray(event.messages) ? event.messages.length : 0,
          },
        });

        activeTraces.delete(sessionKey);
        api.logger.debug?.(`Ended trace for session ${sessionKey} (${durationMs}ms)`);
      });

      // =========================================================================
      // before_tool_call: Called before a tool is executed
      // =========================================================================
      api.on("before_tool_call", async (event, ctx) => {
        const sessionKey = ctx.sessionKey || "unknown";
        const toolId = `${sessionKey}:${event.toolName}:${Date.now()}`;

        // Record start time for duration calculation
        toolStartTimes.set(toolId, Date.now());

        const traceCtx = activeTraces.get(sessionKey);
        const agentId = ctx.agentId || "clawdbot";
        const numericAgentId = hashAgentId(agentId);
        const numericSessionId = hashSessionKey(sessionKey);

        // Send tool start trace
        await sendToolTrace(config, api.logger, {
          tenant_id: config.tenantId,
          project_id: config.projectId,
          agent_id: numericAgentId,
          session_id: numericSessionId,
          tool_name: event.toolName,
          tool_input: event.toolInput,
          parent_edge_id: traceCtx?.parentEdgeId,
          metadata: {
            session_key: sessionKey,
            status: "started",
          },
        });

        return undefined;
      });

      // =========================================================================
      // after_tool_call: Called after a tool execution completes
      // =========================================================================
      api.on("after_tool_call", async (event, ctx) => {
        const sessionKey = ctx.sessionKey || "unknown";
        const traceCtx = activeTraces.get(sessionKey);
        const agentId = ctx.agentId || "clawdbot";
        const numericAgentId = hashAgentId(agentId);
        const numericSessionId = hashSessionKey(sessionKey);

        // Find and remove the start time entry
        let durationMs: number | undefined;
        for (const [key, startTime] of toolStartTimes.entries()) {
          if (key.startsWith(`${sessionKey}:${event.toolName}:`)) {
            durationMs = Date.now() - startTime;
            toolStartTimes.delete(key);
            break;
          }
        }

        // Send tool completion trace
        await sendToolTrace(config, api.logger, {
          tenant_id: config.tenantId,
          project_id: config.projectId,
          agent_id: numericAgentId,
          session_id: numericSessionId,
          tool_name: event.toolName,
          tool_output: event.toolResult,
          duration_ms: durationMs,
          parent_edge_id: traceCtx?.parentEdgeId,
          metadata: {
            session_key: sessionKey,
            status: event.error ? "error" : "completed",
            error: event.error,
          },
        });

        // Special handling for memory tools - send additional memory trace
        if (isMemoryTool(event.toolName)) {
          const memoryOp = getMemoryOperation(event.toolName);
          const details = extractMemoryDetails(event.toolInput, event.toolResult);

          await sendMemoryTrace(config, api.logger, {
            tenant_id: config.tenantId,
            project_id: config.projectId,
            agent_id: numericAgentId,
            session_id: numericSessionId,
            operation: memoryOp,
            memory_id: details.memoryId,
            text: details.text,
            category: details.category,
            importance: details.importance,
            result_count: details.resultCount,
            score: details.score,
            duration_ms: durationMs,
            parent_edge_id: traceCtx?.parentEdgeId,
            metadata: {
              session_key: sessionKey,
              tool_name: event.toolName,
            },
          });
        }
      });

      api.logger.info("Agentreplay hooks registered");
    },

    async stop() {
      // Clean up any remaining traces
      for (const [sessionKey, traceCtx] of activeTraces.entries()) {
        const durationMs = Date.now() - traceCtx.startTime;
        api.logger.debug?.(
          `Cleaning up unfinished trace for session ${sessionKey} (${durationMs}ms)`,
        );
      }
      activeTraces.clear();
      toolStartTimes.clear();
      api.logger.info("Agentreplay service stopped");
    },
  };
}

// =============================================================================
// Memory tool helpers
// =============================================================================

const MEMORY_TOOLS = ["memory_recall", "memory_store", "memory_forget", "memory_search", "memory_update"];

function isMemoryTool(toolName: string): boolean {
  return MEMORY_TOOLS.includes(toolName) || toolName.startsWith("memory_");
}

function getMemoryOperation(toolName: string): MemoryOperation {
  switch (toolName) {
    case "memory_recall":
    case "memory_search":
      return "recall";
    case "memory_store":
      return "store";
    case "memory_forget":
      return "delete";
    case "memory_update":
      return "update";
    default:
      return "recall";
  }
}

interface MemoryDetails {
  memoryId?: string;
  text?: string;
  category?: string;
  importance?: number;
  resultCount?: number;
  score?: number;
}

function extractMemoryDetails(input: unknown, result: unknown): MemoryDetails {
  const details: MemoryDetails = {};

  // Extract from input
  if (input && typeof input === "object") {
    const inp = input as Record<string, unknown>;
    if (typeof inp.text === "string") details.text = inp.text;
    if (typeof inp.query === "string") details.text = inp.query;
    if (typeof inp.memoryId === "string") details.memoryId = inp.memoryId;
    if (typeof inp.category === "string") details.category = inp.category;
    if (typeof inp.importance === "number") details.importance = inp.importance;
  }

  // Extract from result
  if (result && typeof result === "object") {
    const res = result as Record<string, unknown>;

    // Check for details object (common pattern in memory tools)
    const detailsObj = res.details as Record<string, unknown> | undefined;
    if (detailsObj) {
      if (typeof detailsObj.id === "string") details.memoryId = detailsObj.id;
      if (typeof detailsObj.count === "number") details.resultCount = detailsObj.count;
      if (Array.isArray(detailsObj.memories)) {
        details.resultCount = detailsObj.memories.length;
        // Get best score if available
        const firstMem = detailsObj.memories[0] as Record<string, unknown> | undefined;
        if (firstMem && typeof firstMem.score === "number") {
          details.score = firstMem.score;
        }
      }
    }
  }

  return details;
}
