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

import { useMemo, useState, useRef, useCallback } from 'react';
import { Clock, Activity, Zap, AlertTriangle, TrendingUp, ZoomIn, ZoomOut, Maximize2, RotateCcw } from 'lucide-react';

interface Span {
  span_id: string;
  trace_id: string;
  parent_span_id?: string;
  name: string;
  start_time: string | number;
  end_time: string | number;
  attributes?: Record<string, any>;
  status?: string;
}

interface TraceTimelineViewProps {
  spans: Span[];
  onSpanClick?: (span: Span) => void;
}

export function TraceTimelineView({ spans, onSpanClick }: TraceTimelineViewProps) {
  const [showCriticalPath, setShowCriticalPath] = useState(true);
  const [groupByService, setGroupByService] = useState(false);
  const [zoomLevel, setZoomLevel] = useState(1);
  const [panOffset, setPanOffset] = useState(0);
  const containerRef = useRef<HTMLDivElement>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState(0);

  const timeline = useMemo(() => calculateTimeline(spans, showCriticalPath), [spans, showCriticalPath]);

  // Zoom handlers
  const handleZoomIn = useCallback(() => {
    setZoomLevel(prev => Math.min(prev * 1.5, 10));
  }, []);

  const handleZoomOut = useCallback(() => {
    setZoomLevel(prev => Math.max(prev / 1.5, 0.5));
  }, []);

  const handleResetZoom = useCallback(() => {
    setZoomLevel(1);
    setPanOffset(0);
  }, []);

  const handleFitToWindow = useCallback(() => {
    setZoomLevel(1);
    setPanOffset(0);
  }, []);

  // Pan handlers for drag scrolling
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (zoomLevel > 1) {
      setIsDragging(true);
      setDragStart(e.clientX + panOffset);
    }
  }, [zoomLevel, panOffset]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (isDragging) {
      const newOffset = dragStart - e.clientX;
      setPanOffset(Math.max(0, Math.min(newOffset, (zoomLevel - 1) * 100)));
    }
  }, [isDragging, dragStart, zoomLevel]);

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  // Wheel zoom
  const handleWheel = useCallback((e: React.WheelEvent) => {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      if (e.deltaY < 0) {
        handleZoomIn();
      } else {
        handleZoomOut();
      }
    }
  }, [handleZoomIn, handleZoomOut]);

  return (
    <div className="bg-surface rounded-lg border border-border h-full flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-border bg-surface-elevated">
        <div className="flex items-center gap-2">
          <Activity className="w-5 h-5 text-primary" />
          <h3 className="text-lg font-semibold text-textPrimary">Waterfall Timeline</h3>
          <span className="text-sm text-textSecondary ml-2">
            Total: {timeline.totalDuration.toFixed(2)}s
          </span>
        </div>

        <div className="flex items-center gap-4">
          {/* Zoom Controls */}
          <div className="flex items-center gap-1 bg-background rounded-lg border border-border p-1">
            <button
              onClick={handleZoomOut}
              className="p-1.5 rounded hover:bg-surface transition-colors text-textSecondary hover:text-textPrimary"
              title="Zoom Out (Ctrl+Scroll)"
            >
              <ZoomOut className="w-4 h-4" />
            </button>
            <span className="px-2 text-xs font-mono text-textSecondary min-w-[3rem] text-center">
              {(zoomLevel * 100).toFixed(0)}%
            </span>
            <button
              onClick={handleZoomIn}
              className="p-1.5 rounded hover:bg-surface transition-colors text-textSecondary hover:text-textPrimary"
              title="Zoom In (Ctrl+Scroll)"
            >
              <ZoomIn className="w-4 h-4" />
            </button>
            <div className="w-px h-4 bg-border mx-1" />
            <button
              onClick={handleFitToWindow}
              className="p-1.5 rounded hover:bg-surface transition-colors text-textSecondary hover:text-textPrimary"
              title="Fit to Window"
            >
              <Maximize2 className="w-4 h-4" />
            </button>
            <button
              onClick={handleResetZoom}
              className="p-1.5 rounded hover:bg-surface transition-colors text-textSecondary hover:text-textPrimary"
              title="Reset Zoom"
            >
              <RotateCcw className="w-4 h-4" />
            </button>
          </div>

          <label className="flex items-center gap-2 text-sm text-textSecondary cursor-pointer">
            <input
              type="checkbox"
              checked={showCriticalPath}
              onChange={(e) => setShowCriticalPath(e.target.checked)}
              className="rounded border-border"
            />
            <Zap className="w-4 h-4 text-orange-500" />
            <span>Critical Path</span>
          </label>
        </div>
      </div>

      {/* Time axis ruler */}
      <div className="px-4 py-2 bg-surface-elevated border-b border-border">
        <div className="flex items-center">
          <div className="w-56" />
          <div className="flex-1 relative h-6">
            <TimeRuler
              minTime={timeline.minTime}
              maxTime={timeline.maxTime}
              zoomLevel={zoomLevel}
              panOffset={panOffset}
            />
          </div>
          <div className="w-20" />
          <div className="w-6" />
        </div>
      </div>

      {/* Legend */}
      <div className="px-4 py-2 bg-surface-elevated border-b border-border flex items-center gap-6 text-xs">
        <span className="text-textSecondary font-medium">Legend:</span>
        <div className="flex items-center gap-1.5">
          <div className="w-3 h-3 rounded bg-gradient-to-r from-red-500 to-orange-500" />
          <span className="text-textSecondary">Critical Path</span>
        </div>
        <div className="flex items-center gap-1.5">
          <div className="w-3 h-3 rounded bg-blue-500" />
          <span className="text-textSecondary">Normal Span</span>
        </div>
        <div className="flex items-center gap-1.5">
          <div className="w-3 h-3 rounded bg-orange-500" />
          <span className="text-textSecondary">External Call</span>
        </div>
        <div className="flex items-center gap-1.5">
          <div className="w-3 h-3 rounded bg-purple-500 ring-2 ring-purple-300 ring-offset-1 ring-offset-surface" />
          <span className="text-textSecondary">Parallel</span>
        </div>
        <div className="flex items-center gap-1.5">
          <div className="w-3 h-3 rounded bg-red-500" />
          <span className="text-textSecondary">Error</span>
        </div>
      </div>

      {/* Timeline */}
      <div
        ref={containerRef}
        className={`flex-1 p-4 space-y-1 overflow-y-auto overflow-x-hidden ${isDragging ? 'cursor-grabbing' : zoomLevel > 1 ? 'cursor-grab' : ''}`}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onWheel={handleWheel}
      >
        {timeline.spans.length === 0 ? (
          <div className="flex items-center justify-center h-full text-textTertiary">
            <div className="text-center">
              <Activity className="w-12 h-12 mx-auto mb-2 opacity-50" />
              <p>No spans to display</p>
            </div>
          </div>
        ) : (
          timeline.spans.map(span => (
            <TimelineBar
              key={span.span_id}
              span={span}
              timeline={timeline}
              showCriticalPath={showCriticalPath}
              onSpanClick={onSpanClick}
              zoomLevel={zoomLevel}
              panOffset={panOffset}
            />
          ))
        )}
      </div>

      {/* Stats Footer */}
      <div className="px-4 py-3 bg-surface-elevated border-t border-border grid grid-cols-5 gap-4 text-sm">
        <div>
          <div className="text-textTertiary">Total Spans</div>
          <div className="font-semibold text-textPrimary">{timeline.spans.length}</div>
        </div>
        <div>
          <div className="text-textTertiary">Critical Path</div>
          <div className="font-semibold text-orange-500">
            {timeline.criticalPathDuration.toFixed(2)}s
          </div>
        </div>
        <div>
          <div className="text-textTertiary">Parallelism</div>
          <div className="font-semibold text-purple-500">
            {timeline.parallelismScore.toFixed(1)}x
          </div>
        </div>
        <div>
          <div className="text-textTertiary">External Calls</div>
          <div className="font-semibold text-blue-500">{timeline.externalCalls}</div>
        </div>
        <div>
          <div className="text-textTertiary">Max Depth</div>
          <div className="font-semibold text-textPrimary">
            {Math.max(...timeline.spans.map(s => s.depth), 0)}
          </div>
        </div>
      </div>
    </div>
  );
}

