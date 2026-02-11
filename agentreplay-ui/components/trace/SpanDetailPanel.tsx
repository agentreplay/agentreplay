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

import React, { useState } from 'react';
import { 
  X, Clock, Coins, Zap, AlertCircle, Copy, Check, 
  FileText, List, Link2, Code, TrendingUp, Activity 
} from 'lucide-react';
import { Span } from './TraceTree';

export interface SpanDetailPanelProps {
  span: Span | null;
  onClose: () => void;
}

type TabType = 'overview' | 'attributes' | 'events' | 'logs' | 'links' | 'performance' | 'stacktrace';

export function SpanDetailPanel({ span, onClose }: SpanDetailPanelProps) {
  const [activeTab, setActiveTab] = useState<TabType>('overview');
  const [copiedSection, setCopiedSection] = useState<string | null>(null);
  const [expandedSections, setExpandedSections] = useState<Set<string>>(new Set(['overview']));

  if (!span) return null;

  const copyToClipboard = (text: string, section: string) => {
    navigator.clipboard.writeText(text);
    setCopiedSection(section);
    setTimeout(() => setCopiedSection(null), 2000);
  };

  const toggleSection = (section: string) => {
    setExpandedSections((prev) => {
      const next = new Set(prev);
      if (next.has(section)) {
        next.delete(section);
      } else {
        next.add(section);
      }
      return next;
    });
  };

  const formatJSON = (obj: unknown) => {
    try {
      return JSON.stringify(obj, null, 2);
    } catch {
      return String(obj);
    }
  };

  const tabs: { id: TabType; label: string; icon: React.ReactNode }[] = [
    { id: 'overview', label: 'Overview', icon: <FileText className="w-4 h-4" /> },
    { id: 'attributes', label: 'Attributes', icon: <List className="w-4 h-4" /> },
    { id: 'performance', label: 'Performance', icon: <TrendingUp className="w-4 h-4" /> },
    { id: 'events', label: 'Events', icon: <Activity className="w-4 h-4" /> },
    { id: 'logs', label: 'Logs', icon: <Code className="w-4 h-4" /> },
    { id: 'links', label: 'Links', icon: <Link2 className="w-4 h-4" /> },
    { id: 'stacktrace', label: 'Stack Trace', icon: <AlertCircle className="w-4 h-4" /> },
  ];

  return (
    <div className="fixed inset-y-0 right-0 w-[700px] bg-card shadow-2xl border-l border-border overflow-hidden z-50 flex flex-col">
      {/* Header */}
      <div className="bg-gradient-to-r from-blue-600 to-blue-700 text-white px-6 py-4 flex items-center justify-between flex-shrink-0">
        <div className="flex-1 min-w-0">
          <h2 className="text-lg font-semibold truncate">{span.name}</h2>
          <p className="text-sm text-blue-100 mt-1">
            {span.spanType} • {span.status}
          </p>
        </div>
        <button
          onClick={onClose}
          className="ml-4 p-2 hover:bg-blue-500 rounded-lg transition-colors flex-shrink-0"
          aria-label="Close panel"
        >
          <X className="w-5 h-5" />
        </button>
      </div>

      {/* Tabs */}
      <div className="border-b border-border bg-secondary flex-shrink-0">
        <div className="flex overflow-x-auto">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`
                flex items-center gap-2 px-4 py-3 text-sm font-medium border-b-2 transition-colors whitespace-nowrap
                ${
                  activeTab === tab.id
                    ? 'border-blue-600 text-blue-600 bg-card'
                    : 'border-transparent text-muted-foreground hover:text-foreground hover:bg-gray-100'
                }
              `}
            >
              {tab.icon}
              <span>{tab.label}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Tab Content */}
      <div className="flex-1 overflow-y-auto p-6">
        {activeTab === 'overview' && <OverviewTab span={span} copyToClipboard={copyToClipboard} copiedSection={copiedSection} />}
        {activeTab === 'attributes' && <AttributesTab span={span} copyToClipboard={copyToClipboard} copiedSection={copiedSection} />}
        {activeTab === 'performance' && <PerformanceTab span={span} />}
        {activeTab === 'events' && <EventsTab span={span} />}
        {activeTab === 'logs' && <LogsTab span={span} />}
        {activeTab === 'links' && <LinksTab span={span} />}
        {activeTab === 'stacktrace' && <StackTraceTab span={span} copyToClipboard={copyToClipboard} copiedSection={copiedSection} />}
      </div>
    </div>
  );
}

