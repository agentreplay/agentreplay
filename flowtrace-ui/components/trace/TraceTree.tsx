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

import React, { useState, useMemo } from 'react';
import { ChevronDown, ChevronRight, Clock, Coins, Zap, AlertCircle, CheckCircle, Search, X, Bot, User, Wrench, Settings, MessageSquare } from 'lucide-react';

export interface Span {
  id: string;
  name: string;
  spanType: 'root' | 'planning' | 'reasoning' | 'tool_call' | 'tool_response' | 'synthesis' | 'response' | 'error' | 'llm' | 'retrieval';
  status: 'success' | 'error' | 'pending';
  startTime: number;
  endTime: number;
  duration: number;
  inputTokens?: number;
  outputTokens?: number;
  cost?: number;
  children?: Span[];
  metadata?: Record<string, any>;
}

export interface TraceTreeProps {
  spans: Span[];
  onSelectSpan?: (span: Span) => void;
  selectedSpanId?: string;
}

const spanTypeColors: Record<string, string> = {
  root: 'bg-purple-100 text-purple-800 border-purple-300',
  planning: 'bg-indigo-100 text-indigo-800 border-indigo-300',
  reasoning: 'bg-blue-100 text-blue-800 border-blue-300',
  tool_call: 'bg-orange-100 text-orange-800 border-orange-300',
  tool_response: 'bg-amber-100 text-amber-800 border-amber-300',
  synthesis: 'bg-teal-100 text-teal-800 border-teal-300',
  response: 'bg-green-100 text-green-800 border-green-300',
  error: 'bg-red-100 text-red-800 border-red-300',
  llm: 'bg-blue-100 text-blue-800 border-blue-300',
  retrieval: 'bg-green-100 text-green-800 border-green-300',
  // New types for better visualization
  function: 'bg-violet-100 text-violet-800 border-violet-300',
  tool: 'bg-orange-100 text-orange-800 border-orange-300',
  agent: 'bg-purple-100 text-purple-800 border-purple-300',
  chain: 'bg-cyan-100 text-cyan-800 border-cyan-300',
  chat: 'bg-emerald-100 text-emerald-800 border-emerald-300',
  embedding: 'bg-pink-100 text-pink-800 border-pink-300',
  unknown: 'bg-gray-100 text-gray-800 border-gray-300',
};

const spanTypeIcons: Record<string, React.ReactNode> = {
  root: <User className="w-3 h-3" />,
  llm: <Bot className="w-3 h-3" />,
  retrieval: <CheckCircle className="w-3 h-3" />,
  tool_call: <Wrench className="w-3 h-3" />,
  tool_response: <CheckCircle className="w-3 h-3" />,
  error: <AlertCircle className="w-3 h-3" />,
  function: <Zap className="w-3 h-3" />,
  tool: <Wrench className="w-3 h-3" />,
  planning: <Settings className="w-3 h-3" />,
  response: <MessageSquare className="w-3 h-3" />,
  agent: <Bot className="w-3 h-3" />,
  chat: <MessageSquare className="w-3 h-3" />,
  user: <User className="w-3 h-3" />,
  system: <Settings className="w-3 h-3" />,
  assistant: <Bot className="w-3 h-3" />,
};

// Better display labels for span types
const spanTypeLabels: Record<string, string> = {
  llm: 'LLM',
  function: 'Function',
  tool_call: 'Tool Call',
  tool_response: 'Tool Result',
  tool: 'Tool',
  planning: 'System',
  response: 'Response',
  root: 'User',
  reasoning: 'Reasoning',
  synthesis: 'Synthesis',
  retrieval: 'Retrieval',
  error: 'Error',
  agent: 'Agent',
  chain: 'Chain',
  chat: 'Chat',
  embedding: 'Embedding',
};

