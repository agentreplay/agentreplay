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
import { Flame, ZoomIn, ZoomOut, RotateCcw } from 'lucide-react';
import { Span } from './TraceTree';

export interface FlameGraphProps {
  spans: Span[];
  onSelectSpan?: (span: Span) => void;
  selectedSpanId?: string;
}

interface FlameNode {
  span: Span;
  x: number;
  y: number;
  width: number;
  height: number;
  depth: number;
}

const ROW_HEIGHT = 40;
const MIN_WIDTH = 2; // Minimum width to render

export function FlameGraph({ spans, onSelectSpan, selectedSpanId }: FlameGraphProps) {
  const [zoom, setZoom] = useState(1);
  const [focusedSpan, setFocusedSpan] = useState<Span | null>(null);

  // Build flame graph layout
  const flameNodes = useMemo(() => {
    if (spans.length === 0) return [];

    const nodes: FlameNode[] = [];
    const baseSpan = focusedSpan || spans[0];
    const totalDuration = baseSpan.duration;

    const processSpan = (span: Span, depth: number, startOffset: number, parentWidth: number) => {
      const width = (span.duration / totalDuration) * parentWidth;

      if (width * zoom < MIN_WIDTH) return; // Skip if too small

      nodes.push({
        span,
        x: startOffset,
        y: depth * ROW_HEIGHT,
        width,
        height: ROW_HEIGHT - 2,
        depth,
      });

      if (span.children && span.children.length > 0) {
        let childOffset = startOffset;
        span.children.forEach((child) => {
          processSpan(child, depth + 1, childOffset, width);
          childOffset += (child.duration / totalDuration) * parentWidth;
        });
      }
    };

    processSpan(baseSpan, 0, 0, 1000); // Use 1000 as base width for percentage calculation
    return nodes;
  }, [spans, focusedSpan, zoom]);

  const maxDepth = useMemo(() => {
    return Math.max(...flameNodes.map(n => n.depth), 0);
  }, [flameNodes]);

  const handleSpanClick = (node: FlameNode) => {
    if (onSelectSpan) {
      onSelectSpan(node.span);
    }
  };

  const handleZoomIn = () => setZoom(prev => Math.min(prev * 1.5, 10));
  const handleZoomOut = () => setZoom(prev => Math.max(prev / 1.5, 0.5));
  const handleReset = () => {
    setZoom(1);
    setFocusedSpan(null);
  };

  const getColorForSpan = (span: Span): string => {
    // Color based on span type and status
    if (span.status === 'error') return 'rgb(239, 68, 68)'; // red-500

    const colors: Record<string, string> = {
      root: 'rgb(147, 51, 234)', // purple-600
      planning: 'rgb(99, 102, 241)', // indigo-500
      reasoning: 'rgb(59, 130, 246)', // blue-500
      tool_call: 'rgb(249, 115, 22)', // orange-500
      tool_response: 'rgb(251, 146, 60)', // orange-400
      synthesis: 'rgb(20, 184, 166)', // teal-500
      response: 'rgb(34, 197, 94)', // green-500
      llm: 'rgb(59, 130, 246)', // blue-500
      retrieval: 'rgb(34, 197, 94)', // green-500
    };

    return colors[span.spanType] || 'rgb(107, 114, 128)'; // gray-500
  };

  const formatDuration = (ms: number): string => {
    if (ms < 1) return `${(ms * 1000).toFixed(0)}μs`;
    if (ms < 1000) return `${ms.toFixed(1)}ms`;
    return `${(ms / 1000).toFixed(2)}s`;
  };

  if (spans.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
        <Flame className="w-12 h-12 mb-4 text-muted-foreground" />
        <p className="text-lg font-medium">No spans to visualize</p>
        <p className="text-sm">Add traces to see the flame graph</p>
      </div>
    );
  }

  return (
    <div className="bg-card rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="bg-secondary border-b border-border px-4 py-3 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Flame className="w-5 h-5 text-orange-500" />
          <h3 className="font-semibold text-foreground">Flame Graph</h3>
          <span className="text-sm text-muted-foreground">
            {focusedSpan ? `Focused: ${focusedSpan.name}` : 'Full Trace'}
          </span>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={handleZoomOut}
            className="p-2 hover:bg-gray-200 rounded transition-colors"
            title="Zoom Out"
          >
            <ZoomOut className="w-4 h-4 text-muted-foreground" />
          </button>
          <span className="text-sm text-muted-foreground font-mono min-w-[60px] text-center">
            {(zoom * 100).toFixed(0)}%
          </span>
          <button
            onClick={handleZoomIn}
            className="p-2 hover:bg-gray-200 rounded transition-colors"
            title="Zoom In"
          >
            <ZoomIn className="w-4 h-4 text-muted-foreground" />
          </button>
          <button
            onClick={handleReset}
            className="p-2 hover:bg-gray-200 rounded transition-colors ml-2"
            title="Reset View"
          >
            <RotateCcw className="w-4 h-4 text-muted-foreground" />
          </button>
        </div>
      </div>

      {/* Flame Graph Canvas */}
      <div className="overflow-x-auto overflow-y-auto min-h-[400px] max-h-[800px] p-6 bg-secondary">
        <div
          style={{
            width: `${1000 * zoom}px`,
            height: `${(maxDepth + 1) * ROW_HEIGHT}px`,
            position: 'relative',
          }}
        >
          {flameNodes.map((node, idx) => {
            const isSelected = selectedSpanId === node.span.id;
            const color = getColorForSpan(node.span);
            const widthPx = node.width * zoom;

            return (
              <div
                key={`${node.span.id}-${idx}`}
                className="absolute cursor-pointer hover:opacity-90 transition-opacity group"
                style={{
                  left: `${node.x * zoom}px`,
                  top: `${node.y}px`,
                  width: `${widthPx}px`,
                  height: `${node.height}px`,
                  backgroundColor: color,
                  border: isSelected ? '2px solid white' : '1px solid rgba(0,0,0,0.2)',
                  borderRadius: '2px',
                }}
                onClick={(e) => {
                  e.stopPropagation();
                  handleSpanClick(node);
                }}
                onDoubleClick={(e) => {
                  e.stopPropagation();
                  setFocusedSpan(node.span);
                }}
              >
                {/* Span label - only show if wide enough */}
                {widthPx > 40 && (
                  <div className="px-2 text-sm text-white truncate font-medium leading-tight pt-1.5">
                    {node.span.name}
                  </div>
                )}
                {widthPx > 80 && (
                  <div className="px-2 text-xs text-white/80 truncate font-mono">
                    {formatDuration(node.span.duration)}
                  </div>
                )}

                {/* Hover tooltip */}
                <div className="absolute bottom-full left-0 mb-2 px-3 py-2 bg-card border border-border rounded-lg shadow-lg opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-50 text-foreground">
                  <div className="text-sm font-semibold">{node.span.name}</div>
                  <div className="text-xs text-muted-foreground mt-1">
                    <div>Type: {node.span.spanType}</div>
                    <div>Duration: {formatDuration(node.span.duration)}</div>
                    {node.span.inputTokens && (
                      <div>
                        Tokens: {node.span.inputTokens + (node.span.outputTokens || 0)}
                      </div>
                    )}
                    {node.span.cost && <div>Cost: ${node.span.cost.toFixed(6)}</div>}
                  </div>
                  <div className="text-xs text-muted-foreground mt-1 italic">
                    Double-click to focus
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Legend */}
      <div className="bg-secondary border-t border-border px-4 py-2 flex items-center gap-4 text-xs">
        <span className="text-muted-foreground font-medium">Legend:</span>
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded" style={{ backgroundColor: 'rgb(239, 68, 68)' }} />
          <span className="text-gray-700">Error</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded" style={{ backgroundColor: 'rgb(59, 130, 246)' }} />
          <span className="text-gray-700">LLM</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded" style={{ backgroundColor: 'rgb(249, 115, 22)' }} />
          <span className="text-gray-700">Tool Call</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded" style={{ backgroundColor: 'rgb(34, 197, 94)' }} />
          <span className="text-gray-700">Response</span>
        </div>
        <span className="text-muted-foreground ml-auto">Click to select • Double-click to focus</span>
      </div>
    </div>
  );
}

export default FlameGraph;
