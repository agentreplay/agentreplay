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

import { useEffect, useState, useMemo } from 'react';
import { createPortal } from 'react-dom';
import { useParams, useNavigate, useSearchParams } from 'react-router-dom';
import { ArrowLeft, Clock, Zap, Activity, Hash, Box, List, GitBranch, Beaker, MessageSquare, FileText, User, Settings, Sparkles, Flame, Play, PanelRightClose, PanelRightOpen, Trash } from 'lucide-react';
import { agentreplayClient, TraceMetadata } from '../lib/agentreplay-api';
import { SpanInspector } from '../../components/trace/SpanInspector';
import { formatDistanceToNow } from 'date-fns';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../../components/ui/tabs';
import { LatencyBreakdown } from '../../components/analytics/LatencyBreakdown';
import { CostAnalysis } from '../../components/analytics/CostAnalysis';
import { TraceGraphView } from '../../components/trace/TraceGraphView';
import { TraceTimelineView } from '../../components/trace/TraceTimelineView';
import { FlameGraph } from '../../components/trace/FlameGraph';
import { TraceTree, Span as TreeSpan } from '../../components/trace/TraceTree';
import { AITraceAnalysis } from '../../components/trace/AITraceAnalysis';
import { EvaluateTraceButton } from '../components/EvaluateTraceButton';
import { AddToDatasetButton } from '../components/AddToDatasetButton';
import { CommitTraceButton } from '../components/CommitTraceButton';
import { SessionReplay } from '../../components/session/SessionReplay';

