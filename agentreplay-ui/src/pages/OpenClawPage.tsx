// Copyright 2025 AgentReplay (https://github.com/agentreplay)
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

// ============================================================================
// OpenClaw Observability — Unified Agent Gateway Dashboard
//
// Single page covering:
//   • Overview   — aggregated metrics (tokens, costs, sessions, queue)
//   • Agents     — register / manage Moltbot, Clawdbot, OpenClaw agents
//   • Models     — per-model token & cost breakdown
//   • Channels   — per-channel webhook & message metrics
//   • Queue      — queue depth, lanes, webhook / message processing
//   • Memory     — cross-agent skill memory stats
//   • Activity   — OTLP diagnostic event feed
//   • Import     — SKILL.md importer
//
// All data flows through /api/v1/openclaw/* endpoints.
// ============================================================================

import React, { useEffect, useState, useCallback } from 'react';
import {
  Activity,
  AlertCircle,
  AlertTriangle,
  ArrowDownCircle,
  ArrowUpCircle,
  BarChart3,
  Bot,
  Brain,
  CheckCircle,
  Clock,
  Cpu,
  DollarSign,
  FileText,
  Globe,
  Inbox,
  Layers,
  MessageSquare,
  Power,
  RefreshCw,
  Server,
  Settings,
  Share2,
  Shield,
  TrendingUp,
  Upload,
  Webhook,
  XCircle,
  Zap,
} from 'lucide-react';
import { API_BASE_URL } from '../lib/agentreplay-api-core';

// ============================================================================
// Types — mirrors backend openclaw_enrichment types
// ============================================================================

interface TokenBreakdown {
  input: number;
  output: number;
  cache_read: number;
  cache_write: number;
  total: number;
}

interface ModelUsage {
  provider: string;
  model: string;
  request_count: number;
  tokens: TokenBreakdown;
  cost_usd: number;
  avg_duration_ms: number;
  error_count: number;
}

interface ChannelMetrics {
  channel: string;
  messages_processed: number;
  messages_queued: number;
  webhooks_received: number;
  webhook_errors: number;
  avg_message_duration_ms: number;
  avg_webhook_duration_ms: number;
}

interface SessionStateMetrics {
  idle: number;
  processing: number;
  waiting: number;
  stuck: number;
  total_transitions: number;
}

interface LaneMetrics {
  enqueue_count: number;
  dequeue_count: number;
  current_size: number;
}

interface QueueMetrics {
  current_depth: number;
  total_enqueued: number;
  total_dequeued: number;
  avg_wait_ms: number;
  max_wait_ms: number;
  lanes: Record<string, LaneMetrics>;
}

interface WebhookMetrics {
  received: number;
  processed: number;
  errors: number;
  avg_duration_ms: number;
}

interface MessageMetrics {
  queued: number;
  completed: number;
  skipped: number;
  errors: number;
  avg_duration_ms: number;
}

interface OpenclawEvent {
  event_id: string;
  event_type: string;
  description: string;
  metadata: Record<string, unknown>;
  timestamp: string;
}

interface OpenclawMetrics {
  tokens: TokenBreakdown;
  cost_usd: number;
  model_usage: Record<string, ModelUsage>;
  channel_metrics: Record<string, ChannelMetrics>;
  session_states: SessionStateMetrics;
  queue_metrics: QueueMetrics;
  webhook_metrics: WebhookMetrics;
  message_metrics: MessageMetrics;
  total_runs: number;
  recent_events: OpenclawEvent[];
  last_updated: string;
}

// ── Agent types (from bot_registry) ─────────────────────────────────────────

type BotKind = 'moltbot' | 'clawdbot' | 'openclaw';
type BotStatus = 'online' | 'busy' | 'offline' | 'error' | 'maintenance';

interface BotTool {
  name: string;
  description: string;
  input_schema?: any;
  enabled: boolean;
}

interface BotConfig {
  max_concurrent_sessions: number;
  token_budget: number;
  task_timeout_secs: number;
  skill_sharing_enabled: boolean;
  accept_skills_from: BotKind[];
  system_prompt?: string;
  temperature: number;
  tools: BotTool[];
}

interface BotInstance {
  bot_id: string;
  kind: BotKind;
  name: string;
  description: string;
  version: string;
  model: string;
  status: BotStatus;
  config: BotConfig;
  skill_ids: string[];
  active_sessions: number;
  tasks_completed: number;
  total_tokens: number;
  success_rate: number;
  memory_namespace: string;
  created_at: string;
  updated_at: string;
  last_active_at?: string;
  metadata: Record<string, string>;
}

interface BotActivityEvent {
  event_id: string;
  bot_id: string;
  event_type: string;
  description: string;
  session_id?: string;
  skill_id?: string;
  timestamp: string;
  metadata: Record<string, string>;
}

interface BotRegistryStats {
  total_bots: number;
  bots_by_kind: Record<string, number>;
  bots_by_status: Record<string, number>;
  total_tasks_completed: number;
  total_tokens_consumed: number;
  total_events: number;
}

// ── Memory types ────────────────────────────────────────────────────────────

interface SkillInvocation {
  invocation_id: string;
  skill_id: string;
  bot_kind: string;
  input_summary: string;
  output_summary: string;
  success: boolean;
  duration_ms: number;
  context: Record<string, string>;
  timestamp: string;
}

interface SkillMemoryStats {
  total_skills: number;
  total_invocations: number;
  total_evolutions: number;
  overall_success_rate: number;
  skills_by_bot: Record<string, number>;
  skills_by_category: Record<string, number>;
  skills_by_status: Record<string, number>;
  recent_invocations: SkillInvocation[];
}

// ============================================================================
// Agent theme constants
// ============================================================================

const AGENT_THEMES: Record<BotKind, { color: string; bg: string; border: string; icon: string; label: string }> = {
  moltbot: {
    color: 'text-purple-600 dark:text-purple-400',
    bg: 'bg-purple-500/10',
    border: 'border-purple-500/30',
    icon: '🔮',
    label: 'Moltbot',
  },
  clawdbot: {
    color: 'text-orange-600 dark:text-orange-400',
    bg: 'bg-orange-500/10',
    border: 'border-orange-500/30',
    icon: '🦀',
    label: 'Clawdbot',
  },
  openclaw: {
    color: 'text-red-600 dark:text-red-400',
    bg: 'bg-red-500/10',
    border: 'border-red-500/30',
    icon: '🦞',
    label: 'OpenClaw',
  },
};

