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

'use client';

import { useEffect, useState, useRef } from 'react';
import { GitBranch, Clock, Coins, ChevronRight, MessageSquare, Wrench, Bot, User, Settings, CheckCircle, AlertCircle, X } from 'lucide-react';
import { API_BASE_URL } from '../../src/lib/agentreplay-api';

interface GraphNode {
  node_id: string;
  span_id: string;
  label: string;
  span_type: string;
  duration_ms: number;
  start_offset_ms: number;
  tokens: number;
  cost: number;
  confidence?: number;
  status: string;
  content?: string;
  role?: string;
  model?: string;
  inputTokens?: number;
  outputTokens?: number;
  metadata?: Record<string, any>;
  position?: {
    x: number;
    y: number;
  };
}

interface GraphEdge {
  source: string;
  target: string;
  edge_type: string;
  label?: string;
}

interface GraphResponse {
  nodes: GraphNode[];
  edges: GraphEdge[];
  layout?: any;
}

// Helper to build graph from rich trace metadata
function buildGraphFromTraceMetadata(trace: any): GraphResponse {
  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];
  const metadata = trace.metadata || trace.attributes || {};
  
  // Parse prompts from gen_ai.prompt.N.* pattern
  const prompts: Array<{ index: number; role: string; content: string; toolCalls?: any[]; toolCallId?: string }> = [];
  const promptGroups: Record<number, any> = {};
  
  Object.entries(metadata).forEach(([key, value]) => {
    const match = key.match(/^gen_ai\.prompt\.(\d+)\.(role|content|tool_calls\..*|tool_call_id)$/);
    if (match) {
      const index = parseInt(match[1]);
      if (!promptGroups[index]) promptGroups[index] = { index };
      
      if (match[2] === 'role') promptGroups[index].role = value;
      else if (match[2] === 'content') promptGroups[index].content = value;
      else if (match[2] === 'tool_call_id') promptGroups[index].toolCallId = value;
      else if (match[2].startsWith('tool_calls.')) {
        if (!promptGroups[index].toolCalls) promptGroups[index].toolCalls = [];
        const toolMatch = match[2].match(/tool_calls\.(\d+)\.(.*)/);
        if (toolMatch) {
          const toolIndex = parseInt(toolMatch[1]);
          if (!promptGroups[index].toolCalls[toolIndex]) {
            promptGroups[index].toolCalls[toolIndex] = {};
          }
          promptGroups[index].toolCalls[toolIndex][toolMatch[2]] = value;
        }
      }
    }
  });
  
  // Sort by index
  Object.values(promptGroups).sort((a, b) => a.index - b.index).forEach(p => prompts.push(p as any));
  
  // Get completion
  const completion = {
    role: 'assistant',
    content: metadata['gen_ai.completion.0.content'] || trace.output || ''
  };
  
  // Get model info
  const modelName = metadata['gen_ai.request.model'] || metadata['gen_ai.response.model'] || trace.model || 'LLM';
  const inputTokens = parseInt(metadata['gen_ai.usage.input_tokens'] || '0');
  const outputTokens = parseInt(metadata['gen_ai.usage.output_tokens'] || '0');
  
  let prevNodeId: string | null = null;
  
  // Add root LLM call node at the top
  nodes.push({
    node_id: 'llm-root',
    span_id: 'llm-root',
    label: `chat ${modelName}`,
    span_type: 'llm',
    duration_ms: trace.duration_ms || 0,
    start_offset_ms: 0,
    tokens: inputTokens + outputTokens,
    inputTokens,
    outputTokens,
    cost: trace.cost || 0,
    status: trace.status || 'completed',
    role: 'llm',
    model: modelName,
    position: { x: 0, y: 0 }
  });
  prevNodeId = 'llm-root';
  
  // Add nodes for each prompt/message
  prompts.forEach((prompt, idx) => {
    const nodeId = `msg-${idx}`;
    
    // Parse content if it's JSON
    let displayContent = prompt.content || '';
    try {
      if (typeof displayContent === 'string' && displayContent.startsWith('[')) {
        const parsed = JSON.parse(displayContent);
        if (Array.isArray(parsed) && parsed[0]?.text) {
          displayContent = parsed[0].text;
        }
      }
    } catch { /* keep original */ }
    
    // Determine node type and label
    let nodeType = prompt.role || 'message';
    let label = '';
    
    if (prompt.role === 'system') {
      label = 'System Prompt';
      nodeType = 'system';
    } else if (prompt.role === 'user') {
      label = 'User Input';
      nodeType = 'user';
    } else if (prompt.role === 'tool') {
      label = 'Tool Result';
      nodeType = 'tool_result';
    } else if (prompt.toolCalls && prompt.toolCalls[0]?.name) {
      label = prompt.toolCalls[0].name;
      nodeType = 'tool_call';
    } else {
      label = 'Assistant';
      nodeType = 'assistant';
    }
    
    nodes.push({
      node_id: nodeId,
      span_id: nodeId,
      label,
      span_type: nodeType,
      duration_ms: 0,
      start_offset_ms: 0,
      tokens: 0,
      cost: 0,
      status: 'completed',
      content: displayContent?.substring(0, 80) + (displayContent?.length > 80 ? '...' : ''),
      role: prompt.role,
      metadata: prompt.toolCalls ? { 
        tool_calls: prompt.toolCalls,
        arguments: prompt.toolCalls[0]?.arguments 
      } : undefined,
      position: { x: 0, y: 0 }
    });
    
    if (prevNodeId) {
      edges.push({ source: prevNodeId, target: nodeId, edge_type: 'flow' });
    }
    prevNodeId = nodeId;
  });
  
  // Add final response node
  if (completion.content) {
    const nodeId = 'response';
    nodes.push({
      node_id: nodeId,
      span_id: nodeId,
      label: 'Final Response',
      span_type: 'response',
      duration_ms: 0,
      start_offset_ms: 0,
      tokens: outputTokens,
      outputTokens,
      cost: 0,
      status: 'completed',
      content: completion.content.substring(0, 80) + (completion.content.length > 80 ? '...' : ''),
      role: 'assistant',
      position: { x: 0, y: 0 }
    });
    
    if (prevNodeId) {
      edges.push({ source: prevNodeId, target: nodeId, edge_type: 'flow' });
    }
  }
  
  return { nodes, edges };
}

