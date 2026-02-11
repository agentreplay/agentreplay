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

import { useState, useEffect } from 'react';
import { Clock, X, Search, ArrowRight, Trash2 } from 'lucide-react';

interface RecentSearch {
  query: string;
  timestamp: number;
  resultCount?: number;
}

interface RecentSearchesProps {
  onSelect: (query: string) => void;
  projectId?: string;
  maxItems?: number;
}

const STORAGE_KEY = 'agentreplay_recent_searches';

export function saveRecentSearch(query: string, projectId: string, resultCount?: number) {
  try {
    const key = `${STORAGE_KEY}_${projectId}`;
    const existing = JSON.parse(localStorage.getItem(key) || '[]') as RecentSearch[];

    // Remove duplicate if exists
    const filtered = existing.filter(s => s.query.toLowerCase() !== query.toLowerCase());

    // Add new search at the beginning
    const updated = [
      { query, timestamp: Date.now(), resultCount },
      ...filtered.slice(0, 19), // Keep max 20 items
    ];

    localStorage.setItem(key, JSON.stringify(updated));
  } catch (e) {
    console.warn('Failed to save recent search:', e);
  }
}

export function getRecentSearches(projectId: string, max: number = 5): RecentSearch[] {
  try {
    const key = `${STORAGE_KEY}_${projectId}`;
    const searches = JSON.parse(localStorage.getItem(key) || '[]') as RecentSearch[];
    return searches.slice(0, max);
  } catch {
    return [];
  }
}

export function clearRecentSearches(projectId: string) {
  try {
    const key = `${STORAGE_KEY}_${projectId}`;
    localStorage.removeItem(key);
  } catch {
    // Ignore
  }
}

function formatTimeAgo(timestamp: number): string {
  const seconds = Math.floor((Date.now() - timestamp) / 1000);

  if (seconds < 60) return 'just now';
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  if (seconds < 604800) return `${Math.floor(seconds / 86400)}d ago`;
  return new Date(timestamp).toLocaleDateString();
}

export function RecentSearches({ onSelect, projectId, maxItems = 5 }: RecentSearchesProps) {
  const [searches, setSearches] = useState<RecentSearch[]>([]);
  const [isExpanded, setIsExpanded] = useState(true);

  useEffect(() => {
    if (projectId) {
      setSearches(getRecentSearches(projectId, maxItems));
    }
  }, [projectId, maxItems]);

  const handleClear = () => {
    if (projectId) {
      clearRecentSearches(projectId);
      setSearches([]);
    }
  };

  const handleRemove = (query: string) => {
    if (!projectId) return;

    const key = `${STORAGE_KEY}_${projectId}`;
    const existing = JSON.parse(localStorage.getItem(key) || '[]') as RecentSearch[];
    const filtered = existing.filter(s => s.query !== query);
    localStorage.setItem(key, JSON.stringify(filtered));
    setSearches(filtered.slice(0, maxItems));
  };

  if (searches.length === 0) {
    return null;
  }

  return (
    <div className="glass-card rounded-2xl overflow-hidden">
      {/* Header */}
      <div
        className="flex items-center justify-between px-5 py-3 cursor-pointer hover:bg-white/[0.03] transition-colors"
        onClick={() => setIsExpanded(!isExpanded)}
      >
        <div className="flex items-center gap-2.5">
          <div className="flex items-center justify-center w-7 h-7 rounded-lg bg-primary/10">
            <Clock className="w-3.5 h-3.5 text-primary" />
          </div>
          <span className="text-sm font-semibold text-textPrimary">Recent Searches</span>
          <span className="text-[11px] font-medium text-textTertiary bg-surface-hover rounded-full px-2 py-0.5">{searches.length}</span>
        </div>

        <button
          onClick={(e) => {
            e.stopPropagation();
            handleClear();
          }}
          className="text-[11px] font-medium text-textTertiary hover:text-error transition-colors flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-error/5"
        >
          <Trash2 className="w-3 h-3" />
          Clear all
        </button>
      </div>

      {/* Search List */}
      {isExpanded && (
        <div className="border-t border-border/30">
          {searches.map((search, index) => (
            <div
              key={`${search.query}-${index}`}
              className="group flex items-center gap-3 px-5 py-3 hover:bg-primary/[0.03] transition-all cursor-pointer border-b border-border/20 last:border-b-0"
              onClick={() => onSelect(search.query)}
            >
              <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-surface-hover/80 group-hover:bg-primary/10 transition-colors flex-shrink-0">
                <Search className="w-3.5 h-3.5 text-textTertiary group-hover:text-primary transition-colors" />
              </div>

              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium text-textPrimary truncate group-hover:text-primary transition-colors">{search.query}</div>
                <div className="flex items-center gap-2 text-[11px] text-textTertiary mt-0.5">
                  <span>{formatTimeAgo(search.timestamp)}</span>
                  {search.resultCount !== undefined && (
                    <>
                      <span className="w-1 h-1 rounded-full bg-textTertiary/40" />
                      <span>{search.resultCount} results</span>
                    </>
                  )}
                </div>
              </div>

              <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-all">
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleRemove(search.query);
                  }}
                  className="p-1.5 rounded-lg hover:bg-error/10 text-textTertiary hover:text-error transition-all"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
                <ArrowRight className="w-4 h-4 text-primary" />
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// Suggested searches component
const SUGGESTED_SEARCHES = [
  { query: 'errors in last 24h', description: 'Find failed requests' },
  { query: 'slow responses', description: 'High latency calls' },
  { query: 'gpt-4 completions', description: 'GPT-4 model usage' },
  { query: 'tool calls', description: 'Function/tool invocations' },
  { query: 'high token usage', description: 'Expensive requests' },
];

export function SuggestedSearches({ onSelect }: { onSelect: (query: string) => void }) {
  return (
    <div className="glass-card rounded-2xl overflow-hidden">
      <div className="px-5 py-3 border-b border-border/30">
        <div className="flex items-center gap-2.5">
          <div className="flex items-center justify-center w-7 h-7 rounded-lg bg-warning/10">
            <Search className="w-3.5 h-3.5 text-warning" />
          </div>
          <span className="text-sm font-semibold text-textPrimary">Suggested Searches</span>
        </div>
      </div>

      <div className="grid grid-cols-2 lg:grid-cols-3 gap-2.5 p-4">
        {SUGGESTED_SEARCHES.map((suggestion) => (
          <button
            key={suggestion.query}
            onClick={() => onSelect(suggestion.query)}
            className="group relative flex flex-col items-start p-4 rounded-xl glass-card hover:border-primary/30 hover:shadow-md hover:shadow-primary/5 transition-all text-left hover:-translate-y-0.5"
          >
            <span className="text-sm font-semibold text-textPrimary group-hover:text-primary transition-colors">
              {suggestion.query}
            </span>
            <span className="text-[11px] text-textTertiary mt-1 leading-relaxed">
              {suggestion.description}
            </span>
            <ArrowRight className="absolute top-4 right-3 w-3.5 h-3.5 text-textTertiary/30 group-hover:text-primary group-hover:translate-x-0.5 transition-all" />
          </button>
        ))}
      </div>
    </div>
  );
}
