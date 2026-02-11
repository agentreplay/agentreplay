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

"use client";

import React, { useMemo, useState } from 'react';
import { Span } from './TraceTree';
import { Clock, AlertCircle, CheckCircle, Zap, Database, MessageSquare, Activity } from 'lucide-react';

interface TraceFlameGraphProps {
    spans: Span[];
    onSelectSpan?: (span: Span) => void;
    selectedSpanId?: string;
}

interface FlameGraphNode {
    span: Span;
    width: number; // percentage 0-100
    left: number; // percentage 0-100
    depth: number;
    children: FlameGraphNode[];
}

const spanTypeColors: Record<string, string> = {
    root: 'bg-purple-200 hover:bg-purple-300 border-purple-400 text-purple-900',
    planning: 'bg-indigo-200 hover:bg-indigo-300 border-indigo-400 text-indigo-900',
    reasoning: 'bg-blue-200 hover:bg-blue-300 border-blue-400 text-blue-900',
    tool_call: 'bg-orange-200 hover:bg-orange-300 border-orange-400 text-orange-900',
    tool_response: 'bg-amber-200 hover:bg-amber-300 border-amber-400 text-amber-900',
    synthesis: 'bg-teal-200 hover:bg-teal-300 border-teal-400 text-teal-900',
    response: 'bg-green-200 hover:bg-green-300 border-green-400 text-green-900',
    error: 'bg-red-200 hover:bg-red-300 border-red-400 text-red-900',
    llm: 'bg-blue-200 hover:bg-blue-300 border-blue-400 text-blue-900',
    retrieval: 'bg-green-200 hover:bg-green-300 border-green-400 text-green-900',
};

export function TraceFlameGraph({ spans, onSelectSpan, selectedSpanId }: TraceFlameGraphProps) {
    const [hoveredSpan, setHoveredSpan] = useState<Span | null>(null);

    // Calculate the total duration and start time of the trace
    const { startTime, totalDuration } = useMemo(() => {
        if (spans.length === 0) return { startTime: 0, totalDuration: 0 };

        const start = Math.min(...spans.map(s => s.startTime));
        const end = Math.max(...spans.map(s => s.endTime));
        return { startTime: start, totalDuration: end - start };
    }, [spans]);

    // Build the flame graph structure
    const flameGraphNodes = useMemo(() => {
        if (totalDuration === 0) return [];

        const buildNodes = (nodes: Span[], depth: number): FlameGraphNode[] => {
            return nodes.map(span => {
                // Calculate relative position and width
                // Ensure minimum width for visibility (0.5%)
                const relativeStart = span.startTime - startTime;
                const left = (relativeStart / totalDuration) * 100;
                const width = Math.max((span.duration / totalDuration) * 100, 0.5);

                return {
                    span,
                    width,
                    left,
                    depth,
                    children: span.children ? buildNodes(span.children, depth + 1) : []
                };
            });
        };

        // Filter for root nodes (those without parents in the current list, or explicitly marked as root)
        // For simplicity, we'll assume the provided 'spans' list contains the roots if it's a tree structure,
        // but if it's a flat list we might need to reconstruct. 
        // Based on TraceTree.tsx, 'spans' seems to be the top-level nodes.
        return buildNodes(spans, 0);
    }, [spans, startTime, totalDuration]);

    // Flatten nodes for rendering to handle depth layout easily
    const flattenedNodes = useMemo(() => {
        const flat: FlameGraphNode[] = [];
        const traverse = (nodes: FlameGraphNode[]) => {
            nodes.forEach(node => {
                flat.push(node);
                traverse(node.children);
            });
        };
        traverse(flameGraphNodes);
        return flat;
    }, [flameGraphNodes]);

    const maxDepth = useMemo(() => {
        return Math.max(...flattenedNodes.map(n => n.depth), 0);
    }, [flattenedNodes]);

    const rowHeight = 24;
    const totalHeight = (maxDepth + 1) * (rowHeight + 2) + 40; // + padding

    if (spans.length === 0) {
        return (
            <div className="flex flex-col items-center justify-center py-12 text-muted-foreground bg-card rounded-lg border border-border h-[400px]">
                <Activity className="w-12 h-12 mb-4 text-muted-foreground" />
                <p className="text-lg font-medium">No data for Flame Graph</p>
            </div>
        );
    }

    return (
        <div className="bg-card rounded-lg border border-border overflow-hidden flex flex-col h-[600px]">
            {/* Header / Legend / Controls could go here */}
            <div className="bg-secondary border-b border-border px-3 py-2 flex justify-between items-center flex-shrink-0">
                <div className="text-xs font-medium text-muted-foreground uppercase">Flame Graph</div>
                <div className="text-xs text-muted-foreground">
                    Total Duration: {totalDuration.toFixed(2)}ms
                </div>
            </div>

            {/* Tooltip Area */}
            <div className="h-8 bg-card border-b border-border px-3 flex items-center text-xstext-foreground flex-shrink-0">
                {hoveredSpan ? (
                    <div className="flex items-center gap-2">
                        <span className={`w-2 h-2 rounded-full ${spanTypeColors[hoveredSpan.spanType]?.split(' ')[0] || 'bg-gray-400'}`} />
                        <span className="font-bold">{hoveredSpan.name}</span>
                        <span className="text-muted-foreground">|</span>
                        <span className="font-mono">{hoveredSpan.duration.toFixed(2)}ms</span>
                        <span className="text-muted-foreground">|</span>
                        <span className="capitalize">{hoveredSpan.spanType}</span>
                        {hoveredSpan.status === 'error' && (
                            <>
                                <span className="text-muted-foreground">|</span>
                                <span className="text-red-600 flex items-center gap-1">
                                    <AlertCircle className="w-3 h-3" /> Error
                                </span>
                            </>
                        )}
                    </div>
                ) : (
                    <span className="text-gray-400 italic">Hover over a bar to see details</span>
                )}
            </div>

            {/* Graph Area */}
            <div className="flex-1 overflow-auto relative p-4">
                <div style={{ height: totalHeight, minWidth: '100%', position: 'relative' }}>
                    {flattenedNodes.map((node) => {
                        const isSelected = selectedSpanId === node.span.id;
                        const colorClass = spanTypeColors[node.span.spanType] || 'bg-gray-200 border-gray-300text-foreground';

                        return (
                            <div
                                key={node.span.id}
                                className={`
                  absolute border rounded-sm text-[10px] overflow-hidden whitespace-nowrap px-1 cursor-pointer transition-opacity
                  ${colorClass}
                  ${isSelected ? 'ring-2 ring-blue-500 z-10' : ''}
                  ${hoveredSpan && hoveredSpan.id !== node.span.id ? 'opacity-60' : 'opacity-100'}
                `}
                                style={{
                                    left: `${node.left}%`,
                                    width: `${node.width}%`,
                                    top: node.depth * (rowHeight + 2),
                                    height: rowHeight,
                                }}
                                onMouseEnter={() => setHoveredSpan(node.span)}
                                onMouseLeave={() => setHoveredSpan(null)}
                                onClick={() => onSelectSpan?.(node.span)}
                            >
                                {node.width > 2 && ( // Only show text if wide enough
                                    <div className="flex items-center gap-1 h-full">
                                        <span className="font-medium truncate">{node.span.name}</span>
                                        {node.width > 5 && <span className="opacity-75 text-[9px] ml-auto">{node.span.duration.toFixed(0)}ms</span>}
                                    </div>
                                )}
                            </div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
}