interface TraceGraphViewProps {
  traceId: string;
  tenantId?: number;
  projectId?: number;
  trace?: any;
  onNodeClick?: (node: GraphNode) => void;
}

// Clean, professional color palette for different node types
const nodeStyles: Record<string, { 
  bgClass: string; 
  borderClass: string; 
  iconBg: string;
  icon: React.ReactNode;
  label: string;
}> = {
  llm: { 
    bgClass: 'bg-blue-500/10', 
    borderClass: 'border-blue-500/30',
    iconBg: 'bg-blue-500',
    icon: <Bot className="w-4 h-4 text-white" />,
    label: 'LLM'
  },
  system: { 
    bgClass: 'bg-purple-500/10', 
    borderClass: 'border-purple-500/30',
    iconBg: 'bg-purple-500',
    icon: <Settings className="w-4 h-4 text-white" />,
    label: 'System'
  },
  user: { 
    bgClass: 'bg-sky-500/10', 
    borderClass: 'border-sky-500/30',
    iconBg: 'bg-sky-500',
    icon: <User className="w-4 h-4 text-white" />,
    label: 'User'
  },
  assistant: { 
    bgClass: 'bg-emerald-500/10', 
    borderClass: 'border-emerald-500/30',
    iconBg: 'bg-emerald-500',
    icon: <Bot className="w-4 h-4 text-white" />,
    label: 'Assistant'
  },
  tool_call: { 
    bgClass: 'bg-amber-500/10', 
    borderClass: 'border-amber-500/30',
    iconBg: 'bg-amber-500',
    icon: <Wrench className="w-4 h-4 text-white" />,
    label: 'Tool Call'
  },
  tool_result: { 
    bgClass: 'bg-orange-500/10', 
    borderClass: 'border-orange-500/30',
    iconBg: 'bg-orange-500',
    icon: <CheckCircle className="w-4 h-4 text-white" />,
    label: 'Tool Result'
  },
  response: { 
    bgClass: 'bg-green-500/10', 
    borderClass: 'border-green-500/30',
    iconBg: 'bg-green-500',
    icon: <MessageSquare className="w-4 h-4 text-white" />,
    label: 'Response'
  },
  message: { 
    bgClass: 'bg-muted', 
    borderClass: 'border-border',
    iconBg: 'bg-muted-foreground',
    icon: <MessageSquare className="w-4 h-4 text-white" />,
    label: 'Message'
  },
};