// Tab Components
function OverviewTab({ span, copyToClipboard, copiedSection }: { span: Span; copyToClipboard: (text: string, section: string) => void; copiedSection: string | null }) {
  return (
    <div className="space-y-6">
      {/* Status Badge */}
      <div className="flex items-center gap-2">
        {span.status === 'success' && (
          <span className="inline-flex items-center gap-1 px-3 py-1 bg-green-100 text-green-800 rounded-full text-sm font-medium">
            <Check className="w-4 h-4" />
            Success
          </span>
        )}
        {span.status === 'error' && (
          <span className="inline-flex items-center gap-1 px-3 py-1 bg-red-100 text-red-800 rounded-full text-sm font-medium">
            <AlertCircle className="w-4 h-4" />
            Error
          </span>
        )}
      </div>

      {/* Basic Info */}
      <div className="bg-secondary rounded-lg p-4 space-y-3">
        <MetricRow label="Span ID" value={span.id} />
        <MetricRow label="Type" value={span.spanType} />
        <MetricRow label="Duration" value={`${span.duration}ms`} highlight />
        <MetricRow label="Start Time" value={new Date(span.startTime).toLocaleString()} />
        <MetricRow label="End Time" value={new Date(span.endTime).toLocaleString()} />
      </div>

      {/* Input/Output */}
      {span.metadata?.input && (
        <div>
          <div className="flex items-center justify-between mb-2">
            <h4 className="text-sm font-semibold text-foreground">Input</h4>
            <button
              onClick={() => copyToClipboard(
                typeof span.metadata?.input === 'string' ? span.metadata.input : JSON.stringify(span.metadata?.input, null, 2),
                'input'
              )}
              className="p-1 hover:bg-gray-200 rounded"
            >
              {copiedSection === 'input' ? <Check className="w-4 h-4 text-green-600" /> : <Copy className="w-4 h-4 text-muted-foreground" />}
            </button>
          </div>
          <pre className="bg-gray-900 text-gray-100 p-4 rounded-lg text-xs overflow-x-auto font-mono max-h-60">
            {typeof span.metadata?.input === 'string' ? span.metadata.input : JSON.stringify(span.metadata?.input, null, 2)}
          </pre>
        </div>
      )}

      {span.metadata?.output && (
        <div>
          <div className="flex items-center justify-between mb-2">
            <h4 className="text-sm font-semibold text-foreground">Output</h4>
            <button
              onClick={() => copyToClipboard(
                typeof span.metadata?.output === 'string' ? span.metadata.output : JSON.stringify(span.metadata?.output, null, 2),
                'output'
              )}
              className="p-1 hover:bg-gray-200 rounded"
            >
              {copiedSection === 'output' ? <Check className="w-4 h-4 text-green-600" /> : <Copy className="w-4 h-4 text-muted-foreground" />}
            </button>
          </div>
          <pre className="bg-gray-900 text-gray-100 p-4 rounded-lg text-xs overflow-x-auto font-mono max-h-60">
            {typeof span.metadata.output === 'string' ? span.metadata.output : JSON.stringify(span.metadata.output, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
}

function AttributesTab({ span, copyToClipboard, copiedSection }: { span: Span; copyToClipboard: (text: string, section: string) => void; copiedSection: string | null }) {
  const attributes = span.metadata || {};
  
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-semibold text-foreground">
          All Attributes ({Object.keys(attributes).length})
        </h4>
        <button
          onClick={() => copyToClipboard(JSON.stringify(attributes, null, 2), 'attributes')}
          className="p-2 hover:bg-gray-200 rounded"
        >
          {copiedSection === 'attributes' ? <Check className="w-4 h-4 text-green-600" /> : <Copy className="w-4 h-4 text-muted-foreground" />}
        </button>
      </div>

      <div className="border border-border rounded-lg divide-y divide-gray-200">
        {Object.entries(attributes).map(([key, value]) => (
          <div key={key} className="p-3 hover:bg-secondary">
            <div className="flex justify-between items-start gap-4">
              <span className="text-sm font-mediumtext-foreground break-all">{key}</span>
              <span className="text-sm text-foreground font-mono text-right break-all">
                {typeof value === 'object' ? JSON.stringify(value) : String(value)}
              </span>
            </div>
          </div>
        ))}
      </div>

      {Object.keys(attributes).length === 0 && (
        <div className="text-center py-8 text-muted-foreground">
          <List className="w-12 h-12 mx-auto mb-2 text-muted-foreground" />
          <p>No attributes available</p>
        </div>
      )}
    </div>
  );
}

function PerformanceTab({ span }: { span: Span }) {
  const hasTokens = span.inputTokens || span.outputTokens;
  const totalTokens = (span.inputTokens || 0) + (span.outputTokens || 0);
  const tokensPerMs = span.duration > 0 ? totalTokens / span.duration : 0;

  return (
    <div className="space-y-6">
      {/* Token Metrics */}
      {hasTokens && (
        <div>
          <h4 className="text-sm font-semibold text-foreground mb-3">Token Usage</h4>
          <div className="grid grid-cols-2 gap-4">
            <div className="bg-blue-50 rounded-lg p-4">
              <div className="text-xs text-blue-600 font-medium mb-1">Input Tokens</div>
              <div className="text-2xl font-bold text-blue-900">{(span.inputTokens || 0).toLocaleString()}</div>
            </div>
            <div className="bg-green-50 rounded-lg p-4">
              <div className="text-xs text-green-600 font-medium mb-1">Output Tokens</div>
              <div className="text-2xl font-bold text-green-900">{(span.outputTokens || 0).toLocaleString()}</div>
            </div>
            <div className="bg-purple-50 rounded-lg p-4">
              <div className="text-xs text-purple-600 font-medium mb-1">Total Tokens</div>
              <div className="text-2xl font-bold text-purple-900">{totalTokens.toLocaleString()}</div>
            </div>
            <div className="bg-orange-50 rounded-lg p-4">
              <div className="text-xs text-orange-600 font-medium mb-1">Tokens/ms</div>
              <div className="text-2xl font-bold text-orange-900">{tokensPerMs.toFixed(2)}</div>
            </div>
          </div>
        </div>
      )}

      {/* Cost Breakdown */}
      {span.cost && (
        <div>
          <h4 className="text-sm font-semibold text-foreground mb-3">Cost Analysis</h4>
          <div className="bg-gradient-to-r from-green-50 to-blue-50 rounded-lg p-4">
            <div className="text-sm text-muted-foreground mb-1">Estimated Cost</div>
            <div className="text-3xl font-bold text-foreground">${span.cost.toFixed(6)}</div>
            {totalTokens > 0 && (
              <div className="text-xs text-muted-foreground mt-2">
                ${(span.cost / totalTokens * 1000).toFixed(4)} per 1K tokens
              </div>
            )}
          </div>
        </div>
      )}

      {/* Duration Metrics */}
      <div>
        <h4 className="text-sm font-semibold text-foreground mb-3">Duration</h4>
        <div className="bg-secondary rounded-lg p-4">
          <div className="text-sm text-muted-foreground mb-1">Total Duration</div>
          <div className="text-2xl font-bold text-foreground">{span.duration}ms</div>
          <div className="text-xs text-muted-foreground mt-1">
            {(span.duration / 1000).toFixed(3)}s
          </div>
        </div>
      </div>

      {!hasTokens && !span.cost && (
        <div className="text-center py-8 text-muted-foreground">
          <TrendingUp className="w-12 h-12 mx-auto mb-2 text-muted-foreground" />
          <p>No performance metrics available</p>
        </div>
      )}
    </div>
  );
}

function EventsTab({ span }: { span: Span }) {
  const events = span.metadata?.events || span.metadata?.span_events || [];
  
  return (
    <div className="space-y-4">
      {Array.isArray(events) && events.length > 0 ? (
        events.map((event: any, idx: number) => (
          <div key={idx} className="border border-border rounded-lg p-4">
            <div className="flex items-center justify-between mb-2">
              <span className="font-medium text-foreground">{event.name || `Event ${idx + 1}`}</span>
              <span className="text-xs text-muted-foreground">{event.timestamp || 'N/A'}</span>
            </div>
            {event.attributes && (
              <pre className="bg-secondary p-3 rounded text-xs font-mono overflow-x-auto">
                {JSON.stringify(event.attributes, null, 2)}
              </pre>
            )}
          </div>
        ))
      ) : (
        <div className="text-center py-12 text-muted-foreground">
          <Activity className="w-12 h-12 mx-auto mb-2 text-muted-foreground" />
          <p>No events recorded</p>
        </div>
      )}
    </div>
  );
}

function LogsTab({ span }: { span: Span }) {
  const logs = span.metadata?.logs || [];
  
  return (
    <div className="space-y-2">
      {Array.isArray(logs) && logs.length > 0 ? (
        logs.map((log: any, idx: number) => (
          <div key={idx} className="bg-gray-900 text-gray-100 rounded-lg p-3 font-mono text-xs">
            <div className="flex items-center gap-2 mb-1">
              <span className="text-muted-foreground">[{log.timestamp || 'N/A'}]</span>
              <span className={`font-semibold ${
                log.level === 'error' ? 'text-red-600 dark:text-red-400' :
                log.level === 'warn' ? 'text-yellow-600 dark:text-yellow-400' :
                log.level === 'info' ? 'text-blue-600 dark:text-blue-400' :
                'text-gray-300'
              }`}>
                {log.level?.toUpperCase() || 'LOG'}
              </span>
            </div>
            <div className="text-gray-200">{log.message || JSON.stringify(log)}</div>
          </div>
        ))
      ) : (
        <div className="text-center py-12 text-muted-foreground">
          <Code className="w-12 h-12 mx-auto mb-2 text-muted-foreground" />
          <p>No logs available</p>
        </div>
      )}
    </div>
  );
}

function LinksTab({ span }: { span: Span }) {
  const links = span.metadata?.links || [];
  
  return (
    <div className="space-y-4">
      {Array.isArray(links) && links.length > 0 ? (
        links.map((link: any, idx: number) => (
          <div key={idx} className="border border-border rounded-lg p-4 hover:bg-secondary">
            <div className="flex items-center gap-2 mb-2">
              <Link2 className="w-4 h-4 text-blue-600" />
              <span className="font-medium text-foreground">{link.type || 'Link'}</span>
            </div>
            <div className="space-y-1 text-sm">
              <div className="flex gap-2">
                <span className="text-muted-foreground">Trace ID:</span>
                <span className="font-mono text-foreground">{link.trace_id || 'N/A'}</span>
              </div>
              <div className="flex gap-2">
                <span className="text-muted-foreground">Span ID:</span>
                <span className="font-mono text-foreground">{link.span_id || 'N/A'}</span>
              </div>
            </div>
          </div>
        ))
      ) : (
        <div className="text-center py-12 text-muted-foreground">
          <Link2 className="w-12 h-12 mx-auto mb-2 text-muted-foreground" />
          <p>No linked traces</p>
        </div>
      )}
    </div>
  );
}

function StackTraceTab({ span, copyToClipboard, copiedSection }: { span: Span; copyToClipboard: (text: string, section: string) => void; copiedSection: string | null }) {
  const stackTrace = span.metadata?.error?.stack || span.metadata?.stack_trace || span.metadata?.stacktrace;
  const error = span.metadata?.error;
  
  if (!error && !stackTrace && span.status !== 'error') {
    return (
      <div className="text-center py-12 text-muted-foreground">
        <Check className="w-12 h-12 mx-auto mb-2 text-green-600 dark:text-green-400" />
        <p>No errors - span completed successfully</p>
      </div>
    );
  }
  
  return (
    <div className="space-y-4">
      {error && (
        <div className="bg-red-50 border border-red-200 rounded-lg p-4">
          <div className="flex items-center gap-2 mb-2">
            <AlertCircle className="w-5 h-5 text-red-600" />
            <h4 className="font-semibold text-red-900">Error Details</h4>
          </div>
          <div className="space-y-2 text-sm">
            {error.type && (
              <div><span className="text-red-600 font-medium">Type:</span> <span className="text-red-900">{error.type}</span></div>
            )}
            {error.message && (
              <div><span className="text-red-600 font-medium">Message:</span> <span className="text-red-900">{error.message}</span></div>
            )}
          </div>
        </div>
      )}

      {stackTrace && (
        <div>
          <div className="flex items-center justify-between mb-2">
            <h4 className="text-sm font-semibold text-foreground">Stack Trace</h4>
            <button
              onClick={() => copyToClipboard(
                typeof stackTrace === 'string' ? stackTrace : JSON.stringify(stackTrace, null, 2),
                'stacktrace'
              )}
              className="p-2 hover:bg-gray-200 rounded"
            >
              {copiedSection === 'stacktrace' ? <Check className="w-4 h-4 text-green-600" /> : <Copy className="w-4 h-4 text-muted-foreground" />}
            </button>
          </div>
          <pre className="bg-gray-900 text-red-600 dark:text-red-400 p-4 rounded-lg text-xs overflow-x-auto font-mono max-h-96">
            {typeof stackTrace === 'string' ? stackTrace : JSON.stringify(stackTrace, null, 2)}
          </pre>
        </div>
      )}

      {!error && !stackTrace && span.status === 'error' && (
        <div className="text-center py-8 text-muted-foreground">
          <AlertCircle className="w-12 h-12 mx-auto mb-2 text-muted-foreground" />
          <p>Error occurred but no stack trace available</p>
        </div>
      )}
    </div>
  );
}

// Helper components
interface MetricRowProps {
  label: string;
  value: string;
  highlight?: boolean;
}

function MetricRow({ label, value, highlight }: MetricRowProps) {
  return (
    <div className="flex justify-between items-center py-2 border-b border-border last:border-0">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span
        className={`text-sm font-medium ${
          highlight ? 'text-blue-600 font-semibold' : 'text-foreground'
        }`}
      >
        {value}
      </span>
    </div>
  );
}

export default SpanDetailPanel;
