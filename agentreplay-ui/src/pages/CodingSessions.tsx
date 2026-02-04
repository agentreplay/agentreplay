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

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  Code,
  Clock,
  Filter,
  Loader2,
  RefreshCcw,
  Search,
  ChevronDown,
  ChevronRight,
  Trash2,
  FileEdit,
  Terminal,
  Eye,
  FolderOpen,
  GitBranch,
  Zap,
} from 'lucide-react';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
import { useProjects } from '../context/project-context';
import { cn } from '../../lib/utils';
import { formatDistanceToNow, format } from 'date-fns';
import Tooltip from '../components/Tooltip';

// Types for coding sessions
interface CodingSession {
  session_id: string;
  agent: string;
  agent_name: string;
  working_directory: string;
  git_repo?: string;
  git_branch?: string;
  start_time_us: number;
  end_time_us?: number;
  state: string;
  total_tokens: number;
  total_cost_cents: number;
  observation_count: number;
  file_reads: number;
  file_edits: number;
  bash_commands: number;
  duration_seconds: number;
  summary?: SessionSummary;
}

interface SessionSummary {
  title: string;
  description: string;
  accomplishments: string[];
  files_modified: string[];
  files_read: string[];
  concepts: string[];
  decisions: string[];
  follow_ups: string[];
  generated_at_us: number;
}

interface CodingObservation {
  observation_id: string;
  session_id: string;
  timestamp_us: number;
  sequence: number;
  action: string;
  tool_name: string;
  file_path?: string;
  directory?: string;
  command?: string;
  exit_code?: number;
  search_query?: string;
  input_content?: string;
  output_content?: string;
  duration_ms: number;
  tokens_used: number;
  cost_cents: number;
  success: boolean;
  error?: string;
  line_range?: [number, number];
  lines_changed?: number;
}

const agentColors: Record<string, string> = {
  'claude-code': 'bg-orange-500/15 text-orange-600',
  'cursor': 'bg-purple-500/15 text-purple-600',
  'copilot': 'bg-blue-500/15 text-blue-600',
  'continue': 'bg-green-500/15 text-green-600',
  'windsurf': 'bg-cyan-500/15 text-cyan-600',
  'aider': 'bg-yellow-500/15 text-yellow-600',
  'cline': 'bg-pink-500/15 text-pink-600',
  'other': 'bg-gray-500/15 text-gray-600',
};

const actionIcons: Record<string, React.ReactNode> = {
  read: <Eye className="w-3 h-3" />,
  edit: <FileEdit className="w-3 h-3" />,
  create: <FileEdit className="w-3 h-3" />,
  delete: <Trash2 className="w-3 h-3" />,
  bash: <Terminal className="w-3 h-3" />,
  search: <Search className="w-3 h-3" />,
  list_dir: <FolderOpen className="w-3 h-3" />,
  git: <GitBranch className="w-3 h-3" />,
  other: <Zap className="w-3 h-3" />,
};

