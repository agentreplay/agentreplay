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


import { formatDistanceToNow } from 'date-fns';
import { TraceMetadata } from '../lib/flowtrace-api';
import { useNavigate } from 'react-router-dom';
import { ReactNode, useMemo } from 'react';

interface TraceListProps {
    traces: TraceMetadata[];
    loading?: boolean;
    emptyMessage?: ReactNode;
    showContent?: boolean;
}

// Single trace row component with proper extraction logic (matching Traces.tsx)
function TraceRow({ trace, showContent }: { trace: TraceMetadata; showContent: boolean }) {
    const navigate = useNavigate();

    // Extract input preview - same logic as Traces.tsx TraceRowItem
    const inputPreview = useMemo(() => {
        // Priority 1: Direct input_preview from server
        if (trace.input_preview) {
            return String(trace.input_preview);
        }
        // Priority 2: OpenTelemetry gen_ai.prompt attributes
        const metadata = (trace.metadata || {}) as Record<string, any>;
        for (let i = 0; i <= 2; i++) {
            const roleKey = `gen_ai.prompt.${i}.role`;
            const contentKey = `gen_ai.prompt.${i}.content`;
            const role = metadata[roleKey];
            const content = metadata[contentKey];
            // Skip system messages, prefer user messages
            if (content && role !== 'system') {
                return String(content);
            }
        }
        // Priority 3: Prompts array in metadata
        if (metadata.prompts && Array.isArray(metadata.prompts) && metadata.prompts.length > 0) {
            const userPrompt = metadata.prompts.find((p: any) => p.role === 'user') || metadata.prompts[0];
            return String(userPrompt?.content || '');
        }
        // Priority 4: Direct input field
        if (metadata.input) {
            return String(metadata.input);
        }
        // Priority 5: Messages array
        if (metadata.messages && Array.isArray(metadata.messages)) {
            const userMsg = metadata.messages.find((m: any) => m.role === 'user');
            if (userMsg?.content) return String(userMsg.content);
        }
        return '';
    }, [trace]);

    // Extract output preview - same logic as Traces.tsx TraceRowItem
    const outputPreview = useMemo(() => {
        // Priority 1: Direct output_preview from server
        if (trace.output_preview) {
            return String(trace.output_preview);
        }
        // Priority 2: OpenTelemetry gen_ai.completion attributes
        const metadata = (trace.metadata || {}) as Record<string, any>;
        if (metadata['gen_ai.completion.0.content']) {
            return String(metadata['gen_ai.completion.0.content']);
        }
        // Priority 3: Completions array
        if (metadata.completions && Array.isArray(metadata.completions) && metadata.completions.length > 0) {
            return String(metadata.completions[0]?.content || '');
        }
        // Priority 4: Direct output field
        if (metadata.output) {
            return String(metadata.output);
        }
        // Priority 5: Response/completion field
        if (metadata.response) {
            return String(metadata.response);
        }
        if (metadata.completion) {
            return String(metadata.completion);
        }
        return '';
    }, [trace]);

    // Extract model name - same logic as Traces.tsx TraceRowItem
    const modelName = useMemo(() => {
        const metadata = (trace.metadata || {}) as Record<string, any>;
        // Priority 1: OpenTelemetry gen_ai.request.model or gen_ai.response.model
        if (metadata['gen_ai.request.model']) {
            return String(metadata['gen_ai.request.model']);
        }
        if (metadata['gen_ai.response.model']) {
            return String(metadata['gen_ai.response.model']);
        }
        // Priority 2: Direct model field in metadata
        if (metadata.model) {
            return String(metadata.model);
        }
        // Priority 3: Top-level model or display_name
        return trace.model || trace.display_name || '';
    }, [trace]);

    // Duration formatting
    const durationMs = trace.duration_us ? trace.duration_us / 1000 : 0;
    const durationText = durationMs >= 1000
        ? `${(durationMs / 1000).toFixed(1)}s`
        : `${durationMs.toFixed(0)}ms`;

    // Latency color
    const latencyColor = durationMs < 1000
        ? 'text-success'
        : durationMs < 5000
            ? 'text-warning'
            : 'text-error';

    return (
        <div
            onClick={() => navigate(`/projects/${trace.project_id}/traces/${trace.trace_id}`)}
            className="group grid cursor-pointer grid-cols-[minmax(110px,1fr)_minmax(100px,1fr)_minmax(160px,2fr)_minmax(160px,2fr)_70px_70px_80px_70px] items-center border-b border-border/50 px-4 py-3 text-sm text-textPrimary transition hover:bg-surface-hover"
        >
            {/* Trace ID / Time */}
            <div className="flex flex-col gap-0.5 min-w-0">
                <span className="font-mono text-xs text-primary truncate">
                    {trace.trace_id.substring(0, 10)}...
                </span>
                <span className="text-xs text-textTertiary">
                    {trace.timestamp_us
                        ? formatDistanceToNow(trace.timestamp_us / 1000, { addSuffix: true })
                        : '-'}
                </span>
            </div>

            {/* Model */}
            <div className="min-w-0">
                {modelName ? (
                    <span className="text-xs font-medium text-primary truncate block" title={modelName}>
                        {modelName}
                    </span>
                ) : (
                    <span className="text-xs text-textTertiary">–</span>
                )}
            </div>

            {/* Input */}
            {showContent && (
                <div className="min-w-0 pr-2">
                    {inputPreview ? (
                        <span className="text-xs text-textSecondary line-clamp-2" title={inputPreview}>
                            {inputPreview}
                        </span>
                    ) : (
                        <span className="text-xs text-textTertiary italic">No input</span>
                    )}
                </div>
            )}

            {/* Output */}
            {showContent && (
                <div className="min-w-0 pr-2">
                    {outputPreview ? (
                        <span className="text-xs text-textPrimary line-clamp-2" title={outputPreview}>
                            {outputPreview}
                        </span>
                    ) : (
                        <span className="text-xs text-textTertiary italic">No output</span>
                    )}
                </div>
            )}

            {/* Duration */}
            <div className={`text-xs font-medium ${latencyColor}`}>
                {durationMs > 0 ? durationText : '–'}
            </div>

            {/* Cost */}
            <div className="text-xs text-textSecondary">
                {trace.cost ? `$${trace.cost.toFixed(4)}` : '–'}
            </div>

            {/* Tokens */}
            <div className="text-xs text-textSecondary">
                {(trace.token_count || trace.tokens) ? (trace.token_count || trace.tokens)?.toLocaleString() : '–'}
            </div>

            {/* Status */}
            <div className="text-right">
                {trace.status === 'error' ? (
                    <span className="text-error">✗</span>
                ) : (
                    <span className="text-success">✓</span>
                )}
            </div>
        </div>
    );
}