// Recursive SpanNode component for rendering the tree
const SpanNode = ({
  span,
  depth,
  expandedNodes,
  selectedSpanId,
  onToggle,
  onSelect
}: {
  span: Span;
  depth: number;
  expandedNodes: Set<string>;
  selectedSpanId?: string;
  onToggle: (id: string) => void;
  onSelect: (span: Span) => void;
}) => {
  const isExpanded = expandedNodes.has(span.id);
  const isSelected = selectedSpanId === span.id;
  const hasChildren = span.children && span.children.length > 0;

  const colorClass = spanTypeColors[span.spanType] || 'bg-gray-100 text-gray-800 border-gray-300';
  const icon = spanTypeIcons[span.spanType];

  return (
    <div className="font-sans">
      <div
        className={`
          flex items-center py-1 px-2 cursor-pointer hover:bg-surface-hover transition-colors
          ${isSelected ? 'bg-primary/10 border-l-4 border-primary' : 'border-l-4 border-transparent'}
        `}
        style={{ paddingLeft: `${depth * 20 + 8}px` }}
        onClick={() => onSelect(span)}
      >
        {/* Expand/collapse button */}
        <button
          className="mr-1.5 p-0.5 hover:bg-gray-200 rounded transition-colors flex-shrink-0 text-textSecondary"
          onClick={(e) => {
            e.stopPropagation();
            if (hasChildren) onToggle(span.id);
          }}
        >
          {hasChildren ? (
            isExpanded ? (
              <ChevronDown className="w-3.5 h-3.5" />
            ) : (
              <ChevronRight className="w-3.5 h-3.5" />
            )
          ) : (
            <div className="w-3.5 h-3.5" />
          )}
        </button>

        {/* Span type badge */}
        <div className={`flex items-center gap-1.5 px-1.5 py-0.5 rounded-md border text-[10px] font-semibold ${colorClass} mr-2 flex-shrink-0`}>
          {icon}
          <span>{spanTypeLabels[span.spanType] || span.spanType.charAt(0).toUpperCase() + span.spanType.slice(1)}</span>
        </div>

        {/* Span name */}
        <div className="flex-1 font-medium text-xs text-textPrimary truncate mr-2">
          {span.name}
        </div>

        {/* Metrics */}
        <div className="flex items-center gap-2 ml-auto flex-shrink-0">
          {/* Token count */}
          {(span.inputTokens || span.outputTokens) && (
            <div className="flex items-center gap-1 text-[10px] text-textSecondary bg-gray-100 px-1.5 py-0.5 rounded">
              <Coins className="w-3 h-3" />
              <span className="font-mono">
                {span.inputTokens && span.outputTokens
                  ? `${span.inputTokens}→${span.outputTokens}`
                  : (span.inputTokens || 0) + (span.outputTokens || 0)}
              </span>
            </div>
          )}

          {/* Duration */}
          <div className="flex items-center gap-1 text-xs text-textSecondary">
            <Clock className="w-3 h-3" />
            <span className="font-mono">
              {span.duration >= 1000
                ? `${(span.duration / 1000).toFixed(2)}s`
                : `${span.duration}ms`}
            </span>
          </div>

          {/* Status icon */}
          <div className="w-5 flex justify-center">
            {span.status === 'error' && (
              <AlertCircle className="w-4 h-4 text-red-500" />
            )}
            {span.status === 'success' && (
              <CheckCircle className="w-4 h-4 text-green-500" />
            )}
          </div>
        </div>
      </div>

      {/* Render children recursively */}
      {hasChildren && isExpanded && (
        <div>
          {span.children!.map(child => (
            <SpanNode
              key={child.id}
              span={child}
              depth={depth + 1}
              expandedNodes={expandedNodes}
              selectedSpanId={selectedSpanId}
              onToggle={onToggle}
              onSelect={onSelect}
            />
          ))}
        </div>
      )}
    </div>
  );
};

