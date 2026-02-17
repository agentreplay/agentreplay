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

import React, { useEffect, useState, useCallback } from 'react';
import { useParams, Link } from 'react-router-dom';
import {
  Brain,
  Zap,
  Search,
  Plus,
  RefreshCw,
  ChevronRight,
  BarChart3,
  Tag,
  Clock,
  CheckCircle,
  XCircle,
  ArrowUpCircle,
  Share2,
  Layers,
  Activity,
  TrendingUp,
  Filter,
  Trash2,
  Edit,
  GitBranch,
  Bot,
  Cpu,
  Shield,
  ExternalLink,
  Download,
  Save,
  X,
  FileText,
  Code,
  Eye,
  EyeOff,
  Copy,
  Check,
  Wand2,
  AlertTriangle,
  Info,
  BookOpen,
  ChevronDown,
  ChevronUp,
  Lock,
  Terminal as TerminalIcon,
  Server,
  FileCode,
  Globe,
} from 'lucide-react';
import { API_BASE_URL } from '../lib/agentreplay-api-core';

// ============================================================================
// Types
// ============================================================================

type SkillStatus = 'draft' | 'active' | 'deprecated' | 'archived';

type ValidationSeverity = 'pass' | 'warning' | 'error';

interface EditorValidation {
  check: string;
  severity: ValidationSeverity;
  message: string;
}

interface Skill {
  skill_id: string;
  name: string;
  description: string;
  origin_bot: string;
  category: string;
  tags: string[];
  definition: string;
  input_schema?: string;
  output_schema?: string;
  version: number;
  invocation_count: number;
  success_rate: number;
  avg_duration_ms: number;
  avg_tokens: number;
  shared_with: string[];
  status: SkillStatus;
  parent_skill_id?: string;
  episode_ids: string[];
  created_at: string;
  updated_at: string;
  metadata: Record<string, string>;
}

interface SkillInvocation {
  invocation_id: string;
  skill_id: string;
  bot: string;
  session_id?: string;
  trace_id?: string;
  input: any;
  output: any;
  success: boolean;
  duration_ms: number;
  tokens_used: number;
  error?: string;
  timestamp: string;
}

