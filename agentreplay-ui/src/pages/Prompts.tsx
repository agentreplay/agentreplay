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

import { FormEvent, useEffect, useMemo, useState, useCallback } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
import { PromptRecord, loadPrompts, savePrompts } from '../lib/prompt-store';
import { VideoHelpButton } from '../components/VideoHelpButton';
import { agentreplayClient, GitLogEntry, GitBranch as GitBranchType } from '../lib/agentreplay-api';
import { EvaluateTraceModal } from '../components/EvaluateTraceButton';
import {
  Beaker,
  Cloud,
  Filter,
  History,
  Play,
  Plus,
  Rocket,
  Search,
  Settings2,
  Tag,
  GitBranch,
  Clock,
  User,
  Eye,
  X,
  Copy,
  ExternalLink,
  Zap,
  DollarSign,
  MessageSquare,
  FileText,
  BookOpen,
  Sparkles,
  ArrowRight,
} from 'lucide-react';
import { cn } from '../../lib/utils';
import { formatDistanceToNow } from 'date-fns';
import { motion, AnimatePresence } from 'framer-motion';

interface PromptDraft {
  name: string;
  tags: string;
  description?: string;
  content: string;
}

const defaultDraft: PromptDraft = {
  name: '',
  tags: '',
  content: '',
  description: '',
};

// Tab type for main view
type TabType = 'registry' | 'versions';

// Extended prompt with metadata for committed traces
interface CommittedPrompt extends PromptRecord {
  metadata?: {
    trace_id?: string;
    span_id?: string;
    model?: string;
    output?: string;
    latency_ms?: number;
    cost?: number;
    branch?: string;
  };
}

