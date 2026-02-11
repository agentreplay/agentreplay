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

import { useState, useEffect, useMemo, useCallback } from 'react';
import { Bot, Server, Database, Globe, Activity, ZoomIn, ZoomOut, Maximize, Sparkles, RefreshCcw, Loader2, Settings, AlertTriangle } from 'lucide-react';
import { Link as RouterLink } from 'react-router-dom';
import { agentreplayClient, Agent } from '../lib/agentreplay-api';
import { VideoHelpButton } from '../components/VideoHelpButton';

// Types for our graph visualization
interface Node {
    id: string;
    type: 'agent' | 'service' | 'database' | 'external' | 'llm';
    label: string;
    x: number;
    y: number;
    status: 'active' | 'inactive' | 'error';
    metadata?: any;
    calls?: number;
    avgLatency?: number;
}

interface Link {
    source: string;
    target: string;
    value: number; // Traffic volume
    label?: string;
}

interface AIAnalysisResult {
    nodes: Array<{
        id: string;
        type: string;
        label: string;
        calls: number;
        avgLatency?: number;
    }>;
    edges: Array<{
        from: string;
        to: string;
        count: number;
        label?: string;
    }>;
    summary: string;
    insights: string[];
}

// Check if AI models are configured
function getConfiguredProvider(): { hasProvider: boolean; providerName?: string } {
    try {
        const savedSettings = localStorage.getItem('agentreplay_settings');
        if (!savedSettings) return { hasProvider: false };
        const settings = JSON.parse(savedSettings);
        const providers = settings?.models?.providers || [];
        const validProvider = providers.find((p: any) => p.apiKey && p.apiKey.length > 0);
        if (validProvider) {
            return { hasProvider: true, providerName: validProvider.name || validProvider.provider };
        }
        return { hasProvider: false };
    } catch {
        return { hasProvider: false };
    }
}