// Time ruler component
function TimeRuler({ minTime, maxTime, zoomLevel, panOffset }: {
  minTime: number;
  maxTime: number;
  zoomLevel: number;
  panOffset: number;
}) {
  const totalDuration = maxTime - minTime;
  const tickCount = 10;
  const ticks: React.ReactNode[] = [];

  for (let i = 0; i <= tickCount; i++) {
    const percent = (i / tickCount) * 100;
    const time = (i / tickCount) * totalDuration;
    const timeStr = time < 1000 ? `${time.toFixed(0)}ms` : `${(time / 1000).toFixed(2)}s`;

    ticks.push(
      <div
        key={i}
        className="absolute top-0 h-full flex flex-col items-center"
        style={{ left: `${percent}%`, transform: 'translateX(-50%)' }}
      >
        <div className="h-2 w-px bg-border" />
        <span className="text-[10px] text-textTertiary mt-0.5 whitespace-nowrap">{timeStr}</span>
      </div>
    );
  }

  return (
    <div className="relative h-full" style={{ width: `${zoomLevel * 100}%`, marginLeft: `-${panOffset}%` }}>
      <div className="absolute inset-x-0 top-0 h-px bg-border" />
      {ticks}
    </div>
  );
}

interface TimelineSpan extends Span {
  startMs: number;
  endMs: number;
  durationMs: number;
  depth: number;
  isOnCriticalPath: boolean;
  isParallel: boolean;
  isExternal: boolean;
  service?: string;
}