export default function TraceDetailsPage() {
  const { traceId, projectId: routeProjectId } = useParams<{ traceId: string; projectId: string }>();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const queryProjectId = searchParams.get('project_id');
  const projectId = routeProjectId || queryProjectId;
  const tenantId = searchParams.get('tenant_id');
  const viewParam = searchParams.get('view');
  const fromSession = searchParams.get('from') === 'session';
  const sourceSessionId = searchParams.get('session_id');
  const [trace, setTrace] = useState<TraceMetadata | null>(null);
  const [observations, setObservations] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<'list' | 'graph' | 'conversation' | 'overview' | 'attributes' | 'raw' | 'ai' | 'flamegraph' | 'replay'>(
    (viewParam as any) || 'list'
  );
  const [showInspector, setShowInspector] = useState(true);
  const [selectedSpan, setSelectedSpan] = useState<TraceMetadata | null>(null);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);

  // Auto-hide inspector for complex views (Graph, Replay, Flamegraph) to give more space
  useEffect(() => {
    if (['graph', 'replay', 'flamegraph'].includes(viewMode)) {
      setShowInspector(false);
    } else {
      setShowInspector(true);
    }
  }, [viewMode]);

  // Helper function to decode HTML entities
  const decodeHtmlEntities = (str: string): string => {
    return str
      .replace(/&quot;/g, '"')
      .replace(/&amp;/g, '&')
      .replace(/&lt;/g, '<')
      .replace(/&gt;/g, '>');
  };

  // Helper function to parse and format JSON
  const parseAndFormatJson = (value: any): string => {
    try {
      if (typeof value === 'string') {
        const decoded = decodeHtmlEntities(value);
        const parsed = JSON.parse(decoded);
        return JSON.stringify(parsed, null, 2);
      }
      return JSON.stringify(value, null, 2);
    } catch {
      return typeof value === 'string' ? decodeHtmlEntities(value) : String(value);
    }
  };

  useEffect(() => {
    if (traceId) {
      fetchTraceDetails(traceId);
    }
  }, [traceId]);

  // Helper to build synthetic child spans from rich metadata
  // This parses OTEL gen_ai.* attributes to create a proper conversation flow tree
  const buildSyntheticSpansFromMetadata = (trace: TraceMetadata): TreeSpan[] => {
    const metadata = trace.metadata || {};
    const children: TreeSpan[] = [];
    const baseTime = (trace.started_at || trace.timestamp_us || 0) / 1000;
    const totalDuration = trace.duration_ms || 0;

    // Extract prompts from gen_ai.prompt.N.* pattern
    const prompts: Array<{ index: number; role: string; content: string; toolCalls?: any[]; toolCallId?: string }> = [];
    const promptPattern = /^gen_ai\.prompt\.(\d+)\.(role|content|tool_calls\..*|tool_call_id)$/;

    // Group all prompt attributes by index
    const promptGroups: Record<number, any> = {};
    Object.entries(metadata).forEach(([key, value]) => {
      const match = key.match(promptPattern);
      if (match) {
        const index = parseInt(match[1]);
        if (!promptGroups[index]) promptGroups[index] = { index };

        if (match[2] === 'role') {
          promptGroups[index].role = value;
        } else if (match[2] === 'content') {
          promptGroups[index].content = value;
        } else if (match[2].startsWith('tool_calls.')) {
          if (!promptGroups[index].toolCalls) promptGroups[index].toolCalls = [];
          // Parse tool call attributes
          const toolMatch = match[2].match(/tool_calls\.(\d+)\.(.*)/);
          if (toolMatch) {
            const toolIndex = parseInt(toolMatch[1]);
            const toolAttr = toolMatch[2];
            if (!promptGroups[index].toolCalls[toolIndex]) {
              promptGroups[index].toolCalls[toolIndex] = {};
            }
            promptGroups[index].toolCalls[toolIndex][toolAttr] = value;
          }
        } else if (match[2] === 'tool_call_id') {
          promptGroups[index].toolCallId = value;
        }
      }
    });

    // Sort by index and add to prompts array
    Object.values(promptGroups).sort((a, b) => a.index - b.index).forEach(p => {
      prompts.push(p as any);
    });

    // Extract completion
    const completion = {
      role: metadata['gen_ai.completion.0.role'] || 'assistant',
      content: metadata['gen_ai.completion.0.content'] || metadata.output || '',
      finishReason: metadata['gen_ai.completion.0.finish_reason']
    };

    // Calculate time slices for children
    const numSteps = prompts.length + (completion.content ? 1 : 0);
    const timePerStep = numSteps > 0 ? totalDuration / numSteps : totalDuration;
    let currentTime = baseTime;

    // Create spans for each prompt
    prompts.forEach((prompt, idx) => {
      const spanType = prompt.role === 'system' ? 'planning' :
        prompt.role === 'user' ? 'root' :
          prompt.role === 'tool' ? 'tool_response' :
            prompt.toolCalls ? 'tool_call' : 'reasoning';

      let spanName = prompt.role === 'system' ? 'System Prompt' :
        prompt.role === 'user' ? 'User Message' :
          prompt.role === 'tool' ? 'Tool Response' :
            prompt.role === 'assistant' ? (prompt.toolCalls ? 'Tool Call' : 'Assistant') :
              `Message ${idx + 1}`;

      // If it's a tool call, use the tool name
      if (prompt.toolCalls && prompt.toolCalls[0]?.name) {
        spanName = `🔧 ${prompt.toolCalls[0].name}`;
      }

      // Parse content if it's JSON
      let displayContent = prompt.content || '';
      try {
        if (typeof displayContent === 'string' && displayContent.startsWith('[')) {
          const parsed = JSON.parse(displayContent);
          if (Array.isArray(parsed) && parsed[0]?.text) {
            displayContent = parsed[0].text;
          }
        }
      } catch (e) { /* keep original */ }

      children.push({
        id: `${trace.span_id || trace.trace_id}-prompt-${idx}`,
        name: spanName,
        spanType: spanType as any,
        status: 'success',
        startTime: currentTime,
        endTime: currentTime + timePerStep,
        duration: Math.round(timePerStep),
        metadata: {
          role: prompt.role,
          content: displayContent,
          ...(prompt.toolCalls ? { tool_calls: prompt.toolCalls } : {}),
          ...(prompt.toolCallId ? { tool_call_id: prompt.toolCallId } : {})
        },
        children: []
      });

      currentTime += timePerStep;
    });

    // Add completion span
    if (completion.content) {
      children.push({
        id: `${trace.span_id || trace.trace_id}-completion`,
        name: '💬 Response',
        spanType: 'response',
        status: 'success',
        startTime: currentTime,
        endTime: currentTime + timePerStep,
        duration: Math.round(timePerStep),
        metadata: {
          role: 'assistant',
          content: completion.content,
          finish_reason: completion.finishReason
        },
        children: []
      });
    }

    return children;
  };

  // Helper to parse content - handles JSON array format like [{"type": "text", "text": "..."}]
  const parseContent = (content: any): string => {
    if (!content) return '';
    if (typeof content !== 'string') return String(content);

    // Try to parse as JSON array
    try {
      if (content.startsWith('[')) {
        const parsed = JSON.parse(content);
        if (Array.isArray(parsed)) {
          return parsed
            .filter((item: any) => item.type === 'text' && item.text)
            .map((item: any) => item.text)
            .join('\n');
        }
      }
    } catch (e) {
      // Not valid JSON, return as-is
    }
    return content;
  };

  // Helper to extract tool calls from metadata
  const extractToolCalls = (metadata: any): Array<{ name: string; arguments?: string }> => {
    const tools: Array<{ name: string; arguments?: string }> = [];

    // Look for tool_calls in gen_ai.prompt.N.tool_calls.M.*
    const toolPattern = /^gen_ai\.prompt\.(\d+)\.tool_calls\.(\d+)\.(name|arguments)$/;
    const toolGroups: Record<string, Record<number, { name?: string; arguments?: string }>> = {};

    Object.entries(metadata).forEach(([key, value]) => {
      const match = key.match(toolPattern);
      if (match) {
        const promptIdx = match[1];
        const toolIdx = parseInt(match[2]);
        const attr = match[3];

        if (!toolGroups[promptIdx]) toolGroups[promptIdx] = {};
        if (!toolGroups[promptIdx][toolIdx]) toolGroups[promptIdx][toolIdx] = {};

        if (attr === 'name') toolGroups[promptIdx][toolIdx].name = value as string;
        else if (attr === 'arguments') toolGroups[promptIdx][toolIdx].arguments = value as string;
      }
    });

    // Flatten and collect unique tool names
    const seenTools = new Set<string>();
    Object.values(toolGroups).forEach(promptTools => {
      Object.values(promptTools).forEach(tool => {
        if (tool.name && !seenTools.has(tool.name)) {
          seenTools.add(tool.name);
          tools.push(tool as { name: string; arguments?: string });
        }
      });
    });

    return tools;
  };

  // Helper to extract input (prompts) and output (completion) from trace metadata
  const extractInputOutput = (metadata: any): {
    input: string;
    output: string;
    messages: Array<{ role: string; content: string }>;
    tools: Array<{ name: string; arguments?: string }>
  } => {
    const messages: Array<{ role: string; content: string }> = [];
    let systemPrompt = '';
    let userMessage = '';
    let assistantResponse = '';

    // Extract from gen_ai.prompt.N.* pattern
    const promptPattern = /^gen_ai\.prompt\.(\d+)\.(role|content)$/;
    const promptGroups: Record<number, { role?: string; content?: string }> = {};

    Object.entries(metadata).forEach(([key, value]) => {
      const match = key.match(promptPattern);
      if (match) {
        const index = parseInt(match[1]);
        if (!promptGroups[index]) promptGroups[index] = {};
        if (match[2] === 'role') promptGroups[index].role = value as string;
        else if (match[2] === 'content') promptGroups[index].content = parseContent(value);
      }
    });

    // Sort and build messages
    Object.values(promptGroups)
      .sort((a: any, b: any) => (a.index || 0) - (b.index || 0))
      .forEach((p: any) => {
        if (p.role && p.content) {
          messages.push({ role: p.role, content: p.content });
          if (p.role === 'system') systemPrompt = p.content;
          if (p.role === 'user') userMessage = p.content;
        }
      });

    // Extract completion
    assistantResponse = parseContent(metadata['gen_ai.completion.0.content']) || metadata.output || '';
    if (assistantResponse) {
      messages.push({ role: 'assistant', content: assistantResponse });
    }

    // Extract tools
    const tools = extractToolCalls(metadata);

    // Build input string (combine system + user messages)
    const inputParts: string[] = [];
    if (systemPrompt) inputParts.push(`[System]\n${systemPrompt}`);
    if (userMessage) inputParts.push(`[User]\n${userMessage}`);
    const input = inputParts.length > 0 ? inputParts.join('\n\n') : (metadata.input || metadata.prompt || '');

    return { input, output: assistantResponse, messages, tools };
  };

  // Helper to extract chat messages from OTel 2025 events
  const extractChatFromEvents = (metadata: any) => {
    const messages: Array<{ role: string, content: string }> = [];

    // 1. Try parsing the 2025 "otel.events" blob
    if (metadata['otel.events']) {
      try {
        const events = JSON.parse(metadata['otel.events']);

        // Filter for prompts
        events.filter((e: any) => e.name === 'gen_ai.content.prompt').forEach((e: any) => {
          messages.push({
            role: e.attributes?.['gen_ai.content.role'] || 'user',
            content: e.attributes?.['gen_ai.prompt'] || ''
          });
        });

        // Filter for completions
        events.filter((e: any) => e.name === 'gen_ai.content.completion').forEach((e: any) => {
          messages.push({
            role: 'assistant', // OTel 2025 default
            content: e.attributes?.['gen_ai.completion'] || ''
          });
        });

        if (messages.length > 0) return messages;
      } catch (e) {
        console.error('[TraceDetailsPage] Failed to parse otel.events', e);
      }
    }

    // 2. Fallback to legacy fields
    if (metadata['prompt']) messages.push({ role: 'user', content: metadata['prompt'] });
    if (metadata['response']) messages.push({ role: 'assistant', content: metadata['response'] });

    return messages;
  };

  const fetchTraceDetails = async (id: string) => {
    setLoading(true);
    setError(null);
    try {
      // getTrace returns TraceMetadata (TraceView from backend)
      // Use project_id 0 as default to match SDK default
      const details: any = await agentreplayClient.getTrace(
        id,
        tenantId ? parseInt(tenantId) : 1,
        projectId ? parseInt(projectId) : 0
      );
      // Convert to UI-compatible format
      // Helper to safely extract values from metadata or attributes only (NOT from top-level trace fields)
      const getMeta = (keys: string[]) => {
        const source = { ...details.metadata, ...details.attributes };
        for (const key of keys) {
          if (source[key] !== undefined && source[key] !== null) return source[key];
          if (source[`payload.${key}`] !== undefined) return source[`payload.${key}`];
        }
        return undefined;
      };

      const trace: TraceMetadata = {
        ...details,
        // Ensure these fields are populated from the raw details if they exist
        trace_id: details.trace_id || id,
        span_id: details.span_id,
        parent_span_id: details.parent_span_id,
        project_id: details.project_id || (projectId ? parseInt(projectId) : 0),
        tenant_id: details.tenant_id || (tenantId ? parseInt(tenantId) : 1),
        agent_id: details.agent_id,
        agent_name: details.agent_name,
        span_type: details.span_type || 'Unknown',
        environment: details.environment || 'development',

        // Timestamp conversions
        started_at: details.timestamp_us,
        ended_at: details.timestamp_us + details.duration_us,
        duration_ms: details.duration_ms || (details.duration_us / 1000),
        status: details.status || 'completed',
        tokens: details.tokens || details.token_count || 0,

        // Build metadata object from available sources
        metadata: (() => {
          const mergedMetadata: Record<string, any> = {
            ...(details.metadata || {}),
            ...(details.attributes || {}),
          };

          // Add fallback mappings for common OTEL fields if they exist in top-level details
          const fallbackMappings: Record<string, any> = {
            'gen_ai.system': details.model_provider,
            'gen_ai.request.model': details.model,
            'gen_ai.usage.input_tokens': details.input_tokens,
            'gen_ai.usage.output_tokens': details.output_tokens,
            'gen_ai.usage.total_tokens': details.token_count,
          };

          // Only add fallback values if not already present
          for (const [key, value] of Object.entries(fallbackMappings)) {
            if (value !== undefined && value !== null && !mergedMetadata[key]) {
              mergedMetadata[key] = value;
            }
          }

          // Add custom field mappings using getMeta helper
          const customMappings = {
            'agent_name': getMeta(['agent_name', 'name']),
            'role': getMeta(['role', 'agent_role']),
            'plan': getMeta(['plan', 'planning']),
            'prompt': getMeta(['prompt', 'input', 'query', 'expression']),
            'response': getMeta(['response', 'output', 'result']),
            'tools_used': getMeta(['tools_used', 'tools']),
            'tool_name': getMeta(['tool_name', 'tool']),
            'query': getMeta(['query']),
            'expression': getMeta(['expression']),
            'route': getMeta(['route', 'routing']),
          };

          // Only add custom mappings if they have values
          for (const [key, value] of Object.entries(customMappings)) {
            if (value !== undefined && value !== null && !mergedMetadata[key]) {
              mergedMetadata[key] = value;
            }
          }

          return mergedMetadata;
        })()
      };

      // Add parsed chat messages from OTel events or legacy fields
      const chatMessages = extractChatFromEvents(trace.metadata || {});
      if (chatMessages.length > 0) {
        (trace as any).chat_messages = chatMessages;
      }

      console.log('[TraceDetailsPage] Raw details from API:', details);
      console.log('[TraceDetailsPage] details.metadata:', details.metadata);
      console.log('[TraceDetailsPage] details.attributes:', details.attributes);
      console.log('[TraceDetailsPage] Converted trace:', trace);
      console.log('[TraceDetailsPage] Has metadata?', !!trace.metadata);
      console.log('[TraceDetailsPage] Metadata keys:', trace.metadata ? Object.keys(trace.metadata).length : 0);
      console.log('[TraceDetailsPage] Metadata content:', trace.metadata);
      setTrace(trace);
      setSelectedSpan(trace);

      // Fetch observations for tree/graph views
      try {
        const obs = await agentreplayClient.getTraceObservations(
          id,
          tenantId ? parseInt(tenantId) : 1,
          projectId ? parseInt(projectId) : 0
        );
        console.log('[TraceDetailsPage] Fetched observations:', obs?.length || 0);
        setObservations(obs || []);
      } catch (obsError) {
        console.error('[TraceDetailsPage] Failed to fetch observations:', obsError);
        // Don't fail the whole page if observations fail
      }
    } catch (err: any) {
      // Better error message for 404s
      if (err?.response?.status === 404) {
        setError(`Span not found: ${id}. The span may have been deleted or the ID is incorrect.`);
      } else {
        setError(err instanceof Error ? err.message : 'Failed to fetch trace details');
      }
      console.error('Error fetching trace details:', err);
    } finally {
      setLoading(false);
    }
  };

  const formatTimestamp = (timestamp?: number) => {
    if (!timestamp) return 'N/A';
    return new Date(timestamp / 1000).toLocaleString();
  };

  const formatDuration = (durationMs?: number) => {
    if (!durationMs) return 'N/A';
    if (durationMs < 1000) return `${durationMs.toFixed(0)}ms`;
    return `${(durationMs / 1000).toFixed(2)}s`;
  };

  // Helper function to navigate back with preserved project context
  const handleBackToTraces = () => {
    if (fromSession) {
      navigate(`/projects/${projectId}/sessions`);
    } else if (projectId) {
      navigate(`/traces?project_id=${projectId}`);
    } else {
      navigate('/traces');
    }
  };

  // Build tree from flat observations
  const treeSpans = useMemo(() => {
    // Map status string to valid TraceTree status enum
    const mapStatus = (s?: string): 'success' | 'error' | 'pending' => {
      const status = (s || '').toLowerCase();
      if (status === 'error' || status === 'failed') return 'error';
      if (status === 'running' || status === 'pending') return 'pending';
      return 'success';
    };

    // Try to build synthetic child spans from rich trace metadata
    const syntheticChildren = trace ? buildSyntheticSpansFromMetadata(trace) : [];

    // If no observations, create spans from trace metadata
    if (!observations || observations.length === 0) {
      if (trace) {
        const startTimeMs = (trace.started_at || trace.timestamp_us || 0) / 1000;
        const durationMs = trace.duration_ms || 0;

        // Get a meaningful name for the root span
        const rootName = trace.operation_name ||
          trace.display_name ||
          trace.metadata?.['span.name'] ||
          trace.metadata?.['gen_ai.request.model'] ||
          trace.agent_name ||
          (trace.span_type !== 'Unknown' ? trace.span_type : null) ||
          'LLM Call';

        // Determine span type from metadata
        const hasToolCalls = Object.keys(trace.metadata || {}).some(k => k.includes('tool_calls'));
        const spanType = hasToolCalls ? 'function' :
          trace.metadata?.['gen_ai.system'] === 'openai' ? 'llm' :
            (trace.span_type || 'llm').toLowerCase();

        return [{
          id: trace.span_id || trace.trace_id || 'root',
          name: rootName,
          spanType: spanType as any,
          status: (trace.status || 'success').toLowerCase(),
          startTime: startTimeMs,
          endTime: startTimeMs + durationMs,
          duration: Math.round(durationMs),
          inputTokens: parseInt(trace.metadata?.['gen_ai.usage.input_tokens']) || trace.metadata?.input_tokens,
          outputTokens: parseInt(trace.metadata?.['gen_ai.usage.output_tokens']) || trace.metadata?.output_tokens,
          cost: trace.cost,
          metadata: trace.metadata,
          children: syntheticChildren
        }] as TreeSpan[];
      }
      return [];
    }

    const spanMap = new Map<string, TreeSpan>();
    const roots: TreeSpan[] = [];

    // First pass: create all span objects
    observations.forEach(obs => {
      const id = obs.span_id || obs.edge_id || obs.id;
      if (!id) return;

      // Handle timestamps properly
      let startTimeMs = 0;
      if (obs.start_time) {
        startTimeMs = obs.start_time > 4102444800000000 ? obs.start_time / 1000 :
          obs.start_time > 4102444800000 ? obs.start_time :
            obs.start_time;
      }

      const span: TreeSpan = {
        id,
        name: obs.name || obs.agent_name || obs.span_type || 'Unknown',
        spanType: (obs.type || obs.span_type || obs.spanType || 'span').toLowerCase(),
        status: mapStatus(obs.status),
        startTime: startTimeMs,
        endTime: startTimeMs + (obs.duration_ms || obs.duration || 0),
        duration: obs.duration_ms || obs.duration || 0,
        inputTokens: obs.usage?.input || obs.input_tokens || obs.inputTokens,
        outputTokens: obs.usage?.output || obs.output_tokens || obs.outputTokens,
        cost: obs.cost,
        metadata: obs.metadata || obs.attributes,
        children: []
      };
      spanMap.set(id, span);
    });

    // Add the root trace itself to the map if it's not already there
    // This fixes the issue where getTraceObservations excludes the root span
    if (trace) {
      const rootId = trace.span_id || trace.trace_id || 'root';
      if (!spanMap.has(rootId)) {
        const startTimeMs = (trace.started_at || trace.timestamp_us || 0) / 1000;
        const durationMs = trace.duration_ms || 0;
        const rootSpan: TreeSpan = {
          id: rootId,
          name: trace.operation_name || trace.display_name || trace.agent_name || 'Root Span',
          spanType: (trace.span_type || 'span').toLowerCase() as any,
          status: mapStatus(trace.status),
          startTime: startTimeMs,
          endTime: startTimeMs + durationMs,
          duration: Math.round(durationMs),
          inputTokens: trace.metadata?.input_tokens,
          outputTokens: trace.metadata?.output_tokens,
          cost: trace.cost,
          metadata: trace.metadata,
          children: syntheticChildren // Attach synthetic children to root if present
        };
        spanMap.set(rootId, rootSpan);
      } else if (syntheticChildren.length > 0) {
        // If root exists (from observations), attach synthetic children
        const root = spanMap.get(rootId);
        if (root && (!root.children || root.children.length === 0)) {
          root.children = syntheticChildren;
        }
      }
    }

    // Second pass: build hierarchy
    // Iterate over both observations AND the root trace (which might not be in observations)
    const allSpans = Array.from(spanMap.values());

    // We need to associate observations with their parents
    // Since we're iterating keys/values, we can't easily access the original 'obs' for parentId
    // But we can store parentId in the span object or loop observations again

    // Re-loop observations to link them
    observations.forEach(obs => {
      const id = obs.span_id || obs.edge_id || obs.id;
      if (!id) return;

      const span = spanMap.get(id);
      if (!span) return;

      const parentId = obs.parent_observation_id || obs.parent_span_id || obs.parent_edge_id;
      if (parentId && spanMap.has(parentId)) {
        const parent = spanMap.get(parentId);
        parent!.children!.push(span);
      } else {
        // If parent not found, it's a root (or orphan)
        // If we have a 'trace' object, and this obs belongs to it but has no parent, 
        // maybe it should be a child of 'trace'?
        // However, usually parentId should be set.
        // If parentId is the trace ID (root), we should find it in spanMap now.

        // Check if this span is the root trace itself
        const isRootTrace = trace && (id === trace.span_id || id === trace.trace_id);
        if (!isRootTrace) {
          // If we have a trace object and this is an orphan, check if we should attach to trace
          // But 'parentId' logic above handles 'trace.span_id' as parent.
          // If parentId is missing, it's a root.
          roots.push(span);
        }
      }
    });

    // Also check the main trace object - is it a root?
    if (trace) {
      const rootId = trace.span_id || trace.trace_id || 'root';
      const rootSpan = spanMap.get(rootId);
      if (rootSpan) {
        // The root span is almost always a root
        // Unless it has a parent_span_id
        if (!trace.parent_span_id || !spanMap.has(trace.parent_span_id)) {
          // It's a top-level root in this context
          // Check if we already added it (e.g. via observations loop if it was there and had no parent)
          if (!roots.includes(rootSpan)) {
            roots.push(rootSpan);
          }
        }
      }
    }

    // Sort by start time
    const sortSpans = (spans: TreeSpan[]) => {
      spans.sort((a, b) => a.startTime - b.startTime);
      spans.forEach(s => {
        if (s.children && s.children.length > 0) {
          sortSpans(s.children);
        }
      });
    };

    sortSpans(roots);

    // If we have synthetic children from trace metadata and only 1 root span with no/few children,
    // add the synthetic children to show the LLM conversation flow
    if (syntheticChildren.length > 0 && roots.length === 1) {
      const root = roots[0];
      // Only add synthetic children if the root doesn't have meaningful children already
      if (!root.children || root.children.length === 0) {
        root.children = syntheticChildren;
      } else if (root.children.length === 1) {
        // If there's just one generic child, replace with synthetic children
        const childType = root.children[0].spanType as string;
        if (childType === 'span' || childType === 'unknown') {
          root.children = syntheticChildren;
        }
      }
    }

    // If root is a generic HTTP span (like POST), try to give it a better name
    if (roots.length === 1 && trace) {
      const root = roots[0];
      const rootType = root.spanType as string;
      if (root.name === 'POST' || root.name === 'GET' || rootType === 'span' || rootType === 'http') {
        const betterName = trace.metadata?.['gen_ai.request.model'] ||
          trace.operation_name ||
          trace.display_name ||
          trace.agent_name;
        if (betterName) {
          root.name = `chat ${betterName}`;
          root.spanType = 'llm' as any;
        }
      }
    }

    return roots;
  }, [observations, trace]);



  // Construct replay messages for SessionReplay view
  const replayMessages = useMemo(() => {
    const msgs: any[] = [];

    // Strategy 1: Use synthetic children if available
    if (trace) {
      const synthetic = buildSyntheticSpansFromMetadata(trace);
      if (synthetic.length > 0) {
        synthetic.forEach((span, idx) => {
          if (span.metadata?.role && span.metadata?.content) {
            msgs.push({
              id: span.id || `synthetic-${idx}`,
              role: span.metadata.role,
              content: span.metadata.content,
              timestamp: span.startTime * 1000,
              traceId: trace.trace_id,
              cost: span.cost,
              tokens: (span.inputTokens || 0) + (span.outputTokens || 0)
            });
          }
        });
      }
    }

    // Strategy 2: If we have fine-grained observations, use those
    if (msgs.length === 0 && observations.length > 0) {
      observations.forEach(obs => {
        const role = obs.metadata?.role || obs.attributes?.role;
        const content = obs.metadata?.content || obs.attributes?.content || obs.input_preview || obs.output_preview;

        if (role && content) {
          msgs.push({
            id: obs.span_id || obs.id,
            role: role,
            content: typeof content === 'string' ? content : JSON.stringify(content),
            timestamp: (obs.start_time ? (obs.start_time > 1e12 ? obs.start_time / 1000 : obs.start_time) : 0),
            traceId: obs.trace_id || trace?.trace_id || '',
            cost: obs.cost,
            tokens: obs.tokens || (obs.usage?.total)
          });
        }
      });
      msgs.sort((a, b) => a.timestamp - b.timestamp);
    }

    return msgs;
  }, [trace, observations]);

  if (loading) {
    return (
      <div className="min-h-screen bg-background flex items-center justify-center">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary"></div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="min-h-screen bg-background">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
          <button
            onClick={handleBackToTraces}
            className="flex items-center gap-2 text-textSecondary hover:text-textPrimary mb-6"
          >
            <ArrowLeft className="w-5 h-5" />
            {fromSession ? 'Back to Session' : 'Back to Traces'}
          </button>
          <div className="p-4 bg-error-bg border border-red-500/20 rounded-lg">
            <p className="text-red-500">{error}</p>
          </div>
        </div>
      </div>
    );
  }

  const handleEvaluate = () => {
    navigate(`/evals/pipeline`);
  };

  const handleDelete = () => {
    setShowDeleteConfirm(true);
  };

  const confirmDelete = async () => {
    setIsDeleting(true);
    try {
      if (traceId) {
        await agentreplayClient.deleteTrace(traceId, Number(projectId || 0));
        navigate(`/projects/${projectId || 0}/traces`);
      }
    } catch (err: any) {
      console.error('Failed to delete trace:', err);
      setError(`Failed to delete trace: ${err.message || 'Unknown error'}`);
      setIsDeleting(false);
      setShowDeleteConfirm(false);
    }
  };

  if (!trace) {
    return (
      <div className="min-h-screen bg-background">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
          <button
            onClick={handleBackToTraces}
            className="flex items-center gap-2 text-textSecondary hover:text-textPrimary mb-6"
          >
            <ArrowLeft className="w-5 h-5" />
            Back to Traces
            Back to Traces
          </button>
          <div className="text-center py-12">
            <Activity className="w-16 h-16 text-textTertiary mx-auto mb-4" />
            <h3 className="text-xl font-semibold text-textPrimary mb-2">Trace not found</h3>
          </div>
        </div>
      </div>
    );
  }

  const totalDuration = trace.ended_at && trace.started_at
    ? (trace.ended_at - trace.started_at) / 1000
    : 0;

  return (
    <div className="min-h-screen bg-background">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
        {/* Breadcrumb Navigation */}
        <div className="flex items-center gap-2 text-sm text-textTertiary mb-4">
          <button onClick={handleBackToTraces} className="hover:text-textPrimary transition-colors">
            Traces
          </button>
          <span>/</span>
          <span className="text-textPrimary font-medium">Trace Details</span>
        </div>

        {/* Hero Section with Key Metrics */}
        <div className="bg-surface border border-border rounded-xl p-6 mb-6">
          <div className="flex items-start justify-between mb-4">
            <div className="flex-1">
              <div className="flex items-center gap-3 mb-2">
                <h1 className="text-2xl font-bold text-textPrimary">
                  {trace.display_name ||
                    trace.operation_name ||
                    (trace.metadata?.['span.name'] as string) ||
                    (trace.metadata?.operation_name as string) ||
                    (trace.span_type !== 'Unknown' ? trace.span_type : null) ||
                    'LLM Call'}
                </h1>
                <span className={`px-3 py-1 text-sm font-medium rounded-full ${trace.status === 'completed'
                  ? 'bg-success/10 text-success border border-success/20'
                  : trace.status === 'error'
                    ? 'bg-error/10 text-error border border-error/20'
                    : 'bg-muted/10 text-textSecondary border border-border'
                  }`}>
                  {trace.status === 'completed' ? '✓ Success' : trace.status === 'error' ? '✗ Error' : trace.status}
                </span>
              </div>
              <div className="text-sm text-textSecondary mb-1">
                {(trace.metadata?.['gen_ai.request.model'] as string) ||
                  (trace.metadata?.model as string) ||
                  trace.model ||
                  'LLM'}
                {trace.metadata?.['gen_ai.system'] && (
                  <span className="ml-2 text-textTertiary">via {trace.metadata['gen_ai.system']}</span>
                )}
              </div>
              <div className="font-mono text-xs text-textTertiary">ID: {trace.trace_id}</div>
            </div>

            {/* Quick Actions */}
            <div className="flex gap-2">
              <button
                onClick={() => navigator.clipboard.writeText(trace.trace_id)}
                className="flex items-center gap-2 px-3 py-2 bg-background border border-border rounded-lg hover:bg-surface-hover transition-colors text-sm font-medium"
                title="Copy Trace ID"
              >
                📋 Copy ID
              </button>
              <button
                onClick={() => {
                  const data = JSON.stringify(trace, null, 2);
                  const blob = new Blob([data], { type: 'application/json' });
                  const url = URL.createObjectURL(blob);
                  const a = document.createElement('a');
                  a.href = url;
                  a.download = `trace_${trace.trace_id}.json`;
                  a.click();
                }}
                className="flex items-center gap-2 px-3 py-2 bg-background border border-border rounded-lg hover:bg-surface-hover transition-colors text-sm font-medium"
                title="Export as JSON"
              >
                ⬇️ Export
              </button>
              <CommitTraceButton
                traceId={trace.trace_id}
                spanName={trace.display_name || trace.operation_name || 'Root Span'}
                model={(trace.metadata?.['gen_ai.request.model'] as string) || (trace.metadata?.model as string) || trace.model}
                input={extractInputOutput(trace.metadata || {}).input}
                output={extractInputOutput(trace.metadata || {}).output}
                messages={extractInputOutput(trace.metadata || {}).messages}
                tools={extractInputOutput(trace.metadata || {}).tools}
                latencyMs={trace.duration_ms}
                cost={trace.metadata?.cost as number}
              />
              <EvaluateTraceButton traceId={trace.trace_id} traceMetadata={trace.metadata} />
              <AddToDatasetButton traceId={trace.trace_id} traceMetadata={trace.metadata} />
              <button
                onClick={handleDelete}
                className="flex items-center gap-2 px-3 py-2 bg-red-50 text-red-600 border border-red-200 rounded-lg hover:bg-red-100 transition-colors text-sm font-medium ml-2"
                title="Delete Trace"
              >
                <Trash className="w-4 h-4" />
                Delete
              </button>
            </div>
          </div>

          {/* Key Metrics Bar */}
          <div className="grid grid-cols-4 gap-4 pt-4 border-t border-border/50">
            <div>
              <div className="text-xs text-textTertiary mb-1">LATENCY</div>
              <div className={`text-2xl font-bold tabular-nums ${(trace.duration_ms || 0) < 1000 ? 'text-success' :
                (trace.duration_ms || 0) < 5000 ? 'text-warning' :
                  'text-error'
                }`}>
                {formatDuration(trace.duration_ms)}
              </div>
            </div>
            <div>
              <div className="text-xs text-textTertiary mb-1">COST</div>
              <div className="text-2xl font-bold tabular-nums text-textPrimary">
                ${(trace.metadata?.cost as number || 0).toFixed(4)}
              </div>
            </div>
            <div>
              <div className="text-xs text-textTertiary mb-1">TOKENS</div>
              <div className="text-2xl font-bold tabular-nums text-textPrimary">
                {trace.token_count?.toLocaleString() || '0'}
              </div>
              {trace.metadata?.token_breakdown && (
                <div className="text-xs text-textSecondary mt-1">
                  {(trace.metadata.token_breakdown as any).input_tokens || 0} → {(trace.metadata.token_breakdown as any).output_tokens || 0}
                </div>
              )}

            </div>
            <div>
              <div className="text-xs text-textTertiary mb-1">AGENT</div>
              <div className="text-lg font-semibold text-textPrimary truncate">
                {trace.agent_name || `agent_${trace.agent_id}`}
              </div>
            </div>
          </div>
        </div>

        {/* Split Pane View Layout */}
        <div className="flex h-[calc(100vh-200px)] bg-surface border border-border rounded-lg overflow-hidden shadow-sm">
          {/* LEFT PANE: Visualization (Tree / Timeline / Graph) */}
          <div className={`${(['graph', 'replay', 'flamegraph'].includes(viewMode) || !showInspector) ? 'flex-1' : 'w-[35%] min-w-[320px] max-w-[50%]'} flex flex-col border-r border-border bg-background transition-all duration-300`}>
            {/* View Toggle Toolbar */}
            <div className="flex items-center gap-1 p-2 border-b border-border bg-surface-elevated overflow-x-auto no-scrollbar">
              <button
                onClick={() => setViewMode('list')}
                className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium transition-colors whitespace-nowrap ${viewMode === 'list'
                  ? 'bg-primary/10 text-primary border border-primary/20'
                  : 'text-textSecondary hover:text-textPrimary hover:bg-surface-hover border border-transparent'
                  }`}
                title="Hierarchical Tree View"
              >
                <List className="w-3.5 h-3.5" />
                List
              </button>

              <button
                onClick={() => setViewMode('replay')}
                className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium transition-colors whitespace-nowrap ${viewMode === 'replay'
                  ? 'bg-primary/10 text-primary border border-primary/20'
                  : 'text-textSecondary hover:text-textPrimary hover:bg-surface-hover border border-transparent'
                  }`}
                title="Session Replay"
              >
                <Play className="w-3.5 h-3.5" />
                Replay
              </button>
              <button
                onClick={() => setViewMode('graph')}
                className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium transition-colors whitespace-nowrap ${viewMode === 'graph'
                  ? 'bg-primary/10 text-primary border border-primary/20'
                  : 'text-textSecondary hover:text-textPrimary hover:bg-surface-hover border border-transparent'
                  }`}
                title="Dependency Graph"
              >
                <GitBranch className="w-3.5 h-3.5" />
                Graph
              </button>
              <div className="w-px h-4 bg-border/50 mx-1" />
              <button
                onClick={() => setViewMode('flamegraph')}
                className={`p-1.5 rounded text-textSecondary hover:text-textPrimary hover:bg-surface-hover transition-colors ${viewMode === 'flamegraph' ? 'bg-primary/10 text-primary' : ''}`}
                title="Flame Graph"
              >
                <Flame className="w-4 h-4" />
              </button>
              <button
                onClick={() => setViewMode('ai')}
                className={`p-1.5 rounded text-textSecondary hover:text-textPrimary hover:bg-surface-hover transition-colors ${viewMode === 'ai' ? 'bg-primary/10 text-primary' : ''}`}
                title="AI Analysis"
              >
                <Sparkles className="w-4 h-4" />
              </button>
              <div className="border-l border-border/50 mx-1 h-4" />
              <button
                onClick={() => setShowInspector(!showInspector)}
                className={`p-1.5 rounded text-textSecondary hover:text-textPrimary hover:bg-surface-hover transition-colors ${!showInspector ? 'bg-primary/10 text-primary' : ''}`}
                title={showInspector ? "Hide Details" : "Show Details"}
              >
                {showInspector ? <PanelRightClose className="w-4 h-4" /> : <PanelRightOpen className="w-4 h-4" />}
              </button>
            </div>

            {/* View Content */}
            <div className="flex-1 overflow-hidden relative">
              {viewMode === 'list' && (
                <div className="h-full overflow-hidden">
                  <TraceTree
                    spans={treeSpans}
                    selectedSpanId={selectedSpan?.span_id}
                    onSelectSpan={(span) => {
                      // Logic to find observation and update selectedSpan - duplicated from original
                      const obs = observations.find(o => o.span_id === span.id || o.edge_id === span.id || o.id === span.id);
                      if (obs) {
                        const meta = { ...trace, ...obs }; // Simplified merge, use detailed logic if needed
                        // We need to reconstruct full metadata object roughly
                        const fullMeta: TraceMetadata = {
                          trace_id: trace.trace_id,
                          span_id: obs.id || obs.span_id || obs.edge_id,
                          parent_span_id: obs.parent_span_id || obs.parent_edge_id,
                          tenant_id: trace.tenant_id,
                          project_id: trace.project_id,
                          agent_id: obs.agent_id,
                          agent_name: obs.agent_name || obs.name,
                          session_id: trace.session_id,
                          span_type: obs.span_type || obs.spanType,
                          environment: trace.environment,
                          timestamp_us: obs.startTime ? obs.startTime * 1000 : (obs.start_time ? obs.start_time * 1000 : 0),
                          duration_us: obs.duration ? obs.duration * 1000 : (obs.duration_us || (obs.duration_ms ? obs.duration_ms * 1000 : 0)),
                          token_count: obs.tokens || (obs.inputTokens || 0) + (obs.outputTokens || 0),
                          sensitivity_flags: 0,
                          metadata: obs.metadata || obs.attributes,
                          status: obs.status,
                          duration_ms: obs.duration || obs.duration_ms,
                          started_at: obs.startTime || obs.start_time,
                        };
                        setSelectedSpan(fullMeta);
                      } else {
                        // Synthetic span (from metadata parsing)
                        const meta: TraceMetadata = {
                          trace_id: trace.trace_id,
                          span_id: span.id,
                          span_type: span.spanType,
                          agent_name: span.name,
                          timestamp_us: span.startTime * 1000,
                          duration_us: span.duration * 1000,
                          status: span.status,
                          duration_ms: span.duration,
                          started_at: span.startTime,
                          metadata: span.metadata,
                          parent_span_id: trace.span_id, // Assume child of root for synthetic
                          // Defaults
                          tenant_id: trace.tenant_id,
                          project_id: trace.project_id,
                          agent_id: trace.agent_id,
                          session_id: trace.session_id,
                          environment: trace.environment,
                          token_count: (span.inputTokens || 0) + (span.outputTokens || 0),
                          sensitivity_flags: 0,
                        };
                        setSelectedSpan(meta);
                      }
                    }}
                  />
                </div>
              )}



              {viewMode === 'replay' && (
                <div className="h-full overflow-hidden p-2">
                  <SessionReplay
                    messages={replayMessages}
                    onMessageSelect={(msg) => {
                      const obs = observations.find(o => o.span_id === msg.id || o.id === msg.id);
                      if (obs) {
                        const meta = { ...trace, ...obs, span_id: obs.span_id || obs.id, metadata: obs.metadata || obs.attributes, attributes: obs.attributes };
                        setSelectedSpan(meta as any);
                      } else if (trace) {
                        const synthetic: TraceMetadata = {
                          ...trace,
                          display_name: `Message: ${msg.role}`,
                          metadata: { role: msg.role, content: msg.content },
                          span_type: 'message'
                        };
                        setSelectedSpan(synthetic);
                      }
                    }}
                  />
                </div>
              )}

              {viewMode === 'graph' && (
                <div className="h-full overflow-hidden">
                  <TraceGraphView
                    traceId={traceId!}
                    tenantId={tenantId ? parseInt(tenantId) : 1}
                    projectId={projectId ? parseInt(projectId) : undefined}
                    trace={trace}
                    onNodeClick={(node) => {
                      const obs = observations.find(o => o.edge_id === node.span_id || o.span_id === node.span_id);
                      if (obs) {
                        setSelectedSpan({ ...trace, ...obs, span_id: node.span_id, metadata: obs.metadata || obs.attributes, attributes: obs.attributes });
                      }
                    }}
                  />
                </div>
              )}

              {viewMode === 'flamegraph' && (
                <FlameGraph
                  spans={treeSpans}
                  selectedSpanId={selectedSpan?.span_id}
                  onSelectSpan={(span) => {
                    // logic similar to list view
                  }}
                />
              )}

              {viewMode === 'ai' && (
                <AITraceAnalysis traceId={traceId!} observations={observations} />
              )}
            </div>
          </div>


          {/* RIGHT PANE: Detail Inspector */}
          <div className={`${!showInspector ? 'w-0 hidden' : (['graph', 'replay', 'flamegraph'].includes(viewMode) ? 'w-[450px] border-l border-border' : 'flex-1')} flex flex-col bg-background min-w-0 overflow-hidden relative transition-all duration-300`}>
            {selectedSpan ? (
              <SpanInspector
                trace={selectedSpan}
                hideTabs={false}
                onClose={() => setSelectedSpan(null)}
              // Optionally pass initial active tab if needed
              />
            ) : (
              <div className="flex flex-col items-center justify-center h-full text-textTertiary bg-surface/30">
                <Activity className="w-12 h-12 mb-3 opacity-20" />
                <p className="text-sm">Select a span from the list to view details</p>
              </div>
            )}
          </div>
        </div>
      </div>
      <ConfirmationDialog
        isOpen={showDeleteConfirm}
        onClose={() => setShowDeleteConfirm(false)}
        onConfirm={confirmDelete}
        title="Delete Trace"
        message="Are you sure you want to delete this trace? This action cannot be undone."
        isProcessing={isDeleting}
      />
    </div >
  );
}

interface ConfirmationDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  message: string;
  isProcessing?: boolean;
}

const ConfirmationDialog = ({ isOpen, onClose, onConfirm, title, message, isProcessing }: ConfirmationDialogProps) => {
  if (!isOpen) return null;
  return createPortal(
    <div className="fixed inset-0 bg-black/50 z-[100] flex items-center justify-center p-4">
      <div className="bg-surface border border-border rounded-xl p-6 max-w-md w-full shadow-xl animate-in fade-in zoom-in-95 duration-200">
        <h3 className="text-lg font-semibold text-textPrimary mb-2">{title}</h3>
        <p className="text-textSecondary mb-6">{message}</p>
        <div className="flex justify-end gap-3">
          <button
            onClick={onClose}
            disabled={isProcessing}
            className="px-4 py-2 text-sm font-medium text-textSecondary hover:text-textPrimary hover:bg-surface-hover rounded-lg transition-colors disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            disabled={isProcessing}
            className="px-4 py-2 text-sm font-medium text-white bg-red-600 hover:bg-red-700 rounded-lg transition-colors shadow-sm flex items-center gap-2 disabled:opacity-70"
          >
            {isProcessing ? (
              <>
                <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                Deleting...
              </>
            ) : (
              'Delete'
            )}
          </button>
        </div>
      </div>
    </div>,
    document.body
  );
};