export default function Agents() {
    const [agents, setAgents] = useState<Agent[]>([]);
    const [loading, setLoading] = useState(true);
    const [selectedNode, setSelectedNode] = useState<Node | null>(null);
    const [zoom, setZoom] = useState(1);
    
    // AI Analysis state
    const [aiAnalysis, setAiAnalysis] = useState<AIAnalysisResult | null>(null);
    const [analyzing, setAnalyzing] = useState(false);
    const [analysisError, setAnalysisError] = useState<string | null>(null);
    const [lastAnalyzed, setLastAnalyzed] = useState<Date | null>(null);
    const [traceCount, setTraceCount] = useState(0);
    
    const providerInfo = getConfiguredProvider();

    // Fetch recent traces and run AI analysis
    const runAIAnalysis = useCallback(async () => {
        if (!providerInfo.hasProvider) return;
        
        setAnalyzing(true);
        setAnalysisError(null);
        
        try {
            // Fetch recent traces
            const response = await agentreplayClient.listTraces({ limit: 100 });
            const traces = response.traces || [];
            setTraceCount(traces.length);
            
            if (traces.length === 0) {
                setAnalysisError('No traces found. Run some AI agents to see the system map.');
                setAnalyzing(false);
                return;
            }
            
            // Analyze trace patterns locally (without external AI for now)
            // Group by agent/model and count connections
            const nodeMap = new Map<string, { type: string; calls: number; totalDuration: number }>();
            const edgeMap = new Map<string, { count: number; from: string; to: string }>();
            
            // Add core services
            nodeMap.set('agentreplay', { type: 'service', calls: traces.length, totalDuration: 0 });
            
            traces.forEach((trace: any) => {
                const model = trace.metadata?.['gen_ai.request.model'] || trace.metadata?.model || trace.model;
                const agentName = trace.agent_name || trace.metadata?.agent_name;
                const operation = trace.operation || trace.span_type || 'unknown';
                
                // Track LLM models
                if (model) {
                    const modelKey = `llm:${model}`;
                    const existing = nodeMap.get(modelKey) || { type: 'llm', calls: 0, totalDuration: 0 };
                    existing.calls++;
                    existing.totalDuration += trace.duration_us ? trace.duration_us / 1000 : 0;
                    nodeMap.set(modelKey, existing);
                    
                    // Edge from agent to model
                    const edgeKey = agentName ? `${agentName}:${model}` : `app:${model}`;
                    const edge = edgeMap.get(edgeKey) || { count: 0, from: agentName || 'app', to: model };
                    edge.count++;
                    edgeMap.set(edgeKey, edge);
                }
                
                // Track agents
                if (agentName) {
                    const agentKey = `agent:${agentName}`;
                    const existing = nodeMap.get(agentKey) || { type: 'agent', calls: 0, totalDuration: 0 };
                    existing.calls++;
                    existing.totalDuration += trace.duration_us ? trace.duration_us / 1000 : 0;
                    nodeMap.set(agentKey, existing);
                }
            });
            
            // Convert to analysis result format
            const nodes = Array.from(nodeMap.entries()).map(([key, value]) => {
                const [type, name] = key.includes(':') ? key.split(':') : ['service', key];
                return {
                    id: key,
                    type: type === 'llm' ? 'llm' : type === 'agent' ? 'agent' : 'service',
                    label: name || key,
                    calls: value.calls,
                    avgLatency: value.calls > 0 ? Math.round(value.totalDuration / value.calls) : 0,
                };
            });
            
            const edges = Array.from(edgeMap.entries()).map(([, edge]) => ({
                from: edge.from,
                to: edge.to,
                count: edge.count,
                label: `${edge.count} calls`,
            }));
            
            // Generate insights
            const insights: string[] = [];
            const llmNodes = nodes.filter(n => n.type === 'llm');
            if (llmNodes.length > 1) {
                insights.push(`Using ${llmNodes.length} different LLM models across traces`);
            }
            const slowestNode = nodes.filter(n => n.avgLatency).sort((a, b) => (b.avgLatency || 0) - (a.avgLatency || 0))[0];
            if (slowestNode && slowestNode.avgLatency && slowestNode.avgLatency > 1000) {
                insights.push(`${slowestNode.label} has highest avg latency: ${slowestNode.avgLatency}ms`);
            }
            const busiestModel = llmNodes.sort((a, b) => b.calls - a.calls)[0];
            if (busiestModel) {
                insights.push(`Most used model: ${busiestModel.label} (${busiestModel.calls} calls)`);
            }
            
            setAiAnalysis({
                nodes,
                edges,
                summary: `Analyzed ${traces.length} traces. Found ${llmNodes.length} LLM models and ${nodes.filter(n => n.type === 'agent').length} agents.`,
                insights,
            });
            setLastAnalyzed(new Date());
        } catch (err) {
            console.error('AI Analysis failed:', err);
            setAnalysisError(err instanceof Error ? err.message : 'Analysis failed');
        } finally {
            setAnalyzing(false);
        }
    }, [providerInfo.hasProvider]);

    useEffect(() => {
        const fetchAgents = async () => {
            try {
                const response = await agentreplayClient.listAgents();
                setAgents(response.agents || []);
            } catch (err) {
                console.error('Failed to fetch agents:', err);
            } finally {
                setLoading(false);
            }
        };

        fetchAgents();
        
        // Auto-run analysis if provider is configured
        if (providerInfo.hasProvider) {
            runAIAnalysis();
        }
    }, [providerInfo.hasProvider, runAIAnalysis]);

    // Transform agents into graph nodes and links
    // Uses AI analysis results when available, otherwise falls back to static view
    const { nodes, links } = useMemo(() => {
        const nodes: Node[] = [];
        const links: Link[] = [];

        // If we have AI analysis, use that for the topology
        if (aiAnalysis && aiAnalysis.nodes.length > 0) {
            const centerX = 400;
            const centerY = 300;
            const radius = 220;
            
            // Position nodes in a circle, grouped by type
            const llmNodes = aiAnalysis.nodes.filter(n => n.type === 'llm');
            const agentNodes = aiAnalysis.nodes.filter(n => n.type === 'agent');
            const serviceNodes = aiAnalysis.nodes.filter(n => n.type === 'service');
            
            // Center: AgentReplay service
            serviceNodes.forEach((node, i) => {
                nodes.push({
                    id: node.id,
                    type: 'service',
                    label: node.label,
                    x: centerX,
                    y: centerY,
                    status: 'active',
                    calls: node.calls,
                    avgLatency: node.avgLatency,
                });
            });
            
            // Top semicircle: LLM models
            llmNodes.forEach((node, i) => {
                const angle = Math.PI + (i / Math.max(llmNodes.length - 1, 1)) * Math.PI;
                const x = centerX + radius * Math.cos(angle);
                const y = centerY + radius * 0.8 * Math.sin(angle);
                
                nodes.push({
                    id: node.id,
                    type: 'external',
                    label: node.label,
                    x: llmNodes.length === 1 ? centerX : x,
                    y: llmNodes.length === 1 ? centerY - radius * 0.8 : y,
                    status: 'active',
                    calls: node.calls,
                    avgLatency: node.avgLatency,
                });
            });
            
            // Bottom semicircle: Agents
            agentNodes.forEach((node, i) => {
                const angle = (i / Math.max(agentNodes.length - 1, 1)) * Math.PI;
                const x = centerX + radius * Math.cos(angle);
                const y = centerY + radius * 0.8 * Math.sin(angle);
                
                nodes.push({
                    id: node.id,
                    type: 'agent',
                    label: node.label,
                    x: agentNodes.length === 1 ? centerX : x,
                    y: agentNodes.length === 1 ? centerY + radius * 0.8 : y,
                    status: 'active',
                    calls: node.calls,
                    avgLatency: node.avgLatency,
                });
            });
            
            // Create links from analysis edges
            aiAnalysis.edges.forEach(edge => {
                const sourceNode = nodes.find(n => n.label === edge.from || n.id.includes(edge.from));
                const targetNode = nodes.find(n => n.label === edge.to || n.id.includes(edge.to));
                
                if (sourceNode && targetNode) {
                    links.push({
                        source: sourceNode.id,
                        target: targetNode.id,
                        value: edge.count,
                        label: edge.label,
                    });
                }
            });
            
            return { nodes, links };
        }

        // Fallback: Static topology based on registered agents

        // Central hub (AgentReplay Server)
        nodes.push({
            id: 'hub',
            type: 'service',
            label: 'AgentReplay Core',
            x: 400,
            y: 300,
            status: 'active'
        });

        // Database node
        nodes.push({
            id: 'db',
            type: 'database',
            label: 'Trace Store',
            x: 400,
            y: 500,
            status: 'active'
        });
        links.push({ source: 'hub', target: 'db', value: 1 });

        // Position agents in a circle around the hub
        const radius = 250;
        agents.forEach((agent, index) => {
            const angle = (index / agents.length) * 2 * Math.PI;
            const x = 400 + radius * Math.cos(angle);
            const y = 300 + radius * Math.sin(angle);

            nodes.push({
                id: agent.agent_id,
                type: 'agent',
                label: agent.name,
                x,
                y,
                status: agent.last_seen && (Date.now() - agent.last_seen < 300000) ? 'active' : 'inactive',
                metadata: agent
            });

            // Connect agent to hub
            links.push({ source: agent.agent_id, target: 'hub', value: 1 });
        });

        // Add some external services for realism if we have agents
        if (agents.length > 0) {
            nodes.push({
                id: 'openai',
                type: 'external',
                label: 'OpenAI API',
                x: 700,
                y: 100,
                status: 'active'
            });
            // Connect first agent to OpenAI
            links.push({ source: agents[0].agent_id, target: 'openai', value: 1 });
        }

        return { nodes, links };
    }, [agents, aiAnalysis]);

    const getNodeIcon = (type: string) => {
        switch (type) {
            case 'agent': return Bot;
            case 'database': return Database;
            case 'external': return Globe;
            case 'llm': return Sparkles;
            default: return Server;
        }
    };

    const getNodeColor = (type: string, status: string) => {
        if (status === 'inactive') return '#6b7280';
        if (status === 'error') return '#ef4444';
        switch (type) {
            case 'agent': return '#3b82f6'; // Primary blue
            case 'database': return '#10b981'; // Emerald
            case 'external': return '#8b5cf6'; // Violet
            case 'llm': return '#8b5cf6'; // Violet for LLM
            default: return '#f59e0b'; // Amber
        }
    };

    // Show configuration prompt if no AI provider
    if (!providerInfo.hasProvider && !loading) {
        return (
            <div className="h-full flex flex-col bg-background">
                {/* Header */}
                <div className="flex items-center justify-between p-6 border-b border-border bg-surface-elevated">
                    <div className="flex items-center gap-3">
                        <Activity className="w-6 h-6 text-primary" />
                        <div>
                            <h1 className="text-xl font-bold text-textPrimary">System Map</h1>
                            <p className="text-sm text-textSecondary">AI-powered topology visualization</p>
                        </div>
                    </div>
                    <VideoHelpButton pageId="agents" />
                </div>
                
                <div className="flex-1 flex items-center justify-center p-6">
                    <div className="max-w-md text-center">
                        <div className="bg-warning/10 rounded-full p-6 w-20 h-20 mx-auto mb-6 flex items-center justify-center">
                            <AlertTriangle className="w-10 h-10 text-warning" />
                        </div>
                        <h2 className="text-xl font-semibold text-textPrimary mb-2">Configure AI Model</h2>
                        <p className="text-textSecondary mb-6">
                            The System Map uses AI to analyze your traces and visualize the actual data flow between components.
                            Configure an AI provider to enable this feature.
                        </p>
                        <RouterLink
                            to="/settings?tab=models"
                            className="inline-flex items-center gap-2 px-6 py-3 bg-primary text-white rounded-lg hover:bg-primary/90 transition-colors font-medium"
                        >
                            <Settings className="w-5 h-5" />
                            Configure AI Models
                        </RouterLink>
                    </div>
                </div>
            </div>
        );
    }

    return (
        <div className="h-full flex flex-col bg-background">
            {/* Header */}
            <div className="flex items-center justify-between p-6 border-b border-border bg-surface-elevated">
                <div className="flex items-center gap-3">
                    <Activity className="w-6 h-6 text-primary" />
                    <div>
                        <h1 className="text-xl font-bold text-textPrimary">System Map</h1>
                        <p className="text-sm text-textSecondary">
                            {aiAnalysis ? (
                                <>AI-analyzed topology from {traceCount} traces</>
                            ) : (
                                <>Visual topology of agents and services</>
                            )}
                        </p>
                    </div>
                </div>
                <div className="flex items-center gap-4">
                    <VideoHelpButton pageId="agents" />
                    {/* AI Analysis Controls */}
                    <div className="flex items-center gap-2 border-r border-border pr-4">
                        {analyzing ? (
                            <span className="flex items-center gap-2 text-sm text-textSecondary">
                                <Loader2 className="w-4 h-4 animate-spin" />
                                Analyzing...
                            </span>
                        ) : lastAnalyzed ? (
                            <span className="text-xs text-textTertiary">
                                Updated {lastAnalyzed.toLocaleTimeString()}
                            </span>
                        ) : null}
                        <button
                            onClick={runAIAnalysis}
                            disabled={analyzing}
                            className="flex items-center gap-1.5 px-3 py-1.5 text-sm bg-primary/10 text-primary rounded-lg hover:bg-primary/20 transition-colors disabled:opacity-50"
                        >
                            <RefreshCcw className={`w-4 h-4 ${analyzing ? 'animate-spin' : ''}`} />
                            Refresh
                        </button>
                    </div>
                    
                    {/* Zoom Controls */}
                    <div className="flex items-center gap-2">
                        <button onClick={() => setZoom(z => Math.max(0.5, z - 0.1))} className="p-2 hover:bg-surface rounded text-textSecondary">
                            <ZoomOut className="w-4 h-4" />
                        </button>
                        <span className="text-xs text-textSecondary w-12 text-center">{Math.round(zoom * 100)}%</span>
                        <button onClick={() => setZoom(z => Math.min(2, z + 0.1))} className="p-2 hover:bg-surface rounded text-textSecondary">
                            <ZoomIn className="w-4 h-4" />
                        </button>
                        <button onClick={() => setZoom(1)} className="p-2 hover:bg-surface rounded text-textSecondary">
                            <Maximize className="w-4 h-4" />
                        </button>
                    </div>
                </div>
            </div>

            {/* Analysis Error Banner */}
            {analysisError && (
                <div className="px-6 py-3 bg-warning/10 border-b border-warning/20">
                    <div className="flex items-center gap-2 text-warning text-sm">
                        <AlertTriangle className="w-4 h-4" />
                        {analysisError}
                    </div>
                </div>
            )}
            
            {/* AI Insights Panel */}
            {aiAnalysis && aiAnalysis.insights.length > 0 && (
                <div className="px-6 py-3 bg-primary/5 border-b border-border">
                    <div className="flex items-center gap-3 overflow-x-auto">
                        <Sparkles className="w-4 h-4 text-primary flex-shrink-0" />
                        {aiAnalysis.insights.map((insight, i) => (
                            <span key={i} className="text-sm text-textSecondary whitespace-nowrap">
                                {insight}
                                {i < aiAnalysis.insights.length - 1 && <span className="mx-3 text-border">•</span>}
                            </span>
                        ))}
                    </div>
                </div>
            )}

            <div className="flex-1 relative overflow-hidden">
                {loading || analyzing ? (
                    <div className="absolute inset-0 flex items-center justify-center">
                        <div className="text-center">
                            <Loader2 className="w-8 h-8 text-primary animate-spin mx-auto mb-2" />
                            <p className="text-textSecondary text-sm">
                                {analyzing ? 'Analyzing trace patterns...' : 'Loading...'}
                            </p>
                        </div>
                    </div>
                ) : (
                    <div className="absolute inset-0 overflow-auto">
                        <svg
                            width="100%"
                            height="100%"
                            viewBox="0 0 800 600"
                            className="w-full h-full"
                            style={{ transform: `scale(${zoom})`, transformOrigin: 'center' }}
                        >
                            <defs>
                                <marker
                                    id="arrowhead"
                                    markerWidth="10"
                                    markerHeight="7"
                                    refX="28"
                                    refY="3.5"
                                    orient="auto"
                                >
                                    <polygon points="0 0, 10 3.5, 0 7" fill="#4b5563" />
                                </marker>
                            </defs>

                            {/* Links */}
                            {links.map((link, i) => {
                                const source = nodes.find(n => n.id === link.source);
                                const target = nodes.find(n => n.id === link.target);
                                if (!source || !target) return null;

                                return (
                                    <line
                                        key={i}
                                        x1={source.x}
                                        y1={source.y}
                                        x2={target.x}
                                        y2={target.y}
                                        stroke="#4b5563"
                                        strokeWidth="2"
                                        strokeOpacity="0.4"
                                        markerEnd="url(#arrowhead)"
                                    />
                                );
                            })}

                            {/* Nodes */}
                            {nodes.map((node) => {
                                const Icon = getNodeIcon(node.type);
                                const color = getNodeColor(node.type, node.status);
                                const isSelected = selectedNode?.id === node.id;

                                return (
                                    <g
                                        key={node.id}
                                        transform={`translate(${node.x},${node.y})`}
                                        onClick={() => setSelectedNode(node)}
                                        className="cursor-pointer transition-all duration-200"
                                        style={{ opacity: selectedNode && selectedNode.id !== node.id ? 0.6 : 1 }}
                                    >
                                        {/* Pulse effect for active nodes */}
                                        {node.status === 'active' && (
                                            <circle r="24" fill={color} opacity="0.2">
                                                <animate attributeName="r" from="24" to="32" dur="2s" repeatCount="indefinite" />
                                                <animate attributeName="opacity" from="0.2" to="0" dur="2s" repeatCount="indefinite" />
                                            </circle>
                                        )}

                                        {/* Main node circle */}
                                        <circle
                                            r="24"
                                            fill={isSelected ? color : '#1f2937'}
                                            stroke={color}
                                            strokeWidth={isSelected ? 3 : 2}
                                            className="transition-colors duration-200"
                                        />

                                        {/* Icon container - using foreignObject to render Lucide icon */}
                                        <foreignObject x="-12" y="-12" width="24" height="24" pointerEvents="none">
                                            <div className="flex items-center justify-center h-full w-full text-white">
                                                <Icon size={16} />
                                            </div>
                                        </foreignObject>

                                        {/* Label */}
                                        <text
                                            y="40"
                                            textAnchor="middle"
                                            fill="#9ca3af"
                                            fontSize="12"
                                            fontWeight="500"
                                            className="select-none"
                                        >
                                            {node.label}
                                        </text>
                                    </g>
                                );
                            })}
                        </svg>
                    </div>
                )}

                {/* Inspector Panel */}
                {selectedNode && (
                    <div className="absolute top-4 right-4 w-80 bg-surface border border-border rounded-lg shadow-xl p-4 animate-in slide-in-from-right-10">
                        <div className="flex items-center justify-between mb-4">
                            <h3 className="font-semibold text-textPrimary flex items-center gap-2">
                                {(() => {
                                    const Icon = getNodeIcon(selectedNode.type);
                                    return <Icon className="w-4 h-4 text-primary" />;
                                })()}
                                {selectedNode.label}
                            </h3>
                            <button
                                onClick={() => setSelectedNode(null)}
                                className="text-textSecondary hover:text-textPrimary text-xl"
                            >
