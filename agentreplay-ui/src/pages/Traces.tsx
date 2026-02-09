// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  Activity,
  ArrowUpRight,
  Clock,
  Filter,
  Loader2,
  MoreVertical,
  RefreshCcw,
  Search,
  Zap,
  ChevronDown,
  ChevronRight,
  ChevronLeft,
  ChevronsLeft,
  ChevronsRight,
  DollarSign,
  Radio,
  Trash2,
  MessageCircle,
  List,
} from "lucide-react";
import { agentreplayClient } from "../lib/agentreplay-api";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { useProjects } from "../context/project-context";
import { LIVE_MODE_EVENT } from "../lib/events";
import { cn } from "../../lib/utils";
import { useSSETraces, SSETraceEvent } from "../../hooks/useSSETraces";
import MetricsCards from "../components/MetricsCards";
import { formatDistanceToNow } from "date-fns";
import { VideoHelpButton } from "../components/VideoHelpButton";
import Tooltip from "../components/Tooltip";
import CopyButton from "../components/CopyButton";

interface TraceRow {
  id: string;
  timestamp: number;
  timestampLabel: string;
  model: string;
  durationMs: number;
  cost?: number;
  tokens?: number;
  user?: string;
  status: string;
  score?: number;
  metadata?: Record<string, unknown>;
  display_name?: string;
  provider?: string;
  input_tokens?: number;
  output_tokens?: number;
  agent_name?: string;
  session_id?: string;
  // Direct preview fields from server
  input_preview?: string;
  output_preview?: string;
  // Claude Code specific fields
  tool_name?: string;
  event_type?: string;
  is_claude_code?: boolean;
  agent_id_attr?: string;
}

const PAGE_SIZE = 40;

const statusColors: Record<string, string> = {
  completed: "bg-success/15 text-success",
  error: "bg-error/15 text-error",
  running: "bg-warning/15 text-warning",
};

const timeRangeOptions = [
  { value: "1h", label: "Last hour" },
  { value: "24h", label: "Last 24h" },
  { value: "7d", label: "Last 7 days" },
  { value: "all", label: "All time" },
];

type FilterState = {
  query: string;
  user: string;
  model: string;
  agent: string;
  provider: string;
  status: string;
  timeRange: string;
};

const defaultFilters: FilterState = {
  query: "",
  user: "",
  model: "",
  agent: "",
  provider: "",
  status: "",
  timeRange: "24h",
};

function timeRangeToStart(timeRange: string) {
  if (timeRange === "all") return 0;
  const now = Date.now() * 1000; // Convert to microseconds to match server timestamps
  switch (timeRange) {
    case "1h":
      return now - 60 * 60 * 1000 * 1000;
    case "7d":
      return now - 7 * 24 * 60 * 60 * 1000 * 1000;
    case "24h":
    default:
      return now - 24 * 60 * 60 * 1000 * 1000;
  }
}

// Helper function to calculate percentiles from an array of numbers
function calculatePercentile(arr: number[], percentile: number): number {
  if (arr.length === 0) return 0;
  const sorted = [...arr].sort((a, b) => a - b);
  const index = Math.ceil((percentile / 100) * sorted.length) - 1;
  return sorted[Math.max(0, index)];
}

