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

import React, { useEffect, useState } from 'react';
import {
  Database,
  Layers,
  Search,
  Server,
  CheckCircle,
  AlertCircle,
  RefreshCw,
  BookOpen,
  Terminal,
  Copy,
  Check,
  Activity,
  BarChart3,
  Network,
  Zap,
  HardDrive,
  Cpu
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { API_BASE_URL } from '../lib/agentreplay-api-core';
import { VideoHelpButton } from '../components/VideoHelpButton';

// ============================================================================
// Memory Page - MCP Memory with Tabs
// ============================================================================
// Tabs:
// 1. Overview - Getting started guide
// 2. Memory Traces - Show stored memory/vector traces
// 3. Knowledge Graph - Visual representation of connections
// 4. MCP Server - Server details, stats, and analytics

interface MCPProjectInfo {
  project_id: number;
  project_name: string;
  tenant_id: number;
  description: string;
  created_at: number;
  vector_count: number;
  collection_count: number;
  last_activity: number | null;
  storage_path: string;
}

interface MCPCollection {
  name: string;
  document_count: number;
  vector_count: number;
  embedding_dimension: number;
  created_at: number;
  last_updated: number;
}

interface MCPStatus {
  initialized: boolean;
  server_running: boolean;
  tenant_id: number;
  project_id: number;
  isolation_mode: string;
}

interface MCPInfoResponse {
  project: MCPProjectInfo;
  collections: MCPCollection[];
  status: MCPStatus;
}

interface MemoryTrace {
  id: string;
  collection: string;
  content: string;
  embedding_preview: number[];
  timestamp: number;
  metadata?: Record<string, unknown>;
  similarity_score?: number;
}

type TabType = 'traces' | 'server';

export default function MemoryPage() {
  const [activeTab, setActiveTab] = useState<TabType>('traces');
  const [mcpInfo, setMcpInfo] = useState<MCPInfoResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showDocs, setShowDocs] = useState(false);

  const fetchMCPInfo = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/memory/info`);
      if (!response.ok) {
        throw new Error(`Failed to fetch MCP info: ${response.statusText}`);
      }
      const data = await response.json();
      setMcpInfo(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error');
      // Set default values on error for demo
      setMcpInfo({
        project: {
          project_id: 1000,
          project_name: 'MCP Memory',
          tenant_id: 2,
          description: 'Dedicated project for MCP vector storage and memory operations',
          created_at: Date.now() / 1000,
          vector_count: 0,
          collection_count: 1,
          last_activity: null,
          storage_path: 'project_1000',
        },
        collections: [{
          name: 'default',
          document_count: 0,
          vector_count: 0,
          embedding_dimension: 384,
          created_at: Date.now() / 1000,
          last_updated: Date.now() / 1000,
        }],
        status: {
          initialized: false,
          server_running: false,
          tenant_id: 2,
          project_id: 1000,
          isolation_mode: 'tenant_project',
        },
      });
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchMCPInfo();
  }, []);

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
  };

  const formatDate = (timestamp: number) => {
    if (!timestamp) return 'Never';
    return new Date(timestamp * 1000).toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  const tabs = [
    { id: 'traces' as TabType, label: 'Memories', icon: Database },
    { id: 'server' as TabType, label: 'Server', icon: Server },
  ];

  return (
    <div className="flex flex-col h-full" style={{ paddingTop: '8px' }}>
      {/* Header */}
      <div className="mb-5">
        <div className="flex items-center justify-between mb-2">
          <div className="flex items-center gap-3">
            <div
              className="w-9 h-9 rounded-lg flex items-center justify-center flex-shrink-0"
              style={{ backgroundColor: 'rgba(0,128,255,0.1)' }}
            >
              <Database className="w-4 h-4" style={{ color: '#0080FF' }} />
            </div>
            <div>
              <h1 className="text-[18px] font-bold" style={{ color: '#111827' }}>Memory</h1>
              <p className="text-[13px]" style={{ color: '#9ca3af' }}>Vector storage and semantic memory for AI agents</p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={() => setShowDocs(!showDocs)}
              className="flex items-center gap-2 px-3 py-1.5 text-[13px] font-semibold rounded-lg transition-all"
              style={showDocs
                ? { backgroundColor: '#0080FF', color: '#ffffff' }
                : { backgroundColor: '#ffffff', border: '1px solid #e5e7eb', color: '#6b7280' }
              }
            >
              <BookOpen className="w-4 h-4" />
              {showDocs ? 'Hide Docs' : 'View Docs'}
            </button>
            <VideoHelpButton pageId="memory" />
          </div>
        </div>
      </div>

      {/* Quick Start Guide (collapsible) */}
      {showDocs && (
        <div className="mb-5 rounded-2xl overflow-hidden" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
          <div className="flex items-center justify-between px-5 py-3" style={{ borderBottom: '1px solid #f1f5f9' }}>
            <h3 className="font-bold text-[14px] flex items-center gap-2" style={{ color: '#111827' }}>
              <div className="w-6 h-6 rounded-lg flex items-center justify-center" style={{ background: 'linear-gradient(135deg, #0080FF, #00c8ff)' }}>
                <BookOpen className="w-3.5 h-3.5" style={{ color: '#ffffff' }} />
              </div>
              Quick Start Guide
            </h3>
            <button onClick={() => setShowDocs(false)} className="w-6 h-6 rounded-full flex items-center justify-center transition-all" style={{ color: '#9ca3af', backgroundColor: '#f1f5f9' }}>
              <span className="text-[11px]">✕</span>
            </button>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-0">
            {/* Ingest Block */}
            <div style={{ borderRight: '1px solid #f1f5f9' }}>
              <div className="flex items-center justify-between px-4 py-2" style={{ backgroundColor: '#f8fafc', borderBottom: '1px solid #f1f5f9' }}>
                <div className="flex items-center gap-2">
                  <span className="px-2 py-0.5 rounded text-[10px] font-bold font-mono tracking-wide" style={{ backgroundColor: '#10b981', color: '#ffffff' }}>POST</span>
                  <span className="text-[12px] font-medium" style={{ color: '#6b7280' }}>Ingest Memory</span>
                </div>
                <span className="text-[10px] font-medium" style={{ color: '#d1d5db' }}>REST API</span>
              </div>
              <div className="p-4 rounded-bl-2xl" style={{ backgroundColor: '#0f172a' }}>
                <pre className="text-[11px] font-mono overflow-x-auto leading-relaxed" style={{ color: '#94a3b8' }}><span style={{ color: '#38bdf8' }}>curl</span> -X <span style={{ color: '#a78bfa' }}>POST</span> <span style={{ color: '#fbbf24' }}>http://localhost:47100/api/v1/memory/ingest</span> \{'\n'}  -H <span style={{ color: '#4ade80' }}>&quot;Content-Type: application/json&quot;</span> \{'\n'}  -d <span style={{ color: '#4ade80' }}>{`'{"collection": "docs", "content": "Your content"}'`}</span></pre>
              </div>
            </div>
            {/* Search Block */}
            <div>
              <div className="flex items-center justify-between px-4 py-2" style={{ backgroundColor: '#f8fafc', borderBottom: '1px solid #f1f5f9' }}>
                <div className="flex items-center gap-2">
                  <span className="px-2 py-0.5 rounded text-[10px] font-bold font-mono tracking-wide" style={{ backgroundColor: '#0080FF', color: '#ffffff' }}>POST</span>
                  <span className="text-[12px] font-medium" style={{ color: '#6b7280' }}>Search Memories</span>
                </div>
                <span className="text-[10px] font-medium" style={{ color: '#d1d5db' }}>REST API</span>
              </div>
              <div className="p-4 rounded-br-2xl" style={{ backgroundColor: '#0f172a' }}>
                <pre className="text-[11px] font-mono overflow-x-auto leading-relaxed" style={{ color: '#94a3b8' }}><span style={{ color: '#38bdf8' }}>curl</span> -X <span style={{ color: '#a78bfa' }}>POST</span> <span style={{ color: '#fbbf24' }}>http://localhost:47100/api/v1/memory/retrieve</span> \{'\n'}  -H <span style={{ color: '#4ade80' }}>&quot;Content-Type: application/json&quot;</span> \{'\n'}  -d <span style={{ color: '#4ade80' }}>{`'{"query": "your search", "k": 5}'`}</span></pre>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Tabs */}
      <div className="flex gap-1 mb-5 rounded-xl p-1" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className="flex items-center gap-2 px-4 py-2 rounded-lg text-[13px] font-semibold transition-all"
            style={activeTab === tab.id
              ? { backgroundColor: '#0080FF', color: '#ffffff' }
              : { color: '#6b7280' }
            }
          >
            <tab.icon className="w-4 h-4" />
            {tab.label}
          </button>
        ))}
        <div className="flex-1" />
        <button
          onClick={fetchMCPInfo}
          className="p-2 rounded-md transition-all"
          title="Refresh"
        >
          <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} style={{ color: '#9ca3af' }} />
        </button>
      </div>

      {/* Tab Content */}
      {activeTab === 'traces' && <MemoryTracesTab mcpInfo={mcpInfo} loading={loading} />}
      {activeTab === 'server' && <MCPServerTab mcpInfo={mcpInfo} loading={loading} error={error} formatDate={formatDate} />}
    </div>
  );
}

// ============================================================================
// Overview Tab - Getting Started Guide
// ============================================================================
function OverviewTab({ copyToClipboard, copiedCode }: { copyToClipboard: (text: string, id: string) => void; copiedCode: string | null }) {
  const [dbPath, setDbPath] = useState<string>('Loading...');

  useEffect(() => {
    // Fetch database path from backend
    invoke('health_check')
      .then((status: any) => setDbPath(status.database_path))
      .catch((err) => console.error('Failed to get health status:', err));
  }, []);

  const pythonExample = `# ✅ ONLINE MODE: Safe, concurrent memory (Recommended)
from agentreplay import AgentreplayClient

client = AgentreplayClient(
    url="http://localhost:47100",
    tenant_id=1,
    project_id=0
)

# 1. Ingest a memory
client.ingest_memory(
    collection="agent_history",
    content="User prefers concise answers and dark mode.",
    metadata={"source": "chat_session_123", "confidence": "0.95"}
)

# 2. Retrieve memories
results = client.retrieve_memory(
    collection="agent_history",
    query="What are the user preferences?",
    k=3
)

for mem in results["results"]:
    print(f"Frame recall: {mem['content']} (score: {mem['similarity']})")`;

  const curlExample = `# Raw API usage (if not using SDK)
# Store memory
curl -X POST http://localhost:47100/api/v1/memory/ingest \\
  -H "Content-Type: application/json" \\
  -d '{
    "collection": "default",
    "content": "User prefers dark mode"
  }'