const STATUS_CONFIGS: Record<BotStatus, { color: string; bg: string; label: string }> = {
  online: { color: 'text-green-600 dark:text-green-400', bg: 'bg-green-400', label: 'Online' },
  busy: { color: 'text-yellow-600 dark:text-yellow-400', bg: 'bg-yellow-400', label: 'Busy' },
  offline: { color: 'text-textTertiary', bg: 'bg-gray-400', label: 'Offline' },
  error: { color: 'text-red-600 dark:text-red-400', bg: 'bg-red-400', label: 'Error' },
  maintenance: { color: 'text-blue-600 dark:text-blue-400', bg: 'bg-blue-400', label: 'Maintenance' },
};

// ============================================================================
// API Helpers
// ============================================================================

async function fetchJSON<T>(path: string): Promise<T> {
  const res = await fetch(`${API_BASE_URL}${path}`);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
}

async function postJSON<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${API_BASE_URL}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
}

async function putJSON<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${API_BASE_URL}${path}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
}

// ============================================================================
// Formatting Helpers
// ============================================================================

function fmtNum(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toLocaleString();
}

function fmtCost(n: number): string {
  return `$${n.toFixed(4)}`;
}

function fmtDuration(ms: number): string {
  if (ms === 0) return '—';
  if (ms < 1_000) return `${ms.toFixed(0)}ms`;
  return `${(ms / 1_000).toFixed(2)}s`;
}

function fmtTimestamp(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  } catch {
    return iso;
  }
}

function fmtPercent(n: number): string {
  return `${(n * 100).toFixed(0)}%`;
}

// ============================================================================
// Main Page Component
// ============================================================================

type Tab = 'overview' | 'agents' | 'models' | 'channels' | 'queue' | 'memory' | 'events' | 'import';

