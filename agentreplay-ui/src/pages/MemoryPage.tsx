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
    <div className="container mx-auto p-6 max-w-6xl">
      {/* Header */}
      <div className="mb-6">
        <div className="flex items-center justify-between mb-2">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-purple-500/20 to-blue-500/20 flex items-center justify-center">
              <Database className="w-5 h-5 text-purple-500" />
            </div>
            <div>
              <h1 className="text-2xl font-bold text-textPrimary">Memory</h1>
              <p className="text-sm text-textSecondary">Vector storage and semantic memory for AI agents</p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={() => setShowDocs(!showDocs)}
              className={`flex items-center gap-2 px-3 py-1.5 text-sm rounded-lg transition-colors ${showDocs
                  ? 'bg-primary text-white'
                  : 'bg-surface border border-border text-textSecondary hover:text-textPrimary'
                }`}
            >
              <BookOpen className="w-4 h-4" />
              {showDocs ? 'Hide Docs' : 'View Docs'}
            </button>
            <VideoHelpButton pageId="memory" />
          </div>
        </div>
      </div>

      {/* Docs Panel (collapsible) */}
      {showDocs && (
        <div className="mb-6 bg-surface border border-border rounded-xl p-4">
          <div className="flex items-center justify-between mb-3">
            <h3 className="font-medium text-textPrimary flex items-center gap-2">
              <BookOpen className="w-4 h-4 text-primary" />
              Quick Start
            </h3>
            <button onClick={() => setShowDocs(false)} className="text-textTertiary hover:text-textPrimary">
              ✕
            </button>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm">
            <div className="bg-background rounded-lg p-3">
              <p className="text-xs text-textTertiary mb-2">Ingest memory (REST API):</p>
              <pre className="text-xs text-primary font-mono overflow-x-auto">{`curl -X POST http://localhost:47100/api/v1/memory/ingest \\\n  -H "Content-Type: application/json" \\\n  -d '{"collection": "docs", "content": "Your content"}'`}</pre>
            </div>
            <div className="bg-background rounded-lg p-3">
              <p className="text-xs text-textTertiary mb-2">Search memories:</p>
              <pre className="text-xs text-primary font-mono overflow-x-auto">{`curl -X POST http://localhost:47100/api/v1/memory/retrieve \\\n  -H "Content-Type: application/json" \\\n  -d '{"query": "your search", "k": 5}'`}</pre>
            </div>
          </div>
        </div>
      )}

      {/* Tabs */}
      <div className="flex gap-1 mb-6 bg-surface border border-border rounded-lg p-1">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-colors ${activeTab === tab.id
              ? 'bg-primary text-white'
              : 'text-textSecondary hover:text-textPrimary hover:bg-background'
              }`}
          >
            <tab.icon className="w-4 h-4" />
            {tab.label}
          </button>
        ))}
        <div className="flex-1" />
        <button
          onClick={fetchMCPInfo}
          className="p-2 hover:bg-background rounded-md transition-colors"
          title="Refresh"
        >
          <RefreshCw className={`w-4 h-4 text-textSecondary ${loading ? 'animate-spin' : ''}`} />
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
          <Database className="w-5 h-5 text-blue-400" />
        </div>
        <div className="flex-1">
          <h3 className="font-medium text-blue-400 mb-1">Database Location</h3>
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
            <Layers className="w-4 h-4 text-purple-400" />
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
              <Zap className="w-4 h-4 text-green-400" />
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
              <Server className="w-4 h-4 text-blue-400" />
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
    <div className="space-y-6">
      {/* Search */}
      <div className="bg-surface border border-border rounded-xl p-4">
        <div className="flex gap-3">
          <div className="flex-1 relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-textTertiary" />
            <input
              type="text"
              placeholder="Search memories semantically..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
              className="w-full pl-10 pr-4 py-2 bg-background border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:ring-2 focus:ring-primary/50"
            />
          </div>
          <button
            onClick={handleSearch}
            disabled={searching}
            className="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-hover disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
          >
            {searching ? <RefreshCw className="w-4 h-4 animate-spin" /> : <Search className="w-4 h-4" />}
            Search
          </button>
          <button
            onClick={handleClearSearch}
            disabled={searching}
            className="px-4 py-2 bg-surface border border-border text-textPrimary rounded-lg hover:bg-background disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
            title="Load all stored memories"
          >
            <Database className="w-4 h-4" />
            Browse All
          </button>
        </div>
      </div>

      {/* Stats Bar */}
      <div className="grid grid-cols-4 gap-4">
        <div className="bg-surface border border-border rounded-lg p-4">
          <div className="text-2xl font-bold text-textPrimary">{vectorCount.toLocaleString()}</div>
          <div className="text-sm text-textSecondary">Total Memories</div>
        </div>
        <div className="bg-surface border border-border rounded-lg p-4">
          <div className="text-2xl font-bold text-textPrimary">{mcpInfo?.collections.length || 1}</div>
          <div className="text-sm text-textSecondary">Collections</div>
        </div>
        <div className="bg-surface border border-border rounded-lg p-4">
          <div className="text-2xl font-bold text-textPrimary">384</div>
          <div className="text-sm text-textSecondary">Dimensions</div>
        </div>
        <div className="bg-surface border border-border rounded-lg p-4">
          <div className="text-2xl font-bold text-textPrimary">HNSW</div>
          <div className="text-sm text-textSecondary">Index Type</div>
        </div>
      </div>

      {/* Traces List */}
      {searching ? (
        <div className="bg-surface border border-border rounded-xl p-8 text-center">
          <RefreshCw className="w-8 h-8 text-primary mx-auto mb-3 animate-spin" />
          <p className="text-textSecondary">Loading memories...</p>
        </div>
      ) : traces.length > 0 ? (
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="font-medium text-textPrimary">
              {isSearchMode ? `Search Results for "${searchQuery}"` : 'Stored Memories'} ({totalMemories})
              {isSearchMode && (
                <button onClick={handleClearSearch} className="ml-2 text-xs text-primary hover:underline">✕ Clear</button>
              )}
            </h3>
            {!isSearchMode && totalPages > 1 && (
              <span className="text-xs text-textTertiary">Page {currentPage} of {totalPages}</span>
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
              <div key={trace.id} className="bg-surface border border-border rounded-lg p-4 hover:border-primary/30 transition-colors">
                {/* Header Row */}
                <div className="flex items-start justify-between mb-3">
                  <div className="flex items-center gap-3">
                    <div className="w-9 h-9 rounded-lg bg-primary/10 flex items-center justify-center flex-shrink-0">
                      <Database className="w-4 h-4 text-primary" />
                    </div>
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="px-2 py-0.5 rounded-md bg-blue-500/10 text-blue-400 text-xs font-medium">
                          {trace.collection}
                        </span>
                        {kind && (
                          <span className="px-2 py-0.5 rounded-md bg-purple-500/10 text-purple-400 text-xs font-medium">
                            {kind}
                          </span>
                        )}
                        {project && (
                          <span className="px-2 py-0.5 rounded-md bg-cyan-500/10 text-cyan-400 text-xs font-medium">
                            {project}
                          </span>
                        )}
                      </div>
                      <div className="text-xs text-textTertiary mt-1 font-mono">
                        {trace.id}
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    {trace.similarity_score !== undefined && trace.similarity_score > 0 && (
                      <div className={`px-2 py-1 rounded text-xs font-medium ${
                        trace.similarity_score < 0.7
                          ? 'bg-green-500/10 text-green-400'
                          : trace.similarity_score < 0.85
                            ? 'bg-yellow-500/10 text-yellow-400'
                            : 'bg-red-500/10 text-red-400'
                      }`}>
                        dist: {trace.similarity_score.toFixed(3)}
                      </div>
                    )}
                    {trace.timestamp > 0 && (
                      <span className="text-xs text-textTertiary">
                        {formatTimestamp(trace.timestamp)}
                      </span>
                    )}
                  </div>
                </div>

                {/* Content */}
                <div className="bg-background rounded-lg p-3 mb-3">
                  <p className="text-textSecondary text-sm whitespace-pre-wrap leading-relaxed">{trace.content}</p>
                </div>

                {/* Metadata Row */}
                <div className="flex flex-wrap items-center gap-3 text-xs">
                  {source && (
                    <span className="flex items-center gap-1 text-textTertiary">
                      <Zap className="w-3 h-3" />
                      {source}
                    </span>
                  )}
                  {when && (
                    <span className="flex items-center gap-1 text-textTertiary">
                      <Activity className="w-3 h-3" />
                      {when}
                    </span>
                  )}
                  {tags && (
                    <div className="flex items-center gap-1">
                      {tags.split(',').map((tag, i) => (
                        <span key={i} className="px-1.5 py-0.5 rounded bg-surface border border-border text-textTertiary text-[10px]">
                          {tag.trim()}
                        </span>
                      ))}
                    </div>
                  )}
                  {extraMeta.length > 0 && (
                    <div className="flex items-center gap-2">
                      {extraMeta.map(([key, value]) => (
                        <span key={key} className="text-textTertiary">
                          <span className="text-textTertiary/60">{key}:</span> {String(value)}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            );
          })}

          {/* Pagination Controls */}
          {!isSearchMode && totalPages > 1 && (
            <div className="flex items-center justify-center gap-2 pt-4">
              <button
                onClick={() => loadMemoryPage(1)}
                disabled={currentPage === 1 || searching}
                className="px-3 py-1.5 text-xs bg-surface border border-border rounded-lg disabled:opacity-30 hover:bg-background"
              >
                First
              </button>
              <button
                onClick={() => loadMemoryPage(currentPage - 1)}
                disabled={currentPage === 1 || searching}
                className="px-3 py-1.5 text-xs bg-surface border border-border rounded-lg disabled:opacity-30 hover:bg-background"
              >
                ← Prev
              </button>
              <span className="px-4 py-1.5 text-xs text-textSecondary">
                {currentPage} / {totalPages}
              </span>
              <button
                onClick={() => loadMemoryPage(currentPage + 1)}
                disabled={currentPage === totalPages || searching}
                className="px-3 py-1.5 text-xs bg-surface border border-border rounded-lg disabled:opacity-30 hover:bg-background"
              >
                Next →
              </button>
              <button
                onClick={() => loadMemoryPage(totalPages)}
                disabled={currentPage === totalPages || searching}
                className="px-3 py-1.5 text-xs bg-surface border border-border rounded-lg disabled:opacity-30 hover:bg-background"
              >
                Last
              </button>
            </div>
          )}
        </div>
      ) : vectorCount === 0 ? (
        <div className="bg-surface border border-border rounded-xl p-12 text-center">
          <div className="w-16 h-16 rounded-2xl bg-primary/10 flex items-center justify-center mx-auto mb-4">
            <Database className="w-8 h-8 text-primary" />
          </div>
          <h3 className="text-lg font-semibold text-textPrimary mb-2">No memories stored yet</h3>
          <p className="text-textSecondary mb-4 max-w-md mx-auto">
            Start by ingesting documents using the Memory API. Memories will appear here for browsing and searching.
          </p>

          <div className="bg-background rounded-lg p-4 max-w-lg mx-auto text-left">
            <p className="text-xs text-textTertiary mb-2">Ingest your first memory:</p>
            <pre className="text-xs text-primary font-mono overflow-x-auto whitespace-pre-wrap">
              {`curl -X POST http://localhost:47100/api/v1/memory/ingest \\
  -H "Content-Type: application/json" \\
  -d '{"collection": "default", "content": "Your content here"}'`}
            </pre>
          </div>
        </div>
      ) : searchError ? (
        <div className="bg-surface border border-error/20 rounded-xl p-8 text-center">
          <AlertCircle className="w-12 h-12 text-error mx-auto mb-3" />
          <p className="text-error mb-2">{searchError}</p>
          <button
            onClick={() => loadMemoryPage(1)}
            className="text-sm text-primary hover:underline"
          >
            Try loading memories again
          </button>
        </div>
      ) : hasSearched && searchQuery ? (
        <div className="bg-surface border border-border rounded-xl p-8 text-center">
          <Search className="w-12 h-12 text-textTertiary mx-auto mb-3" />
          <p className="text-textSecondary">No memories match "{searchQuery}"</p>
          <button
            onClick={() => loadMemoryPage(1)}
            className="mt-2 text-sm text-primary hover:underline"
          >
            Browse all memories instead
          </button>
        </div>
      ) : (
        <div className="bg-surface border border-border rounded-xl p-8 text-center">
          <div className="w-12 h-12 rounded-xl bg-green-500/10 flex items-center justify-center mx-auto mb-3">
            <Database className="w-6 h-6 text-green-500" />
          </div>
          <p className="text-textPrimary font-medium mb-1">{vectorCount} memories stored</p>
          <p className="text-textSecondary text-sm mb-4">Search semantically or browse all</p>
          <button
            onClick={() => loadMemoryPage(1)}
            className="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-hover"
          >
            Browse All Memories
          </button>
        </div>
      )}
    </div>
  );
}


// ============================================================================
// Knowledge Graph Tab
// ============================================================================
function KnowledgeGraphTab({ mcpInfo }: { mcpInfo: MCPInfoResponse | null }) {
  const vectorCount = mcpInfo?.project.vector_count || 0;

  // Show empty state if no vectors exist
  if (vectorCount === 0) {
    return (
      <div className="space-y-6">
        {/* Graph Stats - Empty */}
        <div className="grid grid-cols-4 gap-4">
          <div className="bg-surface border border-border rounded-lg p-4">
            <div className="text-2xl font-bold text-textTertiary">0</div>
            <div className="text-sm text-textSecondary">Entities</div>
          </div>
          <div className="bg-surface border border-border rounded-lg p-4">
            <div className="text-2xl font-bold text-textTertiary">0</div>
            <div className="text-sm text-textSecondary">Relations</div>
          </div>
          <div className="bg-surface border border-border rounded-lg p-4">
            <div className="text-2xl font-bold text-textTertiary">0</div>
            <div className="text-sm text-textSecondary">Triples</div>
          </div>
          <div className="bg-surface border border-border rounded-lg p-4">
            <div className="text-2xl font-bold text-textTertiary">—</div>
            <div className="text-sm text-textSecondary">Avg Connectivity</div>
          </div>
        </div>

        {/* Empty State */}
        <div className="bg-surface border border-border rounded-xl p-12">
          <div className="text-center">
            <Network className="w-16 h-16 text-textTertiary mx-auto mb-4 opacity-50" />
            <h3 className="text-lg font-medium text-textPrimary mb-2">No Knowledge Graph Yet</h3>
            <p className="text-textSecondary mb-4 max-w-md mx-auto">
              The knowledge graph will be built automatically from your memory traces.
              Ingest documents using the Memory API to see entities and relationships here.
            </p>

            <div className="bg-background rounded-lg p-4 max-w-lg mx-auto text-left">
              <p className="text-xs text-textTertiary mb-2">Ingest a document to start building the graph:</p>
              <pre className="text-xs text-primary font-mono overflow-x-auto">
                {`curl -X POST http://localhost:47100/api/v1/memory/ingest \\
  -H "Content-Type: application/json" \\
  -d '{"collection": "docs", "content": "Your content here"}'`}
              </pre>
            </div>
          </div>
        </div>
      </div>
    );
  }

  // Real data would come from an API - for now show placeholder with actual counts
  return (
    <div className="space-y-6">
      {/* Graph Stats */}
      <div className="grid grid-cols-4 gap-4">
        <div className="bg-surface border border-border rounded-lg p-4">
          <div className="text-2xl font-bold text-textPrimary">{vectorCount}</div>
          <div className="text-sm text-textSecondary">Vectors</div>
        </div>
        <div className="bg-surface border border-border rounded-lg p-4">
          <div className="text-2xl font-bold text-textTertiary">—</div>
          <div className="text-sm text-textSecondary">Entities</div>
        </div>
        <div className="bg-surface border border-border rounded-lg p-4">
          <div className="text-2xl font-bold text-textTertiary">—</div>
          <div className="text-sm text-textSecondary">Relations</div>
        </div>
        <div className="bg-surface border border-border rounded-lg p-4">
          <div className="text-2xl font-bold text-textTertiary">—</div>
          <div className="text-sm text-textSecondary">Triples</div>
        </div>
      </div>

      {/* Coming Soon */}
      <div className="bg-surface border border-border rounded-xl p-12">
        <div className="text-center">
          <Network className="w-16 h-16 text-primary mx-auto mb-4 opacity-70" />
          <h3 className="text-lg font-medium text-textPrimary mb-2">Knowledge Graph Visualization</h3>
          <p className="text-textSecondary mb-4 max-w-md mx-auto">
            Knowledge graph extraction from memory traces is coming soon.
            The graph will visualize entities, relationships, and semantic connections from your data.
          </p>
          <div className="inline-flex items-center gap-2 px-4 py-2 bg-primary/10 text-primary rounded-lg text-sm">
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-primary opacity-75"></span>
              <span className="relative inline-flex rounded-full h-2 w-2 bg-primary"></span>
            </span>
            {vectorCount} vectors ready for graph extraction
          </div>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// MCP Server Tab
// ============================================================================
function MCPServerTab({
  mcpInfo,
  loading,
  error,
  formatDate
}: {
  mcpInfo: MCPInfoResponse | null;
  loading: boolean;
  error: string | null;
  formatDate: (ts: number) => string;
}) {
  const [pingStatus, setPingStatus] = useState<'idle' | 'testing' | 'success' | 'error'>('idle');
  const [pingLatency, setPingLatency] = useState<number | null>(null);
  const [pingError, setPingError] = useState<string | null>(null);

  const testMcpPing = async () => {
    setPingStatus('testing');
    setPingError(null);
    const startTime = performance.now();

    try {
      const { mcpCall } = await import('../lib/mcpClient');
      await mcpCall('ping');

      const endTime = performance.now();
      const latency = Math.round(endTime - startTime);

      setPingStatus('success');
      setPingLatency(latency);
    } catch (err) {
      setPingStatus('error');
      setPingError(err instanceof Error ? err.message : 'Unknown error');
      setPingLatency(null);
    }
  };

  return (
    <div className="space-y-6">
      {/* Server Status */}
      <div className="bg-surface border border-border rounded-xl p-6">
        <div className="flex items-center justify-between mb-4">
          <h3 className="font-medium text-textPrimary flex items-center gap-2">
            <Server className="w-5 h-5" />
            MCP Server Status
          </h3>
          <div className="flex items-center gap-3">
            {/* Ping Test Button */}
            <button
              onClick={testMcpPing}
              disabled={pingStatus === 'testing'}
              className="flex items-center gap-2 px-3 py-1.5 text-sm bg-primary/10 hover:bg-primary/20 text-primary rounded-lg transition-colors disabled:opacity-50"
            >
              {pingStatus === 'testing' ? (
                <>
                  <div className="w-3 h-3 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
                  Testing...
                </>
              ) : (
                <>
                  <Zap className="w-3 h-3" />
                  Test Ping (MCP)
                </>
              )}
            </button>
            {pingStatus === 'success' && (
              <span className="text-xs font-medium text-green-500 bg-green-500/10 px-2 py-1 rounded">
                Active ({pingLatency}ms)
              </span>
            )}
            {pingStatus === 'error' && (
              <span className="text-xs font-medium text-error bg-error/10 px-2 py-1 rounded" title={pingError || ''}>
                Failed
              </span>
            )}
          </div>
        </div>

        {mcpInfo?.status.initialized ? (
          <div className="flex items-center gap-2 text-green-500">
            <CheckCircle className="w-4 h-4" />
            <span className="text-sm font-medium">Running</span>
          </div>
        ) : (
          <div className="flex items-center gap-2 text-yellow-500">
            <AlertCircle className="w-4 h-4" />
            <span className="text-sm font-medium">Initializing</span>
          </div>
        )}

        {error && (
          <div className="mt-4 p-3 bg-yellow-500/10 border border-yellow-500/20 rounded-lg text-yellow-500 text-sm">
            Note: {error} - Restart the server to enable Memory API
          </div>
        )}

        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mt-4">
          <div className="bg-background rounded-lg p-4">
            <div className="text-xs text-textTertiary mb-1">Tenant ID</div>
            <div className="text-xl font-bold text-primary">{mcpInfo?.status.tenant_id || 2}</div>
          </div>
          <div className="bg-background rounded-lg p-4">
            <div className="text-xs text-textTertiary mb-1">Project ID</div>
            <div className="text-xl font-bold text-textPrimary">{mcpInfo?.status.project_id || 1000}</div>
          </div>
          <div className="bg-background rounded-lg p-4">
            <div className="text-xs text-textTertiary mb-1">Isolation Mode</div>
            <div className="text-xl font-bold text-textPrimary capitalize">{mcpInfo?.status.isolation_mode?.replace('_', ' ') || 'Tenant'}</div>
          </div>
          <div className="bg-background rounded-lg p-4">
            <div className="text-xs text-textTertiary mb-1">Port</div>
            <div className="text-xl font-bold text-textPrimary">47100</div>
          </div>
        </div>
      </div>

      {/* Performance Metrics */}
      <div className="bg-surface border border-border rounded-xl p-6">
        <h3 className="font-medium text-textPrimary mb-4 flex items-center gap-2">
          <BarChart3 className="w-5 h-5" />
          Performance
        </h3>
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Zap className="w-4 h-4 text-yellow-500" />
              <span className="text-textSecondary">Avg Query Time</span>
            </div>
            <span className="font-mono text-textPrimary">&lt;5ms</span>
          </div>
        </div>
      </div>

      {/* Project Info */}
      <div className="bg-surface border border-border rounded-xl p-6">
        <h3 className="font-medium text-textPrimary mb-4 flex items-center gap-2">
          <Database className="w-5 h-5" />
          Project Details
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm">
          <div>
            <span className="text-textTertiary">Name:</span>
            <span className="ml-2 text-textPrimary">{mcpInfo?.project.project_name || 'MCP Memory'}</span>
          </div>
          <div className="md:col-span-2">
            <span className="text-textTertiary">Description:</span>
            <span className="ml-2 text-textPrimary">{mcpInfo?.project.description || 'MCP Memory Project'}</span>
          </div>
        </div>
      </div>

      {/* API Endpoints */}
      <div className="bg-surface border border-border rounded-xl p-6">
        <h3 className="font-medium text-textPrimary mb-4">API Endpoints</h3>
        <div className="space-y-2">
          {[
            { method: 'GET', path: '/api/v1/memory/info', desc: 'Get MCP project info' },
            { method: 'POST', path: '/api/v1/memory/retrieve', desc: 'Semantic search' },
            { method: 'POST', path: '/mcp', desc: 'MCP JSON-RPC endpoint' },
          ].map((endpoint, i) => (
            <div key={i} className="flex items-center gap-3 p-2 hover:bg-background rounded-lg">
              <span className={`px-2 py-0.5 rounded text-xs font-mono font-bold bg-primary/10 text-primary`}>
                {endpoint.method}
              </span>
              <code className="text-sm text-textPrimary flex-1">{endpoint.path}</code>
              <span className="text-sm text-textTertiary">{endpoint.desc}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
