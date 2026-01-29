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
    <div className="flex h-full flex-col gap-6">
      <header className="flex flex-col gap-2">
        <div className="flex items-center justify-between gap-4">
          <div>
            <p className="text-xs uppercase tracking-widest text-textTertiary">Prompts</p>
            <h1 className="text-2xl font-semibold text-textPrimary">Prompt Registry & Playground</h1>
            <p className="text-sm text-textSecondary">Version every template and test against your traces.</p>
          </div>
          <div className="flex gap-2 items-center">
            <VideoHelpButton pageId="prompts" />
            <Button variant="ghost" size="sm" className="gap-2" onClick={() => setShowComposer((prev) => !prev)}>
              <Plus className="h-4 w-4" /> New prompt
            </Button>
            <Button variant="outline" size="sm" className="gap-2 text-textTertiary cursor-not-allowed opacity-60" disabled>
              <Cloud className="h-4 w-4" /> Cloud Sync
              <span className="text-xs bg-primary/20 text-primary px-1.5 py-0.5 rounded">Soon</span>
            </Button>
            <Button variant="outline" size="sm" className="gap-2" onClick={() => navigate('/docs/prompts')}>
              <Beaker className="h-4 w-4" /> Docs
            </Button>
          </div>
        </div>
      </header>

      <section className="grid gap-4 md:grid-cols-3">
        <StatCard label="Total prompts" value={prompts.length.toString()} detail="across this project" />

        <StatCard label="Versions" value={committedPrompts.length.toString()} detail="committed responses" accent="text-blue-400" />
      </section>

      {/* Tab Navigation */}
      <div className="flex gap-2 border-b border-border/60 pb-2">
        <button
          onClick={() => setActiveTab('registry')}
          className={cn(
            'px-4 py-2 text-sm font-medium rounded-lg flex items-center gap-2 transition-colors',
            activeTab === 'registry'
              ? 'bg-primary text-white'
              : 'text-textSecondary hover:bg-surface hover:text-textPrimary'
          )}
        >
          <Rocket className="h-4 w-4" />
          Registry
        </button>
        <button
          onClick={() => setActiveTab('versions')}
          className={cn(
            'px-4 py-2 text-sm font-medium rounded-lg flex items-center gap-2 transition-colors',
            activeTab === 'versions'
              ? 'bg-primary text-white'
              : 'text-textSecondary hover:bg-surface hover:text-textPrimary'
          )}
        >
          <GitBranch className="h-4 w-4" />
          Version History
          {committedPrompts.length > 0 && (
            <span className="bg-white/20 px-1.5 py-0.5 rounded text-xs">
              {committedPrompts.length}
            </span>
          )}
        </button>
      </div>

      {activeTab === 'registry' && (
        <>
          <section className="rounded-3xl border border-border/60 bg-background/80 p-4">
            <div className="grid items-center gap-4 md:grid-cols-3">
              <div className="flex items-center gap-2 rounded-2xl border border-border/60 bg-surface/70 px-3">
                <Search className="h-4 w-4 text-textTertiary" />
                <Input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search prompts…" className="border-none bg-transparent px-0 focus-visible:ring-0" />
              </div>
              <div className="flex flex-wrap gap-2">
                <Button variant={selectedTag === null ? 'default' : 'ghost'} size="sm" onClick={() => setSelectedTag(null)}>
                  All tags
                </Button>
                {tags.map((tag) => (
                  <Button key={tag} variant={selectedTag === tag ? 'default' : 'ghost'} size="sm" onClick={() => setSelectedTag(tag)}>
                    <Tag className="mr-2 h-3.5 w-3.5" /> {tag}
                  </Button>
                ))}
              </div>
              <div className="flex justify-end gap-2 text-xs text-textSecondary">

                <span className="inline-flex items-center gap-1 rounded-full border border-border/60 px-2 py-1">
                  <History className="h-3.5 w-3.5" /> Version history
                </span>
              </div>
            </div>
            {showComposer && (
              <form onSubmit={handleCreate} className="mt-4 rounded-2xl border border-border/40 bg-surface/70 p-4">
                <div className="grid gap-4 md:grid-cols-3">
                  <Input required placeholder="Prompt name" value={draft.name} onChange={(event) => setDraft((prev) => ({ ...prev, name: event.target.value }))} />
                  <Input placeholder="Tags (comma separated)" value={draft.tags} onChange={(event) => setDraft((prev) => ({ ...prev, tags: event.target.value }))} />
                  <Input placeholder="Short description" value={draft.description} onChange={(event) => setDraft((prev) => ({ ...prev, description: event.target.value }))} />
                </div>
                <textarea
                  className="mt-3 h-32 w-full rounded-2xl border border-border/60 bg-background px-3 py-2 text-sm text-textPrimary"
                  placeholder="Template body"
                  value={draft.content}
                  onChange={(event) => setDraft((prev) => ({ ...prev, content: event.target.value }))}
                />
                <div className="mt-3 flex justify-end gap-2">
                  <Button type="button" variant="outline" onClick={() => setShowComposer(false)} className="bg-gray-200 border-2 border-gray-300 hover:bg-gray-300 text-gray-800 dark:bg-gray-700 dark:border-gray-600 dark:hover:bg-gray-600 dark:text-white">
                    Cancel
                  </Button>
                  <button type="submit" style={{ backgroundColor: '#2563eb', color: 'white' }} className="inline-flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium hover:bg-blue-700 shadow-md transition-colors">
                    <Plus className="h-4 w-4" /> Save draft
                  </button>
                </div>
              </form>
            )}
          </section>

          <section className="flex-1 rounded-3xl border border-border/60 bg-background/90">
            <header className="flex items-center justify-between border-b border-border/60 px-4 py-3">
              <div>
                <p className="text-xs uppercase tracking-widest text-textTertiary">Registry</p>
                <p className="text-sm text-textSecondary">Every prompt, every version.</p>
              </div>
              <Button variant="ghost" size="sm" className="gap-2 text-textSecondary">
                <Settings2 className="h-4 w-4" /> Manage tags
              </Button>
            </header>
            <div className="max-h-[520px] overflow-y-auto">
              <table className="min-w-full text-sm">
                <thead className="bg-surface/70 text-textTertiary">
                  <tr>
                    <th className="px-4 py-2 text-left font-semibold">Prompt</th>
                    <th className="px-4 py-2 text-left font-semibold">Tags</th>
                    <th className="px-4 py-2 text-left font-semibold">Last edited</th>
                    <th className="px-4 py-2 text-left font-semibold">Version</th>
                    <th className="px-4 py-2 text-left font-semibold">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredPrompts.map((prompt) => (
                    <tr key={prompt.id} className="border-b border-border/40 text-textSecondary">
                      <td className="px-4 py-3 text-textPrimary">
                        <button className="text-left font-semibold hover:underline" onClick={() => projectId && navigate(`/projects/${projectId}/prompts/${prompt.id}`)}>
                          {prompt.name}
                        </button>
                        <p className="text-xs text-textTertiary">{prompt.description || 'Untitled prompt'}</p>
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex flex-wrap gap-1">
                          {prompt.tags.map((tag) => (
                            <Badge key={`${prompt.id}-${tag}`} className="bg-surface/60 text-textSecondary">
                              {tag}
                            </Badge>
                          ))}
                        </div>
                      </td>
                      <td className="px-4 py-3">{new Date(prompt.lastEdited).toLocaleString()}</td>
                      <td className="px-4 py-3">
                        <span className="font-mono text-sm text-textPrimary">
                          v{prompt.activeVersion}
                        </span>

                      </td>
                      <td className="px-4 py-3">
                        <div className="flex gap-2">
                          <Button size="sm" variant="outline" className="gap-1" onClick={() => handleTestInPlayground(prompt)}>
                            <Play className="h-3 w-3" /> Test
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            className="gap-1 bg-purple-500/10 border-purple-500/30 text-purple-400 hover:bg-purple-500/20"
                            onClick={() => {
                              setEvalPrompt(prompt);
                              setShowEvalModal(true);
                            }}
                          >
                            <Beaker className="h-3 w-3" /> Evaluate
                          </Button>

                          <Button size="sm" variant="ghost" onClick={() => projectId && navigate(`/projects/${projectId}/prompts/${prompt.id}`)}>
                            Edit
                          </Button>
                        </div>
                      </td>
                    </tr>
                  ))}
                  {filteredPrompts.length === 0 && (
                    <tr>
                      <td colSpan={5} className="px-4 py-12 text-center text-textSecondary">
                        <Filter className="mx-auto mb-3 h-6 w-6" />
                        No prompts match the current filters.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </section>
        </>
      )}

      {/* Version History Tab Content */}
      {activeTab === 'versions' && (
        <div className="flex flex-1 gap-4">
          {/* Version List */}
          <section className={cn(
            "rounded-3xl border border-border/60 bg-background/90 transition-all",
            selectedPrompt ? "w-1/2" : "flex-1"
          )}>
            <header className="flex items-center justify-between border-b border-border/60 px-4 py-3">
              <div>
                <p className="text-xs uppercase tracking-widest text-textTertiary">Version History</p>
                <p className="text-sm text-textSecondary">
                  {committedPrompts.length} committed response{committedPrompts.length !== 1 ? 's' : ''}
                </p>
              </div>
              <div className="flex items-center gap-2">
                {branches.length > 0 && (
                  <select
                    value={selectedBranch}
                    onChange={(e) => setSelectedBranch(e.target.value)}
                    className="text-sm border border-border/60 rounded-lg px-3 py-1.5 bg-surface"
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
            <div className="max-h-[520px] overflow-y-auto">
              {filteredVersions.length === 0 ? (
                <div className="text-center py-12">
                  <GitBranch className="mx-auto mb-3 h-8 w-8 text-textTertiary" />
                  <p className="text-textSecondary">No versions yet</p>
                  <p className="text-xs text-textTertiary mt-1">
                    Commit LLM responses from traces to track changes over time
                  </p>
                </div>
              ) : (
                <div className="divide-y divide-border/40">
                  {filteredVersions.map((prompt) => {
                    const meta = (prompt as any).metadata || {};
                    const isSelected = selectedPrompt?.id === prompt.id;

                    return (
                      <div
                        key={prompt.id}
                        onClick={() => setSelectedPrompt(prompt)}
                        className={cn(
                          "px-4 py-3 cursor-pointer transition-colors",
                          isSelected
                            ? "bg-primary/10 border-l-2 border-primary"
                            : "hover:bg-surface/50"
                        )}
                      >
                        <div className="flex items-start justify-between">
                          <div className="flex-1">
                            <div className="flex items-center gap-2 flex-wrap">
                              <span className="text-sm font-medium text-textPrimary">
                                {prompt.name}
                              </span>
                              {prompt.tags.map(tag => (
                                <Badge key={tag} className="bg-primary/20 text-primary text-xs">
                                  {tag}
                                </Badge>
                              ))}
                            </div>
                            <p className="text-xs text-textTertiary mt-1 line-clamp-1">
                              {prompt.description}
                            </p>
                            <div className="flex items-center gap-4 mt-2 text-xs text-textTertiary">
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
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={(e) => {
                              e.stopPropagation();
                              setSelectedPrompt(prompt);
                            }}
                          >
                            <Eye className="h-4 w-4" />
                          </Button>
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
                className="w-1/2 rounded-3xl border border-border/60 bg-background/90 flex flex-col"
              >
                <header className="flex items-center justify-between border-b border-border/60 px-4 py-3">
                  <div>
                    <p className="text-xs uppercase tracking-widest text-textTertiary">Details</p>
                    <h3 className="text-lg font-semibold text-textPrimary">{selectedPrompt.name}</h3>
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      size="sm"
                      variant="default"
                      className="gap-1"
                      onClick={() => {
                        // Test in playground - use handleTestInPlayground
                        handleTestInPlayground(selectedPrompt);
                      }}
                    >
                      <Play className="h-3 w-3" /> Test
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => setSelectedPrompt(null)}
                    >
                      <X className="h-4 w-4" />
                    </Button>
                  </div>
                </header>

                <div className="flex-1 overflow-y-auto p-4 space-y-4">
                  {/* Tags */}
                  {selectedPrompt.tags.length > 0 && (
                    <div>
                      <label className="text-xs uppercase tracking-widest text-textTertiary mb-2 block">Tags</label>
                      <div className="flex flex-wrap gap-1">
                        {selectedPrompt.tags.map(tag => (
                          <Badge key={tag} className="bg-primary/20 text-primary">
                            {tag}
                          </Badge>
                        ))}
                      </div>
                    </div>
                  )}

                  {/* Metadata */}
                  {(selectedPrompt as any).metadata && (
                    <div className="grid grid-cols-2 gap-3">
                      {(selectedPrompt as any).metadata.model && (
                        <div className="p-3 bg-surface rounded-xl">
                          <div className="flex items-center gap-2 text-textTertiary text-xs mb-1">
                            <Beaker className="h-3 w-3" /> Model
                          </div>
                          <p className="font-mono text-sm text-textPrimary">
                            {(selectedPrompt as any).metadata.model}
                          </p>
                        </div>
                      )}
                      {(selectedPrompt as any).metadata.latency_ms && (
                        <div className="p-3 bg-surface rounded-xl">
                          <div className="flex items-center gap-2 text-textTertiary text-xs mb-1">
                            <Zap className="h-3 w-3" /> Latency
                          </div>
                          <p className="font-mono text-sm text-textPrimary">
                            {(selectedPrompt as any).metadata.latency_ms}ms
                          </p>
                        </div>
                      )}
                      {(selectedPrompt as any).metadata.cost && (
                        <div className="p-3 bg-surface rounded-xl">
                          <div className="flex items-center gap-2 text-textTertiary text-xs mb-1">
                            <DollarSign className="h-3 w-3" /> Cost
                          </div>
                          <p className="font-mono text-sm text-textPrimary">
                            ${(selectedPrompt as any).metadata.cost.toFixed(4)}
                          </p>
                        </div>
                      )}
                      {(selectedPrompt as any).metadata.trace_id && (
                        <div className="p-3 bg-surface rounded-xl">
                          <div className="flex items-center gap-2 text-textTertiary text-xs mb-1">
                            <ExternalLink className="h-3 w-3" /> Trace
                          </div>
                          <code className="font-mono text-xs text-primary">
                            {(selectedPrompt as any).metadata.trace_id.substring(0, 12)}...
                          </code>
                        </div>
                      )}
                    </div>
                  )}

                  {/* Input/Prompt */}
                  <div>
                    <label className="text-xs uppercase tracking-widest text-textTertiary mb-2 block">
                      <MessageSquare className="h-3 w-3 inline mr-1" /> Input / Prompt
                    </label>
                    <div className="bg-surface rounded-xl p-3 max-h-40 overflow-y-auto">
                      <pre className="text-sm text-textSecondary whitespace-pre-wrap font-mono">
                        {selectedPrompt.content || 'No input captured'}
                      </pre>
                    </div>
                  </div>

                  {/* Output/Response */}
                  {(selectedPrompt as any).metadata?.output && (
                    <div>
                      <label className="text-xs uppercase tracking-widest text-textTertiary mb-2 block">
                        <MessageSquare className="h-3 w-3 inline mr-1" /> Output / Response
                      </label>
                      <div className="bg-surface rounded-xl p-3 max-h-60 overflow-y-auto">
                        <pre className="text-sm text-textSecondary whitespace-pre-wrap font-mono">
                          {(selectedPrompt as any).metadata.output}
                        </pre>
                      </div>
                    </div>
                  )}

                  {/* Description */}
                  {selectedPrompt.description && (
                    <div>
                      <label className="text-xs uppercase tracking-widest text-textTertiary mb-2 block">
                        Commit Message
                      </label>
                      <p className="text-sm text-textSecondary">{selectedPrompt.description}</p>
                    </div>
                  )}

                  {/* Timestamp */}
                  <div className="text-xs text-textTertiary pt-2 border-t border-border/40">
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

function StatCard({ label, value, detail, accent }: { label: string; value: string; detail?: string; accent?: string }) {
  return (
    <div className="rounded-2xl border border-border/60 bg-background/90 p-4">
      <p className="text-xs uppercase tracking-widest text-textTertiary">{label}</p>
      <p className={cn('mt-2 text-3xl font-semibold text-textPrimary', accent)}>{value}</p>
      {detail && <p className="text-xs text-textSecondary">{detail}</p>}
    </div>
  );
}
