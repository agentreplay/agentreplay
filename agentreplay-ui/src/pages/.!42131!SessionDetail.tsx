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
