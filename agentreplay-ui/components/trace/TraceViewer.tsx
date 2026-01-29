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

/**
 * Complete Trace Viewer with All Features
 * 
 * This demonstrates how to integrate all the UI components:
 * - Virtual scrolling trace tree
 * - Flame graph visualization
 * - Search and filtering
 * - Timeline with critical path
 * - Tabbed span detail panel
 * - Statistics dashboard
 */

import React, { useState, useEffect } from 'react';
import { API_BASE_URL } from '../../src/lib/agentreplay-api';

// Import all components (adjust paths based on your project structure)
import { TraceTree, type Span } from './TraceTree';
import { FlameGraph } from './FlameGraph';
import { SpanSearchFilter } from './SpanSearchFilter';
import { TraceTimelineView } from './TraceTimelineView';
import { SpanDetailPanel } from './SpanDetailPanel';
import { TraceStatisticsDashboard } from './TraceStatisticsDashboard';
import { TraceGraphView } from './TraceGraphView';

interface TraceViewerProps {
  traceId: string;
  tenantId: number;
  projectId: number;
}

export function TraceViewer({ traceId, tenantId, projectId }: TraceViewerProps) {
  const [spans, setSpans] = useState<Span[]>([]);
  const [filteredSpans, setFilteredSpans] = useState<Span[]>([]);
  const [selectedSpan, setSelectedSpan] = useState<Span | null>(null);
  const [activeTab, setActiveTab] = useState<'tree' | 'canvas' | 'flame' | 'timeline' | 'analytics'>('tree');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Load trace data with auto-retry for resilience
  useEffect(() => {
    let retryCount = 0;
    const maxRetries = 3;

    async function loadTrace() {
      setLoading(true);
      setError(null);

      for (let attempt = 0; attempt <= maxRetries; attempt++) {
        try {
          // Calculate exponential backoff delay (0ms, 1s, 2s, 4s)
          if (attempt > 0) {
            const delayMs = Math.pow(2, attempt - 1) * 1000;
            console.log(`Retrying trace load after ${delayMs}ms (attempt ${attempt + 1}/${maxRetries + 1})...`);
            await new Promise(resolve => setTimeout(resolve, delayMs));
          }

          // Fetch trace observations
          const response = await fetch(`${API_BASE_URL}/api/v1/traces/${traceId}/observations`, {
            headers: {
              'X-Tenant-ID': tenantId.toString(),
              'X-Project-ID': projectId.toString(),
            },
          });

          if (!response.ok) {
            // For 5xx errors or network issues, retry
            if (response.status >= 500 && attempt < maxRetries) {
              retryCount = attempt + 1;
              console.warn(`Server error (${response.status}), will retry...`);
              continue;
            }
            throw new Error(`Failed to load trace: ${response.statusText}`);
          }

          const data = await response.json();

          // Convert to Span format
          const convertedSpans = convertObservationsToSpans(data);
          setSpans(convertedSpans);
          setFilteredSpans(convertedSpans);
          setLoading(false);
          return; // Success!

        } catch (err) {
          // Network errors: retry if attempts remain
          if (attempt < maxRetries && (err instanceof TypeError || (err as Error).message.includes('fetch'))) {
            retryCount = attempt + 1;
            console.warn(`Network error, will retry...`, err);
            continue;
          }

          // Final failure after all retries
          setError(err instanceof Error ? err.message : 'Unknown error');
          setLoading(false);
          return;
        }
      }
    }

    loadTrace();
  }, [traceId, tenantId, projectId]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-96">
        <div className="text-center">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mx-auto mb-4" />
          <p className="text-gray-600">Loading trace data...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-lg p-6">
        <h3 className="text-red-900 font-semibold mb-2">Error Loading Trace</h3>
        <p className="text-red-700">{error}</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header with Stats */}
      <div className="bg-white rounded-lg border border-gray-200 p-6">
        <h1 className="text-2xl font-bold text-gray-900 mb-2">
          Trace: {traceId}
        </h1>
        <div className="flex items-center gap-6 text-sm text-gray-600">
          <div>
            <span className="font-medium">Total Spans:</span> {spans.length}
          </div>
          <div>
            <span className="font-medium">Filtered:</span> {filteredSpans.length}
          </div>
          {selectedSpan && (
            <div className="text-blue-600">
              <span className="font-medium">Selected:</span> {selectedSpan.name}
            </div>
          )}
        </div>
      </div>

      {/* Search & Filter Bar */}
      <SpanSearchFilter
        spans={spans}
        onFilteredSpansChange={setFilteredSpans}
      />

      {/* Visualization Tabs - Simple version without shadcn/ui */}
      <div className="bg-white rounded-lg border border-gray-200">
        <div className="border-b border-gray-200">
          <nav className="flex gap-4 px-4">
            {['tree', 'canvas', 'flame', 'timeline', 'analytics'].map((tab) => (
              <button
                key={tab}
                onClick={() => setActiveTab(tab as any)}
                className={`
                  px-4 py-3 text-sm font-medium border-b-2 transition-colors
                  ${activeTab === tab
                    ? 'border-blue-600 text-blue-600'
                    : 'border-transparent text-gray-600 hover:text-gray-900'
                  }
                `}
              >
                {tab.charAt(0).toUpperCase() + tab.slice(1)}
              </button>
            ))}
          </nav>
        </div>

        <div className="p-6">
          {/* Tree View */}
          {activeTab === 'tree' && (
            <TraceTree
              spans={filteredSpans}
              onSelectSpan={setSelectedSpan}
              selectedSpanId={selectedSpan?.id}
            />
          )}

          {/* Canvas View */}
          {activeTab === 'canvas' && (
            <TraceGraphView
              traceId={traceId}
              tenantId={tenantId}
              projectId={projectId}
            />
          )}

          {/* Flame Graph */}
          {activeTab === 'flame' && (
            <FlameGraph
              spans={filteredSpans}
              onSelectSpan={setSelectedSpan}
              selectedSpanId={selectedSpan?.id}
            />
          )}

          {/* Timeline */}
          {activeTab === 'timeline' && (
            <TraceTimelineView
              spans={convertSpansToTimelineFormat(filteredSpans)}
            />
          )}

          {/* Analytics */}
          {activeTab === 'analytics' && (
            <TraceStatisticsDashboard
              traces={[convertSpansToTraceFormat(spans, traceId)]}
              spans={filteredSpans}
            />
          )}
        </div>
      </div>

      {/* Span Detail Panel (Slides in from right) */}
      {selectedSpan && (
        <SpanDetailPanel
          span={selectedSpan}
          onClose={() => setSelectedSpan(null)}
        />
      )}
    </div>
  );
}