export function TraceList({ traces, loading, emptyMessage = "No traces found", showContent = true }: TraceListProps) {
    if (loading) {
        return (
            <div className="space-y-2">
                {[1, 2, 3].map((i) => (
                    <div key={i} className="h-16 bg-surface border border-border rounded-lg animate-pulse" />
                ))}
            </div>
        );
    }

    if (traces.length === 0) {
        return (
            <div className="text-center py-12 bg-surface border border-border rounded-lg text-textSecondary">
                {emptyMessage}
            </div>
        );
    }

    return (
        <div className="bg-surface border border-border rounded-lg overflow-hidden">
            {/* Header - matching Traces.tsx structure */}
            <div className="grid grid-cols-[minmax(110px,1fr)_minmax(100px,1fr)_minmax(160px,2fr)_minmax(160px,2fr)_70px_70px_80px_70px] border-b border-border/60 px-4 py-2 text-xs font-semibold uppercase tracking-widest text-textTertiary bg-surface-elevated">
                <span>Trace / Time</span>
                <span>Model</span>
                {showContent && <span>Input</span>}
                {showContent && <span>Output</span>}
                <span>Duration</span>
                <span>Cost</span>
                <span>Tokens</span>
                <span className="text-right">Status</span>
            </div>

            {/* Rows */}
            <div className="divide-y divide-border/30">
                {traces.map((trace, index) => (
                    <TraceRow
                        key={`${trace.trace_id}-${index}`}
                        trace={trace}
                        showContent={showContent}
                    />
                ))}
            </div>
        </div>
    );
}
