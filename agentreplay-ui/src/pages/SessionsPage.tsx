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

import { useState, useEffect } from 'react';
import { DeleteSessionDialog } from '../../components/session/DeleteSessionDialog';
import { useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { motion } from 'framer-motion';
import {
  MessageCircle,
  User,
  Clock,
  DollarSign,
  Search,
  Activity,
  Zap,
  X,
  ExternalLink,
  Bot,
  Copy,
  Download,
  List,
  Play,
  Database,
  Target,
  BarChart2,
  CheckCircle,
  AlertTriangle
} from 'lucide-react';
import { AddToDatasetModal } from '../../components/evals/AddToDatasetModal';
import { VideoHelpButton } from '../components/VideoHelpButton';
import { TraceTimelineView } from '../../components/trace/TraceTimelineView';
import { SessionReplay } from '../../components/session/SessionReplay';
import { EvaluateTraceModal } from '../components/EvaluateTraceButton';

// Determine the API base URL based on environment
function getApiBaseUrl(): string {
  const isTauri = typeof window !== 'undefined' &&
    ('__TAURI__' in window || '__TAURI_INTERNALS__' in window);
  if (isTauri) {
    return 'http://127.0.0.1:9600';
  }
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return ''; // Use Vite proxy
  }
  return 'http://127.0.0.1:9600';
}

const API_BASE_URL = getApiBaseUrl();

interface Message {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: string;
  timestamp_us: number;
  trace_id?: string;
  cost?: number;
  latency_ms?: number;
  feedback?: 'positive' | 'negative';
  tool_calls?: Array<{ name: string; arguments?: string }>;
  tool_call_id?: string;
}

interface SessionTrace {
  trace_id: string;
  timestamp_us: number;
  model?: string;
  duration_ms?: number;
  tokens?: number;
  status?: string;
  // GenAI payload data - from /tree endpoint attributes
  messages: Message[];
  input_tokens?: number;
  output_tokens?: number;
}

interface Session {
  session_id: string | number;  // Can be large integer, handle as string
  project_id: number;
  agent_id: number;
  started_at: number;
  last_message_at: number;
  message_count: number;
  total_tokens: number;
  total_duration_ms: number;
  trace_ids: string[];
  status: 'active' | 'ended';
}