interface SkillEvolution {
  evolution_id: string;
  skill_id: string;
  from_version: number;
  to_version: number;
  reason: string;
  changes: string;
  evolved_by: string;
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
// Constants
// ============================================================================

/** Generic badge style for any source/agent name */
const SOURCE_STYLE = 'text-blue-400 bg-blue-400/10 border-blue-400/30';

const STATUS_COLORS: Record<SkillStatus, string> = {
  draft: 'text-yellow-400 bg-yellow-400/10',
  active: 'text-green-400 bg-green-400/10',
  deprecated: 'text-gray-400 bg-gray-400/10',
  archived: 'text-red-400 bg-red-400/10',
};

// ============================================================================
// Helper API
// ============================================================================

async function fetchJSON<T>(path: string): Promise<T> {
  const res = await fetch(`${API_BASE_URL}${path}`);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
}

async function postJSON<T>(path: string, body: any): Promise<T> {
  const res = await fetch(`${API_BASE_URL}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
}

async function putJSON<T>(path: string, body: any): Promise<T> {
  const res = await fetch(`${API_BASE_URL}${path}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
}

// ============================================================================
// SKILL.md Generator
// ============================================================================

function generateSkillMd(skill: Skill): string {
  const yaml = [
    '---',
    `name: ${skill.name}`,
    `description: ${skill.description}`,
    `version: "${skill.metadata?.version_str || skill.version}"`,
    `category: ${skill.category}`,
    `status: ${skill.status}`,
    `source: ${skill.origin_bot}`,
  ];

  // AgentSkills standard fields
  if (skill.metadata?.license) {
    yaml.push(`license: ${skill.metadata.license}`);
  }
  if (skill.metadata?.compatibility) {
    yaml.push(`compatibility: "${skill.metadata.compatibility}"`);
  }
  if (skill.metadata?.allowed_tools) {
    yaml.push(`allowed-tools: "${skill.metadata.allowed_tools}"`);
  }

  // Requirements block
  const hasRequires =
    skill.metadata?.requires_env || skill.metadata?.requires_bins || skill.metadata?.requires_mcp;
  if (hasRequires) {
    yaml.push('requires:');
    if (skill.metadata?.requires_env) {
      yaml.push(
        `  env: [${skill.metadata.requires_env
          .split(',')
          .map((e) => e.trim())
          .filter(Boolean)
          .join(', ')}]`,
      );
    }
    if (skill.metadata?.requires_bins) {
      yaml.push(
        `  bins: [${skill.metadata.requires_bins
          .split(',')
          .map((b) => b.trim())
          .filter(Boolean)
          .join(', ')}]`,
      );
    }
    if (skill.metadata?.requires_mcp) {
      yaml.push(
        `  mcp: [${skill.metadata.requires_mcp
          .split(',')
          .map((m) => m.trim())
          .filter(Boolean)
          .join(', ')}]`,
      );
    }
  }

  // Gating block
  if (skill.metadata?.gating_file_pattern || skill.metadata?.gating_context) {
    yaml.push('gating:');
    yaml.push('  - ' + [
      skill.metadata?.gating_file_pattern && `file_pattern: "${skill.metadata.gating_file_pattern}"`,
      skill.metadata?.gating_context && `context: ${skill.metadata.gating_context}`,
    ].filter(Boolean).join('\n    '));
  }

  // Summary for progressive disclosure
  if (skill.metadata?.summary) {
    yaml.push(`summary: >`);
    yaml.push(`  ${skill.metadata.summary}`);
  }

  if (skill.tags.length > 0) {
    yaml.push(`tags: [${skill.tags.map((t) => `"${t}"`).join(', ')}]`);
  }
  if (skill.input_schema) {
    yaml.push(`input_schema: ${skill.input_schema}`);
  }
  if (skill.output_schema) {
    yaml.push(`output_schema: ${skill.output_schema}`);
  }
  if (skill.shared_with.length > 0) {
    yaml.push(`shared_with: [${skill.shared_with.map((s) => `"${s}"`).join(', ')}]`);
  }
  yaml.push(`created_at: ${skill.created_at}`);
  yaml.push(`updated_at: ${skill.updated_at}`);
  if (Object.keys(skill.metadata).length > 0) {
    yaml.push('metadata:');
    for (const [k, v] of Object.entries(skill.metadata)) {
      // Skip fields already emitted as top-level
      if (
        [
          'version_str',
          'license',
          'allowed_tools',
          'requires_env',
          'requires_bins',
          'requires_mcp',
          'gating_file_pattern',
          'gating_context',
          'summary',
          'compatibility',
        ].includes(k)
      )
        continue;
      yaml.push(`  ${k}: ${v}`);
    }
  }
  yaml.push('---');
  yaml.push('');
  yaml.push(`# ${skill.name}`);
  yaml.push('');
  yaml.push(skill.definition);
  return yaml.join('\n');
}

function downloadSkillMd(skill: Skill) {
  const content = generateSkillMd(skill);
  const blob = new Blob([content], { type: 'text/markdown' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${skill.name.toLowerCase().replace(/\s+/g, '-')}.SKILL.md`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// ============================================================================
// Skill Memory Page
// ============================================================================

export default function SkillMemoryPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const [tab, setTab] = useState<'overview' | 'skills' | 'activity'>('overview');
  const [stats, setStats] = useState<SkillMemoryStats | null>(null);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [selectedSkill, setSelectedSkill] = useState<Skill | null>(null);
  const [invocations, setInvocations] = useState<SkillInvocation[]>([]);
  const [evolutions, setEvolutions] = useState<SkillEvolution[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [filterBot, setFilterBot] = useState<string>('');
  const [filterStatus, setFilterStatus] = useState<SkillStatus | ''>('');
  const [showEditor, setShowEditor] = useState(false);
  const [editingSkill, setEditingSkill] = useState<Skill | null>(null);

  const loadData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const params = new URLSearchParams();
      if (searchQuery) params.set('search', searchQuery);
      if (filterBot) params.set('bot', filterBot);
      if (filterStatus) params.set('status', filterStatus);

      const [statsRes, skillsRes] = await Promise.all([
        fetchJSON<SkillMemoryStats>('/api/v1/skill-memory/stats'),
        fetchJSON<{ skills: Skill[]; total: number }>(
          `/api/v1/skill-memory/skills?${params.toString()}`
        ),
      ]);
      setStats(statsRes);
      setSkills(skillsRes.skills);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }, [searchQuery, filterBot, filterStatus]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const selectSkill = async (skill: Skill) => {
    setSelectedSkill(skill);
    try {
      const [invRes, evoRes] = await Promise.all([
        fetchJSON<{ invocations: SkillInvocation[] }>(
          `/api/v1/skill-memory/skills/${skill.skill_id}/invocations?limit=20`
        ),
        fetchJSON<{ evolutions: SkillEvolution[] }>(
          `/api/v1/skill-memory/skills/${skill.skill_id}/evolutions`
        ),
      ]);
      setInvocations(invRes.invocations);
      setEvolutions(evoRes.evolutions);
    } catch {
      // silently ignore detail load errors
    }
  };

  const deleteSkill = async (skillId: string) => {
    if (!confirm('Delete this skill permanently?')) return;
    try {
      const res = await fetch(`${API_BASE_URL}/api/v1/skill-memory/skills/${skillId}`, { method: 'DELETE' });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      setSelectedSkill(null);
      loadData();
    } catch (e: any) {
      setError(e.message);
    }
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-4 border-b border-border">
        <div className="flex items-center gap-3">
          <Brain className="w-5 h-5 text-purple-500 dark:text-purple-400" />
          <h1 className="text-lg font-semibold text-foreground">AI Memory OS — Skill Memory</h1>
          <span className="text-xs text-muted-foreground bg-secondary px-2 py-0.5 rounded-full">
            SochDB-powered
          </span>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={loadData}
            className="p-2 rounded-lg hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
          >
            <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
          </button>
          <button
            onClick={() => { setEditingSkill(null); setShowEditor(true); }}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-purple-500/15 text-purple-600 dark:text-purple-400 rounded-lg hover:bg-purple-500/25 text-sm transition-colors"
          >
            <Plus className="w-3.5 h-3.5" />
            New Skill
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 px-6 pt-3 border-b border-border">
        {(['overview', 'skills', 'activity'] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`px-4 py-2 text-sm rounded-t-lg transition-colors ${
              tab === t
                ? 'text-foreground bg-secondary border-b-2 border-purple-500 dark:border-purple-400'
                : 'text-muted-foreground hover:text-foreground hover:bg-secondary/60'
            }`}
          >
            {t === 'overview' ? 'Overview' : t === 'skills' ? 'Skills' : 'Activity'}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        {error && (
          <div className="mb-4 p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-red-600 dark:text-red-400 text-sm">
            {error}
          </div>
        )}

        {tab === 'overview' && <OverviewTab stats={stats} loading={loading} />}
        {tab === 'skills' && (
          <SkillsTab
            skills={skills}
            selectedSkill={selectedSkill}
            invocations={invocations}
            evolutions={evolutions}
            searchQuery={searchQuery}
            filterBot={filterBot}
            filterStatus={filterStatus}
            onSearch={setSearchQuery}
            onFilterBot={setFilterBot}
            onFilterStatus={setFilterStatus}
            onSelect={selectSkill}
            onDelete={deleteSkill}
            onEdit={(skill) => { setEditingSkill(skill); setShowEditor(true); }}
            onRefresh={loadData}
            loading={loading}
            projectId={projectId}
          />
        )}
        {tab === 'activity' && (
          <ActivityTab stats={stats} loading={loading} />
        )}
      </div>

      {/* Skill Editor Panel */}
      {showEditor && (
        <SkillEditorPanel
          skill={editingSkill}
          onClose={() => { setShowEditor(false); setEditingSkill(null); }}
          onSaved={() => {
            setShowEditor(false);
            setEditingSkill(null);
            loadData();
          }}
        />
      )}
    </div>
  );
}

// ============================================================================
// Overview Tab
// ============================================================================

function OverviewTab({ stats, loading }: { stats: SkillMemoryStats | null; loading: boolean }) {
  if (loading || !stats) {
    return (
      <div className="flex items-center justify-center h-64 text-muted-foreground">
        <RefreshCw className="w-5 h-5 animate-spin mr-2" />
        Loading skill memory stats...
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Stats Grid */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <StatCard icon={<Brain className="w-5 h-5" />} label="Total Skills" value={stats.total_skills} color="purple" />
        <StatCard icon={<Zap className="w-5 h-5" />} label="Invocations" value={stats.total_invocations} color="blue" />
        <StatCard icon={<GitBranch className="w-5 h-5" />} label="Evolutions" value={stats.total_evolutions} color="green" />
        <StatCard
          icon={<TrendingUp className="w-5 h-5" />}
          label="Success Rate"
          value={`${(stats.overall_success_rate * 100).toFixed(1)}%`}
          color="emerald"
        />
      </div>

      {/* Skills by Source */}
      {Object.keys(stats.skills_by_bot).length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {Object.entries(stats.skills_by_bot).map(([source, count]) => (
            <div
              key={source}
              className={`p-4 rounded-xl border ${SOURCE_STYLE} bg-opacity-5`}
            >
              <div className="flex items-center gap-2 mb-3">
                <Bot className="w-5 h-5" />
                <span className="font-medium">{source}</span>
              </div>
              <div className="text-2xl font-bold mb-1">{count}</div>
              <div className="text-xs opacity-60">skills learned</div>
            </div>
          ))}
        </div>
      )}

      {/* By Category */}
      {Object.keys(stats.skills_by_category).length > 0 && (
        <div className="bg-card rounded-xl border border-border p-5">
          <h3 className="text-sm font-medium text-muted-foreground mb-3 flex items-center gap-2">
            <Layers className="w-4 h-4" /> Skills by Category
          </h3>
          <div className="flex flex-wrap gap-2">
            {Object.entries(stats.skills_by_category).map(([cat, count]) => (
              <span
                key={cat}
                className="px-3 py-1 rounded-full bg-secondary text-foreground text-sm border border-border"
              >
                {cat} <span className="text-muted-foreground ml-1">{count}</span>
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Recent Activity */}
      {stats.recent_invocations.length > 0 && (
        <div className="bg-card rounded-xl border border-border p-5">
          <h3 className="text-sm font-medium text-muted-foreground mb-3 flex items-center gap-2">
            <Activity className="w-4 h-4" /> Recent Invocations
          </h3>
          <div className="space-y-2">
            {stats.recent_invocations.slice(0, 5).map((inv) => (
              <div
                key={inv.invocation_id}
                className="flex items-center justify-between px-3 py-2 rounded-lg bg-secondary/50 border border-border"
              >
                <div className="flex items-center gap-3">
                  {inv.success ? (
                    <CheckCircle className="w-4 h-4 text-green-500 dark:text-green-400" />
                  ) : (
                    <XCircle className="w-4 h-4 text-red-500 dark:text-red-400" />
                  )}
                  <span className={`text-xs px-2 py-0.5 rounded border ${SOURCE_STYLE}`}>
                    {inv.bot}
                  </span>
                  <span className="text-sm text-muted-foreground">
                    {inv.skill_id.slice(0, 8)}...
                  </span>
                </div>
                <div className="flex items-center gap-4 text-xs text-muted-foreground/70">
                  <span>{inv.duration_ms}ms</span>
                  <span>{inv.tokens_used} tok</span>
                  <span>{new Date(inv.timestamp).toLocaleTimeString()}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Architecture Info */}
      <div className="bg-card rounded-xl border border-border p-5">
        <h3 className="text-sm font-medium text-muted-foreground mb-3 flex items-center gap-2">
          <Cpu className="w-4 h-4" /> Architecture
        </h3>
        <div className="text-xs text-muted-foreground/70 space-y-2 font-mono">
          <p>SochDB MemoryStore → Episode/Event/Entity schema</p>
          <p>HNSW Index (384-dim) → Semantic skill discovery</p>
          <p>HierarchicalMemory → L0 Raw → L1 Summary → L2 Abstraction</p>
          <p>Cross-agent skill sharing via shared memory store</p>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Skills Tab
// ============================================================================

function SkillsTab({
  skills,
  selectedSkill,
  invocations,
  evolutions,
  searchQuery,
  filterBot,
  filterStatus,
  onSearch,
  onFilterBot,
  onFilterStatus,
  onSelect,
  onDelete,
  onEdit,
  onRefresh,
  loading,
  projectId,
}: {
  skills: Skill[];
  selectedSkill: Skill | null;
  invocations: SkillInvocation[];
  evolutions: SkillEvolution[];
  searchQuery: string;
  filterBot: string;
  filterStatus: SkillStatus | '';
  onSearch: (q: string) => void;
  onFilterBot: (b: string) => void;
  onFilterStatus: (s: SkillStatus | '') => void;
  onSelect: (s: Skill) => void;
  onDelete: (id: string) => void;
  onEdit: (s: Skill) => void;
  onRefresh: () => void;
  loading: boolean;
  projectId?: string;
}) {
  return (
    <div className="flex gap-4 h-full">
      {/* Left: Skill list */}
      <div className="w-1/2 flex flex-col gap-3">
        {/* Filters */}
        <div className="flex gap-2">
          <div className="flex-1 relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground/50" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => onSearch(e.target.value)}
              placeholder="Search skills..."
              className="w-full pl-9 pr-3 py-2 bg-secondary border border-border rounded-lg text-sm text-foreground placeholder-muted-foreground focus:outline-none focus:border-purple-400/50"
            />
          </div>
          <input
            value={filterBot}
            onChange={(e) => onFilterBot(e.target.value)}
            placeholder="Filter by source…"
            className="bg-secondary border border-border rounded-lg px-3 py-1.5 text-sm text-muted-foreground focus:outline-none w-36"
          />
          <select
            value={filterStatus}
            onChange={(e) => onFilterStatus(e.target.value as SkillStatus | '')}
            className="bg-secondary border border-border rounded-lg px-3 py-2 text-sm text-muted-foreground focus:outline-none"
          >
            <option value="">All Status</option>
            <option value="draft">Draft</option>
            <option value="active">Active</option>
            <option value="deprecated">Deprecated</option>
            <option value="archived">Archived</option>
          </select>
        </div>

        {/* List */}
        <div className="flex-1 overflow-auto space-y-2">
          {loading ? (
            <div className="flex items-center justify-center h-32 text-muted-foreground">
              <RefreshCw className="w-4 h-4 animate-spin mr-2" /> Loading...
            </div>
          ) : skills.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-32 text-muted-foreground">
              <Brain className="w-8 h-8 mb-2" />
              <p>No skills found</p>
              <p className="text-xs mt-1">Create your first skill to get started</p>
            </div>
          ) : (
            skills.map((skill) => (
              <button
                key={skill.skill_id}
                onClick={() => onSelect(skill)}
                className={`w-full text-left p-4 rounded-xl border transition-colors ${
                  selectedSkill?.skill_id === skill.skill_id
                    ? 'bg-purple-500/10 border-purple-500/30'
                    : 'bg-card border-border hover:bg-secondary/80'
                }`}
              >
                <div className="flex items-center justify-between mb-2">
                  <span className="font-medium text-foreground text-sm">{skill.name}</span>
                  <span className={`text-xs px-2 py-0.5 rounded-full ${STATUS_COLORS[skill.status]}`}>
                    {skill.status}
                  </span>
                </div>
                <p className="text-xs text-muted-foreground mb-2 line-clamp-2">{skill.description}</p>
                <div className="flex items-center gap-3 text-xs text-muted-foreground/70">
                  <span className={`px-1.5 py-0.5 rounded border ${SOURCE_STYLE}`}>
                    {skill.origin_bot}
                  </span>
                  <span className="flex items-center gap-1">
                    <Tag className="w-3 h-3" /> {skill.category}
                  </span>
                  <span className="flex items-center gap-1">
                    <Zap className="w-3 h-3" /> {skill.invocation_count}
                  </span>
                  <span>v{skill.version}</span>
                  {skill.shared_with.length > 0 && (
                    <span className="flex items-center gap-1">
                      <Share2 className="w-3 h-3" /> {skill.shared_with.length}
                    </span>
                  )}
                </div>
              </button>
            ))
          )}
        </div>
      </div>

      {/* Right: Skill Detail */}
      <div className="w-1/2 overflow-auto">
        {selectedSkill ? (
          <SkillDetail
            skill={selectedSkill}
            invocations={invocations}
            evolutions={evolutions}
            onDelete={onDelete}
            onEdit={onEdit}
            projectId={projectId}
          />
        ) : (
          <div className="flex items-center justify-center h-full text-muted-foreground/50">
            <div className="text-center">
              <ChevronRight className="w-8 h-8 mx-auto mb-2" />
              <p className="text-sm">Select a skill to view details</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Skill Detail
// ============================================================================

function SkillDetail({
  skill,
  invocations,
  evolutions,
  onDelete,
  onEdit,
  projectId,
}: {
  skill: Skill;
  invocations: SkillInvocation[];
  evolutions: SkillEvolution[];
  onDelete: (id: string) => void;
  onEdit: (s: Skill) => void;
  projectId?: string;
}) {
  const [detailTab, setDetailTab] = useState<'info' | 'invocations' | 'evolution'>('info');

  const skillTesterUrl = projectId ? `/projects/${projectId}/skill-tester` : '/skill-tester';

  return (
    <div className="bg-card rounded-xl border border-border p-5">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div>
          <h3 className="text-lg font-semibold text-foreground">{skill.name}</h3>
          <p className="text-xs text-muted-foreground mt-1">{skill.skill_id}</p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => onEdit(skill)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-blue-500/10 text-blue-400 hover:bg-blue-500/20 transition-colors text-xs font-medium"
            title="Edit this skill"
          >
            <Edit className="w-3.5 h-3.5" />
            Edit
          </button>
          <button
            onClick={() => downloadSkillMd(skill)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-green-500/10 text-green-400 hover:bg-green-500/20 transition-colors text-xs font-medium"
            title="Download as SKILL.md"
          >
            <Download className="w-3.5 h-3.5" />
            SKILL.md
          </button>
          <Link
            to={skillTesterUrl}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-purple-500/10 text-purple-400 hover:bg-purple-500/20 transition-colors text-xs font-medium"
            title="Test this skill in Skill Tester"
          >
            <Shield className="w-3.5 h-3.5" />
            Test
            <ExternalLink className="w-3 h-3" />
          </Link>
          <button
            onClick={() => onDelete(skill.skill_id)}
            className="p-1.5 rounded-lg hover:bg-red-500/10 text-muted-foreground hover:text-red-500 transition-colors"
            title="Delete skill"
          >
            <Trash2 className="w-4 h-4" />
          </button>
          <span className={`text-xs px-3 py-1 rounded-full ${STATUS_COLORS[skill.status]}`}>
            {skill.status}
          </span>
        </div>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-4 gap-3 mb-4">
        <MiniStat label="Invocations" value={skill.invocation_count} />
        <MiniStat label="Success" value={`${(skill.success_rate * 100).toFixed(0)}%`} />
        <MiniStat label="Avg Duration" value={`${skill.avg_duration_ms.toFixed(0)}ms`} />
        <MiniStat label="Avg Tokens" value={skill.avg_tokens.toFixed(0)} />
      </div>

      {/* Detail Tabs */}
      <div className="flex gap-1 mb-4 border-b border-border">
        {(['info', 'invocations', 'evolution'] as const).map((t) => (
          <button
            key={t}
            onClick={() => setDetailTab(t)}
            className={`px-3 py-1.5 text-xs rounded-t-lg transition-colors ${
              detailTab === t
                ? 'text-foreground bg-secondary border-b-2 border-purple-500 dark:border-purple-400'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            {t === 'info' ? 'Info' : t === 'invocations' ? 'Invocations' : 'Evolution'}
          </button>
        ))}
      </div>

      {detailTab === 'info' && (
        <div className="space-y-4">
          <div>
            <label className="text-xs text-muted-foreground block mb-1">Description</label>
            <p className="text-sm text-foreground/80">{skill.description}</p>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs text-muted-foreground block mb-1">Source</label>
              <span className={`text-sm px-2 py-1 rounded border ${SOURCE_STYLE}`}>
                {skill.origin_bot}
              </span>
            </div>
            <div>
              <label className="text-xs text-muted-foreground block mb-1">Category</label>
              <span className="text-sm text-foreground/80">{skill.category}</span>
            </div>
            <div>
              <label className="text-xs text-muted-foreground block mb-1">Version</label>
              <span className="text-sm text-foreground/80">v{skill.version}</span>
            </div>
            <div>
              <label className="text-xs text-muted-foreground block mb-1">Created</label>
              <span className="text-sm text-foreground/80">
                {new Date(skill.created_at).toLocaleDateString()}
              </span>
            </div>
          </div>
          {skill.tags.length > 0 && (
            <div>
              <label className="text-xs text-muted-foreground block mb-1">Tags</label>
              <div className="flex flex-wrap gap-1">
                {skill.tags.map((t) => (
                  <span key={t} className="px-2 py-0.5 bg-secondary text-muted-foreground rounded text-xs">
                    {t}
                  </span>
                ))}
              </div>
            </div>
          )}
          {skill.shared_with.length > 0 && (
            <div>
              <label className="text-xs text-muted-foreground block mb-1">Shared With</label>
              <div className="flex gap-2">
                {skill.shared_with.map((b) => (
                  <span key={b} className={`text-xs px-2 py-1 rounded border ${SOURCE_STYLE}`}>
                    {b}
                  </span>
                ))}
              </div>
            </div>
          )}
          <div>
            <div className="flex items-center justify-between mb-1">
              <label className="text-xs text-muted-foreground">Definition</label>
              <div className="flex items-center gap-1">
                <button
                  onClick={() => { navigator.clipboard.writeText(skill.definition); }}
                  className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] text-muted-foreground hover:text-foreground hover:bg-secondary transition-colors"
                  title="Copy definition"
                >
                  <Copy className="w-3 h-3" />
                  Copy
                </button>
                <button
                  onClick={() => downloadSkillMd(skill)}
                  className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] text-muted-foreground hover:text-foreground hover:bg-secondary transition-colors"
                  title="Download as SKILL.md"
                >
                  <Download className="w-3 h-3" />
                  .md
                </button>
              </div>
            </div>
            <pre className="text-xs text-muted-foreground bg-secondary rounded-lg p-3 overflow-auto max-h-40 whitespace-pre-wrap">
              {skill.definition}
            </pre>
          </div>
        </div>
      )}