interface Timeline {
  spans: TimelineSpan[];
  minTime: number;
  maxTime: number;
  totalDuration: number;
  criticalPathDuration: number;
  parallelismScore: number;
  externalCalls: number;
}

function calculateTimeline(spans: Span[], showCriticalPath: boolean): Timeline {
  const timelineSpans: TimelineSpan[] = spans.map(span => {
    // Handle both ISO strings and numeric timestamps (microseconds or milliseconds)
    let startMs: number;
    let endMs: number;

    if (typeof span.start_time === 'number') {
      // Assume microseconds if > year 2100 in milliseconds
      startMs = span.start_time > 4102444800000 ? span.start_time / 1000 : span.start_time;
    } else {
      startMs = new Date(span.start_time).getTime();
    }

    if (typeof span.end_time === 'number') {
      endMs = span.end_time > 4102444800000 ? span.end_time / 1000 : span.end_time;
    } else {
      endMs = new Date(span.end_time).getTime();
    }
    const service = span.attributes?.['service.name'] || span.attributes?.['service'] || 'unknown';
    const isExternal = span.attributes?.['span.kind'] === 'client' ||
      span.name.includes('http') ||
      span.name.includes('api');

    return {
      ...span,
      startMs,
      endMs,
      durationMs: endMs - startMs,
      depth: 0,
      isOnCriticalPath: false,
      isParallel: false,
      isExternal,
      service,
    };
  });

  // Calculate depths
  const depthMap = new Map<string, number>();
  const calculateDepth = (spanId: string, visited = new Set<string>()): number => {
    if (depthMap.has(spanId)) return depthMap.get(spanId)!;
    if (visited.has(spanId)) return 0;

    visited.add(spanId);
    const span = timelineSpans.find(s => s.span_id === spanId);
    if (!span || !span.parent_span_id) {
      depthMap.set(spanId, 0);
      return 0;
    }

    const depth = calculateDepth(span.parent_span_id, visited) + 1;
    depthMap.set(spanId, depth);
    return depth;
  };

  timelineSpans.forEach(span => {
    span.depth = calculateDepth(span.span_id);
  });

  // Calculate critical path (longest chain)
  if (showCriticalPath) {
    const criticalPath = findCriticalPath(timelineSpans);
    criticalPath.forEach(spanId => {
      const span = timelineSpans.find(s => s.span_id === spanId);
      if (span) span.isOnCriticalPath = true;
    });
  }

  // Detect parallel execution
  timelineSpans.forEach((span, i) => {
    const overlapping = timelineSpans.filter((other, j) =>
      i !== j &&
      other.depth === span.depth &&
      other.startMs < span.endMs &&
      other.endMs > span.startMs
    );
    span.isParallel = overlapping.length > 0;
  });

  const minTime = Math.min(...timelineSpans.map(s => s.startMs));
  const maxTime = Math.max(...timelineSpans.map(s => s.endMs));
  const totalDuration = (maxTime - minTime) / 1000;

  // BUG-05 FIX: Critical path duration should be the wall-clock time of the longest chain,
  // not the sum of all span durations (which double-counts nested spans)
  const criticalPathSpans = timelineSpans.filter(s => s.isOnCriticalPath);
  const criticalPathDuration = criticalPathSpans.length > 0
    ? (Math.max(...criticalPathSpans.map(s => s.endMs)) - Math.min(...criticalPathSpans.map(s => s.startMs))) / 1000
    : 0;

  // Calculate parallelism score
  const totalSpanTime = timelineSpans.reduce((sum, s) => sum + s.durationMs, 0) / 1000;
  const parallelismScore = totalDuration > 0 ? totalSpanTime / totalDuration : 1;

  // Count external calls
  const externalCalls = timelineSpans.filter(s => s.isExternal).length;

  return {
    spans: timelineSpans,
    minTime,
    maxTime,
    totalDuration,
    criticalPathDuration,
    parallelismScore,
    externalCalls,
  };
}

