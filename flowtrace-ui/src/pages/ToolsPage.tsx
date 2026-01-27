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

import { useState, useEffect, useCallback } from 'react';
import { useParams } from 'react-router-dom';
import { 
  Wrench, 
  Plus, 
  Search, 
  Play, 
  RefreshCw,
  Server,
  Globe,
  Code,
  Zap,
  Clock,
  CheckCircle,
  XCircle,
  ChevronRight,
  Settings,
  Trash2,
  Copy,
  X
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { motion, AnimatePresence } from 'framer-motion';
import { formatDistanceToNow } from 'date-fns';
import { flowtraceClient, ToolInfo, McpServer, ToolExecution } from '../lib/flowtrace-api';
import { VideoHelpButton } from '../components/VideoHelpButton';

type ToolKind = 'mcp' | 'rest' | 'native' | 'mock';
type ToolStatus = 'active' | 'inactive' | 'error';

export default function ToolsPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [mcpServers, setMcpServers] = useState<McpServer[]>([]);
  const [selectedTool, setSelectedTool] = useState<ToolInfo | null>(null);
  const [activeTab, setActiveTab] = useState<'registry' | 'executions' | 'mcp'>('registry');
  const [searchQuery, setSearchQuery] = useState('');
  const [filterKind, setFilterKind] = useState<ToolKind | null>(null);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);

  const fetchTools = useCallback(async () => {
    try {
      setLoading(true);
      const response = await flowtraceClient.listTools();
      setTools(response.tools || []);
    } catch (error) {
      console.error('Failed to fetch tools:', error);
    } finally {
      setLoading(false);
    }
  }, []);

  const fetchMcpServers = useCallback(async () => {
    try {
      const response = await flowtraceClient.listMcpServers();
      setMcpServers(response.servers || []);
    } catch (error) {
      console.error('Failed to fetch MCP servers:', error);
    }
  }, []);

  useEffect(() => {
    fetchTools();
    fetchMcpServers();
  }, [fetchTools, fetchMcpServers]);

  const handleSyncMcp = async () => {
    setSyncing(true);
    try {
      for (const server of mcpServers) {
        await flowtraceClient.syncMcpServer(server.id);
      }
      await fetchTools();
      await fetchMcpServers();
    } catch (error) {
      console.error('Failed to sync MCP:', error);
    } finally {
      setSyncing(false);
    }
  };

  const filteredTools = tools.filter(tool => {
    const matchesSearch = 
      tool.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      tool.description?.toLowerCase().includes(searchQuery.toLowerCase());
    const matchesKind = !filterKind || tool.kind === filterKind;
    return matchesSearch && matchesKind;
  });

  return (
    <div className="min-h-screen bg-background">
      <div className="flex h-[calc(100vh-3.5rem)]">
        {/* Main Content */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Header */}
          <div className="border-b border-border bg-surface px-6 py-4">
            <div className="flex items-center justify-between mb-4">
              <div>
                <h1 className="text-2xl font-bold text-textPrimary flex items-center gap-2">
                  <Wrench className="w-6 h-6 text-primary" />
                  Tool Registry
                </h1>
                <p className="text-textSecondary text-sm mt-1">
                  Manage MCP servers, REST APIs, and native tools
                </p>
              </div>
              <div className="flex items-center gap-3">
                <VideoHelpButton pageId="tools" />
                <Button variant="outline" size="sm" onClick={handleSyncMcp} disabled={syncing}>
                  <RefreshCw className={`w-4 h-4 mr-2 ${syncing ? 'animate-spin' : ''}`} />
                  Sync MCP
                </Button>
                <Button size="sm">
                  <Plus className="w-4 h-4 mr-2" />
                  Register Tool
                </Button>
              </div>
            </div>

            {/* Tabs - Memory style with solid blue background */}
            <div className="flex gap-1 bg-surface border border-border rounded-lg p-1">
              <button
                onClick={() => setActiveTab('registry')}
                className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                  activeTab === 'registry'
                    ? 'bg-primary text-white'
                    : 'text-textSecondary hover:text-textPrimary hover:bg-background'
                }`}
              >
                <Wrench className="w-4 h-4" />
                Registry
                <span className={`ml-1 px-2 py-0.5 text-xs rounded-full ${
                  activeTab === 'registry' 
                    ? 'bg-white/20 text-white' 
                    : 'bg-primary/20 text-primary'
                }`}>
                  {tools.length}
                </span>
              </button>
              <button
                onClick={() => setActiveTab('executions')}
                className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                  activeTab === 'executions'
                    ? 'bg-primary text-white'
                    : 'text-textSecondary hover:text-textPrimary hover:bg-background'
                }`}
              >
                <Zap className="w-4 h-4" />
                Executions
              </button>
              <button
                onClick={() => setActiveTab('mcp')}
                className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                  activeTab === 'mcp'
                    ? 'bg-primary text-white'
                    : 'text-textSecondary hover:text-textPrimary hover:bg-background'
                }`}
              >
                <Server className="w-4 h-4" />
                MCP Servers
                <span className={`ml-1 px-2 py-0.5 text-xs rounded-full ${
                  activeTab === 'mcp' 
                    ? 'bg-white/20 text-white' 
                    : 'bg-primary/20 text-primary'
                }`}>
                  {mcpServers.length}
                </span>
              </button>
            </div>
          </div>

          {/* Search & Filters */}
          {activeTab === 'registry' && (
            <div className="px-6 py-4 border-b border-border bg-background">
              <div className="flex items-center gap-4">
                <div className="relative flex-1 max-w-md">
                  <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-textTertiary" />
                  <Input
                    placeholder="Search tools..."
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    className="pl-10"
                  />
                </div>
                <div className="flex items-center gap-2">
                  {(['all', 'mcp', 'rest', 'native', 'mock'] as const).map((kind) => (
                    <button
                      key={kind}
                      onClick={() => setFilterKind(kind === 'all' ? null : kind as ToolKind)}
                      className={`px-3 py-1.5 rounded-full text-xs font-medium transition-colors ${
                        (kind === 'all' && !filterKind) || filterKind === kind
                          ? 'bg-primary text-white'
                          : 'bg-surface-elevated text-textSecondary hover:bg-surface-hover'
                      }`}
                    >
                      {kind === 'all' ? 'All' : kind.toUpperCase()}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* Content */}
          <div className="flex-1 overflow-auto p-6">
            {activeTab === 'registry' && (
              loading ? (
                <div className="flex items-center justify-center h-64">
                  <RefreshCw className="w-8 h-8 animate-spin text-primary" />
                </div>
              ) : filteredTools.length === 0 ? (
                <EmptyToolsState onRegister={() => {}} />
              ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                  <AnimatePresence>
                    {filteredTools.map((tool) => (
                      <ToolCard
                        key={tool.id}
                        tool={tool}
                        onClick={() => setSelectedTool(tool)}
                        isSelected={selectedTool?.id === tool.id}
                      />
                    ))}
                  </AnimatePresence>
                </div>
              )
            )}

            {activeTab === 'mcp' && (
              <MCPServerManager 
                servers={mcpServers} 
                onRefresh={fetchMcpServers}
              />
            )}

            {activeTab === 'executions' && (
              <ExecutionsView tools={tools} />
            )}
          </div>
        </div>

        {/* Detail Panel */}
        <AnimatePresence>
          {selectedTool && (
            <ToolDetailPanel
              tool={selectedTool}
              onClose={() => setSelectedTool(null)}
            />
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}

// Empty State
function EmptyToolsState({ onRegister }: { onRegister: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center h-64 text-center">
      <div className="p-4 bg-surface-elevated rounded-full mb-4">
        <Wrench className="w-8 h-8 text-textTertiary" />
      </div>
      <h3 className="text-lg font-semibold text-textPrimary mb-2">No tools registered</h3>
      <p className="text-textSecondary mb-4 max-w-md">
        Register tools to enable agent capabilities. Connect MCP servers or register REST/native tools.
      </p>
      <Button onClick={onRegister}>
        <Plus className="w-4 h-4 mr-2" />
        Register Your First Tool
      </Button>
    </div>
  );
}

// Tool Card Component
function ToolCard({ tool, onClick, isSelected }: { 
  tool: ToolInfo; 
  onClick: () => void;
  isSelected: boolean;
}) {
  const kindColors: Record<ToolKind, string> = {
    mcp: 'bg-purple-500/10 text-purple-500 border-purple-500/20',
    rest: 'bg-blue-500/10 text-blue-500 border-blue-500/20',
    native: 'bg-green-500/10 text-green-500 border-green-500/20',
    mock: 'bg-amber-500/10 text-amber-500 border-amber-500/20',
  };

  const kindIcons: Record<ToolKind, typeof Server> = {
    mcp: Server,
    rest: Globe,
    native: Code,
    mock: Zap,
  };

  const KindIcon = kindIcons[tool.kind];

  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.95 }}
      onClick={onClick}
      className={`
        relative p-4 rounded-lg border cursor-pointer transition-all
        ${isSelected 
          ? 'border-primary bg-primary/5 shadow-lg shadow-primary/10' 
          : 'border-border bg-surface hover:border-primary/50 hover:shadow-md'
        }
      `}
    >
      {/* Status Indicator */}
      <div className="absolute top-3 right-3">
        {tool.status === 'active' && (
          <span className="flex items-center gap-1 text-xs text-green-500">
            <span className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
            Active
          </span>
        )}
        {tool.status === 'inactive' && (
          <span className="flex items-center gap-1 text-xs text-textTertiary">
            <span className="w-2 h-2 bg-gray-400 rounded-full" />
            Inactive
          </span>
        )}
        {tool.status === 'error' && (
          <span className="flex items-center gap-1 text-xs text-red-500">
            <XCircle className="w-3 h-3" />
            Error
          </span>
        )}
      </div>

      {/* Tool Info */}
      <div className="flex items-start gap-3">
        <div className={`p-2 rounded-lg ${kindColors[tool.kind]}`}>
          <KindIcon className="w-5 h-5" />
        </div>
        <div className="flex-1 min-w-0">
          <h3 className="font-semibold text-textPrimary truncate">{tool.name}</h3>
          <p className="text-xs text-textTertiary">v{tool.version}</p>
        </div>
      </div>

      <p className="text-sm text-textSecondary mt-3 line-clamp-2">
        {tool.description || 'No description available'}
      </p>

      {/* Stats */}
      <div className="flex items-center gap-4 mt-4 pt-4 border-t border-border">
        <div className="flex items-center gap-1 text-xs text-textTertiary">
          <Zap className="w-3 h-3" />
          <span>{tool.execution_count.toLocaleString()}</span>
        </div>
        <div className="flex items-center gap-1 text-xs text-textTertiary">
          <Clock className="w-3 h-3" />
          <span>{Math.round(tool.avg_latency_ms)}ms</span>
        </div>
        <div className="flex items-center gap-1 text-xs">
          <CheckCircle className="w-3 h-3 text-green-500" />
          <span className="text-green-500">{(tool.success_rate * 100).toFixed(1)}%</span>
        </div>
      </div>
    </motion.div>
  );
}

// Tool Detail Panel
function ToolDetailPanel({ tool, onClose }: { tool: ToolInfo; onClose: () => void }) {
  const [activeTab, setActiveTab] = useState<'overview' | 'schema' | 'test' | 'history'>('overview');
  const [executions, setExecutions] = useState<ToolExecution[]>([]);
  const [testInput, setTestInput] = useState('{}');
  const [testResult, setTestResult] = useState<any>(null);
  const [testing, setTesting] = useState(false);

  useEffect(() => {
    const fetchExecutions = async () => {
      try {
        const response = await flowtraceClient.getToolExecutions(tool.id, 20);
        setExecutions(response.executions || []);
      } catch (error) {
        console.error('Failed to fetch executions:', error);
      }
    };
    fetchExecutions();
  }, [tool.id]);

  const handleExecute = async () => {
    setTesting(true);
    try {
      const input = JSON.parse(testInput);
      const result = await flowtraceClient.executeTool(tool.id, input);
      setTestResult(result);
    } catch (error: any) {
      setTestResult({ error: error.message });
    } finally {
      setTesting(false);
    }
  };

  return (
    <motion.div
      initial={{ x: '100%' }}
      animate={{ x: 0 }}
      exit={{ x: '100%' }}
      transition={{ type: 'spring', damping: 25, stiffness: 200 }}
      className="w-[480px] border-l border-border bg-surface flex flex-col"
    >
      {/* Header */}
      <div className="px-6 py-4 border-b border-border bg-gradient-to-r from-primary/10 to-transparent">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-primary/10 rounded-lg">
              <Wrench className="w-5 h-5 text-primary" />
            </div>
            <div>
              <h2 className="font-semibold text-textPrimary">{tool.name}</h2>
              <p className="text-xs text-textTertiary">v{tool.version} • {tool.kind.toUpperCase()}</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-surface-hover rounded-lg transition-colors"
          >
            <X className="w-5 h-5 text-textTertiary" />
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="border-b border-border">
        <div className="flex">
          {[
            { id: 'overview', label: 'Overview', icon: Settings },
            { id: 'schema', label: 'Schema', icon: Code },
            { id: 'test', label: 'Test', icon: Play },
            { id: 'history', label: 'History', icon: Clock },
          ].map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id as typeof activeTab)}
              className={`
                flex items-center gap-2 px-4 py-3 text-sm font-medium border-b-2 transition-colors
                ${activeTab === tab.id
                  ? 'border-primary text-primary'
                  : 'border-transparent text-textSecondary hover:text-textPrimary'
                }
              `}
            >
              <tab.icon className="w-4 h-4" />
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        {activeTab === 'overview' && (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-textSecondary mb-2">Description</label>
              <p className="text-textPrimary">{tool.description || 'No description'}</p>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="p-3 bg-surface-elevated rounded-lg">
                <p className="text-xs text-textTertiary mb-1">Executions</p>
                <p className="text-lg font-semibold text-textPrimary">{tool.execution_count.toLocaleString()}</p>
              </div>
              <div className="p-3 bg-surface-elevated rounded-lg">
                <p className="text-xs text-textTertiary mb-1">Avg Latency</p>
                <p className="text-lg font-semibold text-textPrimary">{Math.round(tool.avg_latency_ms)}ms</p>
              </div>
              <div className="p-3 bg-surface-elevated rounded-lg">
                <p className="text-xs text-textTertiary mb-1">Success Rate</p>
                <p className="text-lg font-semibold text-green-500">{(tool.success_rate * 100).toFixed(1)}%</p>
              </div>
              <div className="p-3 bg-surface-elevated rounded-lg">
                <p className="text-xs text-textTertiary mb-1">Last Executed</p>
                <p className="text-sm font-medium text-textPrimary">
                  {tool.last_executed 
                    ? formatDistanceToNow(tool.last_executed, { addSuffix: true })
                    : 'Never'
                  }
                </p>
              </div>
            </div>
            {tool.rate_limit && (
              <div className="p-4 bg-amber-500/10 border border-amber-500/20 rounded-lg">
                <p className="text-sm font-medium text-amber-500">Rate Limited</p>
                <p className="text-xs text-amber-400">
                  {tool.rate_limit.max_requests} requests per {tool.rate_limit.window_seconds}s
                </p>
              </div>
            )}
          </div>
        )}

        {activeTab === 'schema' && (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-textSecondary mb-2">Input Schema</label>
              <pre className="p-4 bg-background border border-border rounded-lg text-sm font-mono overflow-auto max-h-48">
                {tool.input_schema 
                  ? JSON.stringify(tool.input_schema, null, 2) 
                  : 'No input schema defined'
                }
              </pre>
            </div>
            <div>
              <label className="block text-sm font-medium text-textSecondary mb-2">Output Schema</label>
              <pre className="p-4 bg-background border border-border rounded-lg text-sm font-mono overflow-auto max-h-48">
                {tool.output_schema 
                  ? JSON.stringify(tool.output_schema, null, 2) 
                  : 'No output schema defined'
                }
              </pre>
            </div>
          </div>
        )}

        {activeTab === 'test' && (
          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-textSecondary mb-2">
                Input Arguments (JSON)
              </label>
              <textarea
                value={testInput}
                onChange={(e) => setTestInput(e.target.value)}
                className="w-full h-40 px-3 py-2 bg-background border border-border rounded-lg font-mono text-sm text-textPrimary focus:outline-none focus:ring-2 focus:ring-primary resize-none"
                placeholder='{"key": "value"}'
              />
            </div>

            <Button
              onClick={handleExecute}
              disabled={testing}
              className="w-full"
            >
              {testing ? (
                <>
                  <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                  Executing...
                </>
              ) : (
                <>
                  <Play className="w-4 h-4 mr-2" />
                  Execute Tool
                </>
              )}
            </Button>

            {testResult && (
              <div className="mt-4">
                <label className="block text-sm font-medium text-textSecondary mb-2">Result</label>
                <pre className={`
                  p-4 rounded-lg text-sm font-mono overflow-auto max-h-60
                  ${testResult.error 
                    ? 'bg-red-500/10 border border-red-500/20 text-red-400'
                    : 'bg-green-500/10 border border-green-500/20 text-green-400'
                  }
                `}>
                  {JSON.stringify(testResult, null, 2)}
                </pre>
              </div>
            )}
          </div>
        )}

        {activeTab === 'history' && (
          <div className="space-y-2">
            {executions.length === 0 ? (
              <p className="text-textSecondary text-center py-8">No execution history</p>
            ) : (
              executions.map((exec) => (
                <div 
                  key={exec.id}
                  className="p-3 bg-surface-elevated rounded-lg border border-border"
                >
                  <div className="flex items-center justify-between mb-2">
                    <span className={`flex items-center gap-1 text-xs ${
                      exec.success ? 'text-green-500' : 'text-red-500'
                    }`}>
                      {exec.success ? <CheckCircle className="w-3 h-3" /> : <XCircle className="w-3 h-3" />}
                      {exec.success ? 'Success' : 'Failed'}
                    </span>
                    <span className="text-xs text-textTertiary">
                      {formatDistanceToNow(exec.executed_at, { addSuffix: true })}
                    </span>
                  </div>
                  <p className="text-xs text-textTertiary">
                    Latency: {exec.latency_ms}ms
                  </p>
                  {exec.error && (
                    <p className="text-xs text-red-400 mt-1">{exec.error}</p>
                  )}
                </div>
              ))
            )}
          </div>
        )}
      </div>
    </motion.div>
  );
}

// MCP Server Manager
function MCPServerManager({ servers, onRefresh }: { servers: McpServer[]; onRefresh: () => void }) {
  const [connecting, setConnecting] = useState(false);

  return (
    <div className="space-y-6">
      {/* Connected Servers */}
      <div className="grid gap-4">
        {servers.map((server) => (
          <div
            key={server.id}
            className="p-4 bg-surface rounded-lg border border-border hover:border-primary/50 transition-all cursor-pointer"
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className={`
                  p-2 rounded-lg
                  ${server.status === 'connected' 
                    ? 'bg-green-500/10' 
                    : 'bg-red-500/10'
                  }
                `}>
                  <Server className={`w-5 h-5 ${
                    server.status === 'connected' 
                      ? 'text-green-500' 
                      : 'text-red-500'
                  }`} />
                </div>
                <div>
                  <h3 className="font-medium text-textPrimary">{server.name}</h3>
                  <p className="text-xs text-textTertiary">{server.uri}</p>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <span className={`
                  px-2 py-1 rounded-full text-xs font-medium
                  ${server.status === 'connected'
                    ? 'bg-green-500/10 text-green-500'
                    : 'bg-red-500/10 text-red-500'
                  }
                `}>
                  {server.status}
                </span>
                <span className="text-xs text-textTertiary">
                  {server.tool_count} tools
                </span>
              </div>
            </div>

            {/* Tools Preview */}
            {server.tools.length > 0 && (
              <div className="mt-4 flex flex-wrap gap-2">
                {server.tools.slice(0, 5).map((tool) => (
                  <span
                    key={tool.name}
                    className="px-2 py-1 bg-surface-elevated rounded text-xs text-textSecondary"
                  >
                    {tool.name}
                  </span>
                ))}
                {server.tools.length > 5 && (
                  <span className="px-2 py-1 text-xs text-textTertiary">
                    +{server.tools.length - 5} more
                  </span>
                )}
              </div>
            )}

            {server.error && (
              <p className="mt-2 text-xs text-red-400">{server.error}</p>
            )}
          </div>
        ))}
      </div>

      {/* Empty State */}
      {servers.length === 0 && (
        <div className="text-center py-12">
          <Server className="w-12 h-12 mx-auto text-textTertiary mb-4" />
          <h3 className="text-lg font-medium text-textPrimary mb-2">No MCP Servers Connected</h3>
          <p className="text-textSecondary mb-4">Connect to an MCP server to import tools</p>
        </div>
      )}

      {/* Add Server */}
      <button 
        onClick={() => {}}
        className="w-full p-4 border-2 border-dashed border-border rounded-lg text-textSecondary hover:border-primary hover:text-primary transition-colors flex items-center justify-center gap-2"
      >
        <Plus className="w-5 h-5" />
        Connect MCP Server
      </button>
    </div>
  );
}

// Executions View
function ExecutionsView({ tools }: { tools: ToolInfo[] }) {
  const [executions, setExecutions] = useState<ToolExecution[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchAllExecutions = async () => {
      setLoading(true);
      try {
        const allExecutions: ToolExecution[] = [];
        for (const tool of tools.slice(0, 10)) {
          const response = await flowtraceClient.getToolExecutions(tool.id, 10);
          allExecutions.push(...(response.executions || []));
        }
        // Sort by timestamp descending
        allExecutions.sort((a, b) => b.executed_at - a.executed_at);
        setExecutions(allExecutions.slice(0, 50));
      } catch (error) {
        console.error('Failed to fetch executions:', error);
      } finally {
        setLoading(false);
      }
    };

    if (tools.length > 0) {
      fetchAllExecutions();
    } else {
      setLoading(false);
    }
  }, [tools]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <RefreshCw className="w-8 h-8 animate-spin text-primary" />
      </div>
    );
  }

  if (executions.length === 0) {
    return (
      <div className="text-center py-12">
        <Zap className="w-12 h-12 mx-auto text-textTertiary mb-4" />
        <h3 className="text-lg font-medium text-textPrimary mb-2">No Executions Yet</h3>
        <p className="text-textSecondary">Tool executions will appear here</p>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {executions.map((exec) => {
        const tool = tools.find(t => t.id === exec.tool_id);
        return (
          <div 
            key={exec.id}
            className="p-4 bg-surface rounded-lg border border-border hover:border-primary/30 transition-all"
          >
            <div className="flex items-center justify-between mb-2">
              <div className="flex items-center gap-2">
                <span className="font-medium text-textPrimary">{tool?.name || 'Unknown Tool'}</span>
                <span className={`flex items-center gap-1 text-xs ${
                  exec.success ? 'text-green-500' : 'text-red-500'
                }`}>
                  {exec.success ? <CheckCircle className="w-3 h-3" /> : <XCircle className="w-3 h-3" />}
                  {exec.success ? 'Success' : 'Failed'}
                </span>
              </div>
              <span className="text-xs text-textTertiary">
                {formatDistanceToNow(exec.executed_at, { addSuffix: true })}
              </span>
            </div>
            <div className="flex items-center gap-4 text-xs text-textTertiary">
              <span>Latency: {exec.latency_ms}ms</span>
              {exec.trace_id && (
                <span>Trace: {exec.trace_id.substring(0, 8)}</span>
              )}
            </div>
            {exec.error && (
              <p className="mt-2 text-xs text-red-400">{exec.error}</p>
            )}
          </div>
        );
      })}
    </div>
  );
}