      {detailTab === 'invocations' && (
        <div className="space-y-2">
          {invocations.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-4">No invocations yet</p>
          ) : (
            invocations.map((inv) => (
              <div
                key={inv.invocation_id}
                className="p-3 bg-secondary/50 rounded-lg border border-border"
              >
                <div className="flex items-center justify-between mb-1">
                  <div className="flex items-center gap-2">
                    {inv.success ? (
                      <CheckCircle className="w-3.5 h-3.5 text-green-400" />
                    ) : (
                      <XCircle className="w-3.5 h-3.5 text-red-400" />
                    )}
                    <span className={`text-xs px-1.5 py-0.5 rounded border ${SOURCE_STYLE}`}>
                      {inv.bot}
                    </span>
                  </div>
                  <span className="text-xs text-muted-foreground/60">
                    {new Date(inv.timestamp).toLocaleString()}
                  </span>
                </div>
                <div className="flex gap-4 text-xs text-muted-foreground mt-1">
                  <span>{inv.duration_ms}ms</span>
                  <span>{inv.tokens_used} tokens</span>
                  {inv.error && <span className="text-red-400">{inv.error}</span>}
                </div>
              </div>
            ))
          )}
        </div>
      )}

      {detailTab === 'evolution' && (
        <div className="space-y-2">
          {evolutions.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-4">No evolutions yet</p>
          ) : (
            evolutions.map((evo) => (
              <div
                key={evo.evolution_id}
                className="p-3 bg-secondary/50 rounded-lg border border-border"
              >
                <div className="flex items-center justify-between mb-1">
                  <div className="flex items-center gap-2">
                    <ArrowUpCircle className="w-3.5 h-3.5 text-blue-500 dark:text-blue-400" />
                    <span className="text-sm text-foreground/80">
                      v{evo.from_version} → v{evo.to_version}
                    </span>
                  </div>
                  <span className={`text-xs px-1.5 py-0.5 rounded border ${SOURCE_STYLE}`}>
                    {evo.evolved_by}
                  </span>
                </div>
                <p className="text-xs text-muted-foreground mt-1">{evo.reason}</p>
                <p className="text-xs text-muted-foreground/60 mt-1">{evo.changes}</p>
                <span className="text-xs text-muted-foreground/50 mt-1 block">
                  {new Date(evo.timestamp).toLocaleString()}
                </span>
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Activity Tab
// ============================================================================

function ActivityTab({ stats, loading }: { stats: SkillMemoryStats | null; loading: boolean }) {
  if (loading || !stats) {
    return (
      <div className="flex items-center justify-center h-64 text-muted-foreground">
        <RefreshCw className="w-5 h-5 animate-spin mr-2" /> Loading...
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-medium text-muted-foreground flex items-center gap-2">
        <Activity className="w-4 h-4" /> Recent Skill Invocations
      </h3>
      {stats.recent_invocations.length === 0 ? (
        <div className="text-center py-8 text-muted-foreground">
          <Zap className="w-8 h-8 mx-auto mb-2" />
          <p>No activity yet</p>
        </div>
      ) : (
        <div className="space-y-2">
          {stats.recent_invocations.map((inv) => (
            <div
              key={inv.invocation_id}
              className="p-4 bg-card rounded-xl border border-border flex items-center justify-between"
            >
              <div className="flex items-center gap-3">
                {inv.success ? (
                  <CheckCircle className="w-4 h-4 text-green-500 dark:text-green-400" />
                ) : (
                  <XCircle className="w-4 h-4 text-red-500 dark:text-red-400" />
                )}
                <div>
                  <span className={`text-xs px-2 py-0.5 rounded border ${SOURCE_STYLE}`}>
                    {inv.bot}
                  </span>
                  <span className="text-sm text-muted-foreground ml-2">
                    Skill {inv.skill_id.slice(0, 8)}...
                  </span>
                </div>
              </div>
              <div className="flex items-center gap-6 text-xs text-muted-foreground/70">
                <span className="flex items-center gap-1">
                  <Clock className="w-3 h-3" /> {inv.duration_ms}ms
                </span>
                <span>{inv.tokens_used} tok</span>
                <span>{new Date(inv.timestamp).toLocaleString()}</span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Skill Editor Panel (Create + Edit)
// ============================================================================

// AgentSkills name format: 1-64 chars, lowercase alphanumeric + hyphens, no --, no leading/trailing -
const AGENT_SKILLS_NAME_REGEX = /^[a-z0-9]([a-z0-9-]*[a-z0-9])?$/;
const SEMVER_REGEX = /^\d+\.\d+\.\d+(-[a-zA-Z0-9.]+)?$/;
const SUSPICIOUS_PATTERNS = [
  'ignore previous instructions',
  'ignore all previous',
  'system override',
  'maintenance mode',
  'cat /etc/passwd',
  'cat ~/.ssh',
  'eval(',
  'exec(',
  'rm -rf',
];

function validateSkillEditor(fields: {
  name: string;
  description: string;
  versionStr: string;
  definition: string;
  summary: string;
  category: string;
}): EditorValidation[] {
  const findings: EditorValidation[] = [];

  // Name format per Agent Skills spec
  if (!fields.name) {
    findings.push({ check: 'Name', severity: 'error', message: 'Required' });
  } else if (fields.name.length > 64) {
    findings.push({ check: 'Name', severity: 'error', message: `${fields.name.length}/64 chars — too long` });
  } else if (fields.name.includes('--')) {
    findings.push({ check: 'Name', severity: 'error', message: 'No consecutive hyphens (--) allowed' });
  } else if (!AGENT_SKILLS_NAME_REGEX.test(fields.name)) {
    findings.push({ check: 'Name', severity: 'error', message: 'Must be lowercase a-z, 0-9, hyphens only' });
  } else {
    findings.push({ check: 'Name', severity: 'pass', message: `OK (${fields.name.length}/64)` });
  }

  // Description length per Agent Skills spec (1-1024 chars)
  if (!fields.description) {
    findings.push({ check: 'Description', severity: 'error', message: 'Required (1-1024 chars)' });
  } else if (fields.description.length > 1024) {
    findings.push({ check: 'Description', severity: 'error', message: `${fields.description.length}/1024 — exceeds limit` });
  } else {
    findings.push({ check: 'Description', severity: 'pass', message: `${fields.description.length}/1024 chars` });
  }

  // Category
  if (!fields.category) {
    findings.push({ check: 'Category', severity: 'error', message: 'Required' });
  } else {
    findings.push({ check: 'Category', severity: 'pass', message: 'OK' });
  }

  // Semantic version
  if (!fields.versionStr) {
    findings.push({ check: 'Version', severity: 'warning', message: 'Recommended (semver e.g. 1.0.0)' });
  } else if (!SEMVER_REGEX.test(fields.versionStr)) {
    findings.push({ check: 'Version', severity: 'warning', message: 'Should be semver (e.g. 1.0.0)' });
  } else {
    findings.push({ check: 'Version', severity: 'pass', message: 'Valid semver' });
  }

  // Definition body
  if (!fields.definition.trim()) {
    findings.push({ check: 'Definition', severity: 'error', message: 'Required — instructions body' });
  } else {
    const lines = fields.definition.split('\n').length;
    findings.push({ check: 'Definition', severity: 'pass', message: `${lines} lines` });
  }

  // Progressive disclosure
  if (!fields.summary) {
    findings.push({ check: 'Summary', severity: 'warning', message: 'Missing — full instructions loaded on match' });
  } else {
    findings.push({ check: 'Summary', severity: 'pass', message: 'Progressive disclosure ready' });
  }

  // Security: suspicious patterns
  const defLower = fields.definition.toLowerCase();
  const found = SUSPICIOUS_PATTERNS.filter((p) => defLower.includes(p));
  if (found.length > 0) {
    findings.push({ check: 'Security', severity: 'error', message: `Suspicious: ${found.join(', ')}` });
  } else if (fields.definition.trim()) {
    findings.push({ check: 'Security', severity: 'pass', message: 'No suspicious patterns' });
  }

  return findings;
}

/** Reference data for the standards guide */
const STANDARDS_GUIDE = [
  { field: 'name', rule: '1-64 chars, lowercase a-z 0-9 hyphens, no -- or leading/trailing -', required: true },
  { field: 'description', rule: '1-1024 chars, concise human-readable summary', required: true },
  { field: 'version', rule: 'Semantic versioning (e.g. 1.0.0)', required: false },
  { field: 'license', rule: 'SPDX identifier (e.g. Apache-2.0, MIT)', required: false },
  { field: 'allowed-tools', rule: 'Space-delimited whitelist: Bash(git:*) Read Write', required: false },
  { field: 'requires.env', rule: 'Environment variables needed (e.g. API_KEY)', required: false },
  { field: 'requires.bins', rule: 'Binary executables needed (e.g. git, docker)', required: false },
  { field: 'requires.mcp', rule: 'MCP server dependencies (e.g. @anthropic/mcp-fs)', required: false },
  { field: 'gating', rule: 'When to trigger: file_pattern glob + context type', required: false },
  { field: 'summary', rule: 'Progressive disclosure: ~100 token summary loaded first', required: false },
  { field: 'resources', rule: 'Referenced files, each < 50KB', required: false },
];

const SKILL_TEMPLATE = `---
name: my-skill
description: A brief description of what this skill does
version: 1.0.0
license: Apache-2.0
compatibility: "Requires git and access to the internet"
allowed-tools: "Bash(git:*) Read Write"
requires:
  env: []
  bins: [git]
  mcp: []
  config: []
gating:
  - file_pattern: "*.py"
    context: pull_request
resources: []
summary: >
  One-paragraph summary for progressive disclosure (~100 tokens).
  This is shown to the agent first before full instructions are loaded.
metadata:
  token_budget:
    metadata_scan: 100
    full_load: 5000
---

# Skill Name

## Purpose
Describe what this skill does and when it should be invoked.

## Instructions
1. Step one...
2. Step two...
3. Step three...

## Input Format
Describe expected input format or provide a JSON schema example.

## Output Format
Describe expected output format or provide a JSON schema example.

## Examples

### Example 1
**Input:** ...
**Output:** ...

## Gating Rules
This skill activates when:
- File matches: \`*.py\`
- Context: pull_request

## Notes
- Any constraints, edge cases, or security considerations
- Token budget: ~5000 tokens for full instructions
`;

function SkillEditorPanel({
  skill,
  onClose,
  onSaved,
}: {
  skill: Skill | null; // null = create mode
  onClose: () => void;
  onSaved: () => void;
}) {
  const isEdit = skill !== null;
  const [name, setName] = useState(skill?.name || '');
  const [description, setDescription] = useState(skill?.description || '');
  const [category, setCategory] = useState(skill?.category || '');
  const [originBot, setOriginBot] = useState(skill?.origin_bot || '');
  const [definition, setDefinition] = useState(skill?.definition || '');
  const [tags, setTags] = useState(skill?.tags.join(', ') || '');
  const [status, setStatus] = useState<SkillStatus>(skill?.status || 'draft');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [previewMode, setPreviewMode] = useState(false);
  const [copied, setCopied] = useState(false);

  // AgentSkills standard fields (stored in metadata)
  const [versionStr, setVersionStr] = useState(skill?.metadata?.version_str || '1.0.0');
  const [license, setLicense] = useState(skill?.metadata?.license || '');
  const [allowedTools, setAllowedTools] = useState(skill?.metadata?.allowed_tools || '');
  const [requiresEnv, setRequiresEnv] = useState(skill?.metadata?.requires_env || '');
  const [requiresBins, setRequiresBins] = useState(skill?.metadata?.requires_bins || '');
  const [requiresMcp, setRequiresMcp] = useState(skill?.metadata?.requires_mcp || '');
  const [gatingFilePattern, setGatingFilePattern] = useState(skill?.metadata?.gating_file_pattern || '');
  const [gatingContext, setGatingContext] = useState(skill?.metadata?.gating_context || '');
  const [summary, setSummary] = useState(skill?.metadata?.summary || '');
  const [compatibility, setCompatibility] = useState(skill?.metadata?.compatibility || '');

  // UI state
  const [showStandardsGuide, setShowStandardsGuide] = useState(false);
  const [showAgentSkillsFields, setShowAgentSkillsFields] = useState(
    !!(license || allowedTools || requiresEnv || requiresBins || requiresMcp || gatingFilePattern || summary)
  );

  // Real-time validation
  const validations = validateSkillEditor({ name, description, versionStr, definition, summary, category });
  const errorCount = validations.filter((v) => v.severity === 'error').length;
  const warnCount = validations.filter((v) => v.severity === 'warning').length;
  const passCount = validations.filter((v) => v.severity === 'pass').length;

  // Line count for the editor gutter
  const lineCount = definition.split('\n').length;

  const handleSave = async () => {
    if (!name || !description || !category || !definition) {
      setError('Name, description, category, and definition are required');
      return;
    }
    // Validate name format
    if (!AGENT_SKILLS_NAME_REGEX.test(name) || name.includes('--') || name.length > 64) {
      setError('Name must be 1-64 lowercase chars (a-z, 0-9, hyphens), no consecutive hyphens');
      return;
    }
    if (description.length > 1024) {
      setError('Description must be 1-1024 characters');
      return;
    }
    setSaving(true);
    setError('');

    // Compile AgentSkills metadata
    const agentSkillsMeta: Record<string, string> = {
      ...(skill?.metadata || {}),
    };
    if (versionStr) agentSkillsMeta.version_str = versionStr;
    if (license) agentSkillsMeta.license = license;
    if (allowedTools) agentSkillsMeta.allowed_tools = allowedTools;
    if (requiresEnv) agentSkillsMeta.requires_env = requiresEnv;
    if (requiresBins) agentSkillsMeta.requires_bins = requiresBins;
    if (requiresMcp) agentSkillsMeta.requires_mcp = requiresMcp;
    if (gatingFilePattern) agentSkillsMeta.gating_file_pattern = gatingFilePattern;
    if (gatingContext) agentSkillsMeta.gating_context = gatingContext;
    if (summary) agentSkillsMeta.summary = summary;
    if (compatibility) agentSkillsMeta.compatibility = compatibility;

    try {
      if (isEdit && skill) {
        await putJSON(`/api/v1/skill-memory/skills/${skill.skill_id}`, {
          name,
          description,
          category,
          tags: tags.split(',').map((t) => t.trim()).filter(Boolean),
          definition,
          status,
          metadata: agentSkillsMeta,
        });
      } else {
        await postJSON('/api/v1/skill-memory/skills', {
          name,
          description,
          origin_bot: originBot,
          category,
          tags: tags.split(',').map((t) => t.trim()).filter(Boolean),
          definition,
          metadata: agentSkillsMeta,
        });
      }
      onSaved();
    } catch (e: any) {
      setError(e.message);
    } finally {
      setSaving(false);
    }
  };

  const handleInsertTemplate = () => {
    if (definition.trim() && !confirm('Replace current definition with AgentSkills standard template?')) return;
    setDefinition(
      SKILL_TEMPLATE.replace('my-skill', name || 'my-skill').replace('Skill Name', name || 'Skill Name'),
    );
    // Auto-populate standard fields from template
    if (!versionStr) setVersionStr('1.0.0');
    if (!license) setLicense('Apache-2.0');
    setShowAgentSkillsFields(true);
  };

  const handleCopyDefinition = () => {
    navigator.clipboard.writeText(definition);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleDownloadDraft = () => {
    const agentSkillsMeta: Record<string, string> = {
      ...(skill?.metadata || {}),
    };
    if (versionStr) agentSkillsMeta.version_str = versionStr;
    if (license) agentSkillsMeta.license = license;
    if (allowedTools) agentSkillsMeta.allowed_tools = allowedTools;
    if (requiresEnv) agentSkillsMeta.requires_env = requiresEnv;
    if (requiresBins) agentSkillsMeta.requires_bins = requiresBins;
    if (requiresMcp) agentSkillsMeta.requires_mcp = requiresMcp;
    if (gatingFilePattern) agentSkillsMeta.gating_file_pattern = gatingFilePattern;
    if (gatingContext) agentSkillsMeta.gating_context = gatingContext;
    if (summary) agentSkillsMeta.summary = summary;
    if (compatibility) agentSkillsMeta.compatibility = compatibility;

    const draftSkill: Skill = {
      skill_id: skill?.skill_id || 'draft',
      name: name || 'untitled',
      description,
      origin_bot: originBot,
      category,
      tags: tags.split(',').map((t) => t.trim()).filter(Boolean),
      definition,
      version: skill?.version || 1,
      invocation_count: skill?.invocation_count || 0,
      success_rate: skill?.success_rate || 0,
      avg_duration_ms: skill?.avg_duration_ms || 0,
      avg_tokens: skill?.avg_tokens || 0,
      shared_with: skill?.shared_with || [],
      status,
      episode_ids: skill?.episode_ids || [],
      created_at: skill?.created_at || new Date().toISOString(),
      updated_at: new Date().toISOString(),
      metadata: agentSkillsMeta,
    };
    downloadSkillMd(draftSkill);
  };

  return (
    <div className="fixed inset-0 z-50 flex bg-black/60 backdrop-blur-sm">
      <div className="flex flex-col w-full max-w-5xl mx-auto my-4 bg-card border border-border rounded-2xl shadow-2xl overflow-hidden">
        {/* Editor Header */}
        <div className="flex items-center justify-between px-6 py-3 border-b border-border bg-secondary/30">
          <div className="flex items-center gap-3">
            <Code className="w-5 h-5 text-purple-400" />
            <h2 className="text-base font-semibold text-foreground">
              {isEdit ? `Edit: ${skill.name}` : 'Create New Skill'}
            </h2>
            {isEdit && (
              <span className="text-xs bg-blue-500/10 text-blue-400 px-2 py-0.5 rounded">
                v{skill.version}
              </span>
            )}
            {/* Standards compliance badge */}
            <span
              className={`text-xs px-2 py-0.5 rounded flex items-center gap-1 ${
                errorCount > 0
                  ? 'bg-red-500/10 text-red-400'
                  : warnCount > 0
                    ? 'bg-yellow-500/10 text-yellow-400'
                    : 'bg-green-500/10 text-green-400'
              }`}
            >
              {errorCount > 0 ? (
                <AlertTriangle className="w-3 h-3" />
              ) : warnCount > 0 ? (
                <AlertTriangle className="w-3 h-3" />
              ) : (
                <CheckCircle className="w-3 h-3" />
              )}
              {errorCount > 0
                ? `${errorCount} error${errorCount > 1 ? 's' : ''}`
                : warnCount > 0
                  ? `${warnCount} warning${warnCount > 1 ? 's' : ''}`
                  : 'Standards OK'}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={handleDownloadDraft}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-green-500/10 text-green-400 hover:bg-green-500/20 transition-colors text-xs font-medium"
              title="Download as SKILL.md"
            >
              <Download className="w-3.5 h-3.5" />
              Export .md
            </button>
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Two-column layout: metadata left, editor right */}
        <div className="flex flex-1 overflow-hidden">
          {/* Left: Metadata Fields */}
          <div className="w-80 border-r border-border p-5 overflow-y-auto space-y-4 bg-secondary/10">
            <div>
              <label className="text-xs font-medium text-muted-foreground block mb-1.5">
                Name *
                <span className="text-[9px] text-muted-foreground/50 ml-1">lowercase, a-z 0-9 hyphens</span>
              </label>
              <input
                value={name}
                onChange={(e) => setName(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, '-'))}
                placeholder="e.g. code-review-agent"
                className={`w-full px-3 py-2 bg-background border rounded-lg text-sm text-foreground placeholder-muted-foreground focus:outline-none focus:border-purple-400/50 font-mono ${
                  name && (!AGENT_SKILLS_NAME_REGEX.test(name) || name.includes('--') || name.length > 64)
                    ? 'border-red-500/50'
                    : 'border-border'
                }`}
              />
              {name && (!AGENT_SKILLS_NAME_REGEX.test(name) || name.includes('--')) && (
                <p className="text-[10px] text-red-400 mt-0.5">
                  Must be lowercase, a-z 0-9 hyphens only, no consecutive hyphens
                </p>
              )}
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground block mb-1.5">
                Description *
                <span
                  className={`ml-1 text-[9px] ${
                    description.length > 1024 ? 'text-red-400' : 'text-muted-foreground/50'
                  }`}
                >
                  {description.length}/1024
                </span>
              </label>
              <textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="What does this skill do?"
                rows={3}
                className={`w-full px-3 py-2 bg-background border rounded-lg text-sm text-foreground placeholder-muted-foreground focus:outline-none focus:border-purple-400/50 resize-none ${
                  description.length > 1024 ? 'border-red-500/50' : 'border-border'
                }`}
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="text-xs font-medium text-muted-foreground block mb-1.5">Source / Agent</label>
                <input
                  value={originBot}
                  onChange={(e) => setOriginBot(e.target.value)}
                  placeholder="Agent name"
                  disabled={isEdit}
                  className="w-full px-3 py-2 bg-background border border-border rounded-lg text-sm text-foreground placeholder-muted-foreground focus:outline-none focus:border-purple-400/50 disabled:opacity-50"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground block mb-1.5">Category *</label>
                <input
                  value={category}
                  onChange={(e) => setCategory(e.target.value)}
                  placeholder="e.g. coding"
                  className="w-full px-3 py-2 bg-background border border-border rounded-lg text-sm text-foreground placeholder-muted-foreground focus:outline-none focus:border-purple-400/50"
                />
              </div>
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground block mb-1.5">Tags</label>
              <input
                value={tags}
                onChange={(e) => setTags(e.target.value)}
                placeholder="code, review, quality (comma-separated)"
                className="w-full px-3 py-2 bg-background border border-border rounded-lg text-sm text-foreground placeholder-muted-foreground focus:outline-none focus:border-purple-400/50"
              />
            </div>
            {isEdit && (
              <div>
                <label className="text-xs font-medium text-muted-foreground block mb-1.5">Status</label>
                <select
                  value={status}
                  onChange={(e) => setStatus(e.target.value as SkillStatus)}
                  className="w-full px-3 py-2 bg-background border border-border rounded-lg text-sm text-foreground focus:outline-none focus:border-purple-400/50"
                >
                  <option value="draft">Draft</option>
                  <option value="active">Active</option>
                  <option value="deprecated">Deprecated</option>
                  <option value="archived">Archived</option>
                </select>
              </div>
            )}

            {/* ── AgentSkills Standard Fields ── */}
            <div className="border-t border-border pt-3">
              <button
                onClick={() => setShowAgentSkillsFields(!showAgentSkillsFields)}
                className="flex items-center justify-between w-full text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
              >
                <span className="flex items-center gap-1.5">
                  <Shield className="w-3.5 h-3.5 text-purple-400" />
                  AgentSkills Standard
                </span>
                {showAgentSkillsFields ? (
                  <ChevronUp className="w-3.5 h-3.5" />
                ) : (
                  <ChevronDown className="w-3.5 h-3.5" />
                )}
              </button>

              {showAgentSkillsFields && (
                <div className="mt-3 space-y-3">
                  {/* Version + License row */}
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="text-xs font-medium text-muted-foreground block mb-1 flex items-center gap-1">
                        Version
                        <span className="text-[9px] text-muted-foreground/50" title="Semantic versioning (e.g. 1.0.0)">
                          semver
                        </span>
                      </label>
                      <input
                        value={versionStr}
                        onChange={(e) => setVersionStr(e.target.value)}
                        placeholder="1.0.0"
                        className={`w-full px-3 py-1.5 bg-background border rounded-lg text-sm text-foreground placeholder-muted-foreground focus:outline-none focus:border-purple-400/50 ${
                          versionStr && !SEMVER_REGEX.test(versionStr)
                            ? 'border-yellow-500/50'
                            : 'border-border'
                        }`}
                      />
                    </div>
                    <div>
                      <label className="text-xs font-medium text-muted-foreground block mb-1 flex items-center gap-1">
                        <Lock className="w-3 h-3" />
                        License
                      </label>
                      <input
                        value={license}
                        onChange={(e) => setLicense(e.target.value)}
                        placeholder="Apache-2.0"
                        className="w-full px-3 py-1.5 bg-background border border-border rounded-lg text-sm text-foreground placeholder-muted-foreground focus:outline-none focus:border-purple-400/50"
                      />
                    </div>
                  </div>

                  {/* Allowed Tools */}
                  <div>
                    <label className="text-xs font-medium text-muted-foreground block mb-1">
                      <span className="flex items-center gap-1">
                        <Shield className="w-3 h-3" />
                        Allowed Tools
                        <span className="text-[9px] text-muted-foreground/50">space-delimited</span>
                      </span>
                    </label>
                    <input
                      value={allowedTools}
                      onChange={(e) => setAllowedTools(e.target.value)}
                      placeholder='Bash(git:*) Bash(jq:*) Read Write'
                      className="w-full px-3 py-1.5 bg-background border border-border rounded-lg text-xs text-foreground placeholder-muted-foreground/40 focus:outline-none focus:border-purple-400/50 font-mono"
                    />
                  </div>

                  {/* Summary (progressive disclosure) */}
                  <div>
                    <label className="text-xs font-medium text-muted-foreground block mb-1 flex items-center gap-1">
                      <FileText className="w-3 h-3" />
                      Summary
                      <span className="text-[9px] text-muted-foreground/50">~100 tokens, loaded first</span>
                    </label>
                    <textarea
                      value={summary}
                      onChange={(e) => setSummary(e.target.value)}
                      placeholder="One-paragraph summary for progressive disclosure. Agents read this first to decide whether to load full instructions."
                      rows={2}
                      className="w-full px-3 py-1.5 bg-background border border-border rounded-lg text-xs text-foreground placeholder-muted-foreground/40 focus:outline-none focus:border-purple-400/50 resize-none"
                    />
                  </div>

                  {/* Compatibility */}
                  <div>
                    <label className="text-xs font-medium text-muted-foreground block mb-1 flex items-center gap-1">
                      <Globe className="w-3 h-3" />
                      Compatibility
                    </label>
                    <input
                      value={compatibility}
                      onChange={(e) => setCompatibility(e.target.value)}
                      placeholder="e.g. Requires git and internet access"
                      className="w-full px-3 py-1.5 bg-background border border-border rounded-lg text-xs text-foreground placeholder-muted-foreground/40 focus:outline-none focus:border-purple-400/50"
                    />
                  </div>

                  {/* Requirements section */}
                  <div className="space-y-2">
                    <p className="text-[10px] font-medium text-muted-foreground/70 uppercase tracking-wider">
                      Requirements
                    </p>
                    <div>
                      <label className="text-[10px] text-muted-foreground flex items-center gap-1 mb-0.5">
                        <TerminalIcon className="w-3 h-3" /> Environment Vars
                      </label>
                      <input
                        value={requiresEnv}
                        onChange={(e) => setRequiresEnv(e.target.value)}
                        placeholder="API_KEY, SECRET (comma-separated)"
                        className="w-full px-2 py-1 bg-background border border-border rounded text-[11px] text-foreground placeholder-muted-foreground/40 focus:outline-none focus:border-purple-400/50 font-mono"
                      />
                    </div>
                    <div>
                      <label className="text-[10px] text-muted-foreground flex items-center gap-1 mb-0.5">
                        <TerminalIcon className="w-3 h-3" /> Binaries
                      </label>
                      <input
                        value={requiresBins}
                        onChange={(e) => setRequiresBins(e.target.value)}
                        placeholder="git, docker, jq (comma-separated)"
                        className="w-full px-2 py-1 bg-background border border-border rounded text-[11px] text-foreground placeholder-muted-foreground/40 focus:outline-none focus:border-purple-400/50 font-mono"
                      />
                    </div>
                    <div>
                      <label className="text-[10px] text-muted-foreground flex items-center gap-1 mb-0.5">
                        <Server className="w-3 h-3" /> MCP Servers
                      </label>
                      <input
                        value={requiresMcp}
                        onChange={(e) => setRequiresMcp(e.target.value)}
                        placeholder="@anthropic/mcp-fs (comma-separated)"
                        className="w-full px-2 py-1 bg-background border border-border rounded text-[11px] text-foreground placeholder-muted-foreground/40 focus:outline-none focus:border-purple-400/50 font-mono"
                      />
                    </div>
                  </div>

                  {/* Gating predicates */}
                  <div className="space-y-2">
                    <p className="text-[10px] font-medium text-muted-foreground/70 uppercase tracking-wider">
                      Gating (When to Trigger)
                    </p>
                    <div>
                      <label className="text-[10px] text-muted-foreground flex items-center gap-1 mb-0.5">
                        <FileCode className="w-3 h-3" /> File Pattern
                      </label>
                      <input
                        value={gatingFilePattern}
                        onChange={(e) => setGatingFilePattern(e.target.value)}
                        placeholder="*.py, src/**/*.ts (glob)"
                        className="w-full px-2 py-1 bg-background border border-border rounded text-[11px] text-foreground placeholder-muted-foreground/40 focus:outline-none focus:border-purple-400/50 font-mono"
                      />
                    </div>
                    <div>
                      <label className="text-[10px] text-muted-foreground flex items-center gap-1 mb-0.5">
                        <Activity className="w-3 h-3" /> Context
                      </label>
                      <select
                        value={gatingContext}
                        onChange={(e) => setGatingContext(e.target.value)}
                        className="w-full px-2 py-1 bg-background border border-border rounded text-[11px] text-foreground focus:outline-none focus:border-purple-400/50"
                      >
                        <option value="">— any —</option>
                        <option value="pull_request">pull_request</option>
                        <option value="issue">issue</option>
                        <option value="chat">chat</option>
                        <option value="commit">commit</option>
                        <option value="review">review</option>
                        <option value="build">build</option>
                        <option value="deploy">deploy</option>
                      </select>
                    </div>
                  </div>
                </div>
              )}
            </div>

            {/* ── Standards Compliance ── */}
            <div className="border-t border-border pt-3">
              <p className="text-xs font-medium text-muted-foreground mb-2 flex items-center gap-1">
                <CheckCircle className="w-3.5 h-3.5 text-green-400" />
                Compliance ({passCount}/{validations.length})
              </p>
              <div className="space-y-1">
                {validations.map((v, i) => (
                  <div key={i} className="flex items-center gap-1.5 text-[11px]">
                    {v.severity === 'pass' ? (
                      <CheckCircle className="w-3 h-3 text-green-400 flex-shrink-0" />
                    ) : v.severity === 'warning' ? (
                      <AlertTriangle className="w-3 h-3 text-yellow-400 flex-shrink-0" />
                    ) : (
                      <XCircle className="w-3 h-3 text-red-400 flex-shrink-0" />
                    )}
                    <span className="text-muted-foreground">{v.check}:</span>
                    <span
                      className={
                        v.severity === 'pass'
                          ? 'text-green-400/80'
                          : v.severity === 'warning'
                            ? 'text-yellow-400/80'
                            : 'text-red-400/80'
                      }
                    >
                      {v.message}
                    </span>
                  </div>
                ))}
              </div>
            </div>

            {/* ── Standards Reference Guide ── */}
            <div className="border-t border-border pt-3">
              <button
                onClick={() => setShowStandardsGuide(!showStandardsGuide)}
                className="flex items-center justify-between w-full text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
              >
                <span className="flex items-center gap-1.5">
                  <BookOpen className="w-3.5 h-3.5 text-blue-400" />
                  Standards Reference
                </span>
                {showStandardsGuide ? (
                  <ChevronUp className="w-3.5 h-3.5" />
                ) : (
                  <ChevronDown className="w-3.5 h-3.5" />
                )}
              </button>
              {showStandardsGuide && (
                <div className="mt-2 space-y-1.5">
                  <p className="text-[10px] text-muted-foreground/60 mb-2">
                    AgentSkills SKILL.md format · MCP Tools spec · Claude CLAUDE.md conventions
                  </p>
                  {STANDARDS_GUIDE.map((item) => (
                    <div
                      key={item.field}
                      className="flex items-start gap-1.5 text-[10px] bg-background/50 rounded p-1.5"
                    >
                      <span
                        className={`flex-shrink-0 mt-0.5 w-1.5 h-1.5 rounded-full ${
                          item.required ? 'bg-red-400' : 'bg-blue-400/50'
                        }`}
                      />
                      <div>
                        <span className="font-mono text-foreground/80">{item.field}</span>
                        {item.required && <span className="text-red-400 ml-0.5">*</span>}
                        <p className="text-muted-foreground/60 mt-0.5">{item.rule}</p>
                      </div>
                    </div>
                  ))}
                  <div className="text-[9px] text-muted-foreground/40 pt-1 border-t border-border/30">
                    Sources: AgentSkills spec (parser.rs) · MCP Tools (modelcontextprotocol.io) · Claude Code
                    CLAUDE.md · Semantic Kernel Plugins
                  </div>
                </div>
              )}
            </div>

            {/* Quick stats when editing */}
            {isEdit && skill && (
              <div className="pt-3 border-t border-border space-y-2">
                <p className="text-xs font-medium text-muted-foreground">Stats</p>
                <div className="grid grid-cols-2 gap-2 text-xs">
                  <div className="bg-background rounded-lg p-2 text-center">
                    <div className="text-foreground font-medium">{skill.invocation_count}</div>
                    <div className="text-muted-foreground">Invocations</div>
                  </div>
                  <div className="bg-background rounded-lg p-2 text-center">
                    <div className="text-foreground font-medium">{(skill.success_rate * 100).toFixed(0)}%</div>
                    <div className="text-muted-foreground">Success</div>
                  </div>
                </div>
              </div>
            )}

            {error && (
              <div className="p-2 bg-red-500/10 border border-red-500/20 rounded-lg text-red-400 text-xs">
                {error}
              </div>
            )}
          </div>

          {/* Right: Definition Editor */}
          <div className="flex-1 flex flex-col overflow-hidden">
            {/* Editor toolbar */}
            <div className="flex items-center justify-between px-4 py-2 border-b border-border bg-secondary/20">
              <div className="flex items-center gap-2">
                <span className="text-xs font-medium text-muted-foreground">Definition *</span>
                <span className="text-[10px] text-muted-foreground/60">
                  {lineCount} lines · {definition.length} chars
                </span>
              </div>
              <div className="flex items-center gap-1.5">
                <button
                  onClick={handleInsertTemplate}
                  className="flex items-center gap-1 px-2 py-1 rounded text-[11px] text-muted-foreground hover:text-foreground hover:bg-secondary transition-colors"
                  title="Insert SKILL.md template"
                >
                  <Wand2 className="w-3 h-3" />
                  Template
                </button>
                <button
                  onClick={handleCopyDefinition}
                  className="flex items-center gap-1 px-2 py-1 rounded text-[11px] text-muted-foreground hover:text-foreground hover:bg-secondary transition-colors"
                  title="Copy to clipboard"
                >
                  {copied ? <Check className="w-3 h-3 text-green-400" /> : <Copy className="w-3 h-3" />}
                  {copied ? 'Copied' : 'Copy'}
                </button>
                <button
                  onClick={() => setPreviewMode(!previewMode)}
                  className={`flex items-center gap-1 px-2 py-1 rounded text-[11px] transition-colors ${
                    previewMode ? 'bg-blue-500/20 text-blue-400' : 'text-muted-foreground hover:text-foreground hover:bg-secondary'
                  }`}
                  title="Toggle preview"
                >
                  {previewMode ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />}
                  Preview
                </button>
              </div>
            </div>

            {/* Editor content */}
            {previewMode ? (
              <div className="flex-1 overflow-auto p-6">
                <div className="prose prose-sm prose-invert max-w-none">
                  <pre className="whitespace-pre-wrap text-sm text-foreground/90 font-mono leading-relaxed">
                    {definition || '(empty definition)'}
                  </pre>
                </div>
              </div>
            ) : (
              <div className="flex-1 overflow-auto flex">
                {/* Line numbers gutter */}
                <div className="flex-shrink-0 py-3 pr-2 pl-3 text-right select-none border-r border-border/50 bg-secondary/10">
                  {Array.from({ length: Math.max(lineCount, 20) }, (_, i) => (
                    <div key={i} className="text-[11px] leading-[1.625rem] text-muted-foreground/30 font-mono">
                      {i + 1}
                    </div>
                  ))}
                </div>
                {/* Text area */}
                <textarea
                  value={definition}
                  onChange={(e) => setDefinition(e.target.value)}
                  placeholder="Write your skill definition, procedure, or prompt template here...

You can use the Template button above to start with a structured SKILL.md format."
                  className="flex-1 p-3 bg-transparent text-sm text-foreground font-mono leading-[1.625rem] resize-none focus:outline-none placeholder-muted-foreground/40"
                  spellCheck={false}
                />
              </div>
            )}
          </div>
        </div>

        {/* Footer actions */}
        <div className="flex items-center justify-between px-6 py-3 border-t border-border bg-secondary/30">
          <div className="text-xs text-muted-foreground">
            {isEdit ? `Editing ${skill.name} · Last updated ${new Date(skill.updated_at).toLocaleDateString()}` : 'Creating new skill'}
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={onClose}
              className="px-4 py-2 text-sm text-muted-foreground hover:text-foreground transition-colors rounded-lg hover:bg-secondary"
            >
              Cancel
            </button>
            <button
              onClick={handleSave}
              disabled={saving}
              className="flex items-center gap-1.5 px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-500 text-sm font-medium transition-colors disabled:opacity-50"
            >
              {saving ? (
                <RefreshCw className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <Save className="w-3.5 h-3.5" />
              )}
              {isEdit ? 'Save Changes' : 'Create Skill'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Shared components
// ============================================================================

function StatCard({
  icon,
  label,
  value,
  color,
}: {
  icon: React.ReactNode;
  label: string;
  value: number | string;
  color: string;
}) {
  const colorClasses: Record<string, string> = {
    purple: 'text-purple-400 bg-purple-400/10 border-purple-400/20',
    blue: 'text-blue-400 bg-blue-400/10 border-blue-400/20',
    green: 'text-green-400 bg-green-400/10 border-green-400/20',
    emerald: 'text-emerald-400 bg-emerald-400/10 border-emerald-400/20',
  };

  return (
    <div className={`p-4 rounded-xl border ${colorClasses[color] || colorClasses.purple}`}>
      <div className="flex items-center gap-2 mb-2 opacity-70">{icon}<span className="text-xs">{label}</span></div>
      <div className="text-2xl font-bold">{value}</div>
    </div>
  );
}

function MiniStat({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="text-center p-2 bg-secondary/50 rounded-lg border border-border">
      <div className="text-xs text-muted-foreground mb-0.5">{label}</div>
      <div className="text-sm font-medium text-foreground/80">{value}</div>
    </div>
  );
}
