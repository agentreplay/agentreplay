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

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
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
  AlertCircle,
} from 'lucide-react';
import { agentreplayClient } from '../lib/agentreplay-api';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
import { useProjects } from '../context/project-context';
import { LIVE_MODE_EVENT } from '../lib/events';
import { cn } from '../../lib/utils';
import { useSSETraces, SSETraceEvent } from '../../hooks/useSSETraces';
import MetricsCards from '../components/MetricsCards';
import { formatDistanceToNow } from 'date-fns';
import { VideoHelpButton } from '../components/VideoHelpButton';
import Tooltip from '../components/Tooltip';
import CopyButton from '../components/CopyButton';

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
  completed: 'bg-success/15 text-success',
  error: 'bg-error/15 text-error',
  running: 'bg-warning/15 text-warning',
};

const timeRangeOptions = [
  { value: '1h', label: 'Last hour' },
  { value: '24h', label: 'Last 24h' },
  { value: '7d', label: 'Last 7 days' },
  { value: 'all', label: 'All time' },
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
  query: '',
  user: '',
  model: '',
  agent: '',
  provider: '',
  status: '',
  timeRange: '24h',
};

function timeRangeToStart(timeRange: string) {
  if (timeRange === 'all') return 0;
  const now = Date.now() * 1000; // Convert to microseconds to match server timestamps
  switch (timeRange) {
    case '1h':
      return now - 60 * 60 * 1000 * 1000;
    case '7d':
      return now - 7 * 24 * 60 * 60 * 1000 * 1000;
    case '24h':
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
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; trace: TraceRow } | null>(null);
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
        navigate('/projects/new');
      }
      // If there are remaining projects, refreshProjects already selected one
      // and we stay on this page showing traces for the newly selected project

    } catch (err) {
      console.error('Failed to delete project:', err);
      setError(err instanceof Error ? err.message : 'Failed to delete project');
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
  const handleSSETrace = useCallback((sseTrace: SSETraceEvent) => {
    // Filter by project if specified
    if (effectiveProjectId && sseTrace.project_id !== parseInt(effectiveProjectId)) {
      return;
    }

    // Convert SSE trace event to TraceRow format
    const newTrace: TraceRow = {
      id: sseTrace.span_id,
      timestamp: sseTrace.timestamp_us / 1000,
      timestampLabel: new Date(sseTrace.timestamp_us / 1000).toLocaleString(),
      model: sseTrace.span_type || 'Unknown',
      durationMs: (sseTrace.duration_us || 0) / 1000,
      cost: 0,
      tokens: sseTrace.token_count || 0,
      user: sseTrace.session_id?.toString() || 'anonymous',
      status: 'completed',
      display_name: sseTrace.span_type,
      session_id: sseTrace.session_id?.toString(),
    };

    // Prepend new trace (most recent first)
    setRawTraces((prev) => {
      const existingIndex = prev.findIndex(t => t.id === newTrace.id);

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
  }, [effectiveProjectId]);

  const { connected: sseConnected, connecting: sseConnecting, error: sseError } = useSSETraces({
    enabled: liveMode,
    maxTraces: 100,
    onTrace: handleSSETrace,
    onLag: (skipped) => console.warn('SSE lagged, skipped', skipped, 'traces'),
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
        ].filter(Boolean).join(' ').toLowerCase();
        if (!searchable.includes(q)) {
          return false;
        }
      }
      if (filters.user && !trace.user?.toLowerCase().includes(filters.user.toLowerCase())) {
        return false;
      }
      if (filters.model && !trace.model.toLowerCase().includes(filters.model.toLowerCase())) {
        return false;
      }
      if (filters.agent && !trace.agent_name?.toLowerCase().includes(filters.agent.toLowerCase())) {
        return false;
      }
      if (filters.provider && !trace.provider?.toLowerCase().includes(filters.provider.toLowerCase())) {
        return false;
      }
      if (filters.status && filters.status !== 'all' && trace.status !== filters.status) {
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
        console.log('🔍 Fetching traces for project:', effectiveProjectId, 'page:', page, 'offset:', offset);
        const response = await agentreplayClient.listTraces({
          limit: PAGE_SIZE,
          offset: offset,
          start_time: timeRangeToStart(filters.timeRange),
          project_id: parseInt(effectiveProjectId),
          // TODO: parent_span_id: null if backend supports it for filtering roots
        });

        console.log('📦 API Response:', response);

        if (reset && response.total !== undefined) {
          setTotalTraceCount(response.total);
        }

        const mapped: TraceRow[] = (response.traces || []).map((trace: any) => {
          const meta = trace.metadata || {};

          // Extract Claude Code fields from metadata (Tauri list_traces stores raw attributes in metadata)
          const agentIdAttr = trace.agent_id_attr || meta['agent_id'] || '';
          const isClaudeCode = trace.is_claude_code || agentIdAttr === 'claude-code';
          const toolName = trace.tool_name || meta['tool.name'] || '';
          const eventType = trace.event_type || meta['event.type'] || '';

          // Extract model from multiple possible sources
          const modelFromMetadata = meta['gen_ai.request.model'] ||
            meta['gen_ai.response.model'] ||
            meta['model'] ||
            meta['llm.model'];
          // For Claude Code traces, show tool_name in model column; for agents show LLM model
          const model = isClaudeCode
            ? (toolName || eventType || '')
            : (trace.model || modelFromMetadata || '');

          // Build display name from available fields
          let displayName = trace.display_name;
          if (!displayName || /^\d+$/.test(displayName) || displayName === 'chain.unknown') {
            if (isClaudeCode && toolName) {
              displayName = `Tool Call (${toolName})`;
            } else if (isClaudeCode && eventType) {
              displayName = eventType === 'session_start' ? 'Session Start'
                : eventType === 'session_end' ? 'Session End'
                  : eventType;
            } else {
              displayName = trace.operation_name || model || trace.agent_name || displayName;
              if (displayName === 'chain.unknown') {
                displayName = trace.agent_name ? `Agent: ${trace.agent_name}` : 'LangGraph Workflow';
              }
            }
          }

          // Extract input/output preview - also check tool.input/tool.output in metadata
          const inputPreview = trace.input_preview || meta['tool.input'] || '';
          const outputPreview = trace.output_preview || meta['tool.output'] || '';

          return {
            id: trace.trace_id || trace.span_id || 'unknown',
            timestamp: (trace.started_at || trace.timestamp_us || 0) / 1000,
            timestampLabel: new Date((trace.started_at || trace.timestamp_us || 0) / 1000).toLocaleString(),
            model: model || '',
            durationMs: trace.duration_ms || (trace.duration_us ? trace.duration_us / 1000 : 0) || 0,
            cost: trace.cost || 0,
            tokens: trace.tokens || trace.token_count || 0,
            user: trace.session_id?.toString() || 'anonymous',
            status: trace.status || 'completed',
            score: meta.score,
            metadata: meta,
            display_name: displayName,
            provider: trace.provider,
            input_tokens: meta.input_tokens || parseInt(meta['gen_ai.usage.input_tokens']) || 0,
            output_tokens: meta.output_tokens || parseInt(meta['gen_ai.usage.output_tokens']) || 0,
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
            const hasId = trace.id && trace.id !== 'unknown';
            const hasModel = trace.model && trace.model.length > 0;
            const hasTokens = trace.tokens && trace.tokens > 0;
            const hasContent = trace.input_preview || trace.output_preview;
            const hasDuration = trace.durationMs > 0;
            const hasDisplayName = trace.display_name && trace.display_name.length > 0;
            return hasId && (hasModel || hasTokens || hasContent || hasDuration || hasDisplayName);
          });

        // Trust the server-side project_id filtering
        setRawTraces(mapped);
        setCurrentPage(page);
      } catch (err) {
        console.error('Failed to load data', err);
        setError(err instanceof Error ? err.message : 'Failed to load data');
      } finally {
        setIsLoading(false);
      }
    },
    [filters.timeRange, effectiveProjectId]
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
    fetchPage(1, true).catch(err => {
      console.error('Failed to fetch traces on project change:', err);
    });
  }, [effectiveProjectId, filters.timeRange]);  // eslint-disable-line react-hooks/exhaustive-deps

  // Live mode now uses SSE streaming instead of polling!
  // SSE is handled by useSSETraces hook above

  useEffect(() => {
    const handler = () => setLiveMode((prev) => !prev);
    window.addEventListener(LIVE_MODE_EVENT, handler);
    return () => window.removeEventListener(LIVE_MODE_EVENT, handler);
  }, []);

  useEffect(() => {
    const dismiss = () => setContextMenu(null);
    window.addEventListener('click', dismiss);
    return () => window.removeEventListener('click', dismiss);
  }, []);

  const handleContextMenu = (event: React.MouseEvent, trace: TraceRow) => {
    event.preventDefault();
    setContextMenu({ x: event.clientX, y: event.clientY, trace });
  };

  const copyTraceId = async (traceId: string) => {
    try {
      await navigator.clipboard.writeText(traceId);
    } catch (err) {
      console.warn('Clipboard unavailable', err);
    }
    setContextMenu(null);
  };

  // Provider color dot helper
  const getProviderDotColor = (provider?: string, model?: string) => {
    const key = (provider || model || '').toLowerCase();
    if (key.includes('openai') || key.includes('gpt')) return 'bg-emerald-400';
    if (key.includes('anthropic') || key.includes('claude')) return 'bg-purple-400';
    if (key.includes('google') || key.includes('gemini')) return 'bg-blue-400';
    if (key.includes('mistral')) return 'bg-orange-400';
    if (key.includes('cohere')) return 'bg-rose-400';
    if (key.includes('llama') || key.includes('meta')) return 'bg-sky-400';
    return 'bg-textTertiary';
  };

  const TraceRowItem = ({
    trace,
    index = 0,
    isGroup = false,
    isExpanded = false,
    onToggle
  }: {
    trace: TraceRow;
    index?: number;
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
      } else if (trace.metadata?.type === 'session') {
        navigate(`/projects/${projectId}/sessions/${trace.session_id}`);
      } else {
        navigate(`/projects/${projectId}/traces/${trace.id}`);
      }
    };

    // Color coding for latency
    const latencyColor = trace.durationMs < 1000
      ? 'text-success'
      : trace.durationMs < 5000
        ? 'text-warning'
        : 'text-error';

    // Color coding for cost
    const costColor = (trace.cost || 0) > 0.01
      ? 'text-error'
      : (trace.cost || 0) > 0.001
        ? 'text-warning'
        : 'text-textSecondary';

    // Format duration for tooltip
    const durationText = trace.durationMs >= 1000
      ? `${(trace.durationMs / 1000).toFixed(2)} seconds`
      : `${trace.durationMs.toFixed(0)} milliseconds`;

    // Status config
    const statusConfig = trace.status === 'completed'
      ? { icon: '✓', label: 'OK' }
      : trace.status === 'error'
        ? { icon: '✗', label: 'Err' }
        : { icon: '⋯', label: 'Run' };

    // Extract input/output from metadata
    const inputPreview = useMemo(() => {
      if ((trace as any).input_preview) return String((trace as any).input_preview);
      const metadata = trace.metadata as Record<string, any> || {};
      for (let i = 0; i <= 2; i++) {
        const role = metadata[`gen_ai.prompt.${i}.role`];
        const content = metadata[`gen_ai.prompt.${i}.content`];
        if (content && role !== 'system') return String(content);
      }
      if (trace.metadata?.prompts && Array.isArray(trace.metadata.prompts) && trace.metadata.prompts.length > 0) {
        const userPrompt = (trace.metadata.prompts as any[]).find(p => p.role === 'user') || trace.metadata.prompts[0];
        return String(userPrompt?.content || '');
      }
      if (trace.metadata?.input) return String(trace.metadata.input);
      if (trace.metadata?.messages && Array.isArray(trace.metadata.messages)) {
        const userMsg = (trace.metadata.messages as any[]).find(m => m.role === 'user');
        if (userMsg?.content) return String(userMsg.content);
      }
      return '';
    }, [trace.metadata, trace]);

    const outputPreview = useMemo(() => {
      if ((trace as any).output_preview) return String((trace as any).output_preview);
      const metadata = trace.metadata as Record<string, any> || {};
      if (metadata['gen_ai.completion.0.content']) return String(metadata['gen_ai.completion.0.content']);
      if (trace.metadata?.completions && Array.isArray(trace.metadata.completions) && trace.metadata.completions.length > 0) {
        return String((trace.metadata.completions as any[])[0]?.content || '');
      }
      if (trace.metadata?.output) return String(trace.metadata.output);
      if (trace.metadata?.response) return String(trace.metadata.response);
      if (trace.metadata?.completion) return String(trace.metadata.completion);
      return '';
    }, [trace.metadata, trace]);

    const modelName = useMemo(() => {
      if (trace.is_claude_code) {
        if (trace.tool_name) return trace.tool_name;
        if (trace.event_type) return trace.event_type.replace(/_/g, ' ');
        return trace.display_name || 'Claude Code';
      }
      const metadata = trace.metadata as Record<string, any> || {};
      if (metadata['gen_ai.request.model']) return String(metadata['gen_ai.request.model']);
      if (metadata['gen_ai.response.model']) return String(metadata['gen_ai.response.model']);
      if (trace.metadata?.model) return String(trace.metadata.model);
      return trace.model || (trace as any).display_name || 'Unknown';
    }, [trace.metadata, trace.model, trace.is_claude_code, trace.tool_name, trace.event_type, trace.display_name]);

    return (
      <div
        onClick={handleClick}
        onContextMenu={(event) => handleContextMenu(event, trace)}
        style={{ animationDelay: `${index * 30}ms` }}
        className={cn(
          "trace-row-accent animate-stagger-in group grid cursor-pointer grid-cols-[minmax(140px,1fr)_minmax(120px,1fr)_minmax(180px,2fr)_minmax(180px,2fr)_80px_80px_100px_90px] items-center border-b border-border/30 px-5 py-3.5 text-sm text-textPrimary transition-colors duration-150 hover:bg-surface-hover/80",
          isGroup && "bg-surface-hover/50 font-medium",
          index % 2 === 1 && "bg-surface/30"
        )}
      >
        {/* Trace ID with tooltip and copy button */}
        <div className="flex items-center gap-2.5">
          {isGroup ? (
            <div className="flex items-center justify-center w-5 h-5 rounded bg-surface-hover">
              {isExpanded ? <ChevronDown className="h-3.5 w-3.5 text-textSecondary" /> : <ChevronRight className="h-3.5 w-3.5 text-textSecondary" />}
            </div>
          ) : (
            <div className="flex items-center justify-center w-7 h-7 rounded-lg bg-primary/10">
              <Activity className="h-3.5 w-3.5 text-primary" />
            </div>
          )}
          <div className="min-w-0">
            <Tooltip content={
              <div className="flex items-center gap-2">
                <span className="font-mono">{trace.id}</span>
                <CopyButton value={trace.id} size="sm" className="opacity-100" />
              </div>
            }>
              <p className="font-mono text-xs text-textPrimary font-medium truncate">{trace.id.slice(0, 10)}…</p>
            </Tooltip>
            <Tooltip content={new Date(trace.timestamp).toLocaleString()}>
              <p className="text-[11px] text-textTertiary mt-0.5">
                {formatDistanceToNow(trace.timestamp, { addSuffix: true })}
              </p>
            </Tooltip>
          </div>
        </div>

        {/* Model Name / Tool Name */}
        <Tooltip content={trace.is_claude_code
          ? `Tool: ${trace.tool_name || 'N/A'}\nType: ${trace.display_name || trace.event_type || 'Unknown'}\nAgent: Claude Code`
          : `Model: ${modelName}\nProvider: ${trace.provider || 'Unknown'}`
        }>
          <div className="truncate font-medium">
            {trace.is_claude_code ? (
              <span className="inline-flex items-center gap-1.5 px-2 py-1 rounded-md text-[10px] font-semibold bg-purple-500/10 text-purple-600 dark:text-purple-400 border border-purple-500/20 uppercase tracking-wide">
                <span className="w-1.5 h-1.5 rounded-full bg-purple-600 dark:bg-purple-400" />
                {trace.tool_name || trace.event_type?.replace(/_/g, ' ') || '⚡'}
              </span>
            ) : (
              <span className="flex items-center gap-1.5 text-textPrimary">
                <span className={cn('w-2 h-2 rounded-full flex-shrink-0', getProviderDotColor(trace.provider, modelName))} />
                <span className="truncate">{modelName}</span>
              </span>
            )}
          </div>
        </Tooltip>

        {/* Input Preview */}
        <Tooltip content={
          inputPreview ? (
            <div className="max-w-lg">
              <div className="text-xs font-semibold text-primary mb-1">📥 Full Input:</div>
              <div className="text-xs bg-surface/50 rounded-lg p-2.5 max-h-48 overflow-y-auto whitespace-pre-wrap">
                {inputPreview.substring(0, 500)}
                {inputPreview.length > 500 && '...'}
              </div>
            </div>
          ) : 'No input captured'
        }>
          <div className="line-clamp-2 text-xs text-textSecondary break-words leading-relaxed group-hover:text-textPrimary transition-colors">
            {inputPreview ? (
              <span>{inputPreview}</span>
            ) : (
              <span className="text-textTertiary italic">—</span>
            )}
          </div>
        </Tooltip>

        {/* Output Preview */}
        <Tooltip content={
          outputPreview ? (
            <div className="max-w-lg">
              <div className="text-xs font-semibold text-success mb-1">📤 Full Output:</div>
              <div className="text-xs bg-surface/50 rounded-lg p-2.5 max-h-48 overflow-y-auto whitespace-pre-wrap">
                {outputPreview.substring(0, 500)}
                {outputPreview.length > 500 && '...'}
              </div>
            </div>
          ) : 'No output captured'
        }>
          <div className="line-clamp-2 text-xs text-textSecondary break-words leading-relaxed group-hover:text-textPrimary transition-colors">
            {outputPreview ? (
              <span>{outputPreview}</span>
            ) : (
              <span className="text-textTertiary italic">—</span>
            )}
          </div>
        </Tooltip>

        {/* Latency */}
        <Tooltip content={durationText}>
          <div className={cn('text-sm font-bold tabular-nums', latencyColor)}>
            {trace.durationMs >= 1000
              ? `${(trace.durationMs / 1000).toFixed(1)}s`
              : `${trace.durationMs.toFixed(0)}ms`}
          </div>
        </Tooltip>

        {/* Cost */}
        <Tooltip content={`Estimated cost based on token usage`}>
          <div className={cn('font-medium text-xs tabular-nums', costColor)}>
            ${trace.cost?.toFixed(4) ?? '0.00'}
          </div>
        </Tooltip>

        {/* Tokens */}
        <Tooltip content={
          trace.input_tokens || trace.output_tokens ? (
            <div className="text-xs">
              <div>Input: {(trace.input_tokens || 0).toLocaleString()}</div>
              <div>Output: {(trace.output_tokens || 0).toLocaleString()}</div>
              <div className="mt-1 border-t border-border/30 pt-1 font-semibold">
                Total: {(trace.tokens || 0).toLocaleString()}
              </div>
            </div>
          ) : 'No token data'
        }>
          <div className="font-mono text-xs text-textSecondary tabular-nums">
            {trace.tokens?.toLocaleString() ?? '—'}
          </div>
        </Tooltip>

        {/* Status badge with icon + label */}
        <div className="flex justify-end">
          <Tooltip content={
            trace.status === 'error' ? (
              <div className="max-w-sm">
                <div className="font-semibold text-error mb-1">Error Details:</div>
                <div className="text-xs">
                  {String(trace.metadata?.error || trace.metadata?.error_message || 'Error occurred')}
                </div>
              </div>
            ) : `Status: ${trace.status}`
          }>
            <span className={cn(
              'inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-[11px] font-semibold tracking-wide',
              statusColors[trace.status] || 'bg-muted/40 text-textSecondary'
            )}>
              <span>{statusConfig.icon}</span>
              <span>{statusConfig.label}</span>
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
              <h3 className="text-lg font-semibold text-textPrimary">Delete Project</h3>
            </div>
            <p className="text-textSecondary mb-2">
              Are you sure you want to delete <strong className="text-textPrimary">{currentProject?.name || 'this project'}</strong>?
            </p>
            <p className="text-sm text-textTertiary mb-6">
              This will permanently delete all traces, sessions, and data for this project. This action cannot be undone.
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

      {/* Header */}
      <header className="flex flex-col gap-5 pt-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div
              className="w-9 h-9 rounded-lg flex items-center justify-center"
              style={{ backgroundColor: 'rgba(0,128,255,0.1)' }}
            >
              <Activity className="w-4.5 h-4.5" style={{ color: '#0080FF' }} />
            </div>
            <div>
              <h1 className="text-lg font-bold tracking-tight text-foreground">{currentProject?.name || 'Traces'}</h1>
              <p className="text-xs text-muted-foreground">Observability · Traces</p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              className={cn(
                'gap-1.5 rounded-lg px-3 h-8',
                liveMode && sseConnected ? 'text-success bg-success/10' :
                  liveMode && sseConnecting ? 'text-warning bg-warning/10' :
                    liveMode && sseError ? 'text-error bg-error/10' :
                      'text-textSecondary'
              )}
              onClick={() => setLiveMode((prev) => !prev)}
              title={liveMode ? (sseConnected ? 'Connected via SSE' : 'Connecting...') : 'Enable live updates'}
            >
              {sseConnecting ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : sseConnected ? (
                <Radio className="h-3.5 w-3.5 animate-pulse" />
              ) : (
                <Zap className="h-3.5 w-3.5" />
              )}
              Live
            </Button>
            <Button variant="ghost" size="sm" onClick={() => fetchPage(currentPage, true)} className="rounded-lg h-8 w-8 p-0" title="Refresh">
              {isLoading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCcw className="h-3.5 w-3.5" />}
            </Button>
            <VideoHelpButton pageId="traces" />
            {effectiveProjectId && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setShowDeleteConfirm(true)}
                className="rounded-lg h-8 w-8 p-0 text-red-500 hover:text-red-600 dark:text-red-400 hover:bg-red-500/10"
                title="Delete project"
              >
                <Trash2 className="h-3.5 w-3.5" />
              </Button>
            )}
          </div>
        </div>

        {/* Glass Metric Cards */}
        <div className="grid grid-cols-3 gap-3">
          <div className="glass-card rounded-xl px-4 py-3 flex items-center gap-3">
            <div className="flex items-center justify-center w-9 h-9 rounded-lg bg-primary/10">
              <Activity className="h-4 w-4 text-primary" />
            </div>
            <div>
              <p className="text-lg font-bold text-textPrimary tabular-nums">{visibleTraces.length.toLocaleString()}</p>
              <p className="text-[11px] text-textTertiary font-medium uppercase tracking-wider">Traces</p>
            </div>
          </div>
          <div className="glass-card rounded-xl px-4 py-3 flex items-center gap-3">
            <div className="flex items-center justify-center w-9 h-9 rounded-lg bg-warning/10">
              <Clock className="h-4 w-4 text-warning" />
            </div>
            <div>
              <p className="text-lg font-bold text-textPrimary tabular-nums">
                {(() => {
                  const latencies = visibleTraces.map(t => t.durationMs).filter(d => d > 0);
                  return latencies.length ? `${Math.round(calculatePercentile(latencies, 50))}ms` : '—';
                })()}
              </p>
              <p className="text-[11px] text-textTertiary font-medium uppercase tracking-wider">p50 Latency</p>
            </div>
          </div>
          <div className="glass-card rounded-xl px-4 py-3 flex items-center gap-3">
            <div className="flex items-center justify-center w-9 h-9 rounded-lg bg-success/10">
              <DollarSign className="h-4 w-4 text-success" />
            </div>
            <div>
              <p className="text-lg font-bold text-textPrimary tabular-nums">
                ${visibleTraces.reduce((sum, trace) => sum + (trace.cost || 0), 0).toFixed(2)}
              </p>
              <p className="text-[11px] text-textTertiary font-medium uppercase tracking-wider">Total Spent</p>
            </div>
          </div>
        </div>
      </header>


      {/* Search & Filters */}
      <section>
        <div className="flex items-center gap-2">
          <div className="flex-1 relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
            <input
              type="text"
              value={filters.query}
              onChange={(event) => setFilters((prev) => ({ ...prev, query: event.target.value }))}
              placeholder="Search traces by ID, model, input, output…"
              className="w-full pl-9 pr-3 py-2.5 rounded-xl text-[13px] focus:outline-none transition-all bg-input border border-border text-foreground placeholder:text-muted-foreground"
            />
          </div>
          <button
            type="button"
            onClick={() => setShowFilters(!showFilters)}
            className={`flex items-center justify-center rounded-xl transition-all flex-shrink-0 w-[38px] h-[38px] ${showFilters
              ? 'bg-primary text-primary-foreground'
              : 'bg-card border border-border text-muted-foreground hover:text-foreground'
              }`}
            title="Toggle filters"
          >
            <Filter className="w-4 h-4" />
          </button>
        </div>

        {/* Advanced filters */}
        {showFilters && (
          <div className="mt-3 pt-3 border-t border-border/30 grid gap-3 md:grid-cols-3 lg:grid-cols-6 animate-slide-in">
            <div>
              <label className="text-[11px] font-semibold text-textTertiary uppercase tracking-wider">Time Range</label>
              <select
                value={filters.timeRange}
                onChange={(event) => setFilters((prev) => ({ ...prev, timeRange: event.target.value }))}
                className="mt-1.5 w-full rounded-xl border border-border/50 text-sm h-9 px-3 bg-background cursor-pointer focus:outline-none"
              >
                {timeRangeOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="text-[11px] font-semibold text-textTertiary uppercase tracking-wider">Status</label>
              <select
                value={filters.status}
                onChange={(event) => setFilters((prev) => ({ ...prev, status: event.target.value }))}
                className="mt-1.5 w-full rounded-xl border border-border/50 text-sm h-9 px-3 bg-background cursor-pointer focus:outline-none"
              >
                <option value="all">All Status</option>
                <option value="completed">Completed</option>
                <option value="error">Error</option>
              </select>
            </div>
            <div>
              <label className="text-[11px] font-semibold text-textTertiary uppercase tracking-wider">Session</label>
              <Input
                value={filters.user}
                onChange={(event) => setFilters((prev) => ({ ...prev, user: event.target.value }))}
                placeholder="Session ID"
                className="mt-1.5 rounded-xl border-border/50 text-sm h-9"
              />
            </div>
            <div>
              <label className="text-[11px] font-semibold text-textTertiary uppercase tracking-wider">Model</label>
              <Input
                value={filters.model}
                onChange={(event) => setFilters((prev) => ({ ...prev, model: event.target.value }))}
                placeholder="gpt-4o, llama…"
                className="mt-1.5 rounded-xl border-border/50 text-sm h-9"
              />
            </div>
            <div>
              <label className="text-[11px] font-semibold text-textTertiary uppercase tracking-wider">Agent</label>
              <Input
                value={filters.agent}
                onChange={(event) => setFilters((prev) => ({ ...prev, agent: event.target.value }))}
                placeholder="Agent name"
                className="mt-1.5 rounded-xl border-border/50 text-sm h-9"
              />
            </div>
            <div>
              <label className="text-[11px] font-semibold text-textTertiary uppercase tracking-wider">Provider</label>
              <Input
                value={filters.provider}
                onChange={(event) => setFilters((prev) => ({ ...prev, provider: event.target.value }))}
                placeholder="OpenAI, Anthropic…"
                className="mt-1.5 rounded-xl border-border/50 text-sm h-9"
              />
            </div>
          </div>
        )
        }

        {
          error && (
            <div className="mt-3 rounded-xl border border-red-500/30 bg-red-500/8 px-4 py-2.5 text-sm text-red-600 dark:text-red-400 flex items-center gap-2">
              <span className="w-1.5 h-1.5 rounded-full bg-red-500 flex-shrink-0" />
              {error}
            </div>
          )
        }
      </section >

      <section className="flex flex-1 flex-col rounded-2xl border border-border/40 bg-background/80 overflow-hidden">
        {/* Sticky table header with glass effect */}
        <div className="sticky top-0 z-10 glass-card grid grid-cols-[minmax(140px,1fr)_minmax(120px,1fr)_minmax(180px,2fr)_minmax(180px,2fr)_80px_80px_100px_90px] border-b border-border/40 px-5 py-3 text-[11px] font-semibold text-textTertiary">
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
            /* Skeleton shimmer loading */
            <div className="flex flex-col">
              {[...Array(6)].map((_, i) => (
                <div
                  key={i}
                  className="grid grid-cols-[minmax(140px,1fr)_minmax(120px,1fr)_minmax(180px,2fr)_minmax(180px,2fr)_80px_80px_100px_90px] items-center border-b border-border/20 px-5 py-4 gap-4"
                >
                  <div className="flex items-center gap-2.5">
                    <div className="w-7 h-7 rounded-lg animate-shimmer" />
                    <div className="flex flex-col gap-1.5">
                      <div className="w-20 h-3 rounded animate-shimmer" />
                      <div className="w-14 h-2.5 rounded animate-shimmer" />
                    </div>
                  </div>
                  <div className="w-24 h-3 rounded animate-shimmer" />
                  <div className="w-full h-3 rounded animate-shimmer" />
                  <div className="w-full h-3 rounded animate-shimmer" />
                  <div className="w-12 h-3 rounded animate-shimmer" />
                  <div className="w-14 h-3 rounded animate-shimmer" />
                  <div className="w-10 h-3 rounded animate-shimmer" />
                  <div className="flex justify-end"><div className="w-12 h-5 rounded-full animate-shimmer" /></div>
                </div>
              ))}
            </div>
          ) : visibleTraces.length === 0 ? (
            /* Rich empty state */
            <div className="flex flex-col gap-4 py-8 px-6">
              {(filters.query || filters.user || filters.model || filters.agent || filters.provider) ? (
                /* Filtered empty — compact */
                <div className="flex flex-col items-center justify-center gap-3 py-12">
                  <div className="flex items-center justify-center w-14 h-14 rounded-2xl bg-secondary">
                    <Filter className="h-6 w-6 text-muted-foreground" />
                  </div>
                  <div className="text-center">
                    <p className="text-sm font-semibold text-foreground">No matching traces</p>
                    <p className="text-xs mt-1 text-muted-foreground">Try adjusting your filters or search query.</p>
                  </div>
                </div>
              ) : (
                /* True empty — rich guidance */
                <>
                  {/* Step cards */}
                  <div className="grid grid-cols-3 gap-3">
                    {[
                      {
                        step: '1', title: 'Send traces',
                        cardClass: 'bg-blue-50 dark:bg-blue-950/30 border border-blue-200 dark:border-blue-800/40',
                        badgeClass: 'bg-blue-100 dark:bg-blue-900/50 text-blue-600 dark:text-blue-400',
                        desc: 'Use the SDK or API to send your first LLM trace from your application.',
                      },
                      {
                        step: '2', title: 'View in dashboard',
                        cardClass: 'bg-violet-50 dark:bg-violet-950/30 border border-violet-200 dark:border-violet-800/40',
                        badgeClass: 'bg-violet-100 dark:bg-violet-900/50 text-violet-600 dark:text-violet-400',
                        desc: 'Traces appear here instantly with model, tokens, latency, and cost.',
                      },
                      {
                        step: '3', title: 'Search & filter',
                        cardClass: 'bg-emerald-50 dark:bg-emerald-950/30 border border-emerald-200 dark:border-emerald-800/40',
                        badgeClass: 'bg-emerald-100 dark:bg-emerald-900/50 text-emerald-600 dark:text-emerald-400',
                        desc: 'Find specific traces by model, status, time range, or keyword search.',
                      },
                    ].map((s) => (
                      <div
                        key={s.step}
                        className={`rounded-xl p-4 ${s.cardClass}`}
                      >
                        <div className="flex items-center gap-2.5 mb-2.5">
                          <div
                            className={`flex items-center justify-center flex-shrink-0 w-7 h-7 rounded-lg text-xs font-extrabold ${s.badgeClass}`}
                          >
                            {s.step}
                          </div>
                          <span className="text-sm font-bold text-foreground">{s.title}</span>
                        </div>
                        <p className="text-xs leading-relaxed text-muted-foreground">{s.desc}</p>
                      </div>
                    ))}
                  </div>

                  {/* Capability cards */}
                  <div className="grid grid-cols-3 gap-3">
                    {[
                      { icon: <List className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" />, label: 'Models & Providers', detail: 'Track which LLMs are being used across traces', iconBgClass: 'bg-blue-100 dark:bg-blue-900/40' },
                      { icon: <Zap className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" />, label: 'Token Usage', detail: 'Input, output, and total token counts', iconBgClass: 'bg-amber-100 dark:bg-amber-900/40' },
                      { icon: <Clock className="w-3.5 h-3.5 text-emerald-600 dark:text-emerald-400" />, label: 'Latency Tracking', detail: 'Duration and P50 percentile metrics', iconBgClass: 'bg-emerald-100 dark:bg-emerald-900/40' },
                      { icon: <AlertCircle className="w-3.5 h-3.5 text-red-600 dark:text-red-400" />, label: 'Error Detection', detail: 'Catch failures, timeouts, and rate limits', iconBgClass: 'bg-red-100 dark:bg-red-900/40' },
                      { icon: <MessageCircle className="w-3.5 h-3.5 text-indigo-600 dark:text-indigo-400" />, label: 'Tool Calls', detail: 'Function invocations and their results', iconBgClass: 'bg-indigo-100 dark:bg-indigo-900/40' },
                      { icon: <DollarSign className="w-3.5 h-3.5 text-emerald-600 dark:text-emerald-400" />, label: 'Cost Analysis', detail: 'Per-trace and cumulative spend tracking', iconBgClass: 'bg-emerald-100 dark:bg-emerald-900/40' },
                    ].map((c) => (
                      <div
                        key={c.label}
                        className="flex items-start gap-3 rounded-xl p-3.5 bg-card border border-border"
                      >
                        <div
                          className={`flex items-center justify-center flex-shrink-0 rounded-lg w-[30px] h-[30px] ${c.iconBgClass}`}
                        >
                          {c.icon}
                        </div>
                        <div className="min-w-0">
                          <p className="text-xs font-semibold text-foreground">{c.label}</p>
                          <p className="text-[11px] mt-0.5 text-muted-foreground">{c.detail}</p>
                        </div>
                      </div>
                    ))}
                  </div>
                </>
              )}
            </div>
          ) : (
            <div className="flex h-full flex-col overflow-y-auto">
              {visibleTraces.map((trace, index) => (
                <TraceRowItem key={`${trace.id}-${index}`} trace={trace} index={index} />
              ))}
            </div>
          )}
        </div>

        {/* Pagination Controls */}
        {totalTraceCount > PAGE_SIZE && (
          <div className="flex items-center justify-between border-t border-border/30 px-5 py-3 glass-card">
            <div className="text-xs text-textTertiary">
              <span className="font-medium text-textSecondary">{((currentPage - 1) * PAGE_SIZE) + 1}–{Math.min(currentPage * PAGE_SIZE, totalTraceCount)}</span>
              <span> of {totalTraceCount.toLocaleString()} traces</span>
            </div>
            <div className="flex items-center gap-1.5">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => fetchPage(1)}
                disabled={currentPage === 1 || isLoading}
                className="page-btn-hover h-8 w-8 p-0 rounded-full"
              >
                <ChevronsLeft className="h-3.5 w-3.5" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => fetchPage(currentPage - 1)}
                disabled={currentPage === 1 || isLoading}
                className="page-btn-hover h-8 w-8 p-0 rounded-full"
              >
                <ChevronLeft className="h-3.5 w-3.5" />
              </Button>

              <div className="flex items-center gap-1">
                {(() => {
                  const totalPages = Math.ceil(totalTraceCount / PAGE_SIZE);
                  const pages: (number | string)[] = [];
                  pages.push(1);
                  if (currentPage > 3) pages.push('...');
                  for (let i = Math.max(2, currentPage - 1); i <= Math.min(totalPages - 1, currentPage + 1); i++) {
                    if (!pages.includes(i)) pages.push(i);
                  }
                  if (currentPage < totalPages - 2) pages.push('...');
                  if (totalPages > 1 && !pages.includes(totalPages)) pages.push(totalPages);

                  return pages.map((page, idx) => (
                    typeof page === 'number' ? (
                      <Button
                        key={page}
                        variant={page === currentPage ? 'default' : 'ghost'}
                        size="sm"
                        onClick={() => fetchPage(page)}
                        disabled={isLoading}
                        className={cn(
                          'page-btn-hover h-8 min-w-[32px] px-2 rounded-full text-xs font-medium',
                          page === currentPage && 'bg-primary text-primary-foreground shadow-md'
                        )}
                      >
                        {page}
                      </Button>
                    ) : (
                      <span key={`ellipsis-${idx}`} className="px-1 text-textTertiary text-xs">…</span>
                    )
                  ));
                })()}
              </div>

              <Button
                variant="ghost"
                size="sm"
                onClick={() => fetchPage(currentPage + 1)}
                disabled={currentPage >= Math.ceil(totalTraceCount / PAGE_SIZE) || isLoading}
                className="page-btn-hover h-8 w-8 p-0 rounded-full"
              >
                <ChevronRight className="h-3.5 w-3.5" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => fetchPage(Math.ceil(totalTraceCount / PAGE_SIZE))}
                disabled={currentPage >= Math.ceil(totalTraceCount / PAGE_SIZE) || isLoading}
                className="page-btn-hover h-8 w-8 p-0 rounded-full"
              >
                <ChevronsRight className="h-3.5 w-3.5" />
              </Button>
            </div>
          </div>
        )}
      </section>

      {
        contextMenu && (
          <div
            className="fixed z-50 min-w-[220px] rounded-xl glass-card shadow-2xl py-1.5 animate-slide-in"
            style={{ top: contextMenu.y, left: contextMenu.x }}
          >
            <button
              className="flex w-full items-center gap-2.5 px-4 py-2.5 text-left text-sm text-textPrimary hover:bg-surface-hover rounded-lg mx-1 transition-colors"
              onClick={() => copyTraceId(contextMenu.trace.id)}
            >
              <MoreVertical className="h-4 w-4 text-primary" />
              <span>Copy trace ID</span>
            </button>
            <button
              className="flex w-full items-center gap-2.5 px-4 py-2.5 text-left text-sm text-textPrimary hover:bg-surface-hover rounded-lg mx-1 transition-colors"
              onClick={() => {
                navigate(`/projects/${projectId}/traces/${contextMenu.trace.id}?action=replay`);
                setContextMenu(null);
              }}
            >
              <Clock className="h-4 w-4 text-warning" />
              <span>Replay trace</span>
            </button>
            <div className="my-1 mx-3 border-t border-border/30" />
            <button
              className="flex w-full items-center gap-2.5 px-4 py-2.5 text-left text-sm text-textPrimary hover:bg-surface-hover rounded-lg mx-1 transition-colors"
              onClick={() => {
                console.log('Add to dataset', contextMenu.trace.id);
                setContextMenu(null);
              }}
            >
              <Activity className="h-4 w-4 text-success" />
              <span>Add to dataset</span>
            </button>
          </div>
        )
      }
    </div >
  );
}
