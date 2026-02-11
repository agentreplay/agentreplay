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

import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  Activity,
  AlertTriangle,
  ArrowLeft,
  Clock,
  Download,
  Loader2,
  MessageCircle,
  Sparkles,
  User,
  Zap,
  Play,
  Wrench,
  Bot,
  Cpu,
} from 'lucide-react';
import { agentreplayClient, TraceMetadata } from '../lib/agentreplay-api';
import { Button } from '../../components/ui/button';
import { cn } from '../../lib/utils';
import { SessionReplay } from '../../components/session/SessionReplay';
import { TraceTimelineView } from '../../components/trace/TraceTimelineView';

interface SessionMeta {
  sessionId: string;
  userLabel?: string;
  startedAt?: number;
  endedAt?: number;
  totalDurationMs?: number;
  totalCost?: number;
  totalTokens?: number;
  traceCount: number;
}

interface SessionMessage {
  id: string;
  role: 'system' | 'user' | 'assistant' | 'tool';
  content: string;
  timestamp: number;
  traceId: string;
  cost?: number;
  tokens?: number;
}

// Full span structure for waterfall view
interface SessionSpan {
  span_id: string;
  trace_id: string;
  parent_span_id?: string;
  name: string;
  start_time: number;
  end_time: number;
  duration_ms: number;
  span_type: 'llm' | 'tool' | 'agent' | 'chain' | 'retrieval' | 'other';
  status: string;
  attributes: Record<string, any>;
  model?: string;
  tokens?: number;
  cost?: number;
}