function findCriticalPath(spans: TimelineSpan[]): string[] {
  // Build dependency graph
  const graph = new Map<string, string[]>();
  const durations = new Map<string, number>();

  spans.forEach(span => {
    durations.set(span.span_id, span.durationMs);
    if (span.parent_span_id) {
      const children = graph.get(span.parent_span_id) || [];
      children.push(span.span_id);
      graph.set(span.parent_span_id, children);
    }
  });

  // Find longest path using DFS
  const memo = new Map<string, { path: string[]; duration: number }>();

  const dfs = (spanId: string): { path: string[]; duration: number } => {
    if (memo.has(spanId)) return memo.get(spanId)!;

    const children = graph.get(spanId) || [];
    if (children.length === 0) {
      const result = { path: [spanId], duration: durations.get(spanId) || 0 };
      memo.set(spanId, result);
      return result;
    }

    let maxPath = { path: [] as string[], duration: 0 };
    children.forEach(childId => {
      const childPath = dfs(childId);
      if (childPath.duration > maxPath.duration) {
        maxPath = childPath;
      }
    });

    const result = {
      path: [spanId, ...maxPath.path],
      duration: (durations.get(spanId) || 0) + maxPath.duration,
    };
    memo.set(spanId, result);
    return result;
  };

  // Find root spans
  const roots = spans.filter(s => !s.parent_span_id);
  let longestPath = { path: [] as string[], duration: 0 };

  roots.forEach(root => {
    const path = dfs(root.span_id);
    if (path.duration > longestPath.duration) {
      longestPath = path;
    }
  });

  return longestPath.path;
}