export default function OpenClawPage() {
  const [tab, setTab] = useState<Tab>('overview');
  const [metrics, setMetrics] = useState<OpenclawMetrics | null>(null);
  const [agents, setAgents] = useState<BotInstance[]>([]);
  const [agentStats, setAgentStats] = useState<BotRegistryStats | null>(null);
  const [agentEvents, setAgentEvents] = useState<BotActivityEvent[]>([]);
  const [memoryStats, setMemoryStats] = useState<SkillMemoryStats | null>(null);
  const [selectedAgent, setSelectedAgent] = useState<BotInstance | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadAll = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [metricsRes, agentsRes, statsRes, eventsRes, memRes] = await Promise.all([
        fetchJSON<{ success: boolean; data: OpenclawMetrics }>('/api/v1/openclaw/metrics'),
        fetchJSON<{ success: boolean; agents: BotInstance[]; total: number }>('/api/v1/openclaw/agents'),
        fetchJSON<{ success: boolean; stats: BotRegistryStats }>('/api/v1/openclaw/agents/stats'),
        fetchJSON<{ success: boolean; events: BotActivityEvent[] }>('/api/v1/openclaw/agents/events?limit=30'),
        fetchJSON<{ success: boolean; stats: SkillMemoryStats | null }>('/api/v1/openclaw/memory/stats'),
      ]);
      setMetrics(metricsRes.data);
      setAgents(agentsRes.agents);
      setAgentStats(statsRes.stats);
      setAgentEvents(eventsRes.events);
      if (memRes.stats) setMemoryStats(memRes.stats);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadAll();
    const iv = setInterval(loadAll, 15_000);
    return () => clearInterval(iv);
  }, [loadAll]);

  const handleAgentStatusChange = async (botId: string, newStatus: BotStatus) => {
    try {
      const res = await putJSON<{ success: boolean; agent: BotInstance }>(
        `/api/v1/openclaw/agents/${botId}`,
        { status: newStatus }
      );
      setAgents((prev) => prev.map((a) => (a.bot_id === botId ? res.agent : a)));
      if (selectedAgent?.bot_id === botId) setSelectedAgent(res.agent);
    } catch (e: any) {
      setError(e.message);
    }
  };

  // ── Tab definitions ────────────────────────────────────────────────────
  const tabs: { id: Tab; label: string; icon: React.ElementType }[] = [
    { id: 'overview', label: 'Overview', icon: BarChart3 },
    { id: 'agents', label: 'Agents', icon: Bot },
    { id: 'models', label: 'Models', icon: Cpu },
    { id: 'channels', label: 'Channels', icon: Globe },
    { id: 'queue', label: 'Queue', icon: Inbox },
    { id: 'memory', label: 'Memory', icon: Brain },
    { id: 'events', label: 'Activity', icon: Activity },
    { id: 'import', label: 'Import', icon: FileText },
  ];

  // ── Render ─────────────────────────────────────────────────────────────
  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-4 border-b border-border">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-lg bg-orange-500/15 flex items-center justify-center">
            <span className="text-lg">🦞</span>
          </div>
          <div>
            <h1 className="text-lg font-semibold text-textPrimary">OpenClaw Observability</h1>
            <p className="text-xs text-textTertiary">
              {metrics
                ? `Last updated ${fmtTimestamp(metrics.last_updated)}`
                : 'Connecting…'}
              {agentStats ? ` · ${agentStats.total_bots} agents` : ''}
            </p>
          </div>
        </div>
        <button
          onClick={loadAll}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 text-xs text-textTertiary hover:text-textPrimary border border-border rounded-lg hover:border-border transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
          Refresh
        </button>
      </div>

      {/* Tab bar */}
      <div className="flex gap-1 px-6 pt-3 pb-0 border-b border-border">
        {tabs.map((t) => {
          const active = tab === t.id;
          return (
            <button
              key={t.id}
              onClick={() => setTab(t.id)}
              className={`flex items-center gap-1.5 px-3 py-2 text-xs font-medium rounded-t-lg transition-colors ${
                active
                  ? 'text-orange-600 dark:text-orange-400 border-b-2 border-orange-500 bg-surface-hover'
                  : 'text-textTertiary hover:text-textSecondary border-b-2 border-transparent'
              }`}
            >
              <t.icon className="w-3.5 h-3.5" />
              {t.label}
            </button>
          );
        })}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-6">
        {error && (
          <div className="mb-4 flex items-center gap-2 text-red-600 dark:text-red-400 bg-red-500/10 border border-red-500/20 rounded-lg px-4 py-3 text-sm">
            <AlertTriangle className="w-4 h-4 flex-shrink-0" />
            {error}
          </div>
        )}

        {loading && !metrics ? (
          <div className="flex items-center justify-center py-24">
            <RefreshCw className="w-6 h-6 text-textTertiary animate-spin" />
          </div>
        ) : (
          <>
            {tab === 'overview' && metrics && (
              <OverviewTab metrics={metrics} agents={agents} agentStats={agentStats} memoryStats={memoryStats} />
            )}
            {tab === 'agents' && (
              <AgentsTab
                agents={agents}
                selectedAgent={selectedAgent}
                setSelectedAgent={setSelectedAgent}
                onStatusChange={handleAgentStatusChange}
                loading={loading}
                agentEvents={agentEvents}
              />
            )}
            {tab === 'models' && metrics && (
              <Section title="Model Usage" icon={Cpu}>
                <ModelUsageTable models={Object.values(metrics.model_usage)} />
              </Section>
            )}
            {tab === 'channels' && metrics && (
              <Section title="Channel Metrics" icon={Globe}>
                <ChannelCards channels={Object.values(metrics.channel_metrics)} />
              </Section>
            )}
            {tab === 'queue' && metrics && (
              <div className="space-y-6">
                <Section title="Queue Metrics" icon={Inbox}>
                  <QueueDetail queue={metrics.queue_metrics} />
                </Section>
                <Section title="Processing" icon={Zap}>
                  <ProcessingStats webhooks={metrics.webhook_metrics} messages={metrics.message_metrics} />
                </Section>
              </div>
            )}
            {tab === 'memory' && (
              <MemoryTab stats={memoryStats} />
            )}
            {tab === 'events' && metrics && (
              <Section title="Activity Feed" icon={Activity}>
                <ActivityFeed events={metrics.recent_events} />
              </Section>
            )}
            {tab === 'import' && (
              <Section title="Import SKILL.md" icon={FileText}>
                <SkillImporter onImported={loadAll} />
              </Section>
            )}
          </>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Overview Tab — enriched with agent + memory stats
// ============================================================================

function OverviewTab({
  metrics,
  agents,
  agentStats,
  memoryStats,
}: {
  metrics: OpenclawMetrics;
  agents: BotInstance[];
  agentStats: BotRegistryStats | null;
  memoryStats: SkillMemoryStats | null;
}) {
  return (
    <div className="space-y-6">
      {/* Key stats */}
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-3">
        <OverviewStatCard
          icon={Zap}
          label="Total Tokens"
          value={fmtNum(metrics.tokens.total)}
          sub={`↓${fmtNum(metrics.tokens.input)} ↑${fmtNum(metrics.tokens.output)}`}
          accent="text-orange-400"
        />
        <OverviewStatCard
          icon={DollarSign}
          label="Total Cost"
          value={fmtCost(metrics.cost_usd)}
          sub={`${Object.keys(metrics.model_usage).length} models`}
          accent="text-emerald-400"
        />
        <OverviewStatCard
          icon={TrendingUp}
          label="Agent Runs"
          value={fmtNum(metrics.total_runs)}
          accent="text-sky-400"
        />
        <OverviewStatCard
          icon={Bot}
          label="Agents"
          value={agentStats ? String(agentStats.total_bots) : '—'}
          sub={agentStats ? `${agentStats.bots_by_status?.['online'] ?? 0} online` : undefined}
          accent="text-purple-400"
        />
        <OverviewStatCard
          icon={Globe}
          label="Channels"
          value={String(Object.keys(metrics.channel_metrics).length)}
          accent="text-indigo-400"
        />
        <OverviewStatCard
          icon={Brain}
          label="Skills"
          value={memoryStats ? fmtNum(memoryStats.total_skills) : '0'}
          sub={memoryStats ? `${fmtNum(memoryStats.total_invocations)} invocations` : undefined}
          accent="text-pink-400"
        />
      </div>

      {/* Agent summary row */}
      {agents.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
          {agents.map((agent) => {
            const theme = AGENT_THEMES[agent.kind];
            const statusCfg = STATUS_CONFIGS[agent.status];
            return (
              <div
                key={agent.bot_id}
                className={`rounded-xl border p-4 ${theme.bg} ${theme.border}`}
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <span className="text-lg">{theme.icon}</span>
                    <span className={`text-sm font-medium ${theme.color}`}>{agent.name}</span>
                  </div>
                  <div className="flex items-center gap-1.5">
                    <div className={`w-2 h-2 rounded-full ${statusCfg.bg} ${agent.status === 'online' ? 'animate-pulse' : ''}`} />
                    <span className={`text-xs ${statusCfg.color}`}>{statusCfg.label}</span>
                  </div>
                </div>
                <div className="grid grid-cols-3 gap-2 text-center">
                  <div>
                    <div className="text-sm font-semibold text-textPrimary">{agent.tasks_completed}</div>
                    <div className="text-[10px] text-textTertiary">Runs</div>
                  </div>
                  <div>
                    <div className="text-sm font-semibold text-textPrimary">{fmtPercent(agent.success_rate)}</div>
                    <div className="text-[10px] text-textTertiary">Success</div>
                  </div>
                  <div>
                    <div className="text-sm font-semibold text-textPrimary">{fmtNum(agent.total_tokens)}</div>
                    <div className="text-[10px] text-textTertiary">Tokens</div>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Tokens breakdown */}
      <Section title="Token Breakdown" icon={BarChart3}>
        <TokenBars tokens={metrics.tokens} />
      </Section>

      {/* Session states */}
      <Section title="Session States" icon={Server}>
        <SessionStates sessions={metrics.session_states} />
      </Section>

      {/* Processing */}
      <Section title="Processing" icon={Zap}>
        <ProcessingStats webhooks={metrics.webhook_metrics} messages={metrics.message_metrics} />
      </Section>

      {/* Recent activity */}
      <Section title="Recent Activity" icon={Activity}>
        <ActivityFeed events={metrics.recent_events.slice(0, 20)} />
      </Section>
    </div>
  );
}

// ============================================================================
// Agents Tab — full agent management with CRUD & monitoring
// ============================================================================

function AgentsTab({
  agents,
  selectedAgent,
  setSelectedAgent,
  onStatusChange,
  loading,
  agentEvents,
}: {
  agents: BotInstance[];
  selectedAgent: BotInstance | null;
  setSelectedAgent: (a: BotInstance | null) => void;
  onStatusChange: (botId: string, status: BotStatus) => void;
  loading: boolean;
  agentEvents: BotActivityEvent[];
}) {
  return (
    <div className="space-y-6">
      {/* Agent Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {loading && agents.length === 0
          ? [1, 2, 3].map((i) => (
              <div key={i} className="h-60 border border-border bg-surface rounded-xl animate-pulse" />
            ))
          : agents.map((agent) => (
              <AgentCard
                key={agent.bot_id}
                agent={agent}
                isSelected={selectedAgent?.bot_id === agent.bot_id}
                onSelect={() =>
                  setSelectedAgent(selectedAgent?.bot_id === agent.bot_id ? null : agent)
                }
                onStatusChange={onStatusChange}
              />
            ))}
      </div>

      {/* Detail Panel */}
      {selectedAgent && (
        <Section title={`${selectedAgent.name} — Configuration`} icon={Settings}>
          <AgentDetailPanel agent={selectedAgent} />
        </Section>
      )}

      {/* Agent Activity */}
      <Section title="Agent Activity" icon={Activity}>
        {agentEvents.length === 0 ? (
          <EmptyState icon={Activity} message="No agent events yet" />
        ) : (
          <div className="space-y-1 max-h-[300px] overflow-y-auto pr-1">
            {agentEvents.map((evt) => {
              const agentBot = agents.find((a) => a.bot_id === evt.bot_id);
              return (
                <div
                  key={evt.event_id}
                  className="flex items-start gap-3 px-3 py-2 rounded-lg hover:bg-surface-hover transition-colors"
                >
                  <AgentEventIcon type={evt.event_type} />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-sm text-textSecondary truncate">{evt.description}</span>
                      {agentBot && (
                        <span className={`text-[10px] ${AGENT_THEMES[agentBot.kind].color} shrink-0`}>
                          {agentBot.name}
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-2 mt-0.5">
                      <span className="text-[10px] text-textTertiary font-mono">{evt.event_type}</span>
                      <span className="text-[10px] text-textTertiary">{fmtTimestamp(evt.timestamp)}</span>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </Section>
    </div>
  );
}

function AgentCard({
  agent,
  isSelected,
  onSelect,
  onStatusChange,
}: {
  agent: BotInstance;
  isSelected: boolean;
  onSelect: () => void;
  onStatusChange: (botId: string, status: BotStatus) => void;
}) {
  const theme = AGENT_THEMES[agent.kind];
  const statusCfg = STATUS_CONFIGS[agent.status];

  return (
    <div
      className={`rounded-xl border p-5 cursor-pointer transition-all ${
        isSelected
          ? `${theme.bg} ${theme.border}`
          : 'border-border bg-surface hover:bg-surface-hover'
      }`}
      onClick={onSelect}
    >
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <span className="text-xl">{theme.icon}</span>
          <div>
            <h3 className={`font-semibold ${theme.color}`}>{agent.name}</h3>
            <span className="text-xs text-textTertiary">v{agent.version}</span>
          </div>
        </div>
        <div className="flex items-center gap-1.5">
          <div className={`w-2 h-2 rounded-full ${statusCfg.bg} ${agent.status === 'online' ? 'animate-pulse' : ''}`} />
          <span className={`text-xs ${statusCfg.color}`}>{statusCfg.label}</span>
        </div>
      </div>

      <p className="text-xs text-textTertiary mb-3 line-clamp-2">{agent.description}</p>

      {/* Model */}
      <div className="flex items-center gap-2 mb-3 text-xs">
        <Cpu className="w-3 h-3 text-textTertiary" />
        <span className="text-textTertiary font-mono text-[11px]">{agent.model}</span>
      </div>

      {/* Mini stats */}
      <div className="grid grid-cols-3 gap-2 mb-3">
        <AgentMiniStat label="Runs" value={agent.tasks_completed} />
        <AgentMiniStat label="Success" value={fmtPercent(agent.success_rate)} />
        <AgentMiniStat label="Tokens" value={fmtNum(agent.total_tokens)} />
      </div>

      {/* Actions */}
      <div className="flex gap-1.5">
        {agent.status === 'online' ? (
          <button
            onClick={(e) => { e.stopPropagation(); onStatusChange(agent.bot_id, 'offline'); }}
            className="flex-1 px-2 py-1.5 bg-red-500/10 text-red-600 dark:text-red-400 rounded-md text-xs hover:bg-red-500/20 transition-colors"
          >
            Stop
          </button>
        ) : (
          <button
            onClick={(e) => { e.stopPropagation(); onStatusChange(agent.bot_id, 'online'); }}
            className="flex-1 px-2 py-1.5 bg-green-500/10 text-green-600 dark:text-green-400 rounded-md text-xs hover:bg-green-500/20 transition-colors"
          >
            Start
          </button>
        )}
      </div>
    </div>
  );
}

function AgentDetailPanel({ agent }: { agent: BotInstance }) {
  return (
    <div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-x-8 gap-y-2 text-xs">
        <ConfigRow label="Agent ID" value={agent.bot_id} mono />
        <ConfigRow label="Kind" value={agent.kind} />
        <ConfigRow label="Model" value={agent.model} mono />
        <ConfigRow label="Memory Namespace" value={agent.memory_namespace} mono />
        <ConfigRow label="Max Sessions" value={agent.config.max_concurrent_sessions.toString()} />
        <ConfigRow label="Token Budget" value={fmtNum(agent.config.token_budget)} />
        <ConfigRow label="Timeout" value={`${agent.config.task_timeout_secs}s`} />
        <ConfigRow label="Temperature" value={agent.config.temperature.toString()} />
        <ConfigRow label="Skill Sharing" value={agent.config.skill_sharing_enabled ? 'Enabled' : 'Disabled'} />
        <ConfigRow label="Skills Loaded" value={agent.skill_ids.length.toString()} />
      </div>

      {agent.config.tools.length > 0 && (
        <div className="mt-4">
          <span className="text-xs text-textTertiary block mb-2">Tools ({agent.config.tools.length})</span>
          <div className="flex flex-wrap gap-1.5">
            {agent.config.tools.map((t) => (
              <span
                key={t.name}
                className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-xs border ${
                  t.enabled ? 'bg-surface border-border text-textSecondary' : 'bg-surface/50 border-border text-textTertiary line-through'
                }`}
              >
                {t.name}
              </span>
            ))}
          </div>
        </div>
      )}

      <div className="mt-4 pt-3 border-t border-border text-xs text-textTertiary flex gap-4">
        <span>Created {new Date(agent.created_at).toLocaleDateString()}</span>
        {agent.last_active_at && <span>Last active {new Date(agent.last_active_at).toLocaleString()}</span>}
      </div>
    </div>
  );
}

function AgentMiniStat({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="text-center p-2 bg-surface-hover rounded-lg">
      <div className="text-sm font-bold text-textPrimary">{value}</div>
      <div className="text-[10px] text-textTertiary">{label}</div>
    </div>
  );
}

function AgentEventIcon({ type }: { type: string }) {
  const classes = 'w-3.5 h-3.5 mt-0.5';
  switch (type) {
    case 'started': return <Power className={`${classes} text-green-500`} />;
    case 'stopped': return <Power className={`${classes} text-red-500`} />;
    case 'task_completed': return <CheckCircle className={`${classes} text-green-500`} />;
    case 'task_failed': return <XCircle className={`${classes} text-red-500`} />;
    case 'skill_learned': return <TrendingUp className={`${classes} text-purple-500`} />;
    case 'skill_shared': return <Share2 className={`${classes} text-blue-500`} />;
    case 'config_updated': return <Settings className={`${classes} text-yellow-500`} />;
    case 'error': return <AlertCircle className={`${classes} text-red-500`} />;
    default: return <Activity className={`${classes} text-textTertiary`} />;
  }
}

// ============================================================================
// Memory Tab — skill memory integration
// ============================================================================

function MemoryTab({ stats }: { stats: SkillMemoryStats | null }) {
  if (!stats) {
    return <EmptyState icon={Brain} message="Skill memory store not available" />;
  }

  const botEntries = Object.entries(stats.skills_by_bot).sort((a, b) => b[1] - a[1]);
  const categoryEntries = Object.entries(stats.skills_by_category).sort((a, b) => b[1] - a[1]);
  const statusEntries = Object.entries(stats.skills_by_status).sort((a, b) => b[1] - a[1]);

  return (
    <div className="space-y-6">
      {/* Stats grid */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <OverviewStatCard icon={Brain} label="Total Skills" value={fmtNum(stats.total_skills)} accent="text-pink-400" />
        <OverviewStatCard icon={Zap} label="Invocations" value={fmtNum(stats.total_invocations)} accent="text-orange-400" />
        <OverviewStatCard icon={TrendingUp} label="Evolutions" value={fmtNum(stats.total_evolutions)} accent="text-purple-400" />
        <OverviewStatCard
          icon={CheckCircle}
          label="Success Rate"
          value={fmtPercent(stats.overall_success_rate)}
          accent="text-emerald-400"
        />
      </div>

      {/* Skills by bot */}
      {botEntries.length > 0 && (
        <Section title="Skills by Agent" icon={Bot}>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
            {botEntries.map(([bot, count]) => {
              const theme = AGENT_THEMES[bot as BotKind] || AGENT_THEMES.openclaw;
              return (
                <div key={bot} className={`rounded-xl border p-4 ${theme.bg} ${theme.border}`}>
                  <div className="flex items-center gap-2 mb-1">
                    <span>{theme.icon}</span>
                    <span className={`text-xs font-medium ${theme.color}`}>{theme.label}</span>
                  </div>
                  <div className="text-2xl font-bold text-textPrimary">{count}</div>
                  <div className="text-[10px] text-textTertiary">skills created</div>
                </div>
              );
            })}
          </div>
        </Section>
      )}

      {/* Skills by category */}
      {categoryEntries.length > 0 && (
        <Section title="Skills by Category" icon={Layers}>
          <div className="flex flex-wrap gap-2">
            {categoryEntries.map(([cat, count]) => (
              <span
                key={cat}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-surface border border-border text-xs"
              >
                <span className="text-textPrimary font-medium capitalize">{cat}</span>
                <span className="text-textTertiary">{count}</span>
              </span>
            ))}
          </div>
        </Section>
      )}

      {/* Skills by status */}
      {statusEntries.length > 0 && (
        <Section title="Skills by Status" icon={Shield}>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            {statusEntries.map(([status, count]) => (
              <div key={status} className="text-center p-3 bg-surface-hover rounded-lg">
                <div className="text-lg font-semibold text-textPrimary">{count}</div>
                <div className="text-xs text-textTertiary capitalize">{status}</div>
              </div>
            ))}
          </div>
        </Section>
      )}

      {/* Recent invocations */}
      {stats.recent_invocations.length > 0 && (
        <Section title="Recent Invocations" icon={Activity}>
          <div className="space-y-1 max-h-[300px] overflow-y-auto pr-1">
            {stats.recent_invocations.map((inv) => (
              <div
                key={inv.invocation_id}
                className="flex items-start gap-3 px-3 py-2 rounded-lg hover:bg-surface-hover transition-colors"
              >
                {inv.success ? (
                  <CheckCircle className="w-3.5 h-3.5 mt-0.5 text-emerald-400" />
                ) : (
                  <XCircle className="w-3.5 h-3.5 mt-0.5 text-red-400" />
                )}
                <div className="flex-1 min-w-0">
                  <span className="text-sm text-textSecondary">{inv.output_summary || inv.input_summary}</span>
                  <div className="flex items-center gap-2 mt-0.5">
                    <span className="text-[10px] text-textTertiary">{inv.bot_kind}</span>
                    <span className="text-[10px] text-textTertiary">{fmtDuration(inv.duration_ms)}</span>
                    <span className="text-[10px] text-textTertiary">{fmtTimestamp(inv.timestamp)}</span>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </Section>
      )}
    </div>
  );
}

// ============================================================================
// Shared Sub-Components
// ============================================================================

function OverviewStatCard({
  icon: Icon,
  label,
  value,
  sub,
  accent = 'text-orange-400',
}: {
  icon: React.ElementType;
  label: string;
  value: string;
  sub?: string;
  accent?: string;
}) {
  return (
    <div className="border border-border bg-surface rounded-xl p-4 flex flex-col gap-1">
      <div className="flex items-center gap-2 text-xs text-textTertiary uppercase tracking-wider">
        <Icon className={`w-3.5 h-3.5 ${accent}`} />
        {label}
      </div>
      <div className="text-2xl font-semibold text-textPrimary">{value}</div>
      {sub && <div className="text-xs text-textTertiary">{sub}</div>}
    </div>
  );
}

// ── Token Bars ──────────────────────────────────────────────────────────────

function TokenBars({ tokens }: { tokens: TokenBreakdown }) {
  const max = Math.max(tokens.input, tokens.output, tokens.cache_read, tokens.cache_write, 1);
  const bars = [
    { label: 'Input', value: tokens.input, color: 'bg-orange-500' },
    { label: 'Output', value: tokens.output, color: 'bg-emerald-500' },
    { label: 'Cache Read', value: tokens.cache_read, color: 'bg-sky-500' },
    { label: 'Cache Write', value: tokens.cache_write, color: 'bg-purple-500' },
  ];
  return (
    <div className="grid gap-2">
      {bars.map((b) => (
        <div key={b.label} className="flex items-center gap-3">
          <span className="w-24 text-xs text-textTertiary text-right">{b.label}</span>
          <div className="flex-1 bg-surface-hover rounded-full h-3 overflow-hidden">
            <div
              className={`${b.color} h-full rounded-full transition-all duration-500`}
              style={{ width: `${Math.max((b.value / max) * 100, 0.5)}%` }}
            />
          </div>
          <span className="w-20 text-xs text-textSecondary text-right font-mono">{fmtNum(b.value)}</span>
        </div>
      ))}
    </div>
  );
}

// ── Model Usage Table ───────────────────────────────────────────────────────

function ModelUsageTable({ models }: { models: ModelUsage[] }) {
  const sorted = [...models].sort((a, b) => b.request_count - a.request_count);
  if (sorted.length === 0) {
    return <EmptyState icon={Cpu} message="No model usage data yet" />;
  }
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="text-xs text-textTertiary uppercase tracking-wider border-b border-border">
            <th className="text-left py-2 px-3">Provider / Model</th>
            <th className="text-right py-2 px-3">Requests</th>
            <th className="text-right py-2 px-3">Tokens</th>
            <th className="text-right py-2 px-3">Cost</th>
            <th className="text-right py-2 px-3">Avg Latency</th>
            <th className="text-right py-2 px-3">Errors</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((m) => (
            <tr
              key={`${m.provider}-${m.model}`}
              className="border-b border-border/50 hover:bg-surface-hover transition-colors"
            >
              <td className="py-2.5 px-3">
                <div className="text-textPrimary font-medium">{m.model}</div>
                <div className="text-xs text-textTertiary">{m.provider}</div>
              </td>
              <td className="text-right py-2.5 px-3 text-textSecondary font-mono">
                {fmtNum(m.request_count)}
              </td>
              <td className="text-right py-2.5 px-3 text-textSecondary font-mono">
                {fmtNum(m.tokens.total)}
              </td>
              <td className="text-right py-2.5 px-3 text-emerald-400 font-mono">
                {fmtCost(m.cost_usd)}
              </td>
              <td className="text-right py-2.5 px-3 text-textTertiary font-mono">
                {fmtDuration(m.avg_duration_ms)}
              </td>
              <td className="text-right py-2.5 px-3">
                {m.error_count > 0 ? (
                  <span className="text-red-500 dark:text-red-400 font-mono">{m.error_count}</span>
                ) : (
                  <span className="text-textTertiary">0</span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ── Channel Cards ───────────────────────────────────────────────────────────

function ChannelCards({ channels }: { channels: ChannelMetrics[] }) {
  if (channels.length === 0) {
    return <EmptyState icon={Globe} message="No channel data yet — connect OpenClaw via OTLP to see metrics" />;
  }
  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
      {channels.map((ch) => (
        <div
          key={ch.channel}
          className="border border-border bg-surface rounded-xl p-4"
        >
          <div className="flex items-center gap-2 mb-3">
            <Globe className="w-4 h-4 text-sky-500 dark:text-sky-400" />
            <span className="text-textPrimary font-medium capitalize">{ch.channel}</span>
          </div>
          <div className="grid grid-cols-2 gap-y-2 text-xs">
            <div>
              <span className="text-textTertiary">Messages</span>
              <div className="text-textSecondary font-mono">{fmtNum(ch.messages_processed)}</div>
            </div>
            <div>
              <span className="text-textTertiary">Queued</span>
              <div className="text-textSecondary font-mono">{fmtNum(ch.messages_queued)}</div>
            </div>
            <div>
              <span className="text-textTertiary">Webhooks</span>
              <div className="text-textSecondary font-mono">{fmtNum(ch.webhooks_received)}</div>
            </div>
            <div>
              <span className="text-textTertiary">Errors</span>
              <div className={`font-mono ${ch.webhook_errors > 0 ? 'text-red-500 dark:text-red-400' : 'text-textTertiary'}`}>
                {ch.webhook_errors}
              </div>
            </div>
            <div>
              <span className="text-textTertiary">Msg Latency</span>
              <div className="text-textSecondary font-mono">{fmtDuration(ch.avg_message_duration_ms)}</div>
            </div>
            <div>
              <span className="text-textTertiary">WH Latency</span>
              <div className="text-textSecondary font-mono">{fmtDuration(ch.avg_webhook_duration_ms)}</div>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

// ── Session State Ring ──────────────────────────────────────────────────────

function SessionStates({ sessions }: { sessions: SessionStateMetrics }) {
  const total = sessions.idle + sessions.processing + sessions.waiting + sessions.stuck;
  const states = [
    { label: 'Idle', value: sessions.idle, color: 'bg-gray-500', text: 'text-textTertiary' },
    { label: 'Processing', value: sessions.processing, color: 'bg-emerald-500', text: 'text-emerald-400' },
    { label: 'Waiting', value: sessions.waiting, color: 'bg-amber-500', text: 'text-amber-400' },
    { label: 'Stuck', value: sessions.stuck, color: 'bg-red-500', text: 'text-red-400' },
  ];
  return (
    <div>
      {total > 0 && (
        <div className="flex rounded-full h-4 overflow-hidden mb-4">
          {states.map((s) =>
            s.value > 0 ? (
              <div
                key={s.label}
                className={`${s.color} transition-all duration-500`}
                style={{ width: `${(s.value / total) * 100}%` }}
                title={`${s.label}: ${s.value}`}
              />
            ) : null
          )}
        </div>
      )}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
        {states.map((s) => (
          <div key={s.label} className="text-center">
            <div className={`text-2xl font-bold ${s.text}`}>{s.value}</div>
            <div className="text-xs text-textTertiary">{s.label}</div>
          </div>
        ))}
      </div>
      <div className="text-center mt-3 text-xs text-textTertiary">
        {fmtNum(sessions.total_transitions)} total transitions
      </div>
    </div>
  );
}

// ── Queue Metrics ───────────────────────────────────────────────────────────

function QueueDetail({ queue }: { queue: QueueMetrics }) {
  const lanes = Object.entries(queue.lanes).sort((a, b) => b[1].current_size - a[1].current_size);
  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
        <QueueMiniStat label="Depth" value={queue.current_depth} icon={Layers} />
        <QueueMiniStat label="Enqueued" value={queue.total_enqueued} icon={ArrowDownCircle} />
        <QueueMiniStat label="Dequeued" value={queue.total_dequeued} icon={ArrowUpCircle} />
        <QueueMiniStat label="Avg Wait" value={fmtDuration(queue.avg_wait_ms)} icon={Clock} />
      </div>
      {lanes.length > 0 && (
        <div>
          <h4 className="text-xs text-gray-500 uppercase tracking-wider mb-2">Lanes</h4>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            {lanes.map(([name, lane]) => (
              <div
                key={name}
                className="flex items-center justify-between border border-border rounded-lg px-3 py-2"
              >
                <span className="text-sm text-textSecondary capitalize">{name}</span>
                <div className="flex gap-3 text-xs font-mono">
                  <span className="text-emerald-400">↓{fmtNum(lane.enqueue_count)}</span>
                  <span className="text-sky-400">↑{fmtNum(lane.dequeue_count)}</span>
                  <span className="text-orange-400">{lane.current_size} queued</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
      {queue.max_wait_ms > 0 && (
        <div className="text-xs text-textTertiary">
          Max wait: {fmtDuration(queue.max_wait_ms)}
        </div>
      )}
    </div>
  );
}

function QueueMiniStat({
  label,
  value,
  icon: Icon,
}: {
  label: string;
  value: string | number;
  icon: React.ElementType;
}) {
  return (
    <div className="text-center">
      <Icon className="w-4 h-4 text-textTertiary mx-auto mb-1" />
      <div className="text-lg font-semibold text-textPrimary">{typeof value === 'number' ? fmtNum(value) : value}</div>
      <div className="text-xs text-textTertiary">{label}</div>
    </div>
  );
}

// ── Webhook & Message Stats ─────────────────────────────────────────────────

function ProcessingStats({
  webhooks,
  messages,
}: {
  webhooks: WebhookMetrics;
  messages: MessageMetrics;
}) {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
      {/* Webhooks */}
      <div className="border border-border bg-surface rounded-xl p-4">
        <div className="flex items-center gap-2 mb-3">
          <Webhook className="w-4 h-4 text-purple-500 dark:text-purple-400" />
          <span className="text-sm text-textSecondary font-medium">Webhooks</span>
        </div>
        <div className="grid grid-cols-2 gap-y-3 text-xs">
          <Stat label="Received" value={webhooks.received} />
          <Stat label="Processed" value={webhooks.processed} color="text-emerald-400" />
          <Stat label="Errors" value={webhooks.errors} color={webhooks.errors > 0 ? 'text-red-400' : undefined} />
          <Stat label="Avg Duration" value={fmtDuration(webhooks.avg_duration_ms)} />
        </div>
      </div>
      {/* Messages */}
      <div className="border border-border bg-surface rounded-xl p-4">
        <div className="flex items-center gap-2 mb-3">
          <MessageSquare className="w-4 h-4 text-sky-500 dark:text-sky-400" />
          <span className="text-sm text-textSecondary font-medium">Messages</span>
        </div>
        <div className="grid grid-cols-2 gap-y-3 text-xs">
          <Stat label="Queued" value={messages.queued} />
          <Stat label="Completed" value={messages.completed} color="text-emerald-400" />
          <Stat label="Skipped" value={messages.skipped} />
          <Stat label="Errors" value={messages.errors} color={messages.errors > 0 ? 'text-red-400' : undefined} />
        </div>
        <div className="mt-2 text-xs text-textTertiary">Avg: {fmtDuration(messages.avg_duration_ms)}</div>
      </div>
    </div>
  );
}

function Stat({
  label,
  value,
  color,
}: {
  label: string;
  value: string | number;
  color?: string;
}) {
  return (
    <div>
      <span className="text-textTertiary">{label}</span>
      <div className={`font-mono ${color ?? 'text-textSecondary'}`}>
        {typeof value === 'number' ? fmtNum(value) : value}
      </div>
    </div>
  );
}

// ── Activity Feed ───────────────────────────────────────────────────────────

const EVENT_TYPE_STYLES: Record<string, { icon: React.ElementType; color: string }> = {
  'model.usage': { icon: Cpu, color: 'text-orange-400' },
  'webhook.processed': { icon: Webhook, color: 'text-emerald-400' },
  'webhook.error': { icon: AlertTriangle, color: 'text-red-400' },
  'message.processed': { icon: MessageSquare, color: 'text-sky-400' },
  'session.stuck': { icon: AlertTriangle, color: 'text-amber-400' },
  'skill.invocation': { icon: Shield, color: 'text-purple-400' },
  'tool.call': { icon: Zap, color: 'text-teal-400' },
  'agent.lifecycle': { icon: Activity, color: 'text-indigo-400' },
  'agent.run': { icon: Bot, color: 'text-orange-400' },
};

function ActivityFeed({ events }: { events: OpenclawEvent[] }) {
  if (events.length === 0) {
    return <EmptyState icon={Activity} message="No activity events yet — configure OpenClaw OTLP to start streaming" />;
  }
  return (
    <div className="space-y-1 max-h-[400px] overflow-y-auto pr-1 custom-scrollbar">
      {events.map((ev) => {
        const style = EVENT_TYPE_STYLES[ev.event_type] ?? {
          icon: Activity,
          color: 'text-textTertiary',
        };
        const EvIcon = style.icon;
        return (
          <div
            key={ev.event_id}
            className="flex items-start gap-3 px-3 py-2 rounded-lg hover:bg-surface-hover transition-colors"
          >
            <EvIcon className={`w-3.5 h-3.5 mt-0.5 flex-shrink-0 ${style.color}`} />
            <div className="flex-1 min-w-0">
              <span className="text-sm text-textSecondary break-words">{ev.description}</span>
              <span className="ml-2 text-xs text-textTertiary">{fmtTimestamp(ev.timestamp)}</span>
            </div>
            <span className="text-[10px] text-textTertiary whitespace-nowrap">{ev.event_type}</span>
          </div>
        );
      })}
    </div>
  );
}

// ── SKILL.md Import ─────────────────────────────────────────────────────────

function SkillImporter({ onImported }: { onImported: () => void }) {
  const [content, setContent] = useState('');
  const [name, setName] = useState('');
  const [importing, setImporting] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);

  const doImport = async () => {
    if (!content.trim()) return;
    setImporting(true);
    setResult(null);
    try {
      const res = await postJSON<{ success: boolean; skill_id?: string; error?: string }>(
        '/api/v1/openclaw/skills/import',
        { content, name: name || undefined }
      );
      if (res.success) {
        setResult({ ok: true, message: `Imported as ${res.skill_id}` });
        setContent('');
        setName('');
        onImported();
      } else {
        setResult({ ok: false, message: res.error ?? 'Unknown error' });
      }
    } catch (e: any) {
      setResult({ ok: false, message: e.message });
    } finally {
      setImporting(false);
    }
  };

  return (
    <div className="space-y-3">
      <p className="text-xs text-textTertiary">
        Paste a SKILL.md file contents below. The{' '}
        <code className="text-orange-600 dark:text-orange-400 bg-surface-hover px-1 rounded">```skill</code> frontmatter
        block will be parsed and imported into Skill Memory.
      </p>
      <input
        className="w-full bg-surface border border-border rounded-lg px-3 py-2 text-sm text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-orange-500/40"
        placeholder="Skill name (optional — auto-detected from frontmatter)"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <textarea
        className="w-full bg-surface border border-border rounded-lg px-3 py-2 text-sm text-textPrimary placeholder:text-textTertiary font-mono h-48 resize-y focus:outline-none focus:border-orange-500/40"
        placeholder="Paste SKILL.md content here..."
        value={content}
        onChange={(e) => setContent(e.target.value)}
      />
      <div className="flex items-center gap-3">
        <button
          onClick={doImport}
          disabled={importing || !content.trim()}
          className="flex items-center gap-2 px-4 py-2 bg-orange-600 hover:bg-orange-500 disabled:bg-surface-hover disabled:text-textTertiary text-white rounded-lg text-sm font-medium transition-colors"
        >
          <Upload className="w-4 h-4" />
          {importing ? 'Importing…' : 'Import Skill'}
        </button>
        {result && (
          <span className={`text-xs ${result.ok ? 'text-emerald-400' : 'text-red-400'}`}>
            {result.ok ? <CheckCircle className="inline w-3 h-3 mr-1" /> : <XCircle className="inline w-3 h-3 mr-1" />}
            {result.message}
          </span>
        )}
      </div>
    </div>
  );
}

// ── Config Row ──────────────────────────────────────────────────────────────

function ConfigRow({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex items-center gap-2 py-0.5">
      <span className="text-textTertiary w-28 shrink-0">{label}</span>
      <span className={`text-textPrimary ${mono ? 'font-mono text-[11px]' : ''} truncate`}>{value}</span>
    </div>
  );
}

// ── Empty State ─────────────────────────────────────────────────────────────

function EmptyState({ icon: Icon, message }: { icon: React.ElementType; message: string }) {
  return (
    <div className="flex flex-col items-center justify-center py-12 text-textTertiary">
      <Icon className="w-8 h-8 mb-2" />
      <span className="text-sm">{message}</span>
    </div>
  );
}

// ── Section wrapper ─────────────────────────────────────────────────────────

function Section({
  title,
  icon: Icon,
  children,
}: {
  title: string;
  icon: React.ElementType;
  children: React.ReactNode;
}) {
  return (
    <div className="border border-border bg-surface rounded-xl overflow-hidden">
      <div className="px-4 py-3 border-b border-border flex items-center gap-2">
        <Icon className="w-4 h-4 text-orange-500 dark:text-orange-400" />
        <h3 className="text-sm font-medium text-textPrimary">{title}</h3>
      </div>
      <div className="p-4">{children}</div>
    </div>
  );
}