export default function SessionDetail() {
  const { projectId, sessionId } = useParams<{ projectId: string; sessionId: string }>();
  const navigate = useNavigate();
  const [meta, setMeta] = useState<SessionMeta | null>(null);
  const [messages, setMessages] = useState<SessionMessage[]>([]);
  const [sessionSpans, setSessionSpans] = useState<SessionSpan[]>([]);
  const [rawTraces, setRawTraces] = useState<TraceMetadata[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<'transcript' | 'replay' | 'waterfall'>('transcript');

  useEffect(() => {
    if (!sessionId) {
      return;
    }

    const loadSession = async () => {
      setLoading(true);
      setError(null);
      try {
        const response = await agentreplayClient.listTraces({ limit: 400, offset: 0 });
        const sessionTraces = (response.traces || []).filter((trace) => trace.session_id?.toString() === sessionId);

        if (sessionTraces.length === 0) {
          setMeta({ sessionId, traceCount: 0 });
          setMessages([]);
          setSessionSpans([]);
          setRawTraces([]);
          return;
        }

        const ordered = [...sessionTraces].sort((a, b) => (a.timestamp_us ?? 0) - (b.timestamp_us ?? 0));
        setRawTraces(ordered);

        // Calculate wall-clock duration (end of last trace - start of first trace)
        const firstStart = ordered[0].timestamp_us ?? 0;
        const lastTrace = ordered[ordered.length - 1];
        const lastEnd = (lastTrace.timestamp_us ?? 0) + (lastTrace.duration_us ?? 0);

        const sessionMeta: SessionMeta = {
          sessionId,
          userLabel: ordered[0].metadata?.user_id || ordered[0].metadata?.session_user,
          startedAt: firstStart,
          endedAt: lastEnd, // BUG-04 FIX: Use actual end time, not start time of last trace
          totalDurationMs: (lastEnd - firstStart) / 1000, // BUG-01 FIX: Wall-clock duration, not sum of spans
          totalCost: ordered.reduce((sum, trace) => {
            // BUG-03 FIX: Extract cost from metadata if not in trace.cost
            const cost = trace.cost ||
              Number(trace.metadata?.['gen_ai.usage.cost'] || 0) ||
              Number(trace.metadata?.['llm.usage.cost'] || 0);
            return sum + cost;
          }, 0),
          totalTokens: ordered.reduce((sum, trace) => {
            // BUG-02 FIX: Extract tokens from metadata if not in trace.token_count
            const tokens = trace.token_count ||
              (Number(trace.metadata?.['gen_ai.usage.input_tokens'] || 0) +
                Number(trace.metadata?.['gen_ai.usage.output_tokens'] || 0)) ||
              Number(trace.metadata?.['llm.usage.total_tokens'] || 0);
            return sum + tokens;
          }, 0),
          traceCount: ordered.length,
        };

        setMeta(sessionMeta);

        // Fetch the FULL SPAN TREE for each trace in the session
        // The tree endpoint returns all spans with their attributes including gen_ai.* data
        const allSpans: SessionSpan[] = [];
        const allMessages: SessionMessage[] = [];

        for (const trace of ordered) {
          try {
            // Use getTraceTree to get the complete span hierarchy with all attributes
            const tree = await agentreplayClient.getTraceTree(trace.trace_id);

            if (tree && tree.spans && Array.isArray(tree.spans) && tree.spans.length > 0) {
              // Convert each span from the tree to our SessionSpan format
              tree.spans.forEach((span: any) => {
                allSpans.push(extractSpanFromTreeSpan(span, trace.trace_id));
                // Extract messages from LLM spans using GenAI payload schema
                const spanMessages = extractMessagesFromTreeSpan(span, trace.trace_id);
                allMessages.push(...spanMessages);
              });
            } else {
              // Fallback: create span from trace metadata if no tree available
              allSpans.push(extractSpanFromTrace(trace));
              // Fallback: extract messages from trace metadata
              allMessages.push(...hydrateMessagesFromTrace(trace));
            }
          } catch (e) {
            // If tree fetch fails, fallback to trace-level span
            console.warn(`Could not fetch span tree for trace ${trace.trace_id}:`, e);
            allSpans.push(extractSpanFromTrace(trace));
            allMessages.push(...hydrateMessagesFromTrace(trace));
          }
        }

        // Sort spans by start time
        allSpans.sort((a, b) => a.start_time - b.start_time);
        setSessionSpans(allSpans);

        // Sort messages by timestamp and set
        allMessages.sort((a, b) => a.timestamp - b.timestamp);
        setMessages(allMessages);
      } catch (err) {
        console.error('Failed to load session', err);
        setError(err instanceof Error ? err.message : 'Unable to load session');
      } finally {
        setLoading(false);
      }
    };

    loadSession();
  }, [sessionId]);

  const totalTurns = useMemo(() => messages.filter((msg) => msg.role !== 'system').length, [messages]);

  const downloadTranscript = () => {
    if (!messages.length) return;
    const blob = new Blob([
      messages
        .map((msg) => `[${new Date(msg.timestamp).toLocaleString()}] ${msg.role.toUpperCase()}: ${msg.content}`)
        .join('\n\n'),
    ]);
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `session-${sessionId}.txt`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  if (!sessionId) {
    return <div className="text-sm text-textSecondary">Missing session id.</div>;
  }

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center text-textSecondary">
        <Loader2 className="mr-2 h-4 w-4 animate-spin" /> Loading conversation…
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
        <AlertTriangle className="h-8 w-8 text-amber-600 dark:text-amber-400" />
        <p className="text-sm text-textSecondary">{error}</p>
        <Button variant="outline" size="sm" onClick={() => navigate(-1)}>
          Go back
        </Button>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-6">
      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <div>
            <button className="mb-1 flex items-center gap-2 text-xs uppercase tracking-widest text-textTertiary" onClick={() => navigate(-1)}>
              <ArrowLeft className="h-3 w-3" /> Sessions
            </button>
            <h1 className="text-2xl font-semibold text-textPrimary">Session {sessionId}</h1>
            <p className="text-sm text-textSecondary">{meta?.userLabel ? `User ${meta.userLabel}` : 'Anonymous user'}</p>
          </div>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              className="gap-2"
              onClick={() => {
                if (!messages.length) return;
                // Export as proper JSONL format
                const jsonlData = messages.map(msg => JSON.stringify({
                  role: msg.role,
                  content: msg.content,
                  timestamp: msg.timestamp,
                  trace_id: msg.traceId,
                  tokens: msg.tokens,
                  cost: msg.cost
                })).join('\n');
                const blob = new Blob([jsonlData], { type: 'application/jsonl' });
                const url = URL.createObjectURL(blob);
                const anchor = document.createElement('a');
                anchor.href = url;
                anchor.download = `session-${sessionId}.jsonl`;
                anchor.click();
                URL.revokeObjectURL(url);
              }}
            >
              <Download className="h-4 w-4" /> Export JSONL
            </Button>
            <Button
              variant="default"
              size="sm"
              className="gap-2"
              onClick={() => {
                // Convert session messages to OpenAI format and open playground
                const formattedMessages = messages.map(msg => ({
                  role: msg.role,
                  content: msg.content
                }));
                // Encode messages for URL
                const encoded = encodeURIComponent(JSON.stringify(formattedMessages));
                // Open in OpenAI Playground (or fallback to copying to clipboard)
                const playgroundUrl = `https://platform.openai.com/playground?mode=chat&messages=${encoded}`;
                try {
                  window.open(playgroundUrl, '_blank');
                } catch (e) {
                  // Fallback: copy messages to clipboard
                  navigator.clipboard.writeText(JSON.stringify(formattedMessages, null, 2));
                  alert('Messages copied to clipboard. Paste them in your preferred LLM playground.');
                }
              }}
            >
              <Sparkles className="h-4 w-4" /> Open in Playground
            </Button>
          </div>
        </div>
      </div>

      <section className="grid gap-4 md:grid-cols-4">
        <SummaryCard icon={MessageCircle} label="Turns" value={totalTurns.toString()} detail={`${meta?.traceCount ?? 0} traces`} />
        <SummaryCard icon={Clock} label="Duration" value={formatDuration(meta?.totalDurationMs)} detail={formatWindow(meta?.startedAt, meta?.endedAt)} />
        <SummaryCard icon={Zap} label="Tokens" value={(meta?.totalTokens ?? 0).toLocaleString()} detail="input + output" />
        <SummaryCard icon={Activity} label="Cost" value={`$${(meta?.totalCost ?? 0).toFixed(4)}`} detail="est. USD" />
      </section>

      <section className="flex flex-1 gap-6">
        <div className="flex-1 rounded-3xl border border-border/60 bg-background/90 p-4">
          <header className="flex items-center justify-between border-b border-border/60 pb-3">
            <div>
              <p className="text-xs uppercase tracking-widest text-textTertiary">Conversation</p>
              <p className="text-sm text-textSecondary">
                {viewMode === 'replay'
                  ? 'Interactive replay with playback controls.'
                  : 'Full transcript with latency + cost markers.'
                }
              </p>
            </div>

            {/* View Mode Toggle */}
            <div className="flex items-center gap-1 bg-surface rounded-lg p-1">
              <button
                onClick={() => setViewMode('transcript')}
                className={cn(
                  "px-3 py-1.5 text-xs font-medium rounded-md transition-colors",
                  viewMode === 'transcript'
                    ? "bg-primary text-white"
                    : "text-textSecondary hover:text-textPrimary hover:bg-surface-hover"
                )}
              >
                Transcript
              </button>
              <button
                onClick={() => setViewMode('waterfall')}
                className={cn(
                  "px-3 py-1.5 text-xs font-medium rounded-md transition-colors flex items-center gap-1",
                  viewMode === 'waterfall'
                    ? "bg-primary text-white"
                    : "text-textSecondary hover:text-textPrimary hover:bg-surface-hover"
                )}
              >
                <Activity className="w-3 h-3" />
                Waterfall
              </button>
              <button
                onClick={() => setViewMode('replay')}
                className={cn(
                  "px-3 py-1.5 text-xs font-medium rounded-md transition-colors flex items-center gap-1",
                  viewMode === 'replay'
                    ? "bg-primary text-white"
                    : "text-textSecondary hover:text-textPrimary hover:bg-surface-hover"
                )}
              >
                <Play className="w-3 h-3" />
                Replay
              </button>
            </div>
          </header>

          {viewMode === 'replay' ? (
            <div className="mt-4 h-[520px]">
              <SessionReplay
                messages={messages}
                onTraceClick={(traceId) => {
                  if (projectId) {
                    navigate(`/projects/${projectId}/traces/${traceId}`);
                  }
                }}
              />
            </div>
          ) : viewMode === 'waterfall' ? (
            <div className="mt-4 h-[520px]">
              {sessionSpans.length === 0 ? (
                <div className="flex h-full flex-col items-center justify-center text-textSecondary">
                  <Activity className="mb-3 h-8 w-8" />
                  No trace spans found for this session.
                </div>
              ) : (
                <TraceTimelineView
                  spans={sessionSpans.map(span => ({
                    span_id: span.span_id,
                    trace_id: span.trace_id,
                    parent_span_id: span.parent_span_id,
                    name: span.name,
                    start_time: span.start_time,
                    end_time: span.end_time,
                    attributes: {
                      ...span.attributes,
                      'service.name': span.span_type,
                      'span.kind': span.span_type,
                      'tokens': span.tokens,
                      'cost': span.cost,
                      'model': span.model,
                      'duration_ms': span.duration_ms,
                    },
                    status: span.status
                  }))}
                  onSpanClick={(span) => {
                    if (projectId) {
                      navigate(`/projects/${projectId}/traces/${span.trace_id}`);
                    }
                  }}
                />
              )}

              {/* Span Details Panel */}
              <div className="mt-4 grid grid-cols-3 gap-3">
                {sessionSpans.slice(0, 6).map((span) => (
                  <div
                    key={span.span_id}
                    className="rounded-xl border border-border/60 bg-surface/70 p-3 cursor-pointer hover:bg-surface-hover transition-colors"
                    onClick={() => projectId && navigate(`/projects/${projectId}/traces/${span.trace_id}`)}
                  >
                    <div className="flex items-center gap-2 mb-2">
                      <SpanTypeIcon type={span.span_type} />
                      <span className="text-xs font-medium text-textPrimary truncate">{span.name}</span>
                    </div>
                    <div className="grid grid-cols-2 gap-1 text-[11px] text-textSecondary">
                      <span>Duration: {span.duration_ms.toFixed(0)}ms</span>
                      {span.tokens && <span>Tokens: {span.tokens}</span>}
                      {span.cost && <span>Cost: ${span.cost.toFixed(4)}</span>}
                      {span.model && <span className="truncate">Model: {span.model}</span>}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <div className="mt-4 flex h-[520px] flex-col gap-4 overflow-y-auto pr-2">
              {messages.length === 0 ? (
                <div className="flex flex-1 flex-col items-center justify-center text-textSecondary">
                  <MessageCircle className="mb-3 h-8 w-8" />
                  No turns captured for this session.
                </div>
              ) : (
                messages.map((message) => (
                  <article
                    key={message.id}
                    className={cn('rounded-2xl border border-border/60 bg-surface/70 p-4 text-sm', {
                      'bg-primary/10 border-primary/40 text-primary-foreground': message.role === 'assistant',
                    })}
                  >
                    <div className="mb-2 flex items-center justify-between text-xs text-textTertiary">
                      <span className="flex items-center gap-2 font-semibold text-textSecondary">
                        <Badge role={message.role} />
                        {new Date(message.timestamp / 1000).toLocaleTimeString()}
                      </span>
                      <span className="flex items-center gap-4 text-textTertiary">
                        {typeof message.tokens === 'number' && <span>{message.tokens.toLocaleString()} tokens</span>}
                        {typeof message.cost === 'number' && <span>${message.cost.toFixed(4)}</span>}
                        <button
                          className="text-primary underline-offset-2 hover:underline"
                          onClick={() => {
                            if (projectId) {
                              navigate(`/projects/${projectId}/traces/${message.traceId}`);
                            }
                          }}
                        >
                          Trace
                        </button>
                      </span>
                    </div>
                    <pre className="whitespace-pre-wrap font-sans text-[13px] leading-relaxed text-textPrimary">
                      {message.content}
                    </pre>
                  </article>
                ))
              )}
            </div>
          )}
        </div>
        <aside className="w-80 rounded-3xl border border-border/60 bg-background/80 p-4">
          <p className="text-xs uppercase tracking-widest text-textTertiary">Flags & Insights</p>
          <div className="mt-3 space-y-3 text-sm">
            <InsightRow label="Hallucination" value={messages.some((msg) => msg.content.toLowerCase().includes('hallucination')) ? 'Possible' : 'None detected'} />
            <InsightRow label="User sentiment" value={scoreSentiment(messages)} />
            <InsightRow label="Live cost burn" value={`$${((meta?.totalCost ?? 0) / Math.max(1, (meta?.traceCount ?? 1))).toFixed(4)} / turn`} />
            <InsightRow label="Last activity" value={meta?.endedAt ? new Date(meta.endedAt / 1000).toLocaleString() : '—'} />
          </div>
        </aside>
      </section>
    </div>
  );
}

// Extract messages from tree span attributes using GenAI payload schema
// This parses gen_ai.prompt.N.role, gen_ai.prompt.N.content, gen_ai.completion.0.content
function extractMessagesFromTreeSpan(span: any, rootTraceId: string): SessionMessage[] {
  const messages: SessionMessage[] = [];
  const attrs = span.attributes || {};
  const baseTimestamp = span.start_time || 0;

  // Find all prompt messages using GenAI semantic conventions
  // Pattern: gen_ai.prompt.N.role, gen_ai.prompt.N.content
  const promptIndices = new Set<number>();
  const promptPattern = /^gen_ai\.prompt\.(\d+)\.(role|content)$/;

  for (const key of Object.keys(attrs)) {
    const match = key.match(promptPattern);
    if (match) {
      promptIndices.add(parseInt(match[1], 10));
    }
  }

  // Sort indices and extract messages
  const sortedIndices = Array.from(promptIndices).sort((a, b) => a - b);
  for (const idx of sortedIndices) {
    const role = (attrs[`gen_ai.prompt.${idx}.role`] || 'user') as string;
    const content = attrs[`gen_ai.prompt.${idx}.content`] as string;

    if (content) {
      messages.push({
        id: `${span.id}-prompt-${idx}`,
        role: role.toLowerCase() as 'user' | 'assistant' | 'system',
        content: sanitize(content),
        timestamp: baseTimestamp + idx, // Slight offset for ordering
        traceId: rootTraceId,
        tokens: undefined,
      });
    }
  }

  // Extract completion (assistant response)
  const completionContent = attrs['gen_ai.completion.0.content'] as string | undefined;
  if (completionContent) {
    const outputTokens = Number(attrs['gen_ai.usage.output_tokens'] || attrs['gen_ai.usage.completion_tokens'] || 0);
    messages.push({
      id: `${span.id}-completion`,
      role: 'assistant',
      content: sanitize(completionContent),
      timestamp: span.end_time || baseTimestamp + (span.duration_ms || 0),
      traceId: rootTraceId,
      tokens: outputTokens || undefined,
      cost: undefined,
    });
  }

  return messages;
}

// Legacy fallback: extract from trace metadata (used when tree fetch fails)
function hydrateMessagesFromTrace(trace: TraceMetadata): SessionMessage[] {
  const messages: SessionMessage[] = [];
  const baseTimestamp = trace.timestamp_us ?? Date.now() * 1000;
  const userContent =
    (trace.metadata?.prompt as string | undefined) ||
    (trace.metadata?.['gen_ai.content.prompt'] as string | undefined) ||
    (trace.metadata?.input as string | undefined);
  if (userContent) {
    messages.push({
      id: `${trace.trace_id}-prompt`,
      role: 'user',
      content: sanitize(userContent),
      timestamp: baseTimestamp,
      traceId: trace.trace_id,
      tokens: Number(trace.metadata?.['gen_ai.usage.prompt_tokens']) || undefined,
    });
  }

  const assistantContent =
    (trace.metadata?.completion as string | undefined) ||
    (trace.metadata?.['gen_ai.content.completion'] as string | undefined) ||
    (trace.metadata?.output as string | undefined);
  if (assistantContent) {
    messages.push({
      id: `${trace.trace_id}-completion`,
      role: 'assistant',
      content: sanitize(assistantContent),
      timestamp: baseTimestamp + (trace.duration_us ?? 0),
      traceId: trace.trace_id,
      tokens: Number(trace.metadata?.['gen_ai.usage.completion_tokens']) || undefined,
      cost: trace.cost,
    });
  }

  return messages;
}

// Extract span data from the trace tree API response
// Tree spans have: id, name, parent_id, start_time, end_time, duration_ms, attributes
function extractSpanFromTreeSpan(span: any, rootTraceId: string): SessionSpan {
  const attrs = span.attributes || {};
  const name = span.name || 'Unknown';

  // Determine span type from span name and attributes
  let spanType: SessionSpan['span_type'] = 'other';
  const spanKind = attrs['span.kind'] || '';
  const genAiSystem = attrs['gen_ai.system'] || '';

  // Check for LLM spans - they often have openai.chat or similar names, or gen_ai attributes
  if (name.includes('openai') || name.includes('chat') || name.includes('llm') ||
    genAiSystem || attrs['gen_ai.request.model'] || attrs['gen_ai.completion.0.content']) {
    spanType = 'llm';
  } else if (name.toLowerCase().includes('tool') || attrs['tool.name']) {
    spanType = 'tool';
  } else if (name.toLowerCase().includes('agent')) {
    spanType = 'agent';
  } else if (name.toLowerCase().includes('chain')) {
    spanType = 'chain';
  } else if (name.toLowerCase().includes('retriev')) {
    spanType = 'retrieval';
  } else if (name === 'POST' || name === 'GET' || attrs['http.method']) {
    // HTTP spans are usually external calls
    spanType = 'chain';
  }

  // Extract model from various attribute patterns
  const model = (
    attrs['gen_ai.request.model'] ||
    attrs['gen_ai.response.model'] ||
    attrs['llm.model_name'] ||
    undefined
  ) as string | undefined;

  // Extract tokens
  const inputTokens = Number(attrs['gen_ai.usage.input_tokens'] || 0);
  const outputTokens = Number(attrs['gen_ai.usage.output_tokens'] || 0);
  const totalTokens = Number(attrs['llm.usage.total_tokens'] || 0) || (inputTokens + outputTokens) || undefined;

  // Build a meaningful display name
  let displayName = name;
  if (spanType === 'llm' && model) {
    displayName = `🤖 ${model}`;
  } else if (name === 'openai.chat' && model) {
    displayName = `💬 Chat: ${model}`;
  } else if (name === 'POST' && attrs['http.url']) {
    // Shorten HTTP span names
    displayName = `📡 API Call`;
  }

  // Extract input/output for display
  const promptContent = attrs['gen_ai.prompt.1.content'] || attrs['gen_ai.prompt.0.content'];
  const completionContent = attrs['gen_ai.completion.0.content'];

  return {
    span_id: span.id,
    trace_id: rootTraceId,
    parent_span_id: span.parent_id || undefined,
    name: displayName,
    start_time: span.start_time || 0,
    end_time: span.end_time || 0,
    duration_ms: span.duration_ms || 0,
    span_type: spanType,
    status: attrs['span.status'] || 'ok',
    attributes: {
      ...attrs,
      // Add parsed content for easier access
      input_preview: promptContent,
      output_preview: completionContent,
    },
    model,
    tokens: totalTokens,
    cost: undefined, // Cost is usually at trace level, not span level
  };
}

// Extract real span data from trace metadata
function extractSpanFromTrace(trace: TraceMetadata): SessionSpan {
  const metadata = trace.metadata || {};

  // Determine span type from metadata
  let spanType: SessionSpan['span_type'] = 'other';
  const spanKind = metadata['span.kind'] || metadata['openinference.span.kind'] || metadata['span_kind'] || '';
  const spanName = trace.span_id || metadata['name'] || 'Unknown';

  if (spanKind.toLowerCase().includes('llm') || metadata['gen_ai.system'] || metadata['llm.model_name']) {
    spanType = 'llm';
  } else if (spanKind.toLowerCase().includes('tool') || metadata['tool.name']) {
    spanType = 'tool';
  } else if (spanKind.toLowerCase().includes('agent')) {
    spanType = 'agent';
  } else if (spanKind.toLowerCase().includes('chain')) {
    spanType = 'chain';
  } else if (spanKind.toLowerCase().includes('retriev')) {
    spanType = 'retrieval';
  }

  // Get model name from various possible metadata keys
  const model = (
    metadata['gen_ai.request.model'] ||
    metadata['gen_ai.response.model'] ||
    metadata['llm.model_name'] ||
    metadata['model'] ||
    metadata['llm.request.model'] ||
    undefined
  ) as string | undefined;

  // Calculate tokens
  const promptTokens = Number(metadata['gen_ai.usage.prompt_tokens'] || metadata['llm.usage.prompt_tokens'] || 0);
  const completionTokens = Number(metadata['gen_ai.usage.completion_tokens'] || metadata['llm.usage.completion_tokens'] || 0);
  const totalTokens = trace.token_count || promptTokens + completionTokens || undefined;

  // Build span name from available data
  let displayName = spanName;
  if (spanType === 'llm' && model) {
    displayName = `LLM: ${model}`;
  } else if (spanType === 'tool') {
    displayName = `Tool: ${metadata['tool.name'] || spanName}`;
  } else if (metadata['name']) {
    displayName = metadata['name'] as string;
  }

  return {
    span_id: trace.span_id,
    trace_id: trace.trace_id,
    parent_span_id: trace.parent_span_id,
    name: displayName,
    start_time: trace.timestamp_us || 0,
    end_time: (trace.timestamp_us || 0) + (trace.duration_us || 0),
    duration_ms: (trace.duration_us || 0) / 1000,
    span_type: spanType,
    status: metadata['status'] as string || 'ok',
    attributes: metadata,
    model,
    tokens: totalTokens,
    cost: trace.cost,
  };
}

function SpanTypeIcon({ type }: { type: SessionSpan['span_type'] }) {
  switch (type) {
    case 'llm':
      return <Bot className="h-3.5 w-3.5 text-purple-600 dark:text-purple-400" />;
    case 'tool':
      return <Wrench className="h-3.5 w-3.5 text-blue-600 dark:text-blue-400" />;
    case 'agent':
      return <Cpu className="h-3.5 w-3.5 text-green-600 dark:text-green-400" />;
    case 'chain':
      return <Activity className="h-3.5 w-3.5 text-orange-600 dark:text-orange-400" />;
    case 'retrieval':
      return <MessageCircle className="h-3.5 w-3.5 text-cyan-600 dark:text-cyan-400" />;
    default:
      return <Zap className="h-3.5 w-3.5 text-textSecondary" />;
  }
}

function sanitize(value: string) {
  return value.trim().replace(/\s+/g, ' ');
}

function formatDuration(durationMs?: number) {
  if (!durationMs) return '—';
  if (durationMs < 1000) return `${durationMs.toFixed(0)} ms`;
  const seconds = durationMs / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)} s`;
  const minutes = Math.floor(seconds / 60);
  const remainder = Math.round(seconds % 60);
  return `${minutes}m ${remainder}s`;
}

function formatWindow(start?: number, end?: number) {
  if (!start || !end) return '—';
  return `${new Date(start / 1000).toLocaleTimeString()} – ${new Date(end / 1000).toLocaleTimeString()}`;
}

function scoreSentiment(messages: SessionMessage[]) {
  const negative = messages.filter((msg) => msg.role === 'user' && /angry|frustrated|bad|ERROR/i.test(msg.content)).length;
  if (!negative) return 'Positive';
  if (negative > 1) return 'Needs review';
  return 'Mixed';
}

function SummaryCard({ icon: Icon, label, value, detail }: { icon: typeof MessageCircle; label: string; value: string; detail?: string }) {
  return (
    <div className="rounded-2xl border border-border/60 bg-background/90 p-4">
      <div className="flex items-center gap-2 text-textTertiary">
        <Icon className="h-4 w-4" />
        <span className="text-xs uppercase tracking-widest">{label}</span>
      </div>
      <p className="mt-2 text-2xl font-semibold text-textPrimary">{value}</p>
      {detail && <p className="text-xs text-textSecondary">{detail}</p>}
    </div>
  );
}

function InsightRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-border/40 bg-surface/70 px-3 py-2">
      <p className="text-xs uppercase tracking-widest text-textTertiary">{label}</p>
      <p className="text-sm font-semibold text-textPrimary">{value}</p>
    </div>
  );
}

function Badge({ role }: { role: SessionMessage['role'] }) {
  const icon = role === 'user' ? <User className="h-3.5 w-3.5" /> : role === 'system' ? <Activity className="h-3.5 w-3.5" /> : <MessageCircle className="h-3.5 w-3.5" />;
  const label = role.charAt(0).toUpperCase() + role.slice(1);
  return (
    <span className="flex items-center gap-1 rounded-full border border-border/50 px-2 py-0.5 text-[11px] text-textSecondary">
      {icon}
      {label}
    </span>
  );
}