function TimelineBar({
  span,
  timeline,
  showCriticalPath,
  onSpanClick,
  zoomLevel,
  panOffset,
}: {
  span: TimelineSpan;
  timeline: Timeline;
  showCriticalPath: boolean;
  onSpanClick?: (span: any) => void;
  zoomLevel: number;
  panOffset: number;
}) {
  const leftPercent = ((span.startMs - timeline.minTime) / (timeline.maxTime - timeline.minTime)) * 100 * zoomLevel - panOffset;
  const widthPercent = (span.durationMs / (timeline.maxTime - timeline.minTime)) * 100 * zoomLevel;
  const duration = (span.durationMs / 1000).toFixed(3);

  const hasError = span.status === 'error';
  const isCritical = showCriticalPath && span.isOnCriticalPath;

  // Determine bar color
  let barColor = 'bg-blue-500';
  if (hasError) {
    barColor = 'bg-red-500';
  } else if (isCritical) {
    barColor = 'bg-gradient-to-r from-red-500 to-orange-500';
  } else if (span.isExternal) {
    barColor = 'bg-orange-500';
  } else if (span.isParallel) {
    barColor = 'bg-purple-500';
  }

  return (
    <div className="group relative">
      <div
        className="flex items-center gap-3 py-1.5 hover:bg-surface-hover transition-colors rounded cursor-pointer"
        style={{ paddingLeft: `${span.depth * 20}px` }}
        onClick={() => onSpanClick?.(span)}
      >
        {/* Span name and service */}
        <div className="w-56 flex flex-col flex-shrink-0">
          <div className="text-sm text-textPrimary truncate font-medium">{span.name}</div>
          <div className="text-xs text-textTertiary truncate">{span.service}</div>
        </div>

        {/* Timeline bar container */}
        <div className="flex-1 relative h-7 overflow-hidden">
          {/* Background track with grid lines */}
          <div className="absolute inset-0 bg-background rounded border border-border" />

          {/* Timeline bar */}
          <div
            className={`absolute h-full rounded transition-all hover:opacity-90 cursor-pointer shadow-sm ${barColor}`}
            style={{
              left: `${leftPercent}%`,
              width: `${Math.max(widthPercent, 0.5)}%`,
              minWidth: '4px',
            }}
          >
            {/* Critical path indicator */}
            {isCritical && (
              <div className="absolute -top-1 -right-1 bg-orange-500 rounded-full p-0.5">
                <Zap className="w-3 h-3 text-white" fill="white" />
              </div>
            )}

            {/* Parallel execution indicator */}
            {span.isParallel && !isCritical && (
              <div className="absolute inset-0 border-2 border-purple-300 rounded animate-pulse" />
            )}

            {/* Hover tooltip */}
            <div className="absolute bottom-full left-0 mb-2 px-3 py-2 bg-gray-900 text-white rounded-lg shadow-xl opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-50 text-xs">
              <div className="font-semibold">{span.name}</div>
              <div className="mt-1 space-y-1">
                <div className="flex items-center gap-2">
                  <Clock className="w-3 h-3" />
                  <span>Duration: {duration}s</span>
                </div>
                <div>Service: {span.service}</div>
                {isCritical && (
                  <div className="flex items-center gap-1 text-orange-600 dark:text-orange-400">
                    <TrendingUp className="w-3 h-3" />
                    <span>Critical Path</span>
                  </div>
                )}
                {span.isParallel && <div className="text-purple-600 dark:text-purple-400">Parallel Execution</div>}
                {span.isExternal && <div className="text-orange-600 dark:text-orange-400">External Call</div>}
                {hasError && <div className="text-red-600 dark:text-red-400">Error</div>}
              </div>
            </div>
          </div>
        </div>

        {/* Duration label */}
        <div className="w-20 text-right text-sm text-textSecondary font-mono flex-shrink-0">
          {duration}s
        </div>

        {/* Status icons */}
        <div className="w-6 flex justify-center flex-shrink-0">
          {hasError && <AlertTriangle className="w-4 h-4 text-red-500" />}
          {isCritical && !hasError && <Zap className="w-4 h-4 text-orange-500" />}
        </div>
      </div>
    </div>
  );
}