export default function Traces() {
  const { projectId } = useParams();
  const navigate = useNavigate();
  const { currentProject, refreshProjects } = useProjects();

  // Use projectId from URL, or fall back to currentProject from context
  const effectiveProjectId = projectId || currentProject?.project_id;

  const [rawTraces, setRawTraces] = useState<TraceRow[]>([]);
  const [totalTraceCount, setTotalTraceCount] = useState<number>(0); // Server-side total count
  const [currentPage, setCurrentPage] = useState<number>(1); // Current page for pagination
  const [filters, setFilters] = useState<FilterState>(defaultFilters);
  const [showFilters, setShowFilters] = useState(false); // Collapsible filters
  const [isLoading, setIsLoading] = useState(true);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    trace: TraceRow;
  } | null>(null);
  const [liveMode, setLiveMode] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // API Base URL for direct fetches (if client doesn't support listSessions yet)
  const API_BASE_URL = "http://127.0.0.1:47100";

  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);

  // Delete project handler
  const handleDeleteProject = async () => {
    if (!effectiveProjectId) return;

    setIsDeleting(true);
    try {
      await agentreplayClient.deleteProject(effectiveProjectId);
      setShowDeleteConfirm(false);

      // Refresh projects list - this will auto-select another project if available
      await refreshProjects();

      // Check if there are remaining projects after refresh
      // We need to re-fetch to get the updated list
      const response = await agentreplayClient.listProjects();
      const remainingProjects = response?.projects || [];

      if (remainingProjects.length === 0) {
        // No projects left, navigate to create project page
        navigate("/projects/new");
      }
      // If there are remaining projects, refreshProjects already selected one
      // and we stay on this page showing traces for the newly selected project
    } catch (err) {
      console.error("Failed to delete project:", err);
      setError(err instanceof Error ? err.message : "Failed to delete project");
      setShowDeleteConfirm(false);
    } finally {
      setIsDeleting(false);
    }
  };

  // Cancel delete - just close the modal
  const handleCancelDelete = () => {
    setShowDeleteConfirm(false);
  };

  // SSE streaming for real-time updates (replaces polling)
  const handleSSETrace = useCallback(
    (sseTrace: SSETraceEvent) => {
      // Filter by project if specified
      if (
        effectiveProjectId &&
        sseTrace.project_id !== parseInt(effectiveProjectId)
      ) {
        return;
      }

      // Convert SSE trace event to TraceRow format
      const newTrace: TraceRow = {
        id: sseTrace.span_id,
        timestamp: sseTrace.timestamp_us / 1000,
        timestampLabel: new Date(sseTrace.timestamp_us / 1000).toLocaleString(),
        model: sseTrace.span_type || "Unknown",
        durationMs: (sseTrace.duration_us || 0) / 1000,
        cost: 0,
        tokens: sseTrace.token_count || 0,
        user: sseTrace.session_id?.toString() || "anonymous",
        status: "completed",
        display_name: sseTrace.span_type,
        session_id: sseTrace.session_id?.toString(),
      };

      // Prepend new trace (most recent first)
      setRawTraces((prev) => {
        const existingIndex = prev.findIndex((t) => t.id === newTrace.id);

        let updated: TraceRow[];
        if (existingIndex >= 0) {
          // Update existing item in place
          updated = [...prev];
          updated[existingIndex] = newTrace;
        } else {
          // Prepend new item
          updated = [newTrace, ...prev];
        }

        // Keep only first 200 traces to prevent memory bloat
        if (updated.length > 200) {
          return updated.slice(0, 200);
        }
        return updated;
      });
    },
    [effectiveProjectId],
  );

  const {
    connected: sseConnected,
    connecting: sseConnecting,
    error: sseError,
  } = useSSETraces({
    enabled: liveMode,
    maxTraces: 100,
    onTrace: handleSSETrace,
    onLag: (skipped) => console.warn("SSE lagged, skipped", skipped, "traces"),
  });

  // No session grouping in flat trace view
  const visibleTraces = useMemo(() => {
    return rawTraces.filter((trace) => {
      if (filters.query) {
        const q = filters.query.toLowerCase();
        const searchable = [
          trace.id,
          trace.display_name,
          trace.input_preview,
          trace.output_preview,
          trace.tool_name,
          trace.model,
          trace.agent_name,
          trace.event_type,
        ]
          .filter(Boolean)
          .join(" ")
          .toLowerCase();
        if (!searchable.includes(q)) {
          return false;
        }
      }
      if (
        filters.user &&
        !trace.user?.toLowerCase().includes(filters.user.toLowerCase())
      ) {
        return false;
      }
      if (
        filters.model &&
        !trace.model.toLowerCase().includes(filters.model.toLowerCase())
      ) {
        return false;
      }
      if (
        filters.agent &&
        !trace.agent_name?.toLowerCase().includes(filters.agent.toLowerCase())
      ) {
        return false;
      }
      if (
        filters.provider &&
        !trace.provider?.toLowerCase().includes(filters.provider.toLowerCase())
      ) {
        return false;
      }
      if (
        filters.status &&
        filters.status !== "all" &&
        trace.status !== filters.status
      ) {
        return false;
      }
      return true;
    });
  }, [rawTraces, filters]);

  const fetchPage = useCallback(
    async (page: number = 1, reset = false) => {
      // Don't fetch if no project is selected
      if (!effectiveProjectId) {
        setRawTraces([]);
        setIsLoading(false);
        return;
      }

      setError(null);
      setIsLoading(true);

      const offset = (page - 1) * PAGE_SIZE;

      try {
        // Fetch FLAT TRACES
        console.log(
          "🔍 Fetching traces for project:",
          effectiveProjectId,
          "page:",
          page,
          "offset:",
          offset,
        );
        const response = await agentreplayClient.listTraces({
          limit: PAGE_SIZE,
          offset: offset,
          start_time: timeRangeToStart(filters.timeRange),
          project_id: parseInt(effectiveProjectId),
          // TODO: parent_span_id: null if backend supports it for filtering roots
        });

        console.log("📦 API Response:", response);

        if (reset && response.total !== undefined) {
          setTotalTraceCount(response.total);
        }

        const mapped: TraceRow[] = (response.traces || [])
          .map((trace: any) => {
            const meta = trace.metadata || {};

            // Extract Claude Code fields from metadata (Tauri list_traces stores raw attributes in metadata)
            const agentIdAttr = trace.agent_id_attr || meta["agent_id"] || "";
            const isClaudeCode =
              trace.is_claude_code || agentIdAttr === "claude-code";
            const toolName = trace.tool_name || meta["tool.name"] || "";
            const eventType = trace.event_type || meta["event.type"] || "";

            // Extract model from multiple possible sources
            const modelFromMetadata =
              meta["gen_ai.request.model"] ||
              meta["gen_ai.response.model"] ||
              meta["model"] ||
              meta["llm.model"];
            // For Claude Code traces, show tool_name in model column; for agents show LLM model
            const model = isClaudeCode
              ? toolName || eventType || ""
              : trace.model || modelFromMetadata || "";

            // Build display name from available fields
            let displayName = trace.display_name;
            if (
              !displayName ||
              /^\d+$/.test(displayName) ||
              displayName === "chain.unknown"
            ) {
              if (isClaudeCode && toolName) {
                displayName = `Tool Call (${toolName})`;
              } else if (isClaudeCode && eventType) {
                displayName =
                  eventType === "session_start"
                    ? "Session Start"
                    : eventType === "session_end"
                      ? "Session End"
                      : eventType;
              } else {
                displayName =
                  trace.operation_name ||
                  model ||
                  trace.agent_name ||
                  displayName;
                if (displayName === "chain.unknown") {
                  displayName = trace.agent_name
                    ? `Agent: ${trace.agent_name}`
                    : "LangGraph Workflow";
                }
              }
            }

            // Extract input/output preview - also check tool.input/tool.output in metadata
            const inputPreview =
              trace.input_preview || meta["tool.input"] || "";
            const outputPreview =
              trace.output_preview || meta["tool.output"] || "";

            return {
              id: trace.trace_id || trace.span_id || "unknown",
              timestamp: (trace.started_at || trace.timestamp_us || 0) / 1000,
              timestampLabel: new Date(
                (trace.started_at || trace.timestamp_us || 0) / 1000,
              ).toLocaleString(),
              model: model || "",
              durationMs:
                trace.duration_ms ||
                (trace.duration_us ? trace.duration_us / 1000 : 0) ||
                0,
              cost: trace.cost || 0,
              tokens: trace.tokens || trace.token_count || 0,
              user: trace.session_id?.toString() || "anonymous",
              status: trace.status || "completed",
              score: meta.score,
              metadata: meta,
              display_name: displayName,
              provider: trace.provider,
              input_tokens:
                meta.input_tokens ||
                parseInt(meta["gen_ai.usage.input_tokens"]) ||
                0,
              output_tokens:
                meta.output_tokens ||
                parseInt(meta["gen_ai.usage.output_tokens"]) ||
                0,
              agent_name: trace.agent_name,
              session_id: trace.session_id?.toString(),
              input_preview: inputPreview,
              output_preview: outputPreview,
              // Claude Code specific
              tool_name: toolName,
              event_type: eventType,
              is_claude_code: isClaudeCode,
              agent_id_attr: agentIdAttr,
            };
          })
          .filter((trace: TraceRow) => {
            const hasId = trace.id && trace.id !== "unknown";
            const hasModel = trace.model && trace.model.length > 0;
            const hasTokens = trace.tokens && trace.tokens > 0;
            const hasContent = trace.input_preview || trace.output_preview;
            const hasDuration = trace.durationMs > 0;
            const hasDisplayName =
              trace.display_name && trace.display_name.length > 0;
            return (
              hasId &&
              (hasModel ||
                hasTokens ||
                hasContent ||
                hasDuration ||
                hasDisplayName)
            );
          });

        // Trust the server-side project_id filtering
        setRawTraces(mapped);
        setCurrentPage(page);
      } catch (err) {
        console.error("Failed to load data", err);
        setError(err instanceof Error ? err.message : "Failed to load data");
      } finally {
        setIsLoading(false);
      }
    },
    [filters.timeRange, effectiveProjectId],
  );

  // Fetch initial data and refetch when project changes
  useEffect(() => {
    // Don't fetch if no project is selected
    if (!effectiveProjectId) {
      setRawTraces([]);
      setIsLoading(false);
      return;
    }

    // Clear existing traces when project changes
    setRawTraces([]);
    setCurrentPage(1);
    setIsLoading(true);

    // Fetch new data
    fetchPage(1, true).catch((err) => {
      console.error("Failed to fetch traces on project change:", err);
    });
  }, [effectiveProjectId, filters.timeRange]); // eslint-disable-line react-hooks/exhaustive-deps

  // Live mode now uses SSE streaming instead of polling!
  // SSE is handled by useSSETraces hook above

  useEffect(() => {
    const handler = () => setLiveMode((prev) => !prev);
    window.addEventListener(LIVE_MODE_EVENT, handler);
    return () => window.removeEventListener(LIVE_MODE_EVENT, handler);
  }, []);

  useEffect(() => {
    const dismiss = () => setContextMenu(null);
    window.addEventListener("click", dismiss);
    return () => window.removeEventListener("click", dismiss);
  }, []);

  const handleContextMenu = (event: React.MouseEvent, trace: TraceRow) => {
    event.preventDefault();
    setContextMenu({ x: event.clientX, y: event.clientY, trace });
  };

  const copyTraceId = async (traceId: string) => {
    try {
      await navigator.clipboard.writeText(traceId);
    } catch (err) {
      console.warn("Clipboard unavailable", err);
    }
    setContextMenu(null);
  };

  const TraceRowItem = ({
    trace,
    isGroup = false,
    isExpanded = false,
    onToggle,
  }: {
    trace: TraceRow;
    isGroup?: boolean;
    isExpanded?: boolean;
    onToggle?: () => void;
  }) => {
    const navigate = useNavigate();
    const { projectId } = useParams();

    const handleClick = (e: React.MouseEvent) => {
      if (isGroup && onToggle) {
        e.stopPropagation();
        onToggle();
      } else if (trace.metadata?.type === "session") {
        // Navigate to session detail if it's a session row
        navigate(`/projects/${projectId}/sessions/${trace.session_id}`);
      } else {
        navigate(`/projects/${projectId}/traces/${trace.id}`);
      }
    };

    // Color coding for latency
    // Color coding for latency
    const latencyColor =
      trace.durationMs < 1000
        ? "text-success"
        : trace.durationMs < 5000
          ? "text-warning"
          : "text-error";

    // Color coding for cost
    const costColor =
      (trace.cost || 0) > 0.01
        ? "text-error"
        : (trace.cost || 0) > 0.001
          ? "text-warning"
          : "text-textSecondary";

    // Format duration for tooltip
    const durationText =
      trace.durationMs >= 1000
        ? `${(trace.durationMs / 1000).toFixed(2)} seconds`
        : `${trace.durationMs.toFixed(0)} milliseconds`;

    // Status icon
    const statusIcon =
      trace.status === "completed" ? "✓" : trace.status === "error" ? "✗" : "⋯";

    // Extract input/output from metadata - check multiple possible paths
    const inputPreview = useMemo(() => {
      // Priority 1: Direct input_preview from server (pre-extracted)
      if ((trace as any).input_preview) {
        return String((trace as any).input_preview);
      }
      // Priority 2: OpenTelemetry gen_ai.prompt attributes (check indices 0, 1, 2)
      const metadata = (trace.metadata as Record<string, any>) || {};
      for (let i = 0; i <= 2; i++) {
        const roleKey = `gen_ai.prompt.${i}.role`;
        const contentKey = `gen_ai.prompt.${i}.content`;
        const role = metadata[roleKey];
        const content = metadata[contentKey];
        // Skip system messages, prefer user messages
        if (content && role !== "system") {
          return String(content);
        }
      }
      // Priority 3: Prompts array in metadata
      if (
        trace.metadata?.prompts &&
        Array.isArray(trace.metadata.prompts) &&
        trace.metadata.prompts.length > 0
      ) {
        // Find first user message or use first prompt
        const userPrompt =
          (trace.metadata.prompts as any[]).find((p) => p.role === "user") ||
          trace.metadata.prompts[0];
        return String(userPrompt?.content || "");
      }
      // Priority 4: Direct input field
      if (trace.metadata?.input) {
        return String(trace.metadata.input);
      }
      // Priority 5: Messages array (common format)
      if (trace.metadata?.messages && Array.isArray(trace.metadata.messages)) {
        const userMsg = (trace.metadata.messages as any[]).find(
          (m) => m.role === "user",
        );
        if (userMsg?.content) return String(userMsg.content);
      }
      return "";
    }, [trace.metadata, trace]);

    const outputPreview = useMemo(() => {
      // Priority 1: Direct output_preview from server (pre-extracted)
      if ((trace as any).output_preview) {
        return String((trace as any).output_preview);
      }
      // Priority 2: OpenTelemetry gen_ai.completion attributes
      const metadata = (trace.metadata as Record<string, any>) || {};
      if (metadata["gen_ai.completion.0.content"]) {
        return String(metadata["gen_ai.completion.0.content"]);
      }
      // Priority 3: Completions array in metadata
      if (
        trace.metadata?.completions &&
        Array.isArray(trace.metadata.completions) &&
        trace.metadata.completions.length > 0
      ) {
        return String((trace.metadata.completions as any[])[0]?.content || "");
      }
      // Priority 4: Direct output field
      if (trace.metadata?.output) {
        return String(trace.metadata.output);
      }
      // Priority 5: Response/completion field
      if (trace.metadata?.response) {
        return String(trace.metadata.response);
      }
      if (trace.metadata?.completion) {
        return String(trace.metadata.completion);
      }
      return "";
    }, [trace.metadata, trace]);

    const modelName = useMemo(() => {
      // For Claude Code traces, show tool name or event type with a badge-like prefix
      if (trace.is_claude_code) {
        if (trace.tool_name) return trace.tool_name;
        if (trace.event_type) return trace.event_type.replace(/_/g, " ");
        return trace.display_name || "Claude Code";
      }

      const metadata = (trace.metadata as Record<string, any>) || {};
      // Priority 1: OpenTelemetry gen_ai.request.model or gen_ai.response.model
      if (metadata["gen_ai.request.model"]) {
        return String(metadata["gen_ai.request.model"]);
      }
      if (metadata["gen_ai.response.model"]) {
        return String(metadata["gen_ai.response.model"]);
      }
      // Priority 2: Direct model field in metadata
      if (trace.metadata?.model) {
        return String(trace.metadata.model);
      }
      // Priority 3: Top-level model or display_name
      return trace.model || (trace as any).display_name || "Unknown";
    }, [
      trace.metadata,
      trace.model,
      trace.is_claude_code,
      trace.tool_name,
      trace.event_type,
      trace.display_name,
    ]);

    return (
      <div
        onClick={handleClick}
        onContextMenu={(event) => handleContextMenu(event, trace)}
        className={cn(
          "group grid cursor-pointer grid-cols-[minmax(140px,1fr)_minmax(120px,1fr)_minmax(180px,2fr)_minmax(180px,2fr)_80px_80px_100px_80px] items-center border-b border-border/50 px-4 py-3 text-sm text-textPrimary transition hover:bg-surface-hover",
          isGroup && "bg-surface-hover/50 font-medium",
        )}
      >
        {/* Trace ID with tooltip and copy button */}
        <div className="flex items-center gap-2">
          {isGroup ? (
            <div className="flex items-center justify-center w-4 h-4 mr-1">
              {isExpanded ? (
                <ChevronDown className="h-4 w-4 text-textSecondary" />
              ) : (
                <ChevronRight className="h-4 w-4 text-textSecondary" />
              )}
            </div>
          ) : (
            <Activity className="h-4 w-4 text-primary" />
          )}
          <div>
            <Tooltip
              content={
                <div className="flex items-center gap-2">
                  <span className="font-mono">{trace.id}</span>
                  <CopyButton
                    value={trace.id}
                    size="sm"
                    className="opacity-100"
                  />
                </div>
              }
            >
              <p className="font-mono text-xs">{trace.id.slice(0, 10)}…</p>
            </Tooltip>
            <Tooltip content={new Date(trace.timestamp).toLocaleString()}>
              <p className="text-xs text-textTertiary">
                {formatDistanceToNow(trace.timestamp, { addSuffix: true })}
              </p>
            </Tooltip>
          </div>
        </div>

        {/* Model Name / Tool Name */}
        <Tooltip
          content={
            trace.is_claude_code
              ? `Tool: ${trace.tool_name || "N/A"}\nType: ${trace.display_name || trace.event_type || "Unknown"}\nAgent: Claude Code`
              : `Model: ${modelName}\nProvider: ${trace.provider || "Unknown"}`
          }
        >
          <div className="truncate font-medium">
            {trace.is_claude_code ? (
              <span className="flex items-center gap-1">
                <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold bg-purple-500/15 text-purple-400 uppercase tracking-wide">
                  {trace.tool_name ||
                    trace.event_type?.replace(/_/g, " ") ||
                    "⚡"}
                </span>
              </span>
            ) : (
              <span className="text-primary">{modelName}</span>
            )}
          </div>
        </Tooltip>

        {/* Input Preview */}
        <Tooltip
          content={
            inputPreview ? (
              <div className="max-w-lg">
                <div className="text-xs font-semibold text-primary mb-1">
                  📥 Full Input:
                </div>
                <div className="text-xs bg-surface/50 rounded p-2 max-h-48 overflow-y-auto whitespace-pre-wrap">
                  {inputPreview.substring(0, 500)}
                  {inputPreview.length > 500 && "..."}
                </div>
              </div>
            ) : (
              "No input captured"
            )
          }
        >
          <div className="line-clamp-2 text-xs text-textSecondary break-words">
            {inputPreview ? (
              <span className="opacity-80">{inputPreview}</span>
            ) : (
              <span className="text-textTertiary italic">—</span>
            )}
          </div>
        </Tooltip>

        {/* Output Preview */}
        <Tooltip
          content={
            outputPreview ? (
              <div className="max-w-lg">
                <div className="text-xs font-semibold text-success mb-1">
                  📤 Full Output:
                </div>
                <div className="text-xs bg-surface/50 rounded p-2 max-h-48 overflow-y-auto whitespace-pre-wrap">
                  {outputPreview.substring(0, 500)}
                  {outputPreview.length > 500 && "..."}
                </div>
              </div>
            ) : (
              "No output captured"
            )
          }
        >
          <div className="line-clamp-2 text-xs text-textSecondary break-words">
            {outputPreview ? (
              <span className="opacity-80">{outputPreview}</span>
            ) : (
              <span className="text-textTertiary italic">—</span>
            )}
          </div>
        </Tooltip>

        {/* Latency with prominent display and color coding */}
        <Tooltip content={durationText}>
          <div className={cn("text-sm font-bold tabular-nums", latencyColor)}>
            {trace.durationMs >= 1000
              ? `${(trace.durationMs / 1000).toFixed(1)}s`
              : `${trace.durationMs.toFixed(0)}ms`}
          </div>
        </Tooltip>

        {/* Cost with color coding */}
        <Tooltip content={`Estimated cost based on token usage`}>
          <div className={cn("font-medium text-xs", costColor)}>
            ${trace.cost?.toFixed(4) ?? "0.00"}
          </div>
        </Tooltip>

        {/* Tokens with input/output breakdown */}
        <Tooltip
          content={
            trace.input_tokens || trace.output_tokens ? (
              <div className="text-xs">
                <div>Input: {(trace.input_tokens || 0).toLocaleString()}</div>
                <div>Output: {(trace.output_tokens || 0).toLocaleString()}</div>
                <div className="mt-1 border-t border-border/30 pt-1 font-semibold">
                  Total: {(trace.tokens || 0).toLocaleString()}
                </div>
              </div>
            ) : (
              "No token data"
            )
          }
        >
          <div className="font-mono text-xs text-textSecondary">
            {trace.tokens?.toLocaleString() ?? "—"}
          </div>
        </Tooltip>

        {/* Status with icon */}
        <div className="flex justify-end">
          <Tooltip
            content={
              trace.status === "error" ? (
                <div className="max-w-sm">
                  <div className="font-semibold text-error mb-1">
                    Error Details:
                  </div>
                  <div className="text-xs">
                    {String(
                      trace.metadata?.error ||
                        trace.metadata?.error_message ||
                        "Error occurred",
                    )}
                  </div>
                </div>
              ) : (
                `Status: ${trace.status}`
              )
            }
          >
            <span
              className={cn(
                "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium",
                statusColors[trace.status] || "bg-muted/40 text-textSecondary",
              )}
            >
              <span>{statusIcon}</span>
            </span>
          </Tooltip>
        </div>
      </div>
    );
  };

  return (
    <div className="flex h-full flex-col gap-6">
      {/* Delete Confirmation Modal */}
      {showDeleteConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
          <div className="bg-surface border border-border rounded-xl p-6 max-w-md w-full mx-4 shadow-xl">
            <div className="flex items-center gap-3 mb-4">
              <div className="p-2 rounded-full bg-red-500/10">
                <Trash2 className="h-6 w-6 text-red-500" />
              </div>
              <h3 className="text-lg font-semibold text-textPrimary">
                Delete Project
              </h3>
            </div>
            <p className="text-textSecondary mb-2">
              Are you sure you want to delete{" "}
              <strong className="text-textPrimary">
                {currentProject?.name || "this project"}
              </strong>
              ?
            </p>
            <p className="text-sm text-textTertiary mb-6">
              This will permanently delete all traces, sessions, and data for
              this project. This action cannot be undone.
            </p>
            <div className="flex gap-3 justify-end">
              <Button
                variant="outline"
                size="sm"
                onClick={() => setShowDeleteConfirm(false)}
                disabled={isDeleting}
              >
                Cancel
              </Button>
              <Button
                variant="destructive"
                size="sm"
                onClick={handleDeleteProject}
                disabled={isDeleting}
                className="bg-red-600 hover:bg-red-700 text-white gap-2"
              >
                {isDeleting ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    Deleting...
                  </>
                ) : (
                  <>
                    <Trash2 className="h-4 w-4" />
                    Delete Project
                  </>
                )}
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* Compact Header + Metrics Row */}
      <header className="flex items-center justify-between gap-4">
        <div className="flex items-center gap-4">
          <div>
            <h1 className="text-xl font-semibold text-textPrimary">
              {currentProject?.name || "Traces"}
            </h1>
          </div>
          {/* Inline Metrics */}
          <div className="flex items-center gap-4 text-sm">
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface/70 border border-border/40">
              <Activity className="h-4 w-4 text-primary" />
              <span className="font-semibold text-textPrimary">
                {visibleTraces.length.toLocaleString()}
              </span>
              <span className="text-textTertiary">traces</span>
            </div>
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface/70 border border-border/40">
              <Clock className="h-4 w-4 text-warning" />
              <span className="font-semibold text-textPrimary">
                {(() => {
                  const latencies = visibleTraces
                    .map((t) => t.durationMs)
                    .filter((d) => d > 0);
                  return latencies.length
                    ? `${Math.round(calculatePercentile(latencies, 50))}ms`
                    : "—";
                })()}
              </span>
              <span className="text-textTertiary">p50</span>
            </div>
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface/70 border border-border/40">
              <DollarSign className="h-4 w-4 text-success" />
              <span className="font-semibold text-textPrimary">
                $
                {visibleTraces
                  .reduce((sum, trace) => sum + (trace.cost || 0), 0)
                  .toFixed(2)}
              </span>
              <span className="text-textTertiary">spent</span>
            </div>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="sm"
            className={cn(
              "gap-1.5",
              liveMode && sseConnected
                ? "text-success"
                : liveMode && sseConnecting
                  ? "text-warning"
                  : liveMode && sseError
                    ? "text-error"
                    : "text-textSecondary",
            )}
            onClick={() => setLiveMode((prev) => !prev)}
            title={
              liveMode
                ? sseConnected
                  ? "Connected via SSE"
                  : "Connecting..."
                : "Enable live updates"
            }
          >
            {sseConnecting ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : sseConnected ? (
              <Radio className="h-4 w-4 animate-pulse" />
            ) : (
              <Zap className="h-4 w-4" />
            )}
            {liveMode ? "Live" : "Live"}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => fetchPage(currentPage, true)}
            className="gap-1.5"
          >
            {isLoading ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <RefreshCcw className="h-4 w-4" />
            )}
          </Button>
          <VideoHelpButton pageId="traces" />
          {effectiveProjectId && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowDeleteConfirm(true)}
              className="text-red-500 hover:text-red-400 hover:bg-red-500/10"
              title="Delete project"
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          )}
        </div>
      </header>

      <section className="rounded-2xl border border-border/50 bg-surface/80 p-3">
        {/* Main filter row - always visible */}
        <div className="flex items-center gap-3">
          {/* Search */}
          <div className="flex-1 flex items-center gap-2 rounded-xl border border-border/60  px-3 py-2">
            <Search className="h-4 w-4 text-textTertiary" />
            <Input
              value={filters.query}
              onChange={(event) =>
                setFilters((prev) => ({ ...prev, query: event.target.value }))
              }
              placeholder="Search traces..."
              className="border-none bg-transparent px-0 py-0 h-auto focus-visible:ring-0 text-sm"
            />
          </div>

          {/* Time Range - always visible */}
          <select
            value={filters.timeRange}
            onChange={(event) =>
              setFilters((prev) => ({ ...prev, timeRange: event.target.value }))
            }
            className="rounded-xl border border-border/60 bg-background px-3 py-2 text-sm focus:outline-none"
          >
            {timeRangeOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>

          {/* Status - always visible */}
          <select
            value={filters.status}
            onChange={(event) =>
              setFilters((prev) => ({ ...prev, status: event.target.value }))
            }
            className="rounded-xl border border-border/60 bg-background px-3 py-2 text-sm focus:outline-none"
          >
            <option value="all">All</option>
            <option value="completed">✓ OK</option>
            <option value="error">✗ Errors</option>
          </select>

          {/* Filters toggle */}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setShowFilters(!showFilters)}
            className={cn("gap-1", showFilters && "bg-primary/10 text-primary")}
          >
            <Filter className="h-4 w-4" />
            Filters
            {showFilters ? (
              <ChevronDown className="h-3 w-3" />
            ) : (
              <ChevronRight className="h-3 w-3" />
            )}
          </Button>
        </div>

        {/* Advanced filters - collapsible */}
        {showFilters && (
          <div className="mt-3 pt-3 border-t border-border/40 grid gap-3 md:grid-cols-5">
            <div>
              <label className="text-xs font-medium text-textTertiary">
                Session
              </label>
              <Input
                value={filters.user}
                onChange={(event) =>
                  setFilters((prev) => ({ ...prev, user: event.target.value }))
                }
                placeholder="Session ID"
                className="mt-1 rounded-xl border-border/60 text-sm"
              />
            </div>
            <div>
              <label className="text-xs font-medium text-textTertiary">
                Model
              </label>
              <Input
                value={filters.model}
                onChange={(event) =>
                  setFilters((prev) => ({ ...prev, model: event.target.value }))
                }
                placeholder="gpt-4o, llama..."
                className="mt-1 rounded-xl border-border/60 text-sm"
              />
            </div>
            <div>
              <label className="text-xs font-medium text-textTertiary">
                Agent
              </label>
              <Input
                value={filters.agent}
                onChange={(event) =>
                  setFilters((prev) => ({ ...prev, agent: event.target.value }))
                }
                placeholder="Agent name"
                className="mt-1 rounded-xl border-border/60 text-sm"
              />
            </div>
            <div>
              <label className="text-xs font-medium text-textTertiary">
                Provider
              </label>
              <Input
                value={filters.provider}
                onChange={(event) =>
                  setFilters((prev) => ({
                    ...prev,
                    provider: event.target.value,
                  }))
                }
                placeholder="OpenAI..."
                className="mt-1 rounded-xl border-border/60 text-sm"
              />
            </div>
          </div>
        )}

        {error && (
          <div className="mt-3 rounded-xl border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-200">
            {error}
          </div>
        )}
      </section>

      <section className="flex flex-1 flex-col rounded-3xl border border-border/60 bg-surface ">
        <div className="grid grid-cols-[minmax(140px,1fr)_minmax(120px,1fr)_minmax(180px,2fr)_minmax(180px,2fr)_80px_80px_100px_80px] border-b border-border/60 px-4 py-2 text-xs font-semibold uppercase tracking-widest text-textTertiary">
          <span>Trace / Time</span>
          <span>Model</span>
          <span>Input</span>
          <span>Output</span>
          <span>Duration</span>
          <span>Cost</span>
          <span>Tokens</span>
          <span className="text-right">Status</span>
        </div>
        <div className="flex-1 min-h-[280px]">
          {isLoading ? (
            <div className="flex h-full items-center justify-center text-textSecondary">
              <Loader2 className="h-5 w-5 animate-spin" /> Loading traces…
            </div>
          ) : visibleTraces.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center gap-2 text-textSecondary">
              <Filter className="h-6 w-6" />
              <p>No traces match your filters.</p>
            </div>
          ) : (
            <div className="flex h-full flex-col overflow-y-auto">
              {visibleTraces.map((trace, index) => (
                <TraceRowItem key={`${trace.id}-${index}`} trace={trace} />
              ))}
            </div>
          )}
        </div>

        {/* Pagination Controls */}
        {totalTraceCount > PAGE_SIZE && (
          <div className="flex items-center justify-between border-t border-border/60 px-4 py-3">
            <div className="text-sm text-textSecondary">
              Showing {(currentPage - 1) * PAGE_SIZE + 1} -{" "}
              {Math.min(currentPage * PAGE_SIZE, totalTraceCount)} of{" "}
              {totalTraceCount.toLocaleString()} traces
            </div>
            <div className="flex items-center gap-2">
              {/* First Page */}
              <Button
                variant="outline"
                size="sm"
                onClick={() => fetchPage(1)}
                disabled={currentPage === 1 || isLoading}
                className="h-8 w-8 p-0"
              >
                <ChevronsLeft className="h-4 w-4" />
              </Button>

              {/* Previous Page */}
              <Button
                variant="outline"
                size="sm"
                onClick={() => fetchPage(currentPage - 1)}
                disabled={currentPage === 1 || isLoading}
                className="h-8 w-8 p-0"
              >
                <ChevronLeft className="h-4 w-4" />
              </Button>

              {/* Page Numbers */}
              <div className="flex items-center gap-1">
                {(() => {
                  const totalPages = Math.ceil(totalTraceCount / PAGE_SIZE);
                  const pages: (number | string)[] = [];

                  // Always show first page
                  pages.push(1);

                  // Show ellipsis if current page is far from start
                  if (currentPage > 3) {
                    pages.push("...");
                  }

                  // Show pages around current
                  for (
                    let i = Math.max(2, currentPage - 1);
                    i <= Math.min(totalPages - 1, currentPage + 1);
                    i++
                  ) {
                    if (!pages.includes(i)) {
                      pages.push(i);
                    }
                  }

                  // Show ellipsis if current page is far from end
                  if (currentPage < totalPages - 2) {
                    pages.push("...");
                  }

                  // Always show last page if more than 1 page
                  if (totalPages > 1 && !pages.includes(totalPages)) {
                    pages.push(totalPages);
                  }

                  return pages.map((page, idx) =>
                    typeof page === "number" ? (
                      <Button
                        key={page}
                        variant={page === currentPage ? "default" : "outline"}
                        size="sm"
                        onClick={() => fetchPage(page)}
                        disabled={isLoading}
                        className={cn(
                          "h-8 min-w-[32px] px-2",
                          page === currentPage &&
                            "bg-primary text-primary-foreground",
                        )}
                      >
                        {page}
                      </Button>
                    ) : (
                      <span
                        key={`ellipsis-${idx}`}
                        className="px-1 text-textSecondary"
                      >
                        ...
                      </span>
                    ),
                  );
                })()}
              </div>

              {/* Next Page */}
              <Button
                variant="outline"
                size="sm"
                onClick={() => fetchPage(currentPage + 1)}
                disabled={
                  currentPage >= Math.ceil(totalTraceCount / PAGE_SIZE) ||
                  isLoading
                }
                className="h-8 w-8 p-0"
              >
                <ChevronRight className="h-4 w-4" />
              </Button>

              {/* Last Page */}
              <Button
                variant="outline"
                size="sm"
                onClick={() =>
                  fetchPage(Math.ceil(totalTraceCount / PAGE_SIZE))
                }
                disabled={
                  currentPage >= Math.ceil(totalTraceCount / PAGE_SIZE) ||
                  isLoading
                }
                className="h-8 w-8 p-0"
              >
                <ChevronsRight className="h-4 w-4" />
              </Button>
            </div>
          </div>
        )}
      </section>

      {contextMenu && (
        <div
          className="fixed z-50 min-w-[200px] rounded-lg border border-border/60 bg-surface shadow-2xl"
          style={{ top: contextMenu.y, left: contextMenu.x }}
        >
          <button
            className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm hover:bg-surface-hover"
            onClick={() => copyTraceId(contextMenu.trace.id)}
          >
            <MoreVertical className="h-4 w-4" /> Copy trace ID
          </button>
          <button
            className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm hover:bg-surface-hover"
            onClick={() => {
              navigate(
                `/projects/${projectId}/traces/${contextMenu.trace.id}?action=replay`,
              );
              setContextMenu(null);
            }}
          >
            <Clock className="h-4 w-4" /> Replay trace
          </button>
          <button
            className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm hover:bg-surface-hover"
            onClick={() => {
              console.log("Add to dataset", contextMenu.trace.id);
              setContextMenu(null);
            }}
          >
            <Activity className="h-4 w-4" /> Add to dataset
          </button>
        </div>
      )}
    </div>
  );
}