// Helper: Convert API observations to Span format
function convertObservationsToSpans(observations: any[]): Span[] {
  const spanMap = new Map<string, Span>();
  const rootSpans: Span[] = [];

  // First pass: create all spans
  observations.forEach((obs) => {
    const span: Span = {
      id: obs.observation_id,
      name: obs.name,
      spanType: obs.span_type?.toLowerCase() || 'unknown',
      status: obs.status || 'pending',
      startTime: obs.start_time,
      endTime: obs.end_time,
      duration: obs.duration_ms || 0,
      inputTokens: obs.input_tokens,
      outputTokens: obs.output_tokens,
      cost: obs.cost,
      children: [],
      metadata: {
        input: obs.input,
        output: obs.output,
        error: obs.error,
        model: obs.model,
        provider: obs.provider,
        ...obs.attributes,
      },
    };

    spanMap.set(span.id, span);

    if (!obs.parent_observation_id) {
      rootSpans.push(span);
    }
  });

  // Second pass: build parent-child relationships
  observations.forEach((obs) => {
    if (obs.parent_observation_id) {
      const parent = spanMap.get(obs.parent_observation_id);
      const child = spanMap.get(obs.observation_id);
      if (parent && child) {
        parent.children = parent.children || [];
        parent.children.push(child);
      }
    }
  });

  return rootSpans;
}

// Helper: Convert spans for timeline view
function convertSpansToTimelineFormat(spans: Span[]): any[] {
  const flatSpans: any[] = [];

  function flatten(span: Span, parentId?: string) {
    flatSpans.push({
      span_id: span.id,
      trace_id: span.id, // Use span id as trace id for now
      parent_span_id: parentId,
      name: span.name,
      start_time: new Date(span.startTime).toISOString(),
      end_time: new Date(span.endTime).toISOString(),
      attributes: span.metadata,
      status: span.status,
    });

    if (span.children) {
      span.children.forEach((child) => flatten(child, span.id));
    }
  }

  spans.forEach((span) => flatten(span));
  return flatSpans;
}

// Helper: Convert spans to trace format for analytics
function convertSpansToTraceFormat(spans: Span[], traceId: string): any {
  let totalDuration = 0;
  let totalCost = 0;
  let totalInputTokens = 0;
  let totalOutputTokens = 0;
  let spanCount = 0;
  let hasError = false;

  function traverse(span: Span) {
    spanCount++;
    totalDuration = Math.max(totalDuration, span.duration);
    totalCost += span.cost || 0;
    totalInputTokens += span.inputTokens || 0;
    totalOutputTokens += span.outputTokens || 0;
    if (span.status === 'error') hasError = true;

    if (span.children) {
      span.children.forEach(traverse);
    }
  }

  spans.forEach(traverse);

  return {
    id: traceId,
    name: spans[0]?.name || 'Unknown',
    startTime: spans[0]?.startTime || new Date().toISOString(),
    endTime: spans[0]?.endTime || new Date().toISOString(),
    duration: totalDuration,
    spanCount,
    status: hasError ? 'error' : 'success',
    cost: totalCost,
    inputTokens: totalInputTokens,
    outputTokens: totalOutputTokens,
  };
}

export default TraceViewer;