export function TraceTree({ spans, onSelectSpan, selectedSpanId }: TraceTreeProps) {
  const [searchQuery, setSearchQuery] = useState('');

  // Initialize expanded nodes with all span IDs
  const [expandedNodes, setExpandedNodes] = useState<Set<string>>(() => {
    const allIds = new Set<string>();
    const collectIds = (nodes: Span[]) => {
      nodes.forEach(node => {
        allIds.add(node.id);
        if (node.children) collectIds(node.children);
      });
    };
    collectIds(spans);
    return allIds;
  });

  // Helper to check if a span or any of its descendants match the search
  const spanMatchesSearch = (span: Span, query: string): boolean => {
    const lowerQuery = query.toLowerCase();
    const nameMatches = span.name.toLowerCase().includes(lowerQuery);
    const typeMatches = span.spanType.toLowerCase().includes(lowerQuery);
    const metadataMatches = span.metadata ?
      JSON.stringify(span.metadata).toLowerCase().includes(lowerQuery) : false;

    if (nameMatches || typeMatches || metadataMatches) return true;

    // Check children
    if (span.children) {
      return span.children.some(child => spanMatchesSearch(child, query));
    }
    return false;
  };

  // Filter spans based on search query
  const filteredSpans = useMemo(() => {
    if (!searchQuery.trim()) return spans;

    const filterSpan = (span: Span): Span | null => {
      const matches = spanMatchesSearch(span, searchQuery);
      if (!matches) return null;

      // If this span matches or has matching children, include it
      const filteredChildren = span.children
        ?.map(child => filterSpan(child))
        .filter((child): child is Span => child !== null);

      return {
        ...span,
        children: filteredChildren && filteredChildren.length > 0 ? filteredChildren : span.children
      };
    };

    return spans
      .map(span => filterSpan(span))
      .filter((span): span is Span => span !== null);
  }, [spans, searchQuery]);

  // Count total spans recursively
  const totalSpanCount = useMemo(() => {
    const countSpans = (spanList: Span[]): number => {
      return spanList.reduce((count, span) => {
        return count + 1 + (span.children ? countSpans(span.children) : 0);
      }, 0);
    };
    return countSpans(filteredSpans);
  }, [filteredSpans]);

  const handleToggle = (spanId: string) => {
    setExpandedNodes((prev) => {
      const next = new Set(prev);
      if (next.has(spanId)) {
        next.delete(spanId);
      } else {
        next.add(spanId);
      }
      return next;
    });
  };

  if (spans.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-gray-500">
        <AlertCircle className="w-12 h-12 mb-4 text-gray-400" />
        <p className="text-lg font-medium">No spans to display</p>
        <p className="text-sm">This trace doesn't have any span data yet.</p>
      </div>
    );
  }

  return (
    <div className="bg-background rounded-lg border border-border overflow-hidden flex flex-col h-full">
      {/* Header with Search */}
      <div className="bg-surface border-b border-border px-3 py-2 flex-shrink-0">
        <div className="flex items-center justify-between gap-4">
          <div className="relative flex-1 max-w-xs">
            <Search className="absolute left-2 top-1/2 transform -translate-y-1/2 w-4 h-4 text-textSecondary" />
            <input
              type="text"
              placeholder="Search spans..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-8 pr-8 py-1.5 text-sm bg-background border border-border rounded-md text-textPrimary placeholder:text-textSecondary focus:outline-none focus:ring-2 focus:ring-primary/50"
            />
            {searchQuery && (
              <button
                onClick={() => setSearchQuery('')}
                className="absolute right-2 top-1/2 transform -translate-y-1/2 text-textSecondary hover:text-textPrimary"
              >
                <X className="w-4 h-4" />
              </button>
            )}
          </div>
          <div className="flex items-center gap-4 text-xs font-medium text-textSecondary uppercase">
            <span>{totalSpanCount} spans</span>
            <span>Duration & Metrics</span>
          </div>
        </div>
      </div>

      {/* Tree content */}
      <div className="flex-1 overflow-auto">
        {filteredSpans.map(span => (
          <SpanNode
            key={span.id}
            span={span}
            depth={0}
            expandedNodes={expandedNodes}
            selectedSpanId={selectedSpanId}
            onToggle={handleToggle}
            onSelect={(s) => onSelectSpan?.(s)}
          />
        ))}
      </div>
    </div>
  );
}

export default TraceTree;
