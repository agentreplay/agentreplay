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

import { useEffect, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { API_BASE_URL } from '../../src/lib/flowtrace-api';
import { 
  X, 
  ChevronDown, 
  ChevronRight, 
  Clock, 
  DollarSign, 
  Zap,
  AlertCircle,
  Copy,
  Check,
  Database
} from 'lucide-react';

interface SpanData {
  span_id: string;
  parent_span_id?: string;
  name: string;
  type: string;
  start_time_us: number;
  duration_ms: number;
  cost: number;
  tokens?: number;
  status: 'success' | 'error';
  input?: any;
  output?: any;
  metadata?: Record<string, any>;
  children?: SpanData[];
}

interface TraceDetails {
  trace_id: string;
  root_span: SpanData;
  total_duration_ms: number;
  total_cost: number;
  total_tokens: number;
  status: 'success' | 'error';
  metadata?: Record<string, any>;
}

export function TraceWaterfallModal({ 
  traceId, 
  onClose 
}: { 
  traceId: string;
  onClose: () => void;
}) {
  const [trace, setTrace] = useState<TraceDetails | null>(null);
  const [loading, setLoading] = useState(true);
  const [expandedSpans, setExpandedSpans] = useState<Set<string>>(new Set());
  const [selectedSpan, setSelectedSpan] = useState<SpanData | null>(null);
  const [copiedField, setCopiedField] = useState<string | null>(null);

  useEffect(() => {
    // Fetch trace details
    fetch(`${API_BASE_URL}/api/v1/traces/${traceId}`)
      .then(res => res.json())
      .then(data => {
        setTrace(data);
        // Auto-expand root
        setExpandedSpans(new Set([data.root_span.span_id]));
        setSelectedSpan(data.root_span);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [traceId]);

  const toggleSpan = (spanId: string) => {
    setExpandedSpans(prev => {
      const next = new Set(prev);
      if (next.has(spanId)) {
        next.delete(spanId);
      } else {
        next.add(spanId);
      }
      return next;
    });
  };

  const copyToClipboard = (text: string, field: string) => {
    navigator.clipboard.writeText(JSON.stringify(text, null, 2));
    setCopiedField(field);
    setTimeout(() => setCopiedField(null), 2000);
  };

  const renderSpan = (span: SpanData, depth: number = 0, startTime: number = 0) => {
    const isExpanded = expandedSpans.has(span.span_id);
    const hasChildren = span.children && span.children.length > 0;
    const relativeStart = (span.start_time_us - startTime) / 1000; // Convert to ms

    return (
      <div key={span.span_id}>
        <button
          onClick={() => {
            if (hasChildren) toggleSpan(span.span_id);
            setSelectedSpan(span);
          }}
          className={`w-full flex items-center gap-3 px-4 py-2 hover:bg-surface-hover transition-colors text-left ${
            selectedSpan?.span_id === span.span_id ? 'bg-surface-elevated border-l-2 border-primary' : ''
          }`}
          style={{ paddingLeft: `${depth * 24 + 16}px` }}
        >
          {hasChildren ? (
            <button
              onClick={(e) => {
                e.stopPropagation();
                toggleSpan(span.span_id);
              }}
              className="flex-shrink-0"
            >
              {isExpanded ? (
                <ChevronDown className="w-4 h-4 text-textSecondary" />
              ) : (
                <ChevronRight className="w-4 h-4 text-textSecondary" />
              )}
            </button>
          ) : (
            <div className="w-4" />
          )}
          
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium text-textPrimary truncate">
                {span.name}
              </span>
              <span className="text-xs text-textTertiary px-2 py-0.5 rounded bg-surface border border-border-subtle">
                {span.type}
              </span>
            </div>
            <div className="flex items-center gap-4 mt-1">
              <span className="text-xs text-textSecondary">
                {(relativeStart || 0).toFixed(0)}ms
              </span>
              <div className="flex items-center gap-1 text-xs text-textSecondary">
                <Clock className="w-3 h-3" />
                {(span.duration_ms || 0).toFixed(0)}ms
              </div>
              {(span.cost || 0) > 0 && (
                <div className="flex items-center gap-1 text-xs text-textSecondary">
                  <DollarSign className="w-3 h-3" />
                  ${(span.cost || 0).toFixed(4)}
                </div>
              )}
              {span.status === 'error' && (
                <AlertCircle className="w-3 h-3 text-red-500" />
              )}
            </div>
          </div>

          {/* Visual timeline bar */}
          <div className="w-48 h-6 bg-surface-elevated rounded overflow-hidden relative">
            <div
              className={`absolute h-full rounded ${
                span.status === 'error' ? 'bg-red-500' : 'bg-primary'
              }`}
              style={{
                left: `${((relativeStart || 0) / (trace?.total_duration_ms || 1)) * 100}%`,
                width: `${((span.duration_ms || 0) / (trace?.total_duration_ms || 1)) * 100}%`,
                minWidth: '2px',
              }}
            />
          </div>
        </button>

        {isExpanded && hasChildren && (
          <div>
            {span.children!.map(child => 
              renderSpan(child, depth + 1, startTime)
            )}
          </div>
        )}
      </div>
    );
  };

  return (
    <AnimatePresence>
      <div className="fixed inset-0 z-[100] flex items-center justify-center">
        {/* Backdrop */}
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={onClose}
          className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        />

        {/* Modal */}
        <motion.div
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.95 }}
          className="relative w-full max-w-[95vw] h-[90vh] bg-surface rounded-xl border border-border shadow-2xl flex flex-col"
        >
          {/* Header */}
          <div className="flex items-center justify-between px-6 py-4 border-b border-border">
            <div>
              <h2 className="text-xl font-bold text-textPrimary">Trace Waterfall</h2>
              <p className="text-sm text-textSecondary mt-1">ID: {traceId}</p>
            </div>
            <div className="flex items-center gap-4">
              {trace && (
                <div className="flex items-center gap-6 text-sm">
                  <div className="flex items-center gap-2">
                    <Clock className="w-4 h-4 text-yellow-500" />
                    <span className="text-textPrimary">{(trace.total_duration_ms || 0).toFixed(0)}ms</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <DollarSign className="w-4 h-4 text-green-500" />
                    <span className="text-textPrimary">${(trace.total_cost || 0).toFixed(4)}</span>
                  </div>
                  {(trace.total_tokens || 0) > 0 && (
                    <div className="flex items-center gap-2">
                      <Zap className="w-4 h-4 text-blue-500" />
                      <span className="text-textPrimary">{(trace.total_tokens || 0).toLocaleString()} tokens</span>
                    </div>
                  )}
                </div>
              )}
              <button
                onClick={onClose}
                className="p-2 rounded-lg hover:bg-surface-hover transition-colors"
              >
                <X className="w-5 h-5 text-textSecondary" />
              </button>
            </div>
          </div>

          {loading ? (
            <div className="flex-1 flex items-center justify-center">
              <div className="text-center">
                <div className="w-12 h-12 border-4 border-primary border-t-transparent rounded-full animate-spin mx-auto mb-4" />
                <p className="text-textSecondary">Loading trace details...</p>
              </div>
            </div>
          ) : trace && trace.root_span ? (
            <div className="flex-1 flex overflow-hidden">
              {/* Left: Waterfall */}
              <div className="flex-1 overflow-y-auto border-r border-border">
                <div className="py-2">
                  {renderSpan(trace.root_span, 0, trace.root_span.start_time_us || 0)}
                </div>
              </div>

              {/* Right: Span Details */}
              <div className="w-[500px] overflow-y-auto">
                {selectedSpan ? (
                  <div className="p-6 space-y-6">
                    <div>
                      <h3 className="text-lg font-bold text-textPrimary mb-2">
                        {selectedSpan.name}
                      </h3>
                      <span className="text-xs text-textTertiary px-2 py-1 rounded bg-surface-elevated border border-border-subtle">
                        {selectedSpan.type}
                      </span>
                    </div>

                    {selectedSpan.input && (
                      <div>
                        <div className="flex items-center justify-between mb-2">
                          <h4 className="text-sm font-semibold text-textPrimary">Input</h4>
                          <button
                            onClick={() => copyToClipboard(selectedSpan.input, 'input')}
                            className="flex items-center gap-1 px-2 py-1 rounded text-xs text-textSecondary hover:text-textPrimary"
                          >
                            {copiedField === 'input' ? (
                              <>
                                <Check className="w-3 h-3 text-green-500" />
                                Copied
                              </>
                            ) : (
                              <>
                                <Copy className="w-3 h-3" />
                                Copy
                              </>
                            )}
                          </button>
                        </div>
                        <pre className="p-3 rounded-lg bg-surface-elevated border border-border-subtle text-xs text-textSecondary overflow-x-auto max-h-60">
                          {JSON.stringify(selectedSpan.input, null, 2)}
                        </pre>
                      </div>
                    )}

                    {selectedSpan.output && (
                      <div>
                        <div className="flex items-center justify-between mb-2">
                          <h4 className="text-sm font-semibold text-textPrimary">Output</h4>
                          <button
                            onClick={() => copyToClipboard(selectedSpan.output, 'output')}
                            className="flex items-center gap-1 px-2 py-1 rounded text-xs text-textSecondary hover:text-textPrimary"
                          >
                            {copiedField === 'output' ? (
                              <>
                                <Check className="w-3 h-3 text-green-500" />
                                Copied
                              </>
                            ) : (
                              <>
                                <Copy className="w-3 h-3" />
                                Copy
                              </>
                            )}
                          </button>
                        </div>
                        <pre className="p-3 rounded-lg bg-surface-elevated border border-border-subtle text-xs text-textSecondary overflow-x-auto max-h-60">
                          {JSON.stringify(selectedSpan.output, null, 2)}
                        </pre>
                      </div>
                    )}

                    {selectedSpan.metadata && Object.keys(selectedSpan.metadata).length > 0 && (
                      <div>
                        <h4 className="text-sm font-semibold text-textPrimary mb-2">Metadata</h4>
                        <div className="space-y-2">
                          {Object.entries(selectedSpan.metadata).map(([key, value]) => (
                            <div key={key} className="flex items-start gap-2">
                              <span className="text-xs text-textTertiary min-w-[100px]">{key}:</span>
                              <span className="text-xs text-textPrimary flex-1 break-all">
                                {typeof value === 'object' ? JSON.stringify(value) : String(value)}
                              </span>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                ) : (
                  <div className="flex items-center justify-center h-full text-textTertiary">
                    <div className="text-center">
                      <Database className="w-12 h-12 mx-auto mb-4 opacity-50" />
                      <p>Select a span to view details</p>
                    </div>
                  </div>
                )}
              </div>
            </div>
          ) : (
            <div className="flex-1 flex items-center justify-center">
              <div className="text-center">
                <AlertCircle className="w-12 h-12 text-red-500 mx-auto mb-4" />
                <p className="text-textPrimary">Failed to load trace</p>
              </div>
            </div>
          )}
        </motion.div>
      </div>
    </AnimatePresence>
  );
}