# Search memories
curl -X POST http://localhost:47100/api/v1/memory/retrieve \\
  -H "Content-Type: application/json" \\
  -d '{ "query": "preferences", "k": 5 }'`;

  // Step-by-step: 1) Ensure Agentreplay Desktop is running
  // 2) Build the bridge: cd agentreplay-claude-bridge && npm install && npm run build
  // 3) Add this config to your editor's MCP settings
  const mcpConfigExample = `{
  "mcpServers": {
    "agentreplay-memory": {
      "command": "node",
      "args": ["<path>/agentreplay-claude-bridge/dist/index.js"],
      "env": {
        "AGENTREPLAY_URL": "http://127.0.0.1:47101/mcp"
      }
    }
  }
}`;

  return (
    <div className="space-y-8">
      {/* Database Location Info */}
      <div className="bg-blue-500/10 border border-blue-500/20 rounded-xl p-4 flex items-start gap-4">
        <div className="w-10 h-10 rounded-lg bg-blue-500/20 flex items-center justify-center flex-shrink-0">
          <Database className="w-5 h-5 text-blue-600 dark:text-blue-400" />
        </div>
        <div className="flex-1">
          <h3 className="font-medium text-blue-600 dark:text-blue-400 mb-1">Database Location</h3>
          <p className="text-sm text-textSecondary mb-2">
            Your semantic memory is stored locally at this path. Reference this path in your SochDB SDK code.
          </p>
          <div className="bg-background rounded px-3 py-2 text-xs font-mono text-textPrimary border border-border flex justify-between items-center">
            <span className="truncate mr-4">{dbPath}</span>
            <button
              onClick={() => copyToClipboard(dbPath, 'path')}
              className="text-textSecondary hover:text-textPrimary flex-shrink-0"
            >
              {copiedCode === 'path' ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
            </button>
          </div>
        </div>
      </div>

      {/* Access Modes Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">

        {/* Offline Mode - SDK */}
        <div className="bg-surface border border-border rounded-xl p-6">
          <h3 className="font-medium text-textPrimary flex items-center gap-2 mb-4">
            <Layers className="w-4 h-4 text-purple-600 dark:text-purple-400" />
            Offline Mode (Python SDK)
          </h3>
          <p className="text-xs text-textSecondary mb-4">
            Direct access to the database using SochDB SDK.
          </p>
          <div className="relative">
            <button
              onClick={() => copyToClipboard(pythonExample, 'python')}
              className="absolute right-2 top-2 p-1 text-textSecondary hover:text-textPrimary bg-background rounded border border-border"
              title="Copy code"
            >
              {copiedCode === 'python' ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
            </button>
            <pre className="bg-background rounded-lg p-4 overflow-x-auto text-xs leading-relaxed border border-border">
              <code className="text-textSecondary text-xs">{pythonExample}</code>
            </pre>
          </div>
        </div>

        {/* Online Mode - REST & MCP */}
        <div className="space-y-6">
          {/* REST API */}
          <div className="bg-surface border border-border rounded-xl p-6">
            <h3 className="font-medium text-textPrimary flex items-center gap-2 mb-4">
              <Zap className="w-4 h-4 text-green-600 dark:text-green-400" />
              Online Mode (REST API)
            </h3>
            <p className="text-xs text-textSecondary mb-4">
              Interact with memory while Agentreplay is running via HTTP API.
            </p>
            <div className="relative">
              <button
                onClick={() => copyToClipboard(curlExample, 'curl')}
                className="absolute right-2 top-2 p-1 text-textSecondary hover:text-textPrimary bg-background rounded border border-border"
                title="Copy code"
              >
                {copiedCode === 'curl' ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
              </button>
              <pre className="bg-background rounded-lg p-4 overflow-x-auto text-xs leading-relaxed border border-border">
                <code className="text-textSecondary text-xs">{curlExample}</code>
              </pre>
            </div>
          </div>

          {/* MCP Config */}
          <div className="bg-surface border border-border rounded-xl p-6">
            <h3 className="font-medium text-textPrimary flex items-center gap-2 mb-4">
              <Server className="w-4 h-4 text-blue-600 dark:text-blue-400" />
              Connect via MCP (Claude/Cursor)
            </h3>
            <p className="text-xs text-textSecondary mb-4">
              Add this to your editor's MCP settings to enable memory access.
              Uses stdio connection.
            </p>
            <div className="relative">
              <button
                onClick={() => copyToClipboard(mcpConfigExample, 'mcp')}
                className="absolute right-2 top-2 p-1 text-textSecondary hover:text-textPrimary bg-background rounded border border-border"
                title="Copy code"
              >
                {copiedCode === 'mcp' ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
              </button>
              <pre className="bg-background rounded-lg p-4 overflow-x-auto text-xs leading-relaxed border border-border">
                <code className="text-textSecondary text-xs">{mcpConfigExample}</code>
              </pre>
            </div>
          </div>
        </div>
      </div>

      {/* Architecture */}
      <div className="bg-surface border border-border rounded-xl p-6">
        <h2 className="text-xl font-semibold text-textPrimary mb-4">Architecture</h2>
        <div className="bg-background rounded-lg p-6">
          <div className="flex flex-col md:flex-row items-center justify-center gap-4 text-center">
            <div className="bg-surface border border-border rounded-lg p-4 w-40">
              <div className="text-2xl mb-2">🤖</div>
              <div className="font-medium text-textPrimary text-sm">AI Agent</div>
              <div className="text-xs text-textSecondary">Claude, GPT, etc.</div>
            </div>
            <div className="text-2xl text-textTertiary">→</div>
            <div className="bg-surface border border-primary/50 rounded-lg p-4 w-40">
              <div className="text-2xl mb-2">🧠</div>
              <div className="font-medium text-primary text-sm">SochDB</div>
              <div className="text-xs text-textSecondary">Local DB or REST API</div>
            </div>
            <div className="text-2xl text-textTertiary">→</div>
            <div className="bg-surface border border-border rounded-lg p-4 w-40">
              <div className="text-2xl mb-2">📊</div>
              <div className="font-medium text-textPrimary text-sm">HNSW Index</div>
              <div className="text-xs text-textSecondary">Vector Embeddings</div>
            </div>
          </div>
          <div className="mt-6 pt-6 border-t border-border">
            <div className="text-center text-sm text-textSecondary">
              <strong className="text-textPrimary">Dual Mode:</strong> Use <strong>Offline Mode</strong> (Python SDK) to access the DB file directly,
              or <strong>Online Mode</strong> (REST/MCP) when Agentreplay is running. Embedding dimension depends on your model
              (384d for FastEmbed, 768d for base models, 1536d for OpenAI).
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Memory Traces Tab
// ============================================================================
function MemoryTracesTab({ mcpInfo, loading }: { mcpInfo: MCPInfoResponse | null; loading: boolean }) {
  const [searchQuery, setSearchQuery] = useState('');
  const [traces, setTraces] = useState<MemoryTrace[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [hasSearched, setHasSearched] = useState(false);
  const [isSearchMode, setIsSearchMode] = useState(false);

  // Pagination state
  const [currentPage, setCurrentPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [totalMemories, setTotalMemories] = useState(0);
  const perPage = 10;

  const vectorCount = mcpInfo?.project.vector_count || 0;

  // Auto-load memories when tab mounts and we have vectors
  useEffect(() => {
    if (vectorCount > 0 && traces.length === 0 && !hasSearched) {
      loadMemoryPage(1);
    }
  }, [vectorCount]);

  // Load memories via the list endpoint (paginated, newest first)
  const loadMemoryPage = async (page: number) => {
    setSearching(true);
    setSearchError(null);
    setIsSearchMode(false);

    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/memory/list?page=${page}&per_page=${perPage}`);

      if (!response.ok) {
        throw new Error(`Failed to load memories: ${response.statusText}`);
      }

      const data = await response.json();
      const results: MemoryTrace[] = (data.memories || []).map((r: any) => ({
        id: r.id,
        collection: r.collection || 'default',
        content: r.content || 'No content',
        embedding_preview: [],
        timestamp: r.created_at || 0,
        metadata: r.metadata || {},
        similarity_score: undefined, // No score for listing
      }));

      setTraces(results);
      setCurrentPage(data.page || 1);
      setTotalPages(data.total_pages || 1);
      setTotalMemories(data.total || 0);
      setHasSearched(true);
    } catch (err) {
      console.error("Load memories error:", err);
      setSearchError(err instanceof Error ? err.message : 'Failed to load memories');
      setTraces([]);
    } finally {
      setSearching(false);
    }
  };

  const handleSearch = async () => {
    if (!searchQuery.trim()) {
      loadMemoryPage(1);
      return;
    }

    setSearching(true);
    setSearchError(null);
    setHasSearched(true);
    setIsSearchMode(true);

    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/memory/retrieve`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          collection: '',
          query: searchQuery,
          k: 20
        })
      });

      if (!response.ok) {
        throw new Error(`Search failed: ${response.statusText}`);
      }

      const data = await response.json();
      const results: MemoryTrace[] = (data.results || []).map((r: any) => ({
        id: r.id,
        collection: r.collection || 'default',
        content: r.content || 'No content',
        embedding_preview: [],
        timestamp: r.timestamp || 0,
        metadata: r.metadata || {},
        similarity_score: r.score || 0,
      }));

      setTraces(results);
      setTotalMemories(results.length);
      setCurrentPage(1);
      setTotalPages(1);
    } catch (err) {
      console.error("Search error:", err);
      setSearchError(err instanceof Error ? err.message : 'Search failed');
      setTraces([]);
    } finally {
      setSearching(false);
    }
  };

  const handleClearSearch = () => {
    setSearchQuery('');
    setIsSearchMode(false);
    loadMemoryPage(1);
  };

  const formatTimestamp = (ts: number) => {
    if (!ts || ts === 0) return '';
    const d = new Date(ts * 1000);
    const now = new Date();
    const diffMs = now.getTime() - d.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return 'just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays < 7) return `${diffDays}d ago`;
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
  };

  return (
    <div className="space-y-5">
      {/* Search */}
      <div className="rounded-2xl p-4" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
        <div className="flex gap-3">
          <div className="flex-1 relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4" style={{ color: '#9ca3af' }} />
            <input
              type="text"
              placeholder="Search memories semantically..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
              className="w-full pl-10 pr-4 py-2 rounded-lg text-[13px]"
              style={{ backgroundColor: '#f8fafc', border: '1px solid #e5e7eb', color: '#111827', outline: 'none' }}
            />
          </div>
          <button
            onClick={handleSearch}
            disabled={searching}
            className="px-4 py-2 rounded-lg flex items-center gap-2 text-[13px] font-semibold transition-all"
            style={{ backgroundColor: '#0080FF', color: '#ffffff', opacity: searching ? 0.5 : 1 }}
          >
            {searching ? <RefreshCw className="w-4 h-4 animate-spin" /> : <Search className="w-4 h-4" />}
            Search
          </button>
          <button
            onClick={handleClearSearch}
            disabled={searching}
            className="px-4 py-2 rounded-lg flex items-center gap-2 text-[13px] font-semibold transition-all"
            style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb', color: '#374151', opacity: searching ? 0.5 : 1 }}
            title="Load all stored memories"
          >
            <Database className="w-4 h-4" />
            Browse All
          </button>
        </div>
      </div>

      {/* Stats Bar */}
      <div className="grid grid-cols-4 gap-4">
        <div className="rounded-xl p-4" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
          <div className="text-[22px] font-bold" style={{ color: '#111827' }}>{vectorCount.toLocaleString()}</div>
          <div className="text-[12px] font-medium" style={{ color: '#9ca3af' }}>Total Memories</div>
        </div>
        <div className="rounded-xl p-4" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
          <div className="text-[22px] font-bold" style={{ color: '#111827' }}>{mcpInfo?.collections.length || 1}</div>
          <div className="text-[12px] font-medium" style={{ color: '#9ca3af' }}>Collections</div>
        </div>
        <div className="rounded-xl p-4" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
          <div className="text-[22px] font-bold" style={{ color: '#111827' }}>384</div>
          <div className="text-[12px] font-medium" style={{ color: '#9ca3af' }}>Dimensions</div>
        </div>
        <div className="rounded-xl p-4" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
          <div className="text-[22px] font-bold" style={{ color: '#111827' }}>HNSW</div>
          <div className="text-[12px] font-medium" style={{ color: '#9ca3af' }}>Index Type</div>
        </div>
      </div>

      {/* Traces List */}
      {searching ? (
        <div className="rounded-2xl p-8 text-center" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
          <RefreshCw className="w-8 h-8 mx-auto mb-3 animate-spin" style={{ color: '#0080FF' }} />
          <p className="text-[14px]" style={{ color: '#6b7280' }}>Loading memories...</p>
        </div>
      ) : traces.length > 0 ? (
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="font-bold text-[14px]" style={{ color: '#111827' }}>
              {isSearchMode ? `Search Results for "${searchQuery}"` : 'Stored Memories'} ({totalMemories})
              {isSearchMode && (
                <button onClick={handleClearSearch} className="ml-2 text-[12px] font-medium" style={{ color: '#0080FF' }}>✕ Clear</button>
              )}
            </h3>
            {!isSearchMode && totalPages > 1 && (
              <span className="text-[12px]" style={{ color: '#9ca3af' }}>Page {currentPage} of {totalPages}</span>
            )}
          </div>
          {traces.map((trace) => {
            const metaEntries = Object.entries(trace.metadata || {}).filter(
              ([k]) => k !== 'type'
            );
            const kind = (trace.metadata?.kind || trace.metadata?.type || '') as string;
            const project = (trace.metadata?.project || '') as string;
            const source = (trace.metadata?.source || trace.metadata?.origin || '') as string;
            const when = (trace.metadata?.when || '') as string;
            const tags = (trace.metadata?.tags || '') as string;
            const extraMeta = metaEntries.filter(
              ([k]) => !['kind', 'type', 'project', 'source', 'origin', 'when', 'tags'].includes(k)
            );

            return (
              <div key={trace.id} className="rounded-xl p-4 transition-all" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
                {/* Header Row */}
                <div className="flex items-start justify-between mb-3">
                  <div className="flex items-center gap-3">
                    <div className="w-9 h-9 rounded-lg flex items-center justify-center flex-shrink-0" style={{ backgroundColor: 'rgba(0,128,255,0.08)' }}>
                      <Database className="w-4 h-4" style={{ color: '#0080FF' }} />
                    </div>
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="px-2 py-0.5 rounded-md text-[11px] font-semibold" style={{ backgroundColor: 'rgba(0,128,255,0.08)', color: '#0080FF' }}>
                          {trace.collection}
                        </span>
                        {kind && (
                          <span className="px-2 py-0.5 rounded-md text-[11px] font-semibold" style={{ backgroundColor: 'rgba(139,92,246,0.08)', color: '#8b5cf6' }}>
                            {kind}
                          </span>
                        )}
                        {project && (
                          <span className="px-2 py-0.5 rounded-md text-[11px] font-semibold" style={{ backgroundColor: 'rgba(6,182,212,0.08)', color: '#06b6d4' }}>
                            {project}
                          </span>
                        )}
                      </div>
                      <div className="text-[11px] mt-1 font-mono" style={{ color: '#9ca3af' }}>
                        {trace.id}
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    {trace.similarity_score !== undefined && trace.similarity_score > 0 && (
                      <div className="px-2 py-1 rounded text-[11px] font-semibold" style={
                        trace.similarity_score < 0.7
                          ? { backgroundColor: 'rgba(16,185,129,0.08)', color: '#10b981' }
                          : trace.similarity_score < 0.85
                            ? { backgroundColor: 'rgba(245,158,11,0.08)', color: '#f59e0b' }
                            : { backgroundColor: 'rgba(239,68,68,0.08)', color: '#ef4444' }
                      }>
                        dist: {trace.similarity_score.toFixed(3)}
                      </div>
                    )}
                    {trace.timestamp > 0 && (
                      <span className="text-[11px]" style={{ color: '#9ca3af' }}>
                        {formatTimestamp(trace.timestamp)}
                      </span>
                    )}
                  </div>
                </div>

                {/* Content */}
                <div className="rounded-xl p-3 mb-3" style={{ backgroundColor: '#f8fafc', border: '1px solid #f1f5f9' }}>
                  <p className="text-[13px] whitespace-pre-wrap leading-relaxed" style={{ color: '#374151' }}>{trace.content}</p>
                </div>

                {/* Metadata Row */}
                <div className="flex flex-wrap items-center gap-3 text-[11px]">
                  {source && (
                    <span className="flex items-center gap-1" style={{ color: '#9ca3af' }}>
                      <Zap className="w-3 h-3" />
                      {source}
                    </span>
                  )}
                  {when && (
                    <span className="flex items-center gap-1" style={{ color: '#9ca3af' }}>
                      <Activity className="w-3 h-3" />
                      {when}
                    </span>
                  )}
                  {tags && (
                    <div className="flex items-center gap-1">
                      {tags.split(',').map((tag, i) => (
                        <span key={i} className="px-1.5 py-0.5 rounded text-[10px]" style={{ backgroundColor: '#f8fafc', border: '1px solid #e5e7eb', color: '#9ca3af' }}>
                          {tag.trim()}
                        </span>
                      ))}
                    </div>
                  )}
                  {extraMeta.length > 0 && (
                    <div className="flex items-center gap-2">
                      {extraMeta.map(([key, value]) => (
                        <span key={key} className="text-[11px]" style={{ color: '#9ca3af' }}>
                          <span style={{ color: '#d1d5db' }}>{key}:</span> {String(value)}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            );
          })}

          {/* Pagination Controls */}
          {
            !isSearchMode && totalPages > 1 && (
              <div className="flex items-center justify-center gap-2 pt-4">
                <button
                  onClick={() => loadMemoryPage(1)}
                  disabled={currentPage === 1 || searching}
                  className="px-3 py-1.5 text-[12px] font-medium rounded-lg disabled:opacity-30 transition-all"
                  style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb', color: '#374151' }}
                >
                  First
                </button>
                <button
                  onClick={() => loadMemoryPage(currentPage - 1)}
                  disabled={currentPage === 1 || searching}
                  className="px-3 py-1.5 text-[12px] font-medium rounded-lg disabled:opacity-30 transition-all"
                  style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb', color: '#374151' }}
                >
                  ← Prev
                </button>
                <span className="px-4 py-1.5 text-[12px]" style={{ color: '#9ca3af' }}>
                  {currentPage} / {totalPages}
                </span>
                <button
                  onClick={() => loadMemoryPage(currentPage + 1)}
                  disabled={currentPage === totalPages || searching}
                  className="px-3 py-1.5 text-[12px] font-medium rounded-lg disabled:opacity-30 transition-all"
                  style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb', color: '#374151' }}
                >