export default function SessionsPage() {
  const navigate = useNavigate();
  const { projectId } = useParams<{ projectId: string }>();
  const [searchParams, setSearchParams] = useSearchParams();
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [selectedSession, setSelectedSession] = useState<Session | null>(null);
  const [sessionTraces, setSessionTraces] = useState<SessionTrace[]>([]);
  const [loadingTraces, setLoadingTraces] = useState(false);
  const [previewMode, setPreviewMode] = useState<'transcript' | 'waterfall' | 'replay'>('waterfall');
  const [copiedSessionId, setCopiedSessionId] = useState(false);
  const [sessionToDelete, setSessionToDelete] = useState<string | null>(null);

  const [isEvalOpen, setIsEvalOpen] = useState(false);

  const [isAddDatasetOpen, setIsAddDatasetOpen] = useState(false);

  // Open session in Playground
  const openInPlayground = (session: Session, traces: SessionTrace[]) => {
    // 1. Collect all messages from the session traces
    const allMessages: { role: string; content: string }[] = [];

    // Sort traces by timestamp
    const sortedTraces = [...traces].sort((a, b) => a.timestamp_us - b.timestamp_us);

    sortedTraces.forEach(trace => {
      trace.messages.forEach(msg => {
        trace.messages.forEach(msg => {
          if (msg.role === 'user' || msg.role === 'system' || msg.role === 'assistant') {
            // Clean content (JSON handling)
            let cleanContent = msg.content;

            // Skip if content seems to be a raw internal attribute dump (e.g. Python dict string)
            if (typeof cleanContent === 'string' &&
              (cleanContent.trim().startsWith('{') || cleanContent.includes("'tool_calls':"))) {
              // Try to rescue actual content if buried in JSON
              try {
                // If it's valid JSON, check for 'content' field
                const parsed = JSON.parse(cleanContent);
                if (parsed.content) {
                  cleanContent = parsed.content;
                } else if (parsed.tool_calls) {
                  // It's a tool call, representing as text for now
                  cleanContent = `[Tool Call: ${parsed.tool_calls.map((tc: any) => tc.name).join(', ')}]`;
                } else {
                  // Likely just metadata, skip
                  return;
                }
              } catch {
                // If it's a Python dict string ("{'key': ...}"), it's likely garbage metadata for the playground
                if (cleanContent.includes("'tool_calls':") || cleanContent.includes("'audio_tokens':")) {
                  return;
                }
              }
            }

            try {
              if (typeof cleanContent === 'string' && cleanContent.trim().startsWith('[')) {
                const parsed = JSON.parse(cleanContent);
                if (Array.isArray(parsed) && parsed[0]?.text) cleanContent = parsed[0].text;
              }
            } catch { }

            if (cleanContent && cleanContent.trim()) {
              allMessages.push({ role: msg.role, content: cleanContent });
            }
          }
        });
      });
    });

    const model = traces.find(t => t.model)?.model || 'gpt-4';

    const playgroundData = {
      prompt: '',
      model: model,
      messages: allMessages,
      sourceSessionId: session.session_id
    };
    sessionStorage.setItem('playground_data', JSON.stringify(playgroundData));
    navigate(`/projects/${projectId}/playground?from=session`);
  };

  const getDatasetData = () => {
    let input = '';
    let output = '';

    if (sessionTraces.length > 0) {
      const allMsgs = sessionTraces.flatMap(t => t.messages).sort((a, b) => a.timestamp_us - b.timestamp_us);
      const lastUser = [...allMsgs].reverse().find(m => m.role === 'user');
      const lastAssistant = [...allMsgs].reverse().find(m => m.role === 'assistant');

      if (lastUser) input = lastUser.content;
      if (lastAssistant) output = lastAssistant.content;
    }
    return { input, output };
  };

  useEffect(() => {
    const fetchSessions = async () => {
      if (!projectId) {
        setError('No project selected');
        setLoading(false);
        return;
      }

      try {
        setLoading(true);
        setError(null);
        const response = await fetch(`${API_BASE_URL}/api/v1/sessions?project_id=${projectId}&limit=100`);

        if (!response.ok) {
          throw new Error(`Failed to fetch sessions: ${response.statusText}`);
        }

        const data = await response.json();
        const loadedSessions = data.sessions || [];
        setSessions(loadedSessions);

        // Auto-select first session if none selected
        if (loadedSessions.length > 0 && !selectedSession) {
          setSelectedSession(loadedSessions[0]);
        }
      } catch (error) {
        console.error('Error fetching sessions:', error);
        setError(error instanceof Error ? error.message : 'Failed to load sessions');
        setSessions([]);
      } finally {
        setLoading(false);
      }
    };

    fetchSessions();

    // Refresh every 30 seconds (sessions are aggregations, don't need rapid updates)
    // Individual traces use SSE for real-time updates
    const interval = setInterval(fetchSessions, 30000);
    return () => clearInterval(interval);
    return () => clearInterval(interval);
  }, [projectId]);

  const handleDeleteSession = async (sessionId: string) => {
    try {
      if (typeof window !== 'undefined' && '__TAURI__' in window) {
        // Tauri
        // @ts-ignore
        const { invoke } = window.__TAURI__.core;
        await invoke('delete_session', { params: { session_id: String(sessionId) } });
      } else {
        // Web
        const response = await fetch(`${API_BASE_URL}/api/v1/sessions/${sessionId}`, {
          method: 'DELETE',
        });
        if (!response.ok) throw new Error('Failed to delete session');
      }

      // Remove from list
      setSessions(prev => prev.filter(s => s.session_id.toString() !== sessionId));
      if (selectedSession?.session_id.toString() === sessionId) {
        setSelectedSession(null);
        setSessionTraces([]);
      }
    } catch (err) {
      console.error('Failed to delete session:', err);
      // Optional: show toast error
      alert('Failed to delete session');
    }
  };

  const handleConfirmDelete = async () => {
    if (sessionToDelete) {
      await handleDeleteSession(sessionToDelete);
      setSessionToDelete(null);
    }
  };

  // Fetch traces when session is selected
  useEffect(() => {
    if (selectedSession) {
      fetchSessionTraces(selectedSession);
      // Update URL with session_id, but prevent loop if already set
      const currentSessionId = searchParams.get('session_id');
      if (currentSessionId !== String(selectedSession.session_id)) {
        setSearchParams({ ...Object.fromEntries(searchParams), session_id: String(selectedSession.session_id) });
      }
    } else {
      // If no session selected, clear from URL? Or keep?
      // Maybe better to NOT clear if we want "back" to work from other pages, 
      // but if user explicitly closes session it should clear. 
      // For now let's leave it.
    }
  }, [selectedSession]);

  // Sync selection with URL on load/change
  useEffect(() => {
    const sessionIdParam = searchParams.get('session_id');
    if (sessionIdParam && sessions.length > 0) {
      const session = sessions.find(s => String(s.session_id) === sessionIdParam);
      if (session && (!selectedSession || String(selectedSession.session_id) !== sessionIdParam)) {
        setSelectedSession(session);
      }
    } else if (!sessionIdParam && sessions.length > 0 && !selectedSession) {
      // Auto-select first ONLY if no param provided
      setSelectedSession(sessions[0]);
    }
  }, [sessions, searchParams]);

  const fetchSessionTraces = async (session: Session) => {
    setLoadingTraces(true);
    try {
      // First get the list of traces for this session
      let traceIds: string[] = [];

      if (session.trace_ids && session.trace_ids.length > 0) {
        // Convert hex trace IDs to decimal if needed
        traceIds = session.trace_ids.map(id => {
          if (id.startsWith('0x')) {
            return BigInt(id).toString();
          }
          return id;
        });
      } else {
        // Fallback: query traces by session_id
        const response = await fetch(`${API_BASE_URL}/api/v1/traces?session_id=${session.session_id}&limit=50`);
        if (response.ok) {
          const data = await response.json();
          traceIds = (data.traces || []).map((t: any) => t.trace_id || t.span_id);
        }
      }

      // For each trace, fetch the TREE to get full GenAI payload attributes
      const traces: SessionTrace[] = [];
      const seenTraceIds = new Set<string>();

      for (const traceId of traceIds.slice(0, 50)) {
        if (seenTraceIds.has(traceId)) continue;

        try {
          const treeResponse = await fetch(`${API_BASE_URL}/api/v1/traces/${traceId}/tree?project_id=${projectId || 0}`);
          if (treeResponse.ok) {
            const tree = await treeResponse.json();

            // Aggregate messages from ALL spans in the tree
            let allMessages: Message[] = [];
            const spans = Array.isArray(tree.spans) ? tree.spans : (tree.spans ? [tree.spans] : []);

            for (const span of spans) {
              const spanAttrs = span.attributes || {};
              // Check if span has GenAI attributes
              if (spanAttrs['gen_ai.system'] ||
                spanAttrs['gen_ai.request.model'] ||
                Object.keys(spanAttrs).some(k => k.startsWith('gen_ai.prompt') || k.startsWith('gen_ai.completion'))) {

                const spanMessages = parseGenAIMessages(spanAttrs, traceId, span.start_time);
                allMessages.push(...spanMessages);
              }
            }

            // Sort aggregated messages by timestamp
            allMessages.sort((a, b) => a.timestamp_us - b.timestamp_us);

            // Find root span for top-level trace metadata
            const rootSpan = spans.find((s: any) => !s.parent_id) || spans[0];
            if (!rootSpan) continue;

            const attrs = rootSpan.attributes || {};

            // If no messages but we have a root span, check if it's just an HTTP wrapper
            // But if we found messages in children, we definitely want to show this trace
            if (allMessages.length === 0 && !attrs['gen_ai.system'] && !attrs['gen_ai.prompt.0.role'] && rootSpan.name === 'POST') {
              continue;
            }

            seenTraceIds.add(traceId);

            traces.push({
              trace_id: traceId,
              timestamp_us: rootSpan.start_time,
              model: attrs['gen_ai.request.model'] || attrs['gen_ai.response.model'] || '',
              duration_ms: rootSpan.duration_ms,
              tokens: Number(attrs['llm.usage.total_tokens'] || 0) ||
                (Number(attrs['gen_ai.usage.input_tokens'] || 0) + Number(attrs['gen_ai.usage.output_tokens'] || 0)),
              status: attrs['span.status'] || 'completed',
              messages: allMessages,
              input_tokens: Number(attrs['gen_ai.usage.input_tokens'] || 0),
              output_tokens: Number(attrs['gen_ai.usage.output_tokens'] || 0),
            });
          }
        } catch (err) {
          console.error(`Error fetching tree for trace ${traceId}:`, err);
        }
      }

      // Sort by timestamp
      traces.sort((a, b) => a.timestamp_us - b.timestamp_us);
      setSessionTraces(traces);
    } catch (err) {
      console.error('Error fetching session traces:', err);
      setSessionTraces([]);
    } finally {
      setLoadingTraces(false);
    }
  };

  // Parse GenAI payload attributes into structured messages
  const parseGenAIMessages = (attrs: Record<string, any>, traceId: string, startTime: number): Message[] => {
    const messages: Message[] = [];

    // Parse gen_ai.prompt.N.* pattern
    for (let i = 0; i < 20; i++) {
      const role = attrs[`gen_ai.prompt.${i}.role`];
      if (!role) break;

      let content = attrs[`gen_ai.prompt.${i}.content`] || '';

      // Handle JSON array content format: [{"type": "text", "text": "..."}]
      if (typeof content === 'string' && content.startsWith('[')) {
        try {
          const parsed = JSON.parse(content);
          if (Array.isArray(parsed) && parsed[0]?.text) {
            content = parsed.map((p: any) => p.text || '').join('\n');
          }
        } catch { }
      }

      // Parse tool calls if present
      const toolCalls: Array<{ name: string; arguments?: string }> = [];
      for (let j = 0; j < 10; j++) {
        const toolName = attrs[`gen_ai.prompt.${i}.tool_calls.${j}.name`];
        if (!toolName) break;
        toolCalls.push({
          name: toolName,
          arguments: attrs[`gen_ai.prompt.${i}.tool_calls.${j}.arguments`],
        });
      }

      messages.push({
        id: `${traceId}-prompt-${i}`,
        role: role as Message['role'],
        content,
        timestamp_us: startTime + i * 1000, // Offset for ordering
        trace_id: traceId,
        tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
        tool_call_id: attrs[`gen_ai.prompt.${i}.tool_call_id`],
      });
    }

    // Parse gen_ai.completion.N.* pattern
    for (let i = 0; i < 5; i++) {
      const content = attrs[`gen_ai.completion.${i}.content`];
      const role = attrs[`gen_ai.completion.${i}.role`] || 'assistant';
      if (!content && i === 0) continue;
      if (!content) break;

      messages.push({
        id: `${traceId}-completion-${i}`,
        role: role as Message['role'],
        content,
        timestamp_us: startTime + 100000 + i * 1000, // After prompts
        trace_id: traceId,
      });
    }

    return messages;
  };

  const [sessionObservations, setSessionObservations] = useState<any[]>([]);
  const [loadingObservations, setLoadingObservations] = useState(false);

  // Fetch observations when switching to waterfall mode
  useEffect(() => {
    if (previewMode === 'waterfall' && sessionTraces.length > 0 && sessionObservations.length === 0) {
      fetchSessionObservations();
    }
  }, [previewMode, sessionTraces]);

  // Reset observations when session changes
  useEffect(() => {
    setSessionObservations([]);
  }, [selectedSession]);

  const fetchSessionObservations = async () => {
    setLoadingObservations(true);
    try {
      const allSpans: any[] = [];
      const seenSpanIds = new Set<string>();

      // First, identify root traces (those without parent_span_id)
      // Child spans may also appear as "traces" in the session, so we need to dedupe
      const rootTraces = sessionTraces.filter(trace => {
        // A trace is a root if it has no parent OR its parent is not in our trace list
        const hasParentInSession = sessionTraces.some(other =>
          other.trace_id !== trace.trace_id &&
          trace.trace_id === other.trace_id // This shouldn't happen
        );
        return !hasParentInSession;
      });

      // Fetch the SPAN TREE for each root trace
      for (const trace of rootTraces) {
        try {
          const response = await fetch(`${API_BASE_URL}/api/v1/traces/${trace.trace_id}/tree?project_id=${projectId || 0}`);
          if (response.ok) {
            const tree = await response.json();

            if (tree && tree.spans && Array.isArray(tree.spans)) {
              // Add each span from the tree, deduping by span_id
              tree.spans.forEach((span: any) => {
                if (!seenSpanIds.has(span.id)) {
                  seenSpanIds.add(span.id);
                  allSpans.push({
                    span_id: span.id,
                    trace_id: tree.root || trace.trace_id,
                    parent_span_id: span.parent_id,
                    name: formatSpanName(span),
                    start_time: span.start_time,
                    end_time: span.end_time,
                    duration_ms: span.duration_ms,
                    status: span.attributes?.['span.status'] || 'ok',
                    attributes: {
                      'service.name': span.attributes?.['service.name'] || span.attributes?.['gen_ai.system'] || 'agentreplay',
                      'span.kind': span.attributes?.['span.kind'] || 'client',
                      'model': span.attributes?.['gen_ai.request.model'] || span.attributes?.['gen_ai.response.model'],
                      'tokens': Number(span.attributes?.['llm.usage.total_tokens'] || 0) ||
                        (Number(span.attributes?.['gen_ai.usage.input_tokens'] || 0) +
                          Number(span.attributes?.['gen_ai.usage.output_tokens'] || 0)),
                      'input': span.attributes?.['gen_ai.prompt.1.content'] || span.attributes?.['gen_ai.prompt.0.content'],
                      'output': span.attributes?.['gen_ai.completion.0.content'],
                      ...span.attributes,
                    }
                  });
                }
              });
            }
          }
        } catch (err) {
          console.error(`Error fetching tree for trace ${trace.trace_id}:`, err);
          // Fallback: add the trace itself as a span
          if (!seenSpanIds.has(trace.trace_id)) {
            seenSpanIds.add(trace.trace_id);
            // Get first user message as name fallback
            const userMsg = trace.messages.find(m => m.role === 'user');
            allSpans.push({
              span_id: trace.trace_id,
              trace_id: trace.trace_id,
              name: trace.model || userMsg?.content?.slice(0, 50) || 'Trace',
              start_time: trace.timestamp_us,
              end_time: trace.timestamp_us + (trace.duration_ms || 0) * 1000,
              duration_ms: trace.duration_ms,
              status: trace.status,
              attributes: {
                'service.name': trace.model || 'unknown',
                'span.kind': 'server',
                'tokens': trace.tokens,
              }
            });
          }
        }
      }

      setSessionObservations(allSpans);
    } catch (err) {
      console.error('Error fetching session observations:', err);
    } finally {
      setLoadingObservations(false);
    }
  };

  // Format span name to be more readable
  const formatSpanName = (span: any): string => {
    const attrs = span.attributes || {};
    const name = span.name || 'Span';
    const model = attrs['gen_ai.request.model'] || attrs['gen_ai.response.model'];

    // LLM spans
    if (name.includes('openai') || name.includes('chat') || attrs['gen_ai.system'] || model) {
      return model ? `🤖 ${model}` : `💬 ${name}`;
    }

    // HTTP spans
    if (name === 'POST' || name === 'GET' || attrs['http.method']) {
      return `📡 API Call`;
    }

    // Tool spans
    if (name.toLowerCase().includes('tool')) {
      return `🔧 ${name}`;
    }

    return name;
  };

  const filteredSessions = sessions.filter(session =>
    session.session_id.toString().includes(searchQuery.toLowerCase())
  );

  const formatTimeAgo = (timestamp: number) => {
    const seconds = Math.floor((Date.now() - timestamp / 1000) / 1000);
    if (seconds < 60) return 'just now';
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
    return `${Math.floor(seconds / 86400)}d ago`;
  };

  return (
    <div className="h-screen flex flex-col bg-background">
      <DeleteSessionDialog
        isOpen={!!sessionToDelete}
        onClose={() => setSessionToDelete(null)}
        onConfirm={handleConfirmDelete}
        sessionId={sessionToDelete || ''}
      />
      <AddToDatasetModal
        isOpen={isAddDatasetOpen}
        onClose={() => setIsAddDatasetOpen(false)}
        initialInput={getDatasetData().input}
        initialOutput={getDatasetData().output}
        metadata={{
          source_session_id: selectedSession?.session_id?.toString() || '',
          model: sessionTraces.find(t => t.model)?.model || 'unknown'
        }}
      />

      <EvaluateTraceModal
        isOpen={isEvalOpen}
        onClose={() => setIsEvalOpen(false)}
        traceId={sessionTraces.slice().reverse().find(t => t.model || t.messages.length > 0)?.trace_id || selectedSession?.trace_ids?.[selectedSession.trace_ids.length - 1] || ''}
        traceMetadata={{
          prompt: getDatasetData().input,
          response: getDatasetData().output,
          model: sessionTraces.find(t => t.model)?.model
        }}
      />

      {/* Header */}
      <div className="border-b border-border px-6 py-4 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-textPrimary mb-2">Sessions</h1>
          <p className="text-sm text-textSecondary">
            Conversational view for chatbot interactions
          </p>
        </div>
        <VideoHelpButton pageId="sessions" />
      </div>

      <div className="flex-1 flex overflow-hidden">
        {/* Left: Sessions List */}
        <div className="w-96 border-r border-border flex flex-col">
          {/* Search */}
          <div className="p-4 border-b border-border">
            <div className="relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-textTertiary" />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search by user ID..."
                className="w-full pl-10 pr-4 py-2 rounded-lg bg-surface border border-border text-textPrimary placeholder:text-textTertiary focus:outline-none focus:ring-2 focus:ring-primary"
              />
            </div>
          </div>

          {/* Sessions */}
          <div className="flex-1 overflow-y-auto">
            {loading ? (
              <div className="text-center py-12">
                <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-primary"></div>
                <p className="text-sm text-textSecondary mt-2">Loading sessions...</p>
              </div>
            ) : error ? (
              <div className="text-center py-12 px-6">
                <MessageCircle className="w-16 h-16 text-red-500 mx-auto mb-4 opacity-50" />
                <p className="text-lg font-semibold text-textPrimary mb-2">Error loading sessions</p>
                <p className="text-sm text-textSecondary">{error}</p>
              </div>
            ) : filteredSessions.length === 0 ? (
              <div className="text-center py-12 px-6">
                <MessageCircle className="w-16 h-16 text-textTertiary mx-auto mb-4 opacity-50" />
                {searchQuery ? (
                  <>
                    <p className="text-lg font-semibold text-textPrimary mb-2">No matching sessions</p>
                    <p className="text-sm text-textSecondary">
                      Try a different search query
                    </p>
                  </>
                ) : (
                  <>
                    <p className="text-lg font-semibold text-textPrimary mb-2">No sessions yet</p>
                    <p className="text-sm text-textSecondary mb-4 max-w-md mx-auto">
                      Sessions group multi-turn conversations. Send traces with the same <code className="px-2 py-1 bg-surface-elevated rounded text-xs">session_id</code> to see them here.
                    </p>
                    <div className="bg-surface-elevated border border-border rounded-lg p-4 max-w-xl mx-auto text-left">
                      <p className="text-xs font-semibold text-textTertiary mb-2">Example:</p>
                      <pre className="text-xs text-textPrimary bg-background p-3 rounded border border-border-subtle overflow-x-auto">
                        {`# Set session_id for multi-turn chats
response = agentreplay.chat.completions.create(
messages=[...],
session_id="user_123_conversation"
)`}
                      </pre>
                      <a
                        href="/"
                        className="inline-flex items-center gap-2 text-sm text-primary hover:underline mt-3"
                      >
                        Learn about session tracking →
                      </a>
                    </div>
                  </>
                )}
              </div>
            ) : (
              filteredSessions.map((session) => (
                <motion.button
                  key={session.session_id}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  onClick={() => setSelectedSession(session)}
                  className={`w-full flex items-start gap-3 p-3 border-b border-border hover:bg-surface-hover transition-colors text-left group ${selectedSession?.session_id === session.session_id ? 'bg-primary/5 border-l-2 border-l-primary' : ''
                    }`}
                >
                  <div className="flex-shrink-0 w-10 h-10 rounded-full bg-primary/20 flex items-center justify-center">
                    <User className="w-5 h-5 text-primary" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center justify-between gap-2 mb-1">
                      <span className="font-medium text-textPrimary truncate font-mono text-sm min-w-0">
                        {session.session_id}
                      </span>
                      <div className="flex items-center gap-2 flex-shrink-0">
                        <span className="text-xs text-textTertiary whitespace-nowrap">
                          {formatTimeAgo(session.last_message_at)}
                        </span>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setSessionToDelete(session.session_id.toString());
                          }}
                          className="p-1 text-textTertiary hover:text-red-500 hover:bg-red-500/10 rounded transition-all flex-shrink-0"
                          title="Delete Session"
                        >
                          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                            <path d="M3 6h18"></path>
                            <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"></path>
                            <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"></path>
                          </svg>
                        </button>
                      </div>
                    </div>
                    <div className="flex items-center gap-2 text-xs text-textSecondary overflow-hidden">
                      <span className="truncate">{session.message_count} spans</span>
                      <span className="flex-shrink-0">•</span>
                      <span className="truncate">{session.total_tokens.toLocaleString()} tokens</span>
                      <span className="flex-shrink-0">•</span>
                      <span className="whitespace-nowrap flex-shrink-0">{(session.total_duration_ms / 1000).toFixed(1)}s</span>
                    </div>
                    <div className="flex items-center gap-2 mt-1">
                      <span className={`px-2 py-0.5 rounded text-xs font-medium flex-shrink-0 ${session.status === 'active'
                        ? 'bg-green-500/10 text-green-500'
                        : 'bg-gray-500/10 text-gray-500'
                        }`}>
                        {session.status}
                      </span>
                      {session.project_id > 0 && (
                        <span className="text-xs text-textTertiary truncate">
                          Project #{session.project_id}
                        </span>
                      )}
                    </div>
                  </div>
                </motion.button>
              ))
            )}
          </div>
        </div>

        {/* Right: Session Preview */}
        <div className="flex-1 flex flex-col bg-surface/30 overflow-hidden">

          {/* BUG-09 FIX: Clear detail panel when selected session is not in filtered results */}
          {selectedSession && filteredSessions.some(s => s.session_id === selectedSession.session_id) ? (
            <>
              {/* Session Header - OBSERVABILITY REDESIGN */}
              <div className="border-b border-border bg-surface-elevated">
                {/* Top Row: Title & Main Actions */}
                <div className="px-6 py-3 flex items-start justify-between">
                  <div>
                    <div className="flex items-center gap-2 mb-1">
                      <h2 className="text-lg font-bold text-textPrimary flex items-center gap-2">
                        <User className="w-5 h-5 text-primary" />
                        Session #{selectedSession.session_id}
                      </h2>
                      <span className={`px-2 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider ${selectedSession.status === 'active'
                        ? 'bg-green-500/20 text-green-500'
                        : 'bg-gray-500/20 text-gray-400'
                        }`}>
                        {selectedSession.status}
                      </span>
                    </div>
                    <div className="flex items-center gap-3 text-xs text-textSecondary">
                      <span className="flex items-center gap-1">
                        <Clock className="w-3 h-3" />
                        {new Date(selectedSession.last_message_at).toLocaleString()}
                      </span>
                      <span>•</span>
                      <span className="font-mono text-textTertiary">
                        {sessionTraces.find(t => t.model)?.model || 'unknown model'}
                      </span>
                    </div>
                  </div>

                  <div className="flex items-center gap-2">
                    {/* METRICS DECK (High Density) */}
                    <div className="flex items-center bg-background border border-border rounded-lg px-3 py-1.5 mr-2 gap-4 shadow-sm">
                      <div className="flex flex-col items-start min-w-[60px]">
                        <span className="text-[10px] uppercase text-textTertiary font-semibold tracking-wider">Latency</span>
                        <span className="text-xs font-mono font-medium text-textPrimary">{(selectedSession.total_duration_ms / 1000).toFixed(2)}s</span>
                      </div>
                      <div className="w-px h-6 bg-border/60"></div>
                      <div className="flex flex-col items-start min-w-[60px]">
                        <span className="text-[10px] uppercase text-textTertiary font-semibold tracking-wider">Tokens</span>
                        <span className="text-xs font-mono font-medium text-textPrimary">{selectedSession.total_tokens.toLocaleString()}</span>
                      </div>
                      <div className="w-px h-6 bg-border/60"></div>
                      <div className="flex flex-col items-start min-w-[60px]">
                        <span className="text-[10px] uppercase text-textTertiary font-semibold tracking-wider">Cost</span>
                        <span className="text-xs font-mono font-medium text-textPrimary">$0.00{/* Placeholder */}</span>
                      </div>
                    </div>

                    {/* Action Buttons */}
                    <div className="flex items-center gap-1">
                      <button
                        onClick={() => setIsEvalOpen(true)}
                        className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-textPrimary bg-surface border border-border hover:bg-surface-hover hover:border-primary/30 rounded-md transition-all shadow-sm"
                        title="Run Evaluations on the latest response"
                      >
                        <Target className="w-3.5 h-3.5 text-purple-500" />
                        Evaluations
                      </button>
                      <button
                        onClick={() => openInPlayground(selectedSession, sessionTraces)}
                        className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-primary bg-primary/10 hover:bg-primary/20 rounded-md transition-colors"
                        title="Open entire session in Playground"
                      >
                        <Play className="w-3.5 h-3.5" />
                        Playground
                      </button>
                      <button
                        onClick={() => setIsAddDatasetOpen(true)}
                        className="p-1.5 text-textSecondary hover:text-textPrimary hover:bg-surface rounded-md transition-colors"
                        title="Add to Evaluation Dataset"
                      >
                        <Database className="w-4 h-4" />
                      </button>
                      <button
                        onClick={() => {
                          if (confirm('Are you sure you want to delete this session? This action cannot be undone.')) {
                            handleDeleteSession(selectedSession.session_id.toString());
                          }
                        }}
                        className="p-1.5 text-textSecondary hover:text-red-500 hover:bg-red-500/10 rounded-md transition-colors"
                        title="Delete Session"
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                          <path d="M3 6h18"></path>
                          <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"></path>
                          <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"></path>
                        </svg>
                      </button>
                    </div>

                    <div className="h-6 w-px bg-border/60 mx-1"></div>

                    {/* Close Button */}
                    <button
                      onClick={() => setSelectedSession(null)}
                      className="p-1.5 text-textTertiary hover:text-textPrimary hover:bg-red-500/10 hover:text-red-500 rounded-md transition-colors"
                    >
                      <X className="w-4 h-4" />
                    </button>
                  </div>
                </div>

                {/* Bottom Row: Tabs */}
                <div className="px-6 flex items-center gap-6 text-sm border-t border-border/40">
                  <button
                    onClick={() => setPreviewMode('transcript')}
                    className={`py-2 border-b-2 transition-colors flex items-center gap-2 ${previewMode === 'transcript'
                      ? 'border-primary text-primary font-medium'
                      : 'border-transparent text-textSecondary hover:text-textPrimary'
                      }`}
                  >
                    <List className="w-3.5 h-3.5" />
                    Transcript
                  </button>
                  <button
                    onClick={() => setPreviewMode('waterfall')}
                    className={`py-2 border-b-2 transition-colors flex items-center gap-2 ${previewMode === 'waterfall'
                      ? 'border-primary text-primary font-medium'
                      : 'border-transparent text-textSecondary hover:text-textPrimary'
                      }`}
                  >
                    <Activity className="w-3.5 h-3.5" />
                    Waterfall
                  </button>
                  <button
                    onClick={() => setPreviewMode('replay')}
                    className={`py-2 border-b-2 transition-colors flex items-center gap-2 ${previewMode === 'replay'
                      ? 'border-primary text-primary font-medium'
                      : 'border-transparent text-textSecondary hover:text-textPrimary'
                      }`}
                  >
                    <Play className="w-3.5 h-3.5" />
                    Replay
                  </button>
                </div>
              </div>


              {/* Traces List / Waterfall */}
              <div className="flex-1 overflow-y-auto p-4 space-y-3">
                {loadingTraces ? (
                  <div className="flex items-center justify-center py-12">
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary"></div>
                  </div>
                ) : sessionTraces.length === 0 ? (
                  <div className="text-center py-12 text-textTertiary">
                    <Activity className="w-12 h-12 mx-auto mb-2 opacity-50" />
                    <p>No traces found for this session</p>
                  </div>
                ) : previewMode === 'waterfall' ? (
                  <div className="h-full min-h-[400px]">
                    {loadingObservations ? (
                      <div className="flex items-center justify-center h-full">
                        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary"></div>
                        <span className="ml-2 text-textSecondary">Loading detailed trace data...</span>
                      </div>
                    ) : sessionObservations.length === 0 ? (
                      <div className="flex items-center justify-center h-full text-textSecondary">
                        <Activity className="w-8 h-8 mr-2 opacity-50" />
                        <span>No span data available for this session</span>
                      </div>
                    ) : (
                      <TraceTimelineView
                        spans={sessionObservations}
                        onSpanClick={(span) => {
                          navigate(`/projects/${projectId}/traces/${span.trace_id}?from=session&session_id=${selectedSession.session_id}`);
                        }}
                      />
                    )}
                  </div>
                ) : previewMode === 'replay' ? (
                  <div className="h-full min-h-[400px]">
                    {sessionTraces.length === 0 ? (
                      <div className="flex items-center justify-center h-full text-textSecondary">
                        <Play className="w-8 h-8 mr-2 opacity-50" />
                        <span>No messages to replay for this session</span>
                      </div>
                    ) : (() => {
                      // Only include LLM traces (those with gpt/llm in model or name, or have messages with user/assistant)
                      const llmTraces = sessionTraces.filter(trace =>
                        trace.model ||
                        trace.messages.some(m => m.role === 'user' || m.role === 'assistant')
                      );

                      // Deduplicate messages by content to avoid repeated history
                      const seenContent = new Set<string>();
                      const uniqueMessages = llmTraces.flatMap(trace =>
                        trace.messages
                          .filter(msg => {
                            // Only include user and assistant messages, skip duplicates
                            if (msg.role !== 'user' && msg.role !== 'assistant') return false;
                            const key = `${msg.role}:${msg.content?.slice(0, 100)}`;
                            if (seenContent.has(key)) return false;
                            seenContent.add(key);
                            return true;
                          })
                          .map(msg => ({
                            id: msg.id,
                            role: msg.role as 'user' | 'assistant' | 'system' | 'tool',
                            content: msg.content,
                            timestamp: msg.timestamp_us,
                            traceId: trace.trace_id,
                            cost: undefined,
                            tokens: trace.tokens
                          }))
                      );

                      return uniqueMessages.length === 0 ? (
                        <div className="flex items-center justify-center h-full text-textSecondary">
                          <Play className="w-8 h-8 mr-2 opacity-50" />
                          <span>No conversation messages to replay</span>
                        </div>
                      ) : (
                        <SessionReplay
                          messages={uniqueMessages}
                          onTraceClick={(traceId) => {
                            console.log('Trace clicked:', traceId);
                          }}
                        />
                      );
                    })()}
                  </div>
                ) : (
                  /* Transcript mode - existing message-based view */
                  sessionTraces.map((trace, idx) => (
                    <div
                      key={trace.trace_id}
                      className="bg-surface border border-border rounded-lg overflow-hidden hover:border-primary/50 transition-colors cursor-pointer"
                      onClick={() => navigate(`/projects/${projectId}/traces/${trace.trace_id}?view=list`)}
                    >
                      {/* Trace Header */}
                      <div className="px-4 py-2 bg-surface-elevated border-b border-border flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <span className={`px-2 py-0.5 rounded text-xs font-medium ${trace.status === 'error' ? 'bg-red-500/10 text-red-500' : 'bg-green-500/10 text-green-500'
                            }`}>
                            {trace.status || 'completed'}
                          </span>
                          {trace.model && (
                            <span className="text-xs text-primary font-medium">{trace.model}</span>
                          )}
                        </div>
                        <div className="flex items-center gap-3 text-xs text-textTertiary">
                          {trace.duration_ms && (
                            <span className="flex items-center gap-1">
                              <Clock className="w-3 h-3" />
                              {trace.duration_ms}ms
                            </span>
                          )}
                          {trace.tokens && (
                            <span className="flex items-center gap-1">
                              <Zap className="w-3 h-3" />
                              {trace.tokens} tokens
                            </span>
                          )}
                        </div>
                      </div>

                      {/* Messages from GenAI Payload */}
                      <div className="p-4 space-y-3 max-h-64 overflow-y-auto">
                        {trace.messages.length > 0 ? (
                          trace.messages.slice(0, 4).map((msg, idx) => (
                            <div key={msg.id || idx}>
                              <div className="flex items-center gap-2 mb-1">
                                {msg.role === 'user' ? (
                                  <>
                                    <User className="w-3 h-3 text-blue-500" />
                                    <span className="text-xs font-medium text-blue-500">User</span>
                                  </>
                                ) : msg.role === 'assistant' ? (
                                  <>
                                    <Bot className="w-3 h-3 text-green-500" />
                                    <span className="text-xs font-medium text-green-500">Assistant</span>
                                  </>
                                ) : msg.role === 'system' ? (
                                  <>
                                    <Activity className="w-3 h-3 text-purple-500" />
                                    <span className="text-xs font-medium text-purple-500">System</span>
                                  </>
                                ) : msg.role === 'tool' ? (
                                  <>
                                    <Zap className="w-3 h-3 text-orange-500" />
                                    <span className="text-xs font-medium text-orange-500">Tool</span>
                                  </>
                                ) : null}
                                {msg.tool_calls && msg.tool_calls.length > 0 && (
                                  <span className="text-xs text-textTertiary">
                                    → {msg.tool_calls.map(t => t.name).join(', ')}
                                  </span>
                                )}
                              </div>
                              <p className="text-sm text-textSecondary line-clamp-2 bg-background p-2 rounded border border-border">
                                {msg.content || (msg.tool_calls ? `Calling: ${msg.tool_calls.map(t => t.name).join(', ')}` : '(empty)')}
                              </p>
                            </div>
                          ))
                        ) : (
                          <p className="text-sm text-textTertiary italic">No messages available</p>
                        )}
                        {trace.messages.length > 4 && (
                          <p className="text-xs text-textTertiary text-center">
                            +{trace.messages.length - 4} more messages
                          </p>
                        )}
                      </div>
                    </div>
                  ))
                )}
              </div>
            </>
          ) : (
            <div className="flex-1 flex items-center justify-center">
              <div className="text-center">
                <MessageCircle className="w-16 h-16 text-textTertiary mx-auto mb-4 opacity-50" />
                <p className="text-textSecondary mb-2">Click a session to view traces</p>
                <p className="text-sm text-textTertiary">
                  Preview model, input/output for each trace
                </p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div >
  );
}