export default function CodingSessions() {
  const navigate = useNavigate();
  const { sessionId } = useParams<{ sessionId?: string }>();
  const { currentProject } = useProjects();
  
  const [sessions, setSessions] = useState<CodingSession[]>([]);
  const [selectedSession, setSelectedSession] = useState<CodingSession | null>(null);
  const [observations, setObservations] = useState<CodingObservation[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingObservations, setLoadingObservations] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [agentFilter, setAgentFilter] = useState<string>('');
  const [showFilters, setShowFilters] = useState(false);
  
  // Fetch sessions
  const fetchSessions = useCallback(async () => {
    if (!currentProject) return;
    
    setLoading(true);
    setError(null);
    
    try {
      const params = new URLSearchParams();
      params.append('project_id', currentProject.project_id);
      params.append('limit', '50');
      if (agentFilter) {
        params.append('agent', agentFilter);
      }
      
      const response = await fetch(`http://127.0.0.1:47100/api/v1/coding-sessions?${params}`);
      if (!response.ok) {
        throw new Error(`Failed to fetch sessions: ${response.statusText}`);
      }
      
      const data = await response.json();
      setSessions(data.sessions || []);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch sessions');
    } finally {
      setLoading(false);
    }
  }, [currentProject, agentFilter]);
  
  // Fetch observations for a session
  const fetchObservations = useCallback(async (sessionId: string) => {
    setLoadingObservations(true);
    
    try {
      const response = await fetch(`http://127.0.0.1:47100/api/v1/coding-sessions/${sessionId}/observations?limit=200`);
      if (!response.ok) {
        throw new Error(`Failed to fetch observations: ${response.statusText}`);
      }
      
      const data = await response.json();
      setObservations(data.observations || []);
    } catch (err) {
      console.error('Failed to fetch observations:', err);
    } finally {
      setLoadingObservations(false);
    }
  }, []);
  
  // Effect to fetch sessions on mount and when project changes
  useEffect(() => {
    fetchSessions();
  }, [fetchSessions]);
  
  // Effect to handle session selection from URL
  useEffect(() => {
    if (sessionId && sessions.length > 0) {
      const session = sessions.find(s => s.session_id === sessionId);
      if (session) {
        setSelectedSession(session);
        fetchObservations(sessionId);
      }
    }
  }, [sessionId, sessions, fetchObservations]);
  
  // Handle session click
  const handleSessionClick = (session: CodingSession) => {
    setSelectedSession(session);
    fetchObservations(session.session_id);
    navigate(`/coding-sessions/${session.session_id}`);
  };
  
  // Handle delete session
  const handleDeleteSession = async (sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm('Are you sure you want to delete this session and all its observations?')) {
      return;
    }
    
    try {
      const response = await fetch(`http://127.0.0.1:47100/api/v1/coding-sessions/${sessionId}`, {
        method: 'DELETE',
      });
      
      if (!response.ok) {
        throw new Error(`Failed to delete session: ${response.statusText}`);
      }
      
      // Refresh sessions list
      fetchSessions();
      
      // Clear selection if deleted session was selected
      if (selectedSession?.session_id === sessionId) {
        setSelectedSession(null);
        setObservations([]);
        navigate('/coding-sessions');
      }
    } catch (err) {
      console.error('Failed to delete session:', err);
    }
  };
  
  // Format duration
  const formatDuration = (seconds: number): string => {
    if (seconds < 60) {
      return `${Math.round(seconds)}s`;
    } else if (seconds < 3600) {
      return `${Math.round(seconds / 60)}m`;
    } else {
      const hours = Math.floor(seconds / 3600);
      const mins = Math.round((seconds % 3600) / 60);
      return `${hours}h ${mins}m`;
    }
  };
  
  // Filtered sessions
  const filteredSessions = useMemo(() => {
    return sessions.filter(session => {
      if (searchQuery) {
        const query = searchQuery.toLowerCase();
        return (
          session.working_directory.toLowerCase().includes(query) ||
          session.agent_name.toLowerCase().includes(query) ||
          session.git_branch?.toLowerCase().includes(query) ||
          session.summary?.title.toLowerCase().includes(query)
        );
      }
      return true;
    });
  }, [sessions, searchQuery]);
  
  // Unique agents for filter
  const uniqueAgents = useMemo(() => {
    return [...new Set(sessions.map(s => s.agent))];
  }, [sessions]);
  
  return (
    <div className="flex h-full">
      {/* Sessions List */}
      <div className="w-80 border-r border-border flex flex-col bg-background">
        {/* Header */}
        <div className="p-4 border-b border-border">
          <div className="flex items-center justify-between mb-3">
            <h1 className="text-lg font-semibold flex items-center gap-2">
              <Code className="w-5 h-5 text-primary" />
              Coding Sessions
            </h1>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => fetchSessions()}
              disabled={loading}
            >
              <RefreshCcw className={cn("w-4 h-4", loading && "animate-spin")} />
            </Button>
          </div>
          
          {/* Search */}
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
            <Input
              placeholder="Search sessions..."
              className="pl-9"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
          </div>
          
          {/* Filters */}
          <div className="mt-2">
            <Button
              variant="ghost"
              size="sm"
              className="w-full justify-start text-xs"
              onClick={() => setShowFilters(!showFilters)}
            >
              <Filter className="w-3 h-3 mr-1" />
              Filters
              {showFilters ? <ChevronDown className="w-3 h-3 ml-auto" /> : <ChevronRight className="w-3 h-3 ml-auto" />}
            </Button>
            
            {showFilters && (
              <div className="mt-2 space-y-2">
                <select
                  className="w-full px-2 py-1 text-xs border rounded bg-background"
                  value={agentFilter}
                  onChange={(e) => setAgentFilter(e.target.value)}
                >
                  <option value="">All agents</option>
                  {uniqueAgents.map(agent => (
                    <option key={agent} value={agent}>{agent}</option>
                  ))}
                </select>
              </div>
            )}
          </div>
        </div>
        
        {/* Sessions List */}
        <div className="flex-1 overflow-y-auto">
          {loading ? (
            <div className="flex items-center justify-center h-32">
              <Loader2 className="w-6 h-6 animate-spin text-muted-foreground" />
            </div>
          ) : error ? (
            <div className="p-4 text-center text-error text-sm">{error}</div>
          ) : filteredSessions.length === 0 ? (
            <div className="p-4 text-center text-muted-foreground text-sm">
              <Code className="w-8 h-8 mx-auto mb-2 opacity-50" />
              <p>No coding sessions yet</p>
              <p className="text-xs mt-1">Sessions will appear when you use a coding agent</p>
            </div>
          ) : (
            <div className="divide-y divide-border">
              {filteredSessions.map(session => (
                <div
                  key={session.session_id}
                  className={cn(
                    "p-3 cursor-pointer hover:bg-accent transition-colors group",
                    selectedSession?.session_id === session.session_id && "bg-accent"
                  )}
                  onClick={() => handleSessionClick(session)}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="flex-1 min-w-0">
                      {/* Agent badge */}
                      <span className={cn(
                        "inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium mb-1",
                        agentColors[session.agent] || agentColors.other
                      )}>
                        {session.agent_name}
                      </span>
                      
                      {/* Directory */}
                      <p className="text-sm font-medium truncate" title={session.working_directory}>
                        {session.working_directory.split('/').pop() || session.working_directory}
                      </p>
                      
                      {/* Git branch */}
                      {session.git_branch && (
                        <p className="text-xs text-muted-foreground flex items-center gap-1 mt-0.5">
                          <GitBranch className="w-3 h-3" />
                          {session.git_branch}
                        </p>
                      )}
                      
                      {/* Stats */}
                      <div className="flex items-center gap-2 mt-1 text-[10px] text-muted-foreground">
                        <span>{session.observation_count} obs</span>
                        <span>•</span>
                        <span>{session.file_edits} edits</span>
                        <span>•</span>
                        <span>{formatDuration(session.duration_seconds)}</span>
                      </div>
                    </div>
                    
                    {/* Actions */}
                    <div className="flex flex-col items-end gap-1">
                      <span className="text-[10px] text-muted-foreground">
                        {formatDistanceToNow(new Date(session.start_time_us / 1000), { addSuffix: true })}
                      </span>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="opacity-0 group-hover:opacity-100 h-6 w-6 p-0"
                        onClick={(e) => handleDeleteSession(session.session_id, e)}
                      >
                        <Trash2 className="w-3 h-3 text-error" />
                      </Button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
      
      {/* Session Detail */}
      <div className="flex-1 overflow-hidden bg-background/50">
        {selectedSession ? (
          <div className="h-full flex flex-col">
            {/* Session Header */}
            <div className="p-4 border-b border-border bg-background">
              <div className="flex items-center justify-between mb-2">
                <div>
                  <span className={cn(
                    "inline-flex items-center px-2 py-0.5 rounded text-xs font-medium",
                    agentColors[selectedSession.agent] || agentColors.other
                  )}>
                    {selectedSession.agent_name}
                  </span>
                  <h2 className="text-lg font-semibold mt-1">
                    {selectedSession.working_directory}
                  </h2>
                </div>
                <div className="text-right text-sm text-muted-foreground">
                  <p>{format(new Date(selectedSession.start_time_us / 1000), 'PPp')}</p>
                  <p className="text-xs">{formatDuration(selectedSession.duration_seconds)} duration</p>
                </div>
              </div>
              
              {/* Stats cards */}
              <div className="grid grid-cols-4 gap-4 mt-4">
                <div className="bg-background border rounded-lg p-3">
                  <p className="text-xs text-muted-foreground">Observations</p>
                  <p className="text-2xl font-bold">{selectedSession.observation_count}</p>
                </div>
                <div className="bg-background border rounded-lg p-3">
                  <p className="text-xs text-muted-foreground">File Edits</p>
                  <p className="text-2xl font-bold">{selectedSession.file_edits}</p>
                </div>
                <div className="bg-background border rounded-lg p-3">
                  <p className="text-xs text-muted-foreground">File Reads</p>
                  <p className="text-2xl font-bold">{selectedSession.file_reads}</p>
                </div>
                <div className="bg-background border rounded-lg p-3">
                  <p className="text-xs text-muted-foreground">Bash Commands</p>
                  <p className="text-2xl font-bold">{selectedSession.bash_commands}</p>
                </div>
              </div>
            </div>
            
            {/* Observations Timeline */}
            <div className="flex-1 overflow-y-auto p-4">
              <h3 className="text-sm font-medium mb-3 flex items-center gap-2">
                <Clock className="w-4 h-4" />
                Activity Timeline
              </h3>
              
              {loadingObservations ? (
                <div className="flex items-center justify-center h-32">
                  <Loader2 className="w-6 h-6 animate-spin text-muted-foreground" />
                </div>
              ) : observations.length === 0 ? (
                <div className="text-center text-muted-foreground text-sm py-8">
                  No observations recorded
                </div>
              ) : (
                <div className="space-y-2">
                  {observations.map((obs) => (
                    <div
                      key={obs.observation_id}
                      className={cn(
                        "flex items-start gap-3 p-3 rounded-lg border bg-background",
                        !obs.success && "border-error/30 bg-error/5"
                      )}
                    >
                      {/* Action icon */}
                      <div className={cn(
                        "w-8 h-8 rounded-full flex items-center justify-center",
                        obs.action === 'edit' || obs.action === 'create' ? 'bg-blue-500/10 text-blue-600' :
                        obs.action === 'read' ? 'bg-green-500/10 text-green-600' :
                        obs.action === 'bash' ? 'bg-orange-500/10 text-orange-600' :
                        obs.action === 'delete' ? 'bg-red-500/10 text-red-600' :
                        'bg-gray-500/10 text-gray-600'
                      )}>
                        {actionIcons[obs.action] || actionIcons.other}
                      </div>
                      
                      {/* Content */}
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="font-medium text-sm capitalize">{obs.action}</span>
                          {obs.tool_name !== obs.action && (
                            <span className="text-xs text-muted-foreground">({obs.tool_name})</span>
                          )}
                          <span className="text-xs text-muted-foreground ml-auto">
                            {obs.duration_ms}ms
                          </span>
                        </div>
                        
                        {/* File path */}
                        {obs.file_path && (
                          <p className="text-xs text-muted-foreground font-mono truncate mt-0.5">
                            {obs.file_path}
                            {obs.line_range && (
                              <span className="ml-1 text-primary">
                                L{obs.line_range[0]}-{obs.line_range[1]}
                              </span>
                            )}
                          </p>
                        )}
                        
                        {/* Command */}
                        {obs.command && (
                          <pre className="text-xs bg-muted/50 p-1.5 rounded mt-1 font-mono truncate">
                            {obs.command}
                          </pre>
                        )}
                        
                        {/* Search query */}
                        {obs.search_query && (
                          <p className="text-xs text-muted-foreground mt-0.5">
                            Query: "{obs.search_query}"
                          </p>
                        )}
                        
                        {/* Error */}
                        {obs.error && (
                          <p className="text-xs text-error mt-1">{obs.error}</p>
                        )}
                        
                        {/* Exit code for bash */}
                        {obs.exit_code !== null && obs.exit_code !== undefined && obs.exit_code !== 0 && (
                          <p className="text-xs text-error mt-0.5">Exit code: {obs.exit_code}</p>
                        )}
                      </div>
                      
                      {/* Sequence number */}
                      <div className="text-[10px] text-muted-foreground">
                        #{obs.sequence + 1}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-center h-full text-muted-foreground">
            <div className="text-center">
              <Code className="w-12 h-12 mx-auto mb-3 opacity-50" />
              <p>Select a session to view details</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
