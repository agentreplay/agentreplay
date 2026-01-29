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

import React, { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { Activity, Beaker, FileText, MessageSquare, Zap, Hash, Clock, List, Wrench, Settings, Copy, Check, ChevronDown, ChevronRight, X, Play, Database, Target, TrendingUp } from 'lucide-react';
import { formatDistanceToNow } from 'date-fns';
import { TraceMetadata } from '../../src/lib/agentreplay-api';
import Tooltip from '../../src/components/Tooltip';
import { AddToDatasetModal } from '../evals/AddToDatasetModal';


interface SpanInspectorProps {
    trace: TraceMetadata;
    onClose?: () => void;
    activeTab?: Tab;
    onTabChange?: (tab: Tab) => void;
    hideTabs?: boolean;
}

type Tab = 'overview' | 'conversation' | 'tools' | 'attributes' | 'raw';

export function SpanInspector({ trace, onClose, activeTab: controlledTab, onTabChange, hideTabs = false }: SpanInspectorProps) {
    const navigate = useNavigate();
    const { projectId } = useParams();

    const [internalTab, setInternalTab] = useState<Tab>(
        (trace.metadata as any)?.prompts?.length > 0 || (trace.metadata as any)?.completions?.length > 0
            ? 'conversation'
            : 'overview'
    );

    const activeTab = controlledTab || internalTab;
    const setActiveTab = (tab: Tab) => {
        if (onTabChange) {
            onTabChange(tab);
        } else {
            setInternalTab(tab);
        }
    };
    const [copiedStates, setCopiedStates] = useState<{ [key: string]: boolean }>({});
    const [expandedMessages, setExpandedMessages] = useState<Set<number>>(new Set());

    const formatTimestamp = (timestamp?: number) => {
        if (!timestamp) return 'N/A';
        return new Date(timestamp / 1000).toLocaleString();
    };

    const copyToClipboard = async (text: string, key: string) => {
        await navigator.clipboard.writeText(text);
        setCopiedStates(prev => ({ ...prev, [key]: true }));
        setTimeout(() => {
            setCopiedStates(prev => ({ ...prev, [key]: false }));
        }, 2000);
    };

    const toggleMessageExpansion = (index: number) => {
        setExpandedMessages(prev => {
            const newSet = new Set(prev);
            if (newSet.has(index)) {
                newSet.delete(index);
            } else {
                newSet.add(index);
            }
            return newSet;
        });
    };

    const formatJSON = (obj: any): string => {
        try {
            return JSON.stringify(typeof obj === 'string' ? JSON.parse(obj) : obj, null, 2);
        } catch {
            return String(obj);
        }
    };

    const SyntaxHighlightedJSON: React.FC<{ json: string }> = ({ json }) => {
        const highlighted = json
            .replace(/"([^"]+)":/g, '<span class="text-blue-400">"$1"</span>:')
            .replace(/: "([^"]+)"/g, ': <span class="text-green-400">"$1"</span>')
            .replace(/: (\d+)/g, ': <span class="text-yellow-400">$1</span>')
            .replace(/: (true|false|null)/g, ': <span class="text-purple-400">$1</span>');

        return (
            <pre
                className="text-xs font-mono text-textPrimary whitespace-pre-wrap"
                dangerouslySetInnerHTML={{ __html: highlighted }}
            />
        );
    };

    // Extract structured data from metadata
    const metadata = trace.metadata as any;

    // Parse prompts from gen_ai.prompt.N.* format (OTEL semantic conventions)
    const parseOtelPrompts = (): Array<{ role: string; content: string; toolCalls?: any[]; toolCallId?: string }> => {
        const promptGroups: Record<number, any> = {};
        const result: Array<{ role: string; content: string; toolCalls?: any[]; toolCallId?: string }> = [];

        if (!metadata) return result;

        Object.entries(metadata).forEach(([key, value]) => {
            // Use more precise regex - require exact match for known fields
            // This prevents content_filter_results from matching 'content'
            const match = key.match(/^gen_ai\.prompt\.(\d+)\.(role|content|tool_call_id)$/) ||
                key.match(/^gen_ai\.prompt\.(\d+)\.(tool_calls\.\d+\.\w+)$/);
            if (match) {
                const index = parseInt(match[1]);
                if (!promptGroups[index]) promptGroups[index] = { index };

                const field = match[2];
                if (field === 'role') {
                    promptGroups[index].role = value;
                } else if (field === 'content') {
                    promptGroups[index].content = value;
                } else if (field === 'tool_call_id') {
                    promptGroups[index].toolCallId = value;
                } else if (field.startsWith('tool_calls.')) {
                    if (!promptGroups[index].toolCalls) promptGroups[index].toolCalls = [];
                    const toolMatch = field.match(/tool_calls\.(\d+)\.(.*)/);
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

        // Sort by index and add to result
        Object.values(promptGroups)
            .sort((a: any, b: any) => a.index - b.index)
            .forEach((p: any) => result.push(p));

        return result;
    };

    // Parse completions from gen_ai.completion.N.* format
    const parseOtelCompletions = (): Array<{ role: string; content: string; finishReason?: string; toolCalls?: any[] }> => {
        const completionGroups: Record<number, any> = {};
        const result: Array<{ role: string; content: string; finishReason?: string; toolCalls?: any[] }> = [];

        if (!metadata) return result;

        Object.entries(metadata).forEach(([key, value]) => {
            // Use more precise regex - require exact match or tool_calls prefix
            // Exclude content_filter_results and other unrelated fields
            const match = key.match(/^gen_ai\.completion\.(\d+)\.(role|content|finish_reason)$/) ||
                key.match(/^gen_ai\.completion\.(\d+)\.(tool_calls\.\d+\.\w+)$/);
            if (match) {
                const index = parseInt(match[1]);
                if (!completionGroups[index]) completionGroups[index] = { index, role: 'assistant' };

                const field = match[2];
                if (field === 'role') {
                    completionGroups[index].role = value;
                } else if (field === 'content') {
                    completionGroups[index].content = value;
                } else if (field === 'finish_reason') {
                    completionGroups[index].finishReason = value;
                } else if (field.startsWith('tool_calls.')) {
                    if (!completionGroups[index].toolCalls) completionGroups[index].toolCalls = [];
                    const toolMatch = field.match(/tool_calls\.(\d+)\.(.*)/);
                    if (toolMatch) {
                        const toolIndex = parseInt(toolMatch[1]);
                        if (!completionGroups[index].toolCalls[toolIndex]) {
                            completionGroups[index].toolCalls[toolIndex] = {};
                        }
                        completionGroups[index].toolCalls[toolIndex][toolMatch[2]] = value;
                    }
                }
            }
        });

        // Sort by index and add to result
        Object.values(completionGroups)
            .sort((a: any, b: any) => a.index - b.index)
            .forEach((p: any) => result.push(p));

        return result;
    };

    // Use OTEL format if available, otherwise fall back to legacy format
    const otelPrompts = parseOtelPrompts();
    const otelCompletions = parseOtelCompletions();

    const prompts = otelPrompts.length > 0 ? otelPrompts : (metadata?.prompts || []);
    const completions = otelCompletions.length > 0 ? otelCompletions : (metadata?.completions || []);
    const toolCalls = metadata?.tool_calls || [];
    const hyperparameters = metadata?.hyperparameters || {};
    const tokenBreakdown = metadata?.token_breakdown || {
        input_tokens: parseInt(metadata?.['gen_ai.usage.input_tokens']) || metadata?.input_tokens || 0,
        output_tokens: parseInt(metadata?.['gen_ai.usage.output_tokens']) || metadata?.output_tokens || 0,
        total_tokens: parseInt(metadata?.['gen_ai.usage.input_tokens'] || 0) + parseInt(metadata?.['gen_ai.usage.output_tokens'] || 0) || metadata?.total_tokens || 0
    };
    const displayName = (trace as any).display_name || trace.operation_name || metadata?.['span.name'] || trace.span_type;
    const tags = (trace as any).tags || [];

    const hasConversation = prompts.length > 0 || completions.length > 0;
    const hasTools = toolCalls.length > 0 || completions.some((c: any) => c.toolCalls?.length > 0) || prompts.some((p: any) => p.toolCalls?.length > 0);

    // Open conversation in Playground with pre-filled prompt
    const openInPlayground = (promptContent?: string) => {
        // Collect messages for the playground
        const inputMessages: { role: string; content: string }[] = [];

        // 1. If opening specific prompt content (e.g. from copy button or specific row)
        if (promptContent) {
            inputMessages.push({ role: 'user', content: promptContent });
        }
        // 2. If trace has conversation structure, use it
        else if (prompts.length > 0) {
            // Include system and user messages
            // We generally DON'T include the assistant response if we want to "test" the prompt again,
            // but for "replay" it might be useful. Let's stick to inputs for "Playground" mode.
            for (const p of prompts) {
                if (p.role === 'system' || p.role === 'user') {
                    // Start: Robust JSON handling for content
                    let cleanContent = p.content;
                    try {
                        if (typeof cleanContent === 'string' && cleanContent.trim().startsWith('[')) {
                            const parsed = JSON.parse(cleanContent);
                            if (Array.isArray(parsed) && parsed[0]?.text) cleanContent = parsed[0].text;
                        }
                    } catch { }
                    // End: Robust JSON handling

                    inputMessages.push({ role: p.role, content: cleanContent });
                }
            }
        }
        // 3. Fallback: use raw input if available
        else if (metadata?.input) {
            inputMessages.push({ role: 'user', content: typeof metadata.input === 'string' ? metadata.input : JSON.stringify(metadata.input) });
        }

        // Store prompt data in sessionStorage
        const playgroundData = {
            prompt: inputMessages.length > 0 ? inputMessages[inputMessages.length - 1].content : '',
            model: metadata?.model || 'gpt-4',
            temperature: hyperparameters?.temperature ?? 0.7,
            messages: inputMessages,
            sourceTraceId: trace.trace_id || trace.span_id,
        };
        sessionStorage.setItem('playground_data', JSON.stringify(playgroundData));
        navigate(`/projects/${projectId}/playground?from=trace`);
    };

    const [isAddDatasetOpen, setIsAddDatasetOpen] = useState(false);

    // Helper to get input/output for dataset
    const getDatasetData = () => {
        let input = '';
        let output = '';

        // Try to get structured conversation
        const lastUserPrompt = [...prompts].reverse().find((p: any) => p.role === 'user');
        const lastAssistantResponse = [...completions].reverse().find((p: any) => p.role === 'assistant');

        // Input strategy: Last user message OR raw input
        if (lastUserPrompt) {
            input = lastUserPrompt.content;
            // JSON clean
            try {
                if (input.startsWith('[')) {
                    const parsed = JSON.parse(input);
                    if (Array.isArray(parsed) && parsed[0]?.text) input = parsed[0].text;
                }
            } catch { }
        } else if (metadata?.input) {
            input = typeof metadata.input === 'string' ? metadata.input : JSON.stringify(metadata.input, null, 2);
        }

        // Output strategy: Last assistant response OR raw output
        if (lastAssistantResponse) {
            output = lastAssistantResponse.content;
        } else if (metadata?.output) {
            output = typeof metadata.output === 'string' ? metadata.output : JSON.stringify(metadata.output, null, 2);
        }

        return { input, output };
    };

    return (
        <div className="h-full flex flex-col bg-surface border-l border-border overflow-hidden">
            <AddToDatasetModal
                isOpen={isAddDatasetOpen}
                onClose={() => setIsAddDatasetOpen(false)}
                initialInput={getDatasetData().input}
                initialOutput={getDatasetData().output}
                metadata={{
                    source_trace_id: trace.trace_id,
                    source_span_id: trace.span_id,
                    model: metadata?.model || 'unknown'
                }}
            />
            {/* Header */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-border bg-surface-elevated">
                <div className="flex-1 min-w-0">
                    <h2 className="font-semibold text-textPrimary flex items-center gap-2 truncate">
                        <List className="w-4 h-4 flex-shrink-0" />
                        <span className="truncate">{displayName}</span>
                    </h2>
                    {tags.length > 0 && (
                        <div className="flex gap-1 mt-1 flex-wrap">
                            {tags.map((tag: string, idx: number) => (
                                <span
                                    key={idx}
                                    className="text-xs px-2 py-0.5 rounded-full bg-primary/10 text-primary border border-primary/20"
                                >
                                    {tag}
                                </span>
                            ))}
                        </div>
                    )}
                </div>
                {onClose && (
                    <button
                        onClick={onClose}
                        className="p-1.5 rounded-md text-textSecondary hover:text-textPrimary hover:bg-surface-hover transition-colors ml-2 flex-shrink-0"
                        title="Close details"
                    >
                        <X className="w-4 h-4" />
                    </button>
                )}
            </div>

            {/* Tabs */}
            {!hideTabs && (
                <div className="flex gap-1 px-2 py-2 border-b border-border bg-surface-elevated overflow-x-auto">
                    {hasConversation && (
                        <button
                            onClick={() => setActiveTab('conversation')}
                            className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm transition-colors whitespace-nowrap ${activeTab === 'conversation'
                                ? 'bg-primary/10 text-primary font-medium'
                                : 'text-textSecondary hover:text-textPrimary hover:bg-surface-hover'
                                }`}
                        >
                            <MessageSquare className="w-4 h-4" />
                            Conversation
                            <span className="text-xs bg-primary/20 px-1.5 py-0.5 rounded-full">{prompts.length + completions.length}</span>
                        </button>
                    )}
                    <button
                        onClick={() => setActiveTab('overview')}
                        className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm transition-colors whitespace-nowrap ${activeTab === 'overview'
                            ? 'bg-primary/10 text-primary font-medium'
                            : 'text-textSecondary hover:text-textPrimary hover:bg-surface-hover'
                            }`}
                    >
                        <Activity className="w-4 h-4" />
                        Overview
                    </button>
                    {hasTools && (
                        <button
                            onClick={() => setActiveTab('tools')}
                            className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm transition-colors whitespace-nowrap ${activeTab === 'tools'
                                ? 'bg-primary/10 text-primary font-medium'
                                : 'text-textSecondary hover:text-textPrimary hover:bg-surface-hover'
                                }`}
                        >
                            <Wrench className="w-4 h-4" />
                            Tools
                            <span className="text-xs bg-primary/20 px-1.5 py-0.5 rounded-full">{toolCalls.length}</span>
                        </button>
                    )}
                    <button
                        onClick={() => setActiveTab('attributes')}
                        className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm transition-colors whitespace-nowrap ${activeTab === 'attributes'
                            ? 'bg-primary/10 text-primary font-medium'
                            : 'text-textSecondary hover:text-textPrimary hover:bg-surface-hover'
                            }`}
                    >
                        <Settings className="w-4 h-4" />
                        Attributes
                    </button>
                    <button
                        onClick={() => setActiveTab('raw')}
                        className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm transition-colors whitespace-nowrap ${activeTab === 'raw'
                            ? 'bg-primary/10 text-primary font-medium'
                            : 'text-textSecondary hover:text-textPrimary hover:bg-surface-hover'
                            }`}
                    >
                        <FileText className="w-4 h-4" />
                        Raw
                    </button>
                </div>
            )}

            {/* Tab Content */}
            <div className="flex-1 overflow-y-auto p-4">
                {activeTab === 'overview' && (
                    <div className="space-y-4">
                        {/* Basic Info */}
                        <div className="grid grid-cols-2 gap-3">
                            <div className="bg-background rounded-lg p-3 border border-border-subtle">
                                <div className="text-xs text-textTertiary mb-1">Span ID</div>
                                <div className="flex items-center gap-2">
                                    <div className="text-sm font-mono text-textPrimary truncate flex-1" title={trace.span_id}>{trace.span_id}</div>
                                    <button
                                        onClick={() => copyToClipboard(trace.span_id, 'span_id')}
                                        className="text-textSecondary hover:text-textPrimary transition-colors"
                                        title="Copy ID"
                                    >
                                        {copiedStates['span_id'] ? <Check className="w-3 h-3 text-success" /> : <Copy className="w-3 h-3" />}
                                    </button>
                                </div>
                            </div>
                            <div className="bg-background rounded-lg p-3 border border-border-subtle">
                                <div className="text-xs text-textTertiary mb-1">Duration</div>
                                <div className="text-sm text-textPrimary font-medium">
                                    {trace.duration_ms ? trace.duration_ms >= 1000
                                        ? `${(trace.duration_ms / 1000).toFixed(2)}s`
                                        : `${trace.duration_ms.toFixed(0)}ms`
                                        : 'N/A'
                                    }
                                </div>
                            </div>
                        </div>

                        {/* Model & Token Info */}
                        {(metadata?.model || tokenBreakdown.total_tokens > 0) && (
                            <>
                                <div className="border-t border-border/50" />
                                <div className="space-y-3">
                                    <h3 className="text-xs font-semibold text-textSecondary uppercase tracking-wider">LLM Details</h3>
                                    {metadata?.model && (
                                        <div className="bg-background rounded-lg p-3 border border-border-subtle">
                                            <div className="text-xs text-textTertiary mb-1">Model</div>
                                            <div className="text-sm font-mono text-primary font-medium">{metadata.model}</div>
                                        </div>
                                    )}
                                    {tokenBreakdown.total_tokens > 0 && (
                                        <div className="grid grid-cols-3 gap-2">
                                            <div className="bg-background p-3 rounded border border-border-subtle">
                                                <div className="text-[10px] text-textTertiary uppercase mb-1">Input</div>
                                                <div className="text-lg font-mono font-bold text-primary">{tokenBreakdown.input_tokens || 0}</div>
                                            </div>
                                            <div className="bg-background p-3 rounded border border-border-subtle">
                                                <div className="text-[10px] text-textTertiary uppercase mb-1">Output</div>
                                                <div className="text-lg font-mono font-bold text-success">{tokenBreakdown.output_tokens || 0}</div>
                                            </div>
                                            <div className="bg-background p-3 rounded border border-border-subtle">
                                                <div className="text-[10px] text-textTertiary uppercase mb-1">Total</div>
                                                <div className="text-lg font-mono font-bold">{tokenBreakdown.total_tokens}</div>
                                            </div>
                                        </div>
                                    )}
                                </div>
                            </>
                        )}

                        {/* Hyperparameters */}
                        {Object.keys(hyperparameters).some(k => hyperparameters[k] !== undefined && hyperparameters[k] !== null) && (
                            <>
                                <div className="border-t border-border/50" />
                                <div className="space-y-3">
                                    <h3 className="text-xs font-semibold text-textSecondary uppercase tracking-wider">Hyperparameters</h3>
                                    <div className="grid grid-cols-2 gap-2">
                                        {hyperparameters.temperature !== undefined && hyperparameters.temperature !== null && (
                                            <div className="bg-background p-2 rounded border border-border-subtle">
                                                <div className="text-[10px] text-textTertiary">Temperature</div>
                                                <div className="text-sm font-mono">{hyperparameters.temperature}</div>
                                            </div>
                                        )}
                                        {hyperparameters.top_p !== undefined && hyperparameters.top_p !== null && (
                                            <div className="bg-background p-2 rounded border border-border-subtle">
                                                <div className="text-[10px] text-textTertiary">Top P</div>
                                                <div className="text-sm font-mono">{hyperparameters.top_p}</div>
                                            </div>
                                        )}
                                        {hyperparameters.max_tokens !== undefined && hyperparameters.max_tokens !== null && (
                                            <div className="bg-background p-2 rounded border border-border-subtle">
                                                <div className="text-[10px] text-textTertiary">Max Tokens</div>
                                                <div className="text-sm font-mono">{hyperparameters.max_tokens}</div>
                                            </div>
                                        )}
                                    </div>
                                </div>
                            </>
                        )}
                    </div>
                )}

                {activeTab === 'conversation' && (
                    <div className="space-y-4">
                        {/* Try in Playground & Add to Dataset Buttons */}
                        <div className="flex justify-end gap-2 mb-4">
                            <button
                                onClick={() => setIsAddDatasetOpen(true)}
                                className="flex items-center gap-2 px-3 py-1.5 bg-surface-elevated hover:bg-surface-hover border border-border rounded-lg text-sm font-medium transition-colors text-textSecondary hover:text-textPrimary"
                            >
                                <Database className="w-4 h-4" />
                                Add to Dataset
                            </button>
                            {hasConversation && (
                                <button
                                    onClick={() => openInPlayground()}
                                    className="flex items-center gap-2 px-3 py-1.5 bg-primary/10 hover:bg-primary/20 text-primary rounded-lg text-sm font-medium transition-colors"
                                >
                                    <Play className="w-4 h-4" />
                                    Try in Playground
                                </button>
                            )}
                        </div>

                        {/* Evaluation Results / Scores */}
                        {(metadata?.scores || metadata?.eval_metrics) && (
                            <div className="mb-4">
                                <h3 className="text-xs font-semibold text-textSecondary uppercase tracking-wider mb-2 flex items-center gap-2">
                                    <Target className="w-3.5 h-3.5" />
                                    Evaluation Scores
                                </h3>
                                <div className="grid grid-cols-2 gap-3">
                                    {Object.entries(metadata.scores || metadata.eval_metrics || {}).map(([key, value]) => {
                                        const score = Number(value);
                                        // Heuristic for color: >0.7/0.8 is green, <0.4 is red
                                        let colorClass = 'text-textPrimary';
                                        if (!isNaN(score)) {
                                            if (score >= 0.8) colorClass = 'text-success';
                                            else if (score >= 0.5) colorClass = 'text-warning';
                                            else colorClass = 'text-error';
                                        }

                                        return (
                                            <div key={key} className="bg-background rounded-lg p-3 border border-border-subtle">
                                                <div className="text-xs text-textTertiary mb-1 capitalize">{key.replace(/_/g, ' ')}</div>
                                                <div className={`text-sm font-mono font-bold ${colorClass}`}>
                                                    {typeof value === 'number' ? value.toFixed(2) : String(value)}
                                                </div>
                                            </div>
                                        );
                                    })}
                                </div>
                                <div className="border-t border-border/50 mt-4" />
                            </div>
                        )}

                        {/* Render all prompts with proper role-based styling */}
                        {prompts.map((prompt: any, idx: number) => {
                            const role = prompt.role || 'user';
                            const isSystem = role === 'system';
                            const isUser = role === 'user';
                            const isAssistant = role === 'assistant';
                            const isTool = role === 'tool';

                            // Parse content - handle JSON-wrapped content
                            let displayContent = prompt.content || '';
                            try {
                                if (typeof displayContent === 'string' && displayContent.startsWith('[{')) {
                                    const parsed = JSON.parse(displayContent);
                                    if (Array.isArray(parsed) && parsed[0]?.text) {
                                        displayContent = parsed[0].text;
                                    }
                                }
                            } catch { /* keep original */ }

                            const isLong = displayContent?.length > 500;
                            const isExpanded = expandedMessages.has(idx);
                            const truncatedContent = isLong && !isExpanded
                                ? displayContent.substring(0, 500) + '...'
                                : displayContent;

                            // Determine icon and colors based on role (dark mode compatible)
                            const iconBg = isSystem ? 'bg-purple-500/20' : isUser ? 'bg-blue-500/20' : isAssistant ? 'bg-green-500/20' : isTool ? 'bg-amber-500/20' : 'bg-muted';
                            const iconColor = isSystem ? 'text-purple-400' : isUser ? 'text-blue-400' : isAssistant ? 'text-green-400' : isTool ? 'text-amber-400' : 'text-muted-foreground';
                            const borderColor = isSystem ? 'border-purple-500/30' : isUser ? 'border-blue-500/30' : isAssistant ? 'border-green-500/30' : isTool ? 'border-amber-500/30' : 'border-border';
                            const bgColor = isSystem ? 'bg-purple-500/10' : isUser ? 'bg-blue-500/10' : isAssistant ? 'bg-green-500/10' : isTool ? 'bg-amber-500/10' : 'bg-muted/50';

                            return (
                                <div key={`prompt-${idx}`} className="space-y-3">
                                    <div className="flex gap-3">
                                        <div className={`flex-shrink-0 w-8 h-8 rounded-full ${iconBg} flex items-center justify-center`}>
                                            {isSystem ? <Settings className={`w-4 h-4 ${iconColor}`} /> :
                                                isUser ? <MessageSquare className={`w-4 h-4 ${iconColor}`} /> :
                                                    isTool ? <Wrench className={`w-4 h-4 ${iconColor}`} /> :
                                                        <Zap className={`w-4 h-4 ${iconColor}`} />}
                                        </div>
                                        <div className="flex-1 space-y-1">
                                            <div className="flex items-center justify-between">
                                                <div className="text-xs font-medium text-textSecondary uppercase">{role}</div>
                                                {displayContent && (
                                                    <button
                                                        onClick={() => copyToClipboard(displayContent, `prompt-${idx}`)}
                                                        className="text-textSecondary hover:text-textPrimary transition-colors"
                                                        title="Copy message"
                                                    >
                                                        {copiedStates[`prompt-${idx}`] ? <Check className="w-3 h-3 text-success" /> : <Copy className="w-3 h-3" />}
                                                    </button>
                                                )}
                                            </div>

                                            {/* Show content if present */}
                                            {displayContent && (
                                                <div className={`${bgColor} rounded-lg p-3 border ${borderColor}`}>
                                                    <pre className="text-sm text-textPrimary whitespace-pre-wrap font-sans">{truncatedContent}</pre>
                                                    {isLong && (
                                                        <button
                                                            onClick={() => toggleMessageExpansion(idx)}
                                                            className="mt-2 text-xs text-primary hover:underline flex items-center gap-1"
                                                        >
                                                            {isExpanded ? (
                                                                <><ChevronDown className="w-3 h-3" /> Show less</>
                                                            ) : (
                                                                <><ChevronRight className="w-3 h-3" /> Show more</>
                                                            )}
                                                        </button>
                                                    )}
                                                </div>
                                            )}

                                            {/* Show tool calls if present (for assistant messages requesting tools) */}
                                            {prompt.toolCalls && prompt.toolCalls.length > 0 && (
                                                <div className="space-y-2 mt-2">
                                                    {prompt.toolCalls.map((tool: any, toolIdx: number) => (
                                                        <div key={toolIdx} className="bg-amber-500/10 rounded-lg p-3 border border-amber-500/30">
                                                            <div className="flex items-center gap-2 mb-2">
                                                                <Wrench className="w-4 h-4 text-amber-400" />
                                                                <span className="font-mono text-sm font-semibold text-amber-300">{tool.name}</span>
                                                            </div>
                                                            {tool.arguments && (
                                                                <pre className="text-xs text-textSecondary font-mono bg-background/50 p-2 rounded overflow-x-auto">
                                                                    {typeof tool.arguments === 'string' ? tool.arguments : JSON.stringify(tool.arguments, null, 2)}
                                                                </pre>
                                                            )}
                                                        </div>
                                                    ))}
                                                </div>
                                            )}

                                            {/* Show tool_call_id if this is a tool response */}
                                            {isTool && prompt.toolCallId && (
                                                <div className="text-xs text-textTertiary mt-1">
                                                    Response to: <span className="font-mono">{prompt.toolCallId}</span>
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            );
                        })}

                        {/* Show completions (final assistant response) */}
                        {completions.map((completion: any, idx: number) => {
                            const hasToolCalls = completion.toolCalls && completion.toolCalls.length > 0;
                            const displayContent = completion.content || '';

                            return (
                                <div key={`completion-${idx}`} className="space-y-3">
                                    <div className="flex gap-3">
                                        <div className="flex-shrink-0 w-8 h-8 rounded-full bg-green-500/20 flex items-center justify-center">
                                            <Zap className="w-4 h-4 text-green-400" />
                                        </div>
                                        <div className="flex-1 space-y-1">
                                            <div className="flex items-center justify-between">
                                                <div className="flex items-center gap-2">
                                                    <div className="text-xs font-medium text-textSecondary uppercase">
                                                        {completion.role || 'ASSISTANT'} RESPONSE
                                                    </div>
                                                    {completion.finishReason && (
                                                        <span className="text-xs text-amber-400 px-2 py-0.5 bg-amber-500/10 rounded border border-amber-500/30">
                                                            {completion.finishReason}
                                                        </span>
                                                    )}
                                                </div>
                                                {displayContent && (
                                                    <button
                                                        onClick={() => copyToClipboard(displayContent, `completion-${idx}`)}
                                                        className="text-textSecondary hover:text-textPrimary transition-colors"
                                                        title="Copy response"
                                                    >
                                                        {copiedStates[`completion-${idx}`] ? <Check className="w-3 h-3 text-success" /> : <Copy className="w-3 h-3" />}
                                                    </button>
                                                )}
                                            </div>

                                            {/* Show content if present */}
                                            {displayContent && (
                                                <div className="bg-green-500/10 rounded-lg p-3 border border-green-500/30">
                                                    <pre className="text-sm text-textPrimary whitespace-pre-wrap font-sans">{displayContent}</pre>
                                                </div>
                                            )}

                                            {/* Show tool calls requested by the model */}
                                            {hasToolCalls && (
                                                <div className="space-y-2 mt-2">
                                                    <div className="text-xs font-medium text-amber-400 uppercase">Tool Calls Requested:</div>
                                                    {completion.toolCalls.map((tool: any, toolIdx: number) => (
                                                        <div key={toolIdx} className="bg-amber-500/10 rounded-lg p-3 border border-amber-500/30">
                                                            <div className="flex items-center gap-2 mb-2">
                                                                <Wrench className="w-4 h-4 text-amber-400" />
                                                                <span className="font-mono text-sm font-semibold text-amber-300">{tool.name}</span>
                                                                {tool.id && (
                                                                    <span className="text-xs text-amber-400/70 font-mono">({tool.id})</span>
                                                                )}
                                                            </div>
                                                            {tool.arguments && (
                                                                <pre className="text-xs text-textSecondary font-mono bg-background/50 p-2 rounded overflow-x-auto">
                                                                    {typeof tool.arguments === 'string' ? tool.arguments : JSON.stringify(tool.arguments, null, 2)}
                                                                </pre>
                                                            )}
                                                        </div>
                                                    ))}
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            );
                        })}

                        {prompts.length === 0 && completions.length === 0 && (
                            <div className="text-sm text-textSecondary italic text-center py-8">
                                No conversation data available
                            </div>
                        )}
                    </div>
                )}

                {activeTab === 'tools' && (
                    <div className="space-y-4">
                        {toolCalls.map((tool: any, idx: number) => (
                            <div key={idx} className="bg-background rounded-lg border border-border-subtle overflow-hidden">
                                <div className="bg-surface-elevated px-4 py-2 border-b border-border flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                        <Wrench className="w-4 h-4 text-primary" />
                                        <span className="font-mono text-sm font-medium text-textPrimary">{tool.name}</span>
                                    </div>
                                    <button
                                        onClick={() => copyToClipboard(JSON.stringify(tool, null, 2), `tool-${idx}`)}
                                        className="text-textSecondary hover:text-textPrimary transition-colors"
                                        title="Copy tool call"
                                    >
                                        {copiedStates[`tool-${idx}`] ? <Check className="w-3 h-3 text-success" /> : <Copy className="w-3 h-3" />}
                                    </button>
                                </div>
                                <div className="p-4 space-y-3">
                                    {tool.arguments && (
                                        <div>
                                            <div className="text-xs text-textSecondary mb-1 font-medium">Arguments</div>
                                            <div className="bg-surface p-3 rounded border border-border-subtle overflow-x-auto">
                                                <SyntaxHighlightedJSON json={formatJSON(tool.arguments)} />
                                            </div>
                                        </div>
                                    )}
                                    {tool.result && (
                                        <div>
                                            <div className="text-xs text-textSecondary mb-1 font-medium">Result</div>
                                            <div className="bg-surface p-3 rounded border border-border-subtle overflow-x-auto">
                                                <SyntaxHighlightedJSON json={formatJSON(tool.result)} />
                                            </div>
                                        </div>
                                    )}
                                </div>
                            </div>
                        ))}

                        {toolCalls.length === 0 && (
                            <div className="text-sm text-textSecondary italic text-center py-8">
                                No tool calls in this span
                            </div>
                        )}
                    </div>
                )}

                {activeTab === 'attributes' && (
                    <div className="space-y-3">
                        {metadata && Object.keys(metadata).length > 0 ? (
                            Object.entries(metadata)
                                .filter(([key]) => !['prompts', 'completions', 'tool_calls', 'hyperparameters', 'token_breakdown'].includes(key))
                                .map(([key, value]) => (
                                    <div key={key} className="bg-background rounded-lg p-3 border border-border-subtle">
                                        <div className="flex items-center justify-between mb-1">
                                            <div className="text-xs text-textTertiary font-mono">{key}</div>
                                            <button
                                                onClick={() => copyToClipboard(typeof value === 'object' ? JSON.stringify(value) : String(value), `attr-${key}`)}
                                                className="text-textSecondary hover:text-textPrimary transition-colors"
                                                title="Copy value"
                                            >
                                                {copiedStates[`attr-${key}`] ? <Check className="w-3 h-3 text-success" /> : <Copy className="w-3 h-3" />}
                                            </button>
                                        </div>
                                        <div className="text-sm text-textPrimary break-all">
                                            {typeof value === 'object' ? JSON.stringify(value) : String(value)}
                                        </div>
                                    </div>
                                ))
                        ) : (
                            <div className="text-sm text-textSecondary italic text-center py-8">
                                No attributes available
                            </div>
                        )}
                    </div>
                )}

                {activeTab === 'raw' && (
                    <div className="relative">
                        <button
                            onClick={() => copyToClipboard(JSON.stringify(trace, null, 2), 'raw')}
                            className="absolute top-2 right-2 text-textSecondary hover:text-textPrimary transition-colors bg-surface-elevated p-2 rounded border border-border"
                            title="Copy raw data"
                        >
                            {copiedStates['raw'] ? <Check className="w-4 h-4 text-success" /> : <Copy className="w-4 h-4" />}
                        </button>
                        <div className="bg-background p-3 rounded border border-border-subtle overflow-x-auto">
                            <SyntaxHighlightedJSON json={JSON.stringify(trace, null, 2)} />
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
}
