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
            <h1 className="text-lg font-bold tracking-tight" style={{ color: '#111827' }}>Prompt Registry</h1>
            <p className="text-xs" style={{ color: '#9ca3af' }}>Version every template and test against your traces</p>
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
            style={{ backgroundColor: '#f8fafc', border: '1px solid #e5e7eb', color: '#6b7280' }}
            disabled
          >
            <Cloud className="w-3.5 h-3.5" /> Cloud Sync
            <span className="text-[10px] px-1.5 py-0.5 rounded-md font-semibold" style={{ backgroundColor: 'rgba(0,128,255,0.1)', color: '#0080FF' }}>Soon</span>
          </button>
          <button
            onClick={() => projectId && navigate(`/projects/${projectId}/docs`)}
            className="flex items-center gap-1.5 px-3 py-2 rounded-xl text-[13px] font-medium transition-all"
            style={{ backgroundColor: '#f8fafc', border: '1px solid #e5e7eb', color: '#374151' }}
          >
            <BookOpen className="w-3.5 h-3.5" /> Docs
          </button>
        </div>
      </header>

      {/* Stats Bar */}
      <div className="flex items-center gap-4 mb-5">
        <div className="flex items-center gap-2 px-3.5 py-2 rounded-xl" style={{ backgroundColor: '#f8fafc', border: '1px solid #f1f5f9' }}>
          <span className="text-[13px] font-bold" style={{ color: '#111827' }}>{prompts.length}</span>
          <span className="text-[12px]" style={{ color: '#9ca3af' }}>prompts</span>
        </div>
        <div className="flex items-center gap-2 px-3.5 py-2 rounded-xl" style={{ backgroundColor: '#f8fafc', border: '1px solid #f1f5f9' }}>
          <span className="text-[13px] font-bold" style={{ color: '#0080FF' }}>{committedPrompts.length}</span>
          <span className="text-[12px]" style={{ color: '#9ca3af' }}>versions committed</span>
        </div>
      </div>

      {/* Tab Navigation */}
      <div className="flex items-center gap-1 mb-5 p-1 rounded-xl" style={{ backgroundColor: '#f1f5f9', width: 'fit-content' }}>
        <button
          onClick={() => setActiveTab('registry')}
          className="flex items-center gap-1.5 px-4 py-2 rounded-lg text-[13px] font-semibold transition-all"
          style={activeTab === 'registry'
            ? { backgroundColor: '#0080FF', color: '#ffffff', boxShadow: '0 2px 8px rgba(0,128,255,0.25)' }
            : { backgroundColor: 'transparent', color: '#6b7280' }
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
            : { backgroundColor: 'transparent', color: '#6b7280' }
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
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4" style={{ color: '#9ca3af' }} />
              <input
                value={search}
                onChange={(event) => setSearch(event.target.value)}