export function TraceGraphView({ traceId, tenantId = 1, projectId = 1, trace, onNodeClick }: TraceGraphViewProps) {
  const [graphData, setGraphData] = useState<GraphResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<GraphNode | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    async function fetchGraph() {
      setLoading(true);
      try {
        // If trace data is provided, build graph from its metadata first
        if (trace && trace.metadata) {
          const graphFromMetadata = buildGraphFromTraceMetadata(trace);
          if (graphFromMetadata.nodes.length > 0) {
            setGraphData(graphFromMetadata);
            setLoading(false);
            return;
          }
        }
        
        // Fetch observations
        const response = await fetch(`${API_BASE_URL}/api/v1/traces/${traceId}/observations`, {
          headers: {
            'X-Tenant-ID': tenantId.toString(),
            'X-Project-ID': projectId.toString(),
          },
        });
        
        if (!response.ok) {
          if (trace && trace.metadata) {
            const graphFromMetadata = buildGraphFromTraceMetadata(trace);
            if (graphFromMetadata.nodes.length > 0) {
              setGraphData(graphFromMetadata);
              setLoading(false);
              return;
            }
          }
          throw new Error('Failed to fetch graph data');
        }
        
        const observations = await response.json();
        
        if (!observations || observations.length === 0) {
          if (trace && trace.metadata) {
            const graphFromMetadata = buildGraphFromTraceMetadata(trace);
            setGraphData(graphFromMetadata);
            setLoading(false);
            return;
          }
        }
        
        // Build graph from observations
        const nodes: GraphNode[] = [];
        const edges: GraphEdge[] = [];
        
        observations.forEach((obs: any, index: number) => {
          const id = obs.id || obs.span_id || `node-${index}`;
          nodes.push({
            node_id: id,
            span_id: id,
            label: obs.name || obs.span_type || 'Unknown',
            span_type: obs.type || obs.span_type || 'span',
            duration_ms: obs.duration_ms || 0,
            start_offset_ms: 0,
            tokens: obs.usage?.total || obs.token_count || 0,
            cost: obs.cost || 0,
            status: obs.status || 'completed',
            position: { x: 0, y: 0 }
          });
          
          if (index > 0) {
            const prevObs = observations[index - 1];
            edges.push({
              source: prevObs.id || prevObs.span_id || `node-${index-1}`,
              target: id,
              edge_type: 'flow'
            });
          }
        });
        
        setGraphData({ nodes, edges });
      } catch (err) {
        if (trace && trace.metadata) {
          const graphFromMetadata = buildGraphFromTraceMetadata(trace);
          if (graphFromMetadata.nodes.length > 0) {
            setGraphData(graphFromMetadata);
            setLoading(false);
            return;
          }
        }
        setError(err instanceof Error ? err.message : 'Unknown error');
      } finally {
        setLoading(false);
      }
    }
    fetchGraph();
  }, [traceId, tenantId, projectId, trace]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full text-textSecondary">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary mr-3"></div>
        Loading graph...
      </div>
    );
  }
  
  if (error) {
    return (
      <div className="flex items-center justify-center h-full text-destructive">
        <AlertCircle className="w-5 h-5 mr-2" />
        Error: {error}
      </div>
    );
  }
  
  if (!graphData || graphData.nodes.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-textSecondary">
        <GitBranch className="w-12 h-12 mb-4 text-gray-300" />
        <p className="text-lg font-medium">No graph data available</p>
        <p className="text-sm">This trace doesn't have enough data to visualize</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full bg-gradient-to-b from-gray-50 to-white relative">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border bg-card">
        <div className="flex items-center gap-2">
          <GitBranch className="w-5 h-5 text-primary" />
          <h3 className="text-base font-semibold text-textPrimary">Execution Flow</h3>
          <span className="px-2 py-0.5 text-xs font-medium bg-primary/10 text-primary rounded-full">
            {graphData.nodes.length} steps
          </span>
        </div>
      </div>

      {/* Flow Visualization */}
      <div 
        ref={containerRef}
        className="flex-1 overflow-auto p-6"
      >
        <div className="flex flex-col items-center gap-0 min-w-max mx-auto max-w-lg">
          {graphData.nodes.map((node, index) => {
            const style = nodeStyles[node.span_type] || nodeStyles.message;
            const isSelected = selectedNode?.node_id === node.node_id;
            
            return (
              <div key={node.node_id} className="flex flex-col items-center w-full">
                {/* Connector line from previous node */}
                {index > 0 && (
                  <div className="flex flex-col items-center py-1">
                    <div className="w-0.5 h-4 bg-border"></div>
                    <div className="w-2 h-2 rounded-full bg-border"></div>
                    <div className="w-0.5 h-4 bg-border"></div>
                  </div>
                )}
                
                {/* Node Card */}
                <div 
                  className={`
                    w-full max-w-md rounded-xl border-2 shadow-sm cursor-pointer
                    transition-all duration-200 hover:shadow-lg hover:scale-[1.02]
                    ${style.bgClass} ${style.borderClass}
                    ${isSelected ? 'ring-2 ring-primary ring-offset-2 shadow-lg' : ''}
                  `}
                  onClick={() => {
                    setSelectedNode(node);
                    onNodeClick?.(node);
                  }}
                >
                  {/* Header */}
                  <div className="flex items-center gap-3 px-4 py-3">
                    <div className={`p-2 rounded-lg ${style.iconBg} shadow-sm`}>
                      {style.icon}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-[10px] font-bold text-muted-foreground uppercase tracking-wider">
                          {style.label}
                        </span>
                        {node.status === 'completed' && (
                          <CheckCircle className="w-3 h-3 text-green-500" />
                        )}
                        {node.status === 'error' && (
                          <AlertCircle className="w-3 h-3 text-red-500" />
                        )}
                      </div>
                      <p className="text-sm font-semibold text-gray-900 truncate mt-0.5">
                        {node.label}
                      </p>
                    </div>
                    
                    {/* Metrics */}
                    <div className="flex flex-col items-end gap-1 text-xs">
                      {node.duration_ms > 0 && (
                        <div className="flex items-center gap-1 text-muted-foreground bg-card/60 px-2 py-0.5 rounded">
                          <Clock className="w-3 h-3" />
                          <span className="font-mono font-medium">{node.duration_ms.toFixed(0)}ms</span>
                        </div>
                      )}
                      {(node.tokens > 0 || node.inputTokens || node.outputTokens) && (
                        <div className="flex items-center gap-1 text-muted-foreground bg-card/60 px-2 py-0.5 rounded">
                          <Coins className="w-3 h-3" />
                          <span className="font-mono font-medium">
                            {node.inputTokens && node.outputTokens 
                              ? `${node.inputTokens}→${node.outputTokens}`
                              : node.tokens}
                          </span>
                        </div>
                      )}
                    </div>
                  </div>
                  
                  {/* Content Preview */}
                  {node.content && (
                    <div className="px-4 py-3 border-t border-border/50">
                      <p className="text-sm text-foreground line-clamp-2 leading-relaxed">
                        {node.content}
                      </p>
                    </div>
                  )}
                  
                  {/* Tool Arguments */}
                  {node.metadata?.arguments && (
                    <div className="px-4 py-2 bg-card/50 border-t border-border/50 rounded-b-xl">
                      <div className="flex items-center gap-2">
                        <span className="text-[10px] font-semibold text-muted-foreground uppercase">Args:</span>
                        <code className="text-xs text-gray-600 font-mono truncate">
                          {node.metadata.arguments}
                        </code>
                      </div>
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Detail Panel - slides in from right when node is selected */}
      {selectedNode && (
        <div className="absolute right-0 top-0 bottom-0 w-96 bg-card border-l border-border shadow-xl overflow-auto">
          <div className="sticky top-0 bg-card border-b border-border px-4 py-3 flex items-center justify-between">
            <div className="flex items-center gap-2">
              {(() => {
                const style = nodeStyles[selectedNode.span_type] || nodeStyles.message;
                return (
                  <>
                    <div className={`p-1.5 rounded-lg ${style.iconBg}`}>
                      {style.icon}
                    </div>
                    <span className="font-semibold text-textPrimary">{selectedNode.label}</span>
                  </>
                );
              })()}
            </div>
            <button 
              onClick={() => setSelectedNode(null)}
              className="p-1 hover:bg-surface-hover rounded text-textSecondary"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
          
          <div className="p-4 space-y-4">
            {/* Type Badge */}
            <div className="flex items-center gap-2">
              <span className="text-xs font-semibold text-textTertiary uppercase">Type:</span>
              <span className={`px-2 py-0.5 text-xs font-medium rounded ${
                nodeStyles[selectedNode.span_type]?.bgClass || 'bg-muted'
              } ${nodeStyles[selectedNode.span_type]?.borderClass || 'border-border'} border`}>
                {nodeStyles[selectedNode.span_type]?.label || selectedNode.span_type}
              </span>
            </div>

            {/* Metrics */}
            {(selectedNode.duration_ms > 0 || selectedNode.tokens > 0) && (
              <div className="bg-background rounded-lg p-3 space-y-2 border border-border">
                <h4 className="text-xs font-semibold text-textTertiary uppercase">Metrics</h4>
                {selectedNode.duration_ms > 0 && (
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-textSecondary">Duration</span>
                    <span className="font-mono text-sm text-textPrimary">{selectedNode.duration_ms.toFixed(0)}ms</span>
                  </div>
                )}
                {(selectedNode.inputTokens || selectedNode.outputTokens) && (
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-textSecondary">Tokens</span>
                    <span className="font-mono text-sm text-textPrimary">
                      {selectedNode.inputTokens || 0} → {selectedNode.outputTokens || 0}
                    </span>
                  </div>
                )}
                {selectedNode.tokens > 0 && !selectedNode.inputTokens && (
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-textSecondary">Tokens</span>
                    <span className="font-mono text-sm text-textPrimary">{selectedNode.tokens}</span>
                  </div>
                )}
                {selectedNode.model && (
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-textSecondary">Model</span>
                    <span className="font-mono text-sm text-textPrimary">{selectedNode.model}</span>
                  </div>
                )}
              </div>
            )}

            {/* Content */}
            {selectedNode.content && (
              <div className="space-y-2">
                <h4 className="text-xs font-semibold text-textTertiary uppercase">Content</h4>
                <div className="bg-background rounded-lg p-3 max-h-48 overflow-auto border border-border">
                  <p className="text-sm text-textSecondary whitespace-pre-wrap">{selectedNode.content}</p>
                </div>
              </div>
            )}

            {/* Tool Arguments */}
            {selectedNode.metadata?.arguments && (
              <div className="space-y-2">
                <h4 className="text-xs font-semibold text-textTertiary uppercase">Arguments</h4>
                <div className="bg-background rounded-lg p-3 max-h-48 overflow-auto border border-border">
                  <pre className="text-xs text-textSecondary font-mono whitespace-pre-wrap">
                    {typeof selectedNode.metadata.arguments === 'string' 
                      ? selectedNode.metadata.arguments 
                      : JSON.stringify(selectedNode.metadata.arguments, null, 2)}
                  </pre>
                </div>
              </div>
            )}

            {/* Tool Calls */}
            {selectedNode.metadata?.tool_calls && (
              <div className="space-y-2">
                <h4 className="text-xs font-semibold text-textTertiary uppercase">Tool Calls</h4>
                <div className="space-y-2">
                  {selectedNode.metadata.tool_calls.map((tool: any, i: number) => (
                    <div key={i} className="bg-amber-500/10 border border-amber-500/30 rounded-lg p-3">
                      <div className="flex items-center gap-2 mb-2">
                        <Wrench className="w-4 h-4 text-amber-600 dark:text-amber-400" />
                        <span className="font-medium text-amber-700 dark:text-amber-300">{tool.name}</span>
                      </div>
                      {tool.arguments && (
                        <pre className="text-xs text-textSecondary font-mono whitespace-pre-wrap bg-background/50 p-2 rounded">
                          {typeof tool.arguments === 'string' 
                            ? tool.arguments 
                            : JSON.stringify(tool.arguments, null, 2)}
                        </pre>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Full Metadata */}
            {selectedNode.metadata && Object.keys(selectedNode.metadata).length > 0 && (
              <details className="group">
                <summary className="text-xs font-semibold text-textTertiary uppercase cursor-pointer hover:text-textSecondary flex items-center gap-1">
                  <ChevronRight className="w-3 h-3 group-open:rotate-90 transition-transform" />
                  All Metadata
                </summary>
                <div className="mt-2 bg-background rounded-lg p-3 max-h-64 overflow-auto border border-border">
                  <pre className="text-xs text-textSecondary font-mono whitespace-pre-wrap">
                    {JSON.stringify(selectedNode.metadata, null, 2)}
                  </pre>
                </div>
              </details>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