export default function Prompts() {
  const { projectId } = useParams<{ projectId: string }>();
  const navigate = useNavigate();
  const [prompts, setPrompts] = useState<CommittedPrompt[]>([]);
  const [search, setSearch] = useState('');
  const [selectedTag, setSelectedTag] = useState<string | null>(null);
  const [draft, setDraft] = useState<PromptDraft>(defaultDraft);
  const [showComposer, setShowComposer] = useState(false);

  // Evaluation modal state
  const [showEvalModal, setShowEvalModal] = useState(false);
  const [evalPrompt, setEvalPrompt] = useState<CommittedPrompt | null>(null);

  // Version history state
  const [activeTab, setActiveTab] = useState<TabType>('registry');
  const [selectedBranch, setSelectedBranch] = useState<string>('');
  const [selectedPrompt, setSelectedPrompt] = useState<CommittedPrompt | null>(null);
  // Fetch prompts from backend API
  useEffect(() => {
    async function fetchPrompts() {
      try {
        // Fetch from backend
        const response = await agentreplayClient.listPrompts();
        // Handle wrapper response or direct array
        const backendPrompts = Array.isArray(response) ? response : (response.prompts || []);

        // Map backend format to frontend PromptRecord
        const mappedPrompts: CommittedPrompt[] = backendPrompts.map((p: any) => ({
          id: p.id.toString(),
          name: p.name,
          tags: p.tags || [],
          description: p.description,
          lastEdited: p.updated_at * 1000,
          deployedVersion: null,
          activeVersion: p.version,
          owner: p.created_by,
          content: p.template,
          variables: (p.variables || []).map((v: string) => ({ key: v, required: true })),
          history: [],
          metadata: p.metadata || {}
        }));

        setPrompts(mappedPrompts);
      } catch (error) {
        console.error('Failed to load prompts from API:', error);
        // Fallback to local for demo/offline
        setPrompts(loadPrompts() as CommittedPrompt[]);
      }
    }

    fetchPrompts();
  }, [activeTab]);

  // Get committed prompts (ones with trace metadata)
  const committedPrompts = useMemo(() => {
    return prompts.filter(p => (p as any).metadata?.trace_id);
  }, [prompts]);

  // Get unique branches/models from committed prompts
  const branches = useMemo(() => {
    const branchSet = new Set<string>();
    committedPrompts.forEach(p => {
      const branch = (p as any).metadata?.branch || (p as any).metadata?.model;
      if (branch) branchSet.add(branch);
    });
    return Array.from(branchSet);
  }, [committedPrompts]);

  // Filter committed prompts by branch
  const filteredVersions = useMemo(() => {
    if (!selectedBranch) return committedPrompts;
    return committedPrompts.filter(p => {
      const branch = (p as any).metadata?.branch || (p as any).metadata?.model;
      return branch === selectedBranch;
    });
  }, [committedPrompts, selectedBranch]);

  const tags = useMemo(() => {
    const tagSet = new Set<string>();
    prompts.forEach((prompt) => prompt.tags.forEach((tag) => tagSet.add(tag)));
    return Array.from(tagSet);
  }, [prompts]);

  const filteredPrompts = prompts.filter((prompt) => {
    const matchesSearch =
      !search ||
      prompt.name.toLowerCase().includes(search.toLowerCase()) ||
      prompt.description?.toLowerCase().includes(search.toLowerCase());
    const matchesTag = !selectedTag || prompt.tags.includes(selectedTag);
    return matchesSearch && matchesTag;
  });



  const handleCreate = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!draft.name.trim()) return;

    const newPromptId = Date.now(); // Simple integer ID generation for now

    const newPromptBackend = {
      id: newPromptId,
      name: draft.name.trim(),
      description: draft.description?.trim() || '',
      template: draft.content || 'Write your template here…',
      variables: [], // TODO: Parse variables from content using regex like {{var}}
      tags: draft.tags.split(',').map((tag) => tag.trim()).filter(Boolean),
      version: 1,
      created_at: Math.floor(Date.now() / 1000), // Backend expects seconds
      updated_at: Math.floor(Date.now() / 1000),
      created_by: 'you@agentreplay.ai',
    };

    // Optimistic update
    const newPromptFrontend: PromptRecord = {
      id: newPromptId.toString(),
      name: newPromptBackend.name,
      tags: newPromptBackend.tags,
      description: newPromptBackend.description,
      lastEdited: newPromptBackend.updated_at * 1000,
      deployedVersion: null,
      activeVersion: newPromptBackend.version,
      owner: newPromptBackend.created_by,
      content: newPromptBackend.template,
      variables: [],
      history: [],
    };

    setPrompts([newPromptFrontend as CommittedPrompt, ...prompts]);
    setDraft(defaultDraft);
    setShowComposer(false);

    // Save to backend using typed method
    agentreplayClient.createPrompt({
      name: newPromptBackend.name,
      description: newPromptBackend.description,
      template: newPromptBackend.template,
      tags: newPromptBackend.tags,
      // Backend generates ID, version, timestamps
    })
      .then((created) => {
        // Replace optimistic entry with real one or navigate
        if (projectId) {
          // Use real ID from backend
          navigate(`/projects/${projectId}/prompts/${created.id}`);
        }
      })
      .catch(err => {
        console.error("Failed to save prompt to backend:", err);
        // Fallback to local storage
        const nextPrompts = [newPromptFrontend, ...loadPrompts()];
        savePrompts(nextPrompts);
      });
  };



  const handleTestInPlayground = (prompt: PromptRecord) => {
    // Always parse the CURRENT content (not metadata.messages which is the original trace)
    // This ensures edits are reflected in the playground
    const content = prompt.content || '';
    let messages: Array<{ role: string; content: string }> = [];

    // Try parsing Jinja-style format: {# SYSTEM PROMPT #} and {# USER MESSAGE #}
    const jinjaSystemMatch = content.match(/\{#\s*SYSTEM PROMPT\s*#\}\n([\s\S]*?)(?=\{#|$)/);
    const jinjaUserMatch = content.match(/\{#\s*USER MESSAGE\s*#\}\n([\s\S]*?)$/);

    if (jinjaSystemMatch || jinjaUserMatch) {
      if (jinjaSystemMatch) {
        messages.push({ role: 'system', content: jinjaSystemMatch[1].trim() });
      }
      if (jinjaUserMatch) {
        messages.push({ role: 'user', content: jinjaUserMatch[1].trim() });
      }
    } else {
      // Try parsing [System]/[User] format
      const systemMatch = content.match(/\[System\]\n([\s\S]*?)(?=\n\n\[User\]|\n\[User\]|$)/);
      const userMatch = content.match(/\[User\]\n([\s\S]*?)$/);

      if (systemMatch) {
        messages.push({ role: 'system', content: systemMatch[1].trim() });
      }
      if (userMatch) {
        messages.push({ role: 'user', content: userMatch[1].trim() });
      }
    }

    // Fallback if parsing failed - treat whole content as system prompt
    if (messages.length === 0) {
      messages = [
        { role: 'system', content: content },
        { role: 'user', content: '' }
      ];
    }

    // Store prompt data in sessionStorage for playground
    const playgroundData = {
      prompt: content,
      messages: messages,
      sourcePromptId: prompt.id,
    };
    sessionStorage.setItem('playground_data', JSON.stringify(playgroundData));
    navigate(`/projects/${projectId}/playground?from=prompt`);
  };

  return (
    <div className="flex flex-col h-full" style={{ paddingTop: '8px' }}>
      {/* Header */}
      <header className="flex items-center justify-between mb-5">
        <div className="flex items-center gap-3">
          <div
            className="w-9 h-9 rounded-lg flex items-center justify-center flex-shrink-0"
            style={{ backgroundColor: 'rgba(0,128,255,0.1)' }}
          >
            <FileText className="w-4 h-4" style={{ color: '#0080FF' }} />
          </div>
          <div>
            <h1 className="text-lg font-bold tracking-tight text-foreground">Prompt Registry</h1>
            <p className="text-xs" style={{ color: 'hsl(var(--muted-foreground))' }}>Version every template and test against your traces</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <VideoHelpButton pageId="prompts" />
          <button
            onClick={() => setShowComposer((prev) => !prev)}
            className="flex items-center gap-1.5 px-3.5 py-2 rounded-xl text-[13px] font-semibold transition-all"
            style={{ backgroundColor: '#0080FF', color: '#ffffff', boxShadow: '0 2px 8px rgba(0,128,255,0.25)' }}
          >
            <Plus className="w-3.5 h-3.5" /> New prompt
          </button>
          <button
            className="flex items-center gap-1.5 px-3 py-2 rounded-xl text-[13px] font-medium transition-all opacity-50 cursor-not-allowed"
            style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--muted-foreground))' }}
            disabled
          >
            <Cloud className="w-3.5 h-3.5" /> Cloud Sync
            <span className="text-[10px] px-1.5 py-0.5 rounded-md font-semibold" style={{ backgroundColor: 'rgba(0,128,255,0.1)', color: '#0080FF' }}>Soon</span>
          </button>
          <button
            onClick={() => projectId && navigate(`/projects/${projectId}/docs`)}
            className="flex items-center gap-1.5 px-3 py-2 rounded-xl text-[13px] font-medium transition-all"
            style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--foreground))' }}
          >
            <BookOpen className="w-3.5 h-3.5" /> Docs
          </button>
        </div>
      </header>

      {/* Stats Bar */}
      <div className="flex items-center gap-4 mb-5">
        <div className="flex items-center gap-2 px-3.5 py-2 rounded-xl" style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))' }}>
          <span className="text-[13px] font-bold text-foreground">{prompts.length}</span>
          <span className="text-[12px]" style={{ color: 'hsl(var(--muted-foreground))' }}>prompts</span>
        </div>
        <div className="flex items-center gap-2 px-3.5 py-2 rounded-xl" style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))' }}>
          <span className="text-[13px] font-bold" style={{ color: '#0080FF' }}>{committedPrompts.length}</span>
          <span className="text-[12px]" style={{ color: 'hsl(var(--muted-foreground))' }}>versions committed</span>
        </div>
      </div>

      {/* Tab Navigation */}
      <div className="flex items-center gap-1 mb-5 p-1 rounded-xl" style={{ backgroundColor: 'hsl(var(--secondary))', width: 'fit-content' }}>
        <button
          onClick={() => setActiveTab('registry')}
          className="flex items-center gap-1.5 px-4 py-2 rounded-lg text-[13px] font-semibold transition-all"
          style={activeTab === 'registry'
            ? { backgroundColor: '#0080FF', color: '#ffffff', boxShadow: '0 2px 8px rgba(0,128,255,0.25)' }
            : { backgroundColor: 'transparent', color: 'hsl(var(--muted-foreground))' }
          }
        >
          <Rocket className="w-3.5 h-3.5" />
          Registry
        </button>
        <button
          onClick={() => setActiveTab('versions')}
          className="flex items-center gap-1.5 px-4 py-2 rounded-lg text-[13px] font-semibold transition-all"
          style={activeTab === 'versions'
            ? { backgroundColor: '#0080FF', color: '#ffffff', boxShadow: '0 2px 8px rgba(0,128,255,0.25)' }
            : { backgroundColor: 'transparent', color: 'hsl(var(--muted-foreground))' }
          }
        >
          <GitBranch className="w-3.5 h-3.5" />
          Version History
          {committedPrompts.length > 0 && (
            <span className="text-[10px] px-1.5 py-0.5 rounded-md font-semibold" style={{ backgroundColor: 'rgba(255,255,255,0.25)', color: '#ffffff' }}>
              {committedPrompts.length}
            </span>
          )}
        </button>
      </div>

      {activeTab === 'registry' && (
        <>
          {/* Search & Filters */}
          <div className="flex items-center gap-3 mb-4">
            <div className="flex-1 relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4" style={{ color: 'hsl(var(--muted-foreground))' }} />
              <input
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="Search prompts…"
                className="w-full pl-9 pr-3 py-2.5 rounded-xl text-[13px] focus:outline-none transition-all bg-card border border-border text-foreground"
              />
            </div>
            <div className="flex items-center gap-1.5">
              <button
                onClick={() => setSelectedTag(null)}
                className="px-3 py-2 rounded-lg text-[12px] font-semibold transition-all"
                style={selectedTag === null
                  ? { backgroundColor: '#0080FF', color: '#ffffff' }
                  : { backgroundColor: 'hsl(var(--secondary))', color: 'hsl(var(--muted-foreground))', border: '1px solid hsl(var(--border))' }
                }
              >
                All tags
              </button>
              {tags.map((tag) => (
                <button
                  key={tag}
                  onClick={() => setSelectedTag(tag)}
                  className="flex items-center gap-1 px-3 py-2 rounded-lg text-[12px] font-medium transition-all"
                  style={selectedTag === tag
                    ? { backgroundColor: '#0080FF', color: '#ffffff' }
                    : { backgroundColor: 'hsl(var(--secondary))', color: 'hsl(var(--muted-foreground))', border: '1px solid hsl(var(--border))' }
                  }
                >
                  <Tag className="w-3 h-3" /> {tag}
                </button>
              ))}
            </div>
          </div>

          {/* New Prompt Composer */}
          {showComposer && (
            <div className="mb-4 rounded-2xl p-5" style={{ backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', boxShadow: '0 1px 3px rgba(0,0,0,0.04)' }}>
              <form onSubmit={handleCreate}>
                <div className="grid gap-3 md:grid-cols-3 mb-3">
                  <input
                    required
                    placeholder="Prompt name"
                    value={draft.name}
                    onChange={(event) => setDraft((prev) => ({ ...prev, name: event.target.value }))}
                    className="px-3 py-2.5 rounded-xl text-[13px] focus:outline-none"
                    style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--foreground))' }}
                  />
                  <input
                    placeholder="Tags (comma separated)"
                    value={draft.tags}
                    onChange={(event) => setDraft((prev) => ({ ...prev, tags: event.target.value }))}
                    className="px-3 py-2.5 rounded-xl text-[13px] focus:outline-none"
                    style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--foreground))' }}
                  />
                  <input
                    placeholder="Short description"
                    value={draft.description}
                    onChange={(event) => setDraft((prev) => ({ ...prev, description: event.target.value }))}
                    className="px-3 py-2.5 rounded-xl text-[13px] focus:outline-none"
                    style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--foreground))' }}
                  />
                </div>
                <textarea
                  className="w-full h-32 rounded-xl px-3 py-2.5 text-[13px] focus:outline-none resize-none"
                  placeholder="Template body — use {{variable}} for dynamic content"
                  value={draft.content}
                  onChange={(event) => setDraft((prev) => ({ ...prev, content: event.target.value }))}
                  style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--foreground))', fontFamily: 'monospace' }}
                />
                <div className="mt-3 flex justify-end gap-2">
                  <button
                    type="button"
                    onClick={() => setShowComposer(false)}
                    className="px-3.5 py-2 rounded-xl text-[13px] font-medium transition-all text-muted-foreground"
                  >
                    Cancel
                  </button>
                  <button
                    type="submit"
                    className="flex items-center gap-1.5 px-3.5 py-2 rounded-xl text-[13px] font-semibold transition-all"
                    style={{ backgroundColor: '#0080FF', color: '#ffffff', boxShadow: '0 2px 8px rgba(0,128,255,0.25)' }}
                  >
                    <Plus className="w-3.5 h-3.5" /> Save prompt
                  </button>
                </div>
              </form>
            </div>
          )}

          {/* Registry Table */}
          <div className="rounded-2xl flex-1" style={{ backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', boxShadow: '0 1px 3px rgba(0,0,0,0.04)' }}>
            <div className="flex items-center justify-between px-5 py-3.5" style={{ borderBottom: '1px solid hsl(var(--border))' }}>
              <div>
                <h2 className="text-[15px] font-bold text-foreground">Registry</h2>
                <p className="text-xs" style={{ color: 'hsl(var(--muted-foreground))' }}>Every prompt, every version.</p>
              </div>
              <button
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[12px] font-medium transition-all text-muted-foreground"
              >
                <Settings2 className="w-3.5 h-3.5" /> Manage tags
              </button>
            </div>
            <div className="max-h-[520px] overflow-y-auto">
              <table className="min-w-full text-sm">
                <thead>
                  <tr style={{ backgroundColor: 'hsl(var(--secondary))' }}>
                    <th className="px-5 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider" style={{ color: 'hsl(var(--muted-foreground))' }}>Prompt</th>
                    <th className="px-5 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider" style={{ color: 'hsl(var(--muted-foreground))' }}>Tags</th>
                    <th className="px-5 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider" style={{ color: 'hsl(var(--muted-foreground))' }}>Last edited</th>
                    <th className="px-5 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider" style={{ color: 'hsl(var(--muted-foreground))' }}>Version</th>
                    <th className="px-5 py-2.5 text-left text-[11px] font-semibold uppercase tracking-wider" style={{ color: 'hsl(var(--muted-foreground))' }}>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredPrompts.map((prompt) => (
                    <tr key={prompt.id} className="transition-colors" style={{ borderBottom: '1px solid hsl(var(--border))' }}>
                      <td className="px-5 py-3.5">
                        <button
                          className="text-left font-semibold text-[13px] transition-colors text-foreground"
                          onClick={() => projectId && navigate(`/projects/${projectId}/prompts/${prompt.id}`)}
                        >
                          {prompt.name}
                        </button>
                        <p className="text-[11px] mt-0.5" style={{ color: 'hsl(var(--muted-foreground))' }}>{prompt.description || 'Untitled prompt'}</p>
                      </td>
                      <td className="px-5 py-3.5">
                        <div className="flex flex-wrap gap-1">
                          {prompt.tags.map((tag) => (
                            <span
                              key={`${prompt.id}-${tag}`}
                              className="text-[10px] font-medium px-2 py-0.5 rounded-md"
                              style={{ backgroundColor: 'rgba(0,128,255,0.06)', color: '#0080FF' }}
                            >
                              {tag}
                            </span>
                          ))}
                        </div>
                      </td>
                      <td className="px-5 py-3.5 text-[12px] text-muted-foreground">{new Date(prompt.lastEdited).toLocaleString()}</td>
                      <td className="px-5 py-3.5">
                        <span className="text-[12px] font-mono font-semibold px-2 py-0.5 rounded-md" style={{ backgroundColor: 'hsl(var(--secondary))', color: 'hsl(var(--foreground))' }}>
                          v{prompt.activeVersion}
                        </span>
                      </td>
                      <td className="px-5 py-3.5">
                        <div className="flex gap-1.5">
                          <button
                            className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-[11px] font-medium transition-all"
                            style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--foreground))' }}
                            onClick={() => handleTestInPlayground(prompt)}
                          >
                            <Play className="w-3 h-3" /> Test
                          </button>
                          <button
                            className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-[11px] font-medium transition-all"
                            style={{ backgroundColor: 'rgba(139,92,246,0.08)', border: '1px solid rgba(139,92,246,0.2)', color: '#8b5cf6' }}
                            onClick={() => {
                              setEvalPrompt(prompt);
                              setShowEvalModal(true);
                            }}
                          >
                            <Beaker className="w-3 h-3" /> Evaluate
                          </button>
                          <button
                            className="px-2.5 py-1.5 rounded-lg text-[11px] font-medium transition-all text-muted-foreground"
                            onClick={() => projectId && navigate(`/projects/${projectId}/prompts/${prompt.id}`)}
                          >
                            Edit
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                  {filteredPrompts.length === 0 && (
                    <tr>
                      <td colSpan={5} className="px-6 py-10">
                        {/* Rich Empty State */}
                        <div className="text-center mb-8">
                          <div className="w-14 h-14 rounded-2xl flex items-center justify-center mx-auto mb-5" style={{ backgroundColor: 'rgba(0,128,255,0.08)' }}>
                            <FileText className="w-7 h-7" style={{ color: '#0080FF' }} />
                          </div>
                          <h3 className="text-lg font-bold mb-2 text-foreground">No prompts yet</h3>
                          <p className="text-sm" style={{ color: 'hsl(var(--muted-foreground))' }}>Create your first prompt template to get started</p>
                        </div>

                        {/* Guide Card — full-width */}
                        <div className="rounded-2xl p-6" style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))' }}>
                          <div className="flex items-center gap-3 mb-6">
                            <div className="w-9 h-9 rounded-lg flex items-center justify-center" style={{ backgroundColor: 'rgba(16,185,129,0.1)' }}>
                              <Sparkles className="w-4 h-4" style={{ color: '#10b981' }} />
                            </div>
                            <div>
                              <h4 className="text-[15px] font-bold text-foreground">How Prompt Registry Works</h4>
                              <p className="text-[13px]" style={{ color: 'hsl(var(--muted-foreground))' }}>3 steps to versioned prompt engineering</p>
                            </div>
                          </div>
                          <div className="grid grid-cols-3 gap-6">
                            <div className="rounded-xl p-4 bg-card border border-border">
                              <div className="flex items-center gap-2.5 mb-3">
                                <div className="w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 text-[13px] font-bold" style={{ backgroundColor: '#0080FF', color: '#ffffff' }}>1</div>
                                <p className="text-[14px] font-bold text-foreground">Create</p>
                              </div>
                              <p className="text-[13px] leading-relaxed text-muted-foreground">Write templates with {'{{variables}}'} for dynamic content. Organize with tags and descriptions.</p>
                            </div>
                            <div className="rounded-xl p-4 bg-card border border-border">
                              <div className="flex items-center gap-2.5 mb-3">
                                <div className="w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 text-[13px] font-bold" style={{ backgroundColor: '#0080FF', color: '#ffffff' }}>2</div>
                                <p className="text-[14px] font-bold text-foreground">Test</p>
                              </div>
                              <p className="text-[13px] leading-relaxed text-muted-foreground">Run prompts in the playground against live traces to validate outputs before committing.</p>
                            </div>
                            <div className="rounded-xl p-4 bg-card border border-border">
                              <div className="flex items-center gap-2.5 mb-3">
                                <div className="w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 text-[13px] font-bold" style={{ backgroundColor: '#0080FF', color: '#ffffff' }}>3</div>
                                <p className="text-[14px] font-bold text-foreground">Version</p>
                              </div>
                              <p className="text-[13px] leading-relaxed text-muted-foreground">Track changes, compare performance across versions, and iterate on what works best.</p>
                            </div>
                          </div>
                        </div>
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </>
      )}

      {/* Version History Tab Content */}
      {activeTab === 'versions' && (
        <div className="flex flex-1 gap-5">
          {/* Version List */}
          <section
            className={cn("rounded-2xl transition-all bg-card border border-border", selectedPrompt ? "w-1/2" : "flex-1")}
          >
            <header className="flex items-center justify-between px-5 py-4" style={{ borderBottom: '1px solid hsl(var(--border))' }}>
              <div>
                <p className="text-[11px] uppercase tracking-wider font-semibold" style={{ color: 'hsl(var(--muted-foreground))' }}>Version History</p>
                <p className="text-[14px] font-semibold mt-0.5 text-foreground">
                  {committedPrompts.length} committed response{committedPrompts.length !== 1 ? 's' : ''}
                </p>
              </div>
              <div className="flex items-center gap-2">
                {branches.length > 0 && (
                  <select
                    value={selectedBranch}
                    onChange={(e) => setSelectedBranch(e.target.value)}
                    className="text-[13px] rounded-lg px-3 py-1.5 focus:outline-none"
                    style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--foreground))' }}
                  >
                    <option value="">All models</option>
                    {branches.map((branch) => (
                      <option key={branch} value={branch}>
                        {branch}
                      </option>
                    ))}
                  </select>
                )}
              </div>
            </header>
            <div style={{ maxHeight: '600px', overflowY: 'auto' }}>
              {filteredVersions.length === 0 ? (
                <div className="py-12 px-6">
                  {/* Rich Empty State */}
                  <div className="text-center mb-8">
                    <div className="w-14 h-14 rounded-2xl flex items-center justify-center mx-auto mb-5" style={{ backgroundColor: 'rgba(0,128,255,0.08)' }}>
                      <GitBranch className="w-7 h-7" style={{ color: '#0080FF' }} />
                    </div>
                    <h3 className="text-lg font-bold mb-2 text-foreground">No versions yet</h3>
                    <p className="text-sm" style={{ color: 'hsl(var(--muted-foreground))' }}>Commit LLM responses from traces to track changes over time</p>
                  </div>

                  {/* Guide Card */}
                  <div className="rounded-2xl p-6" style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))' }}>
                    <div className="flex items-center gap-3 mb-6">
                      <div className="w-9 h-9 rounded-lg flex items-center justify-center" style={{ backgroundColor: 'rgba(139,92,246,0.1)' }}>
                        <GitBranch className="w-4 h-4" style={{ color: '#8b5cf6' }} />
                      </div>
                      <div>
                        <h4 className="text-[15px] font-bold text-foreground">How Version History Works</h4>
                        <p className="text-[13px]" style={{ color: 'hsl(var(--muted-foreground))' }}>Track and compare prompt performance over time</p>
                      </div>
                    </div>
                    <div className="grid grid-cols-3 gap-6">
                      <div className="rounded-xl p-4 bg-card border border-border">
                        <div className="flex items-center gap-2.5 mb-3">
                          <div className="w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 text-[13px] font-bold" style={{ backgroundColor: '#0080FF', color: '#ffffff' }}>1</div>
                          <p className="text-[14px] font-bold text-foreground">Run a Trace</p>
                        </div>
                        <p className="text-[13px] leading-relaxed text-muted-foreground">Send LLM requests through the SDK. Each call is captured as a trace with full metadata.</p>
                      </div>
                      <div className="rounded-xl p-4 bg-card border border-border">
                        <div className="flex items-center gap-2.5 mb-3">
                          <div className="w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 text-[13px] font-bold" style={{ backgroundColor: '#0080FF', color: '#ffffff' }}>2</div>
                          <p className="text-[14px] font-bold text-foreground">Commit Response</p>
                        </div>
                        <p className="text-[13px] leading-relaxed text-muted-foreground">Mark the best responses as committed versions. Add descriptions to track why each was selected.</p>
                      </div>
                      <div className="rounded-xl p-4 bg-card border border-border">
                        <div className="flex items-center gap-2.5 mb-3">
                          <div className="w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 text-[13px] font-bold" style={{ backgroundColor: '#0080FF', color: '#ffffff' }}>3</div>
                          <p className="text-[14px] font-bold text-foreground">Compare & Iterate</p>
                        </div>
                        <p className="text-[13px] leading-relaxed text-muted-foreground">View side-by-side diffs, compare latency and cost across versions, and iterate confidently.</p>
                      </div>
                    </div>
                  </div>
                </div>
              ) : (
                <div>
                  {filteredVersions.map((prompt, idx) => {
                    const meta = (prompt as any).metadata || {};
                    const isSelected = selectedPrompt?.id === prompt.id;

                    return (
                      <div
                        key={prompt.id}
                        onClick={() => setSelectedPrompt(prompt)}
                        className="px-5 py-4 cursor-pointer transition-all"
                        style={{
                          borderBottom: idx < filteredVersions.length - 1 ? '1px solid hsl(var(--border))' : 'none',
                          backgroundColor: isSelected ? 'rgba(0,128,255,0.04)' : 'transparent',
                          borderLeft: isSelected ? '3px solid #0080FF' : '3px solid transparent',
                        }}
                        onMouseEnter={(e) => { if (!isSelected) e.currentTarget.style.backgroundColor = '#f8fafc'; }}
                        onMouseLeave={(e) => { if (!isSelected) e.currentTarget.style.backgroundColor = 'transparent'; }}
                      >
                        <div className="flex items-start justify-between">
                          <div className="flex-1">
                            <div className="flex items-center gap-2 flex-wrap">
                              <span className="text-[14px] font-semibold text-foreground">
                                {prompt.name}
                              </span>
                              {prompt.tags.map(tag => (
                                <span key={tag} className="text-[11px] px-2 py-0.5 rounded-md font-medium" style={{ backgroundColor: 'rgba(0,128,255,0.08)', color: '#0080FF' }}>
                                  {tag}
                                </span>
                              ))}
                            </div>
                            <p className="text-[13px] mt-1 line-clamp-1" style={{ color: 'hsl(var(--muted-foreground))' }}>
                              {prompt.description}
                            </p>
                            <div className="flex items-center gap-4 mt-2 text-[12px]" style={{ color: 'hsl(var(--muted-foreground))' }}>
                              {meta.model && (
                                <span className="flex items-center gap-1">
                                  <Beaker className="h-3 w-3" />
                                  {meta.model}
                                </span>
                              )}
                              {meta.latency_ms && (
                                <span className="flex items-center gap-1">
                                  <Zap className="h-3 w-3" />
                                  {meta.latency_ms}ms
                                </span>
                              )}
                              <span className="flex items-center gap-1">
                                <Clock className="h-3 w-3" />
                                {formatDistanceToNow(prompt.lastEdited, { addSuffix: true })}
                              </span>
                            </div>
                          </div>
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              setSelectedPrompt(prompt);
                            }}
                            className="flex items-center justify-center rounded-lg transition-all flex-shrink-0"
                            style={{ width: '32px', height: '32px', color: 'hsl(var(--muted-foreground))' }}
                            onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'hsl(var(--border))'; }}
                            onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
                          >
                            <Eye className="h-4 w-4" />
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </section>

          {/* Detail Panel */}
          <AnimatePresence>
            {selectedPrompt && (
              <motion.section
                initial={{ opacity: 0, x: 20 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: 20 }}
                className="w-1/2 rounded-2xl flex flex-col bg-card border border-border"
              >
                <header className="flex items-center justify-between px-5 py-4" style={{ borderBottom: '1px solid hsl(var(--border))' }}>
                  <div>
                    <p className="text-[11px] uppercase tracking-wider font-semibold" style={{ color: 'hsl(var(--muted-foreground))' }}>Details</p>
                    <h3 className="text-[16px] font-bold mt-0.5 text-foreground">{selectedPrompt.name}</h3>
                  </div>
                  <div className="flex items-center gap-2">
                    <button
                      className="flex items-center gap-1.5 px-3.5 py-2 rounded-xl text-[13px] font-semibold transition-all"
                      style={{ backgroundColor: '#0080FF', color: '#ffffff' }}
                      onClick={() => handleTestInPlayground(selectedPrompt)}
                    >
                      <Play className="h-3 w-3" /> Test
                    </button>
                    <button
                      onClick={() => setSelectedPrompt(null)}
                      className="flex items-center justify-center rounded-lg transition-all"
                      style={{ width: '32px', height: '32px', color: 'hsl(var(--muted-foreground))' }}
                      onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'hsl(var(--border))'; }}
                      onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
                    >
                      <X className="h-4 w-4" />
                    </button>
                  </div>
                </header>

                <div className="flex-1 overflow-y-auto p-5 space-y-5">
                  {/* Tags */}
                  {selectedPrompt.tags.length > 0 && (
                    <div>
                      <label className="text-[11px] uppercase tracking-wider font-semibold mb-2 block" style={{ color: 'hsl(var(--muted-foreground))' }}>Tags</label>
                      <div className="flex flex-wrap gap-1.5">
                        {selectedPrompt.tags.map(tag => (
                          <span key={tag} className="text-[12px] px-2.5 py-1 rounded-lg font-medium" style={{ backgroundColor: 'rgba(0,128,255,0.08)', color: '#0080FF' }}>
                            {tag}
                          </span>
                        ))}
                      </div>
                    </div>
                  )}

                  {/* Metadata */}
                  {(selectedPrompt as any).metadata && (
                    <div className="grid grid-cols-2 gap-3">
                      {(selectedPrompt as any).metadata.model && (
                        <div className="p-3.5 rounded-xl" style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))' }}>
                          <div className="flex items-center gap-2 text-[11px] font-semibold mb-1.5" style={{ color: 'hsl(var(--muted-foreground))' }}>
                            <Beaker className="h-3 w-3" /> Model
                          </div>
                          <p className="text-[13px] font-semibold" style={{ color: 'hsl(var(--foreground))', fontFamily: 'monospace' }}>
                            {(selectedPrompt as any).metadata.model}
                          </p>
                        </div>
                      )}
                      {(selectedPrompt as any).metadata.latency_ms && (
                        <div className="p-3.5 rounded-xl" style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))' }}>
                          <div className="flex items-center gap-2 text-[11px] font-semibold mb-1.5" style={{ color: 'hsl(var(--muted-foreground))' }}>
                            <Zap className="h-3 w-3" /> Latency
                          </div>
                          <p className="text-[13px] font-semibold" style={{ color: 'hsl(var(--foreground))', fontFamily: 'monospace' }}>
                            {(selectedPrompt as any).metadata.latency_ms}ms
                          </p>
                        </div>
                      )}
                      {(selectedPrompt as any).metadata.cost && (
                        <div className="p-3.5 rounded-xl" style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))' }}>
                          <div className="flex items-center gap-2 text-[11px] font-semibold mb-1.5" style={{ color: 'hsl(var(--muted-foreground))' }}>
                            <DollarSign className="h-3 w-3" /> Cost
                          </div>
                          <p className="text-[13px] font-semibold" style={{ color: 'hsl(var(--foreground))', fontFamily: 'monospace' }}>
                            ${(selectedPrompt as any).metadata.cost.toFixed(4)}
                          </p>
                        </div>
                      )}
                      {(selectedPrompt as any).metadata.trace_id && (
                        <div className="p-3.5 rounded-xl" style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))' }}>
                          <div className="flex items-center gap-2 text-[11px] font-semibold mb-1.5" style={{ color: 'hsl(var(--muted-foreground))' }}>
                            <ExternalLink className="h-3 w-3" /> Trace
                          </div>
                          <code className="text-[12px] font-semibold" style={{ color: '#0080FF', fontFamily: 'monospace' }}>
                            {(selectedPrompt as any).metadata.trace_id.substring(0, 12)}...
                          </code>
                        </div>
                      )}
                    </div>
                  )}

                  {/* Input/Prompt */}
                  <div>
                    <label className="text-[11px] uppercase tracking-wider font-semibold mb-2 block" style={{ color: 'hsl(var(--muted-foreground))' }}>
                      <MessageSquare className="h-3 w-3 inline mr-1" /> Input / Prompt
                    </label>
                    <div className="rounded-xl p-3.5 max-h-40 overflow-y-auto" style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))' }}>
                      <pre className="text-[13px] whitespace-pre-wrap" style={{ color: 'hsl(var(--foreground))', fontFamily: 'monospace' }}>
                        {selectedPrompt.content || 'No input captured'}
                      </pre>
                    </div>
                  </div>

                  {/* Output/Response */}
                  {(selectedPrompt as any).metadata?.output && (
                    <div>
                      <label className="text-[11px] uppercase tracking-wider font-semibold mb-2 block" style={{ color: 'hsl(var(--muted-foreground))' }}>
                        <MessageSquare className="h-3 w-3 inline mr-1" /> Output / Response
                      </label>
                      <div className="rounded-xl p-3.5 max-h-60 overflow-y-auto" style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))' }}>
                        <pre className="text-[13px] whitespace-pre-wrap" style={{ color: 'hsl(var(--foreground))', fontFamily: 'monospace' }}>
                          {(selectedPrompt as any).metadata.output}
                        </pre>
                      </div>
                    </div>
                  )}

                  {/* Description */}
                  {selectedPrompt.description && (
                    <div>
                      <label className="text-[11px] uppercase tracking-wider font-semibold mb-2 block" style={{ color: 'hsl(var(--muted-foreground))' }}>
                        Commit Message
                      </label>
                      <p className="text-[13px]" style={{ color: 'hsl(var(--foreground))' }}>{selectedPrompt.description}</p>
                    </div>
                  )}

                  {/* Timestamp */}
                  <div className="text-[12px] pt-3" style={{ color: 'hsl(var(--muted-foreground))', borderTop: '1px solid hsl(var(--border))' }}>
                    Committed {formatDistanceToNow(selectedPrompt.lastEdited, { addSuffix: true })} by {selectedPrompt.owner}
                  </div>
                </div>
              </motion.section>
            )}
          </AnimatePresence>
        </div>
      )}

      {/* Evaluation Modal */}
      {showEvalModal && evalPrompt && (
        <EvaluateTraceModal
          isOpen={showEvalModal}
          onClose={() => {
            setShowEvalModal(false);
            setEvalPrompt(null);
          }}
          traceId={evalPrompt.id}
          traceMetadata={{
            prompt_name: evalPrompt.name,
            prompt_content: evalPrompt.content,
            model: evalPrompt.metadata?.model,
            version: evalPrompt.activeVersion,
          }}
        />
      )}
    </div>
  );
}

// StatCard kept for potential reuse but no longer used in main view
function StatCard({ label, value, detail, accent }: { label: string; value: string; detail?: string; accent?: string }) {
  return (
    <div className="rounded-2xl p-4 bg-card border border-border">
      <p className="text-[11px] uppercase tracking-wider font-semibold" style={{ color: 'hsl(var(--muted-foreground))' }}>{label}</p>
      <p className="mt-1.5 text-2xl font-bold" style={{ color: accent ? '#0080FF' : '#111827' }}>{value}</p>
      {detail && <p className="text-[11px] mt-0.5" style={{ color: 'hsl(var(--muted-foreground))' }}>{detail}</p>}
    </div>
  );
}
