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

const STORAGE_KEY = 'flowtrace_recent_searches';

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
    <div className="bg-surface rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div 
        className="flex items-center justify-between px-4 py-2.5 bg-surface-elevated cursor-pointer hover:bg-surface-hover transition-colors"
        onClick={() => setIsExpanded(!isExpanded)}
      >
        <div className="flex items-center gap-2 text-sm text-textSecondary">
          <Clock className="w-4 h-4" />
          <span className="font-medium">Recent Searches</span>
          <span className="text-textTertiary">({searches.length})</span>
        </div>
        
        <div className="flex items-center gap-2">
          <button
            onClick={(e) => {
              e.stopPropagation();
              handleClear();
            }}
            className="text-xs text-textTertiary hover:text-red-500 transition-colors flex items-center gap-1"
          >
            <Trash2 className="w-3 h-3" />
            Clear
          </button>
        </div>
      </div>

      {/* Search List */}
      {isExpanded && (
        <div className="divide-y divide-border">
          {searches.map((search, index) => (
            <div
              key={`${search.query}-${index}`}
              className="group flex items-center gap-3 px-4 py-2.5 hover:bg-surface-hover transition-colors cursor-pointer"
              onClick={() => onSelect(search.query)}
            >
              <Search className="w-4 h-4 text-textTertiary flex-shrink-0" />
              
              <div className="flex-1 min-w-0">
                <div className="text-sm text-textPrimary truncate">{search.query}</div>
                <div className="flex items-center gap-2 text-xs text-textTertiary">
                  <span>{formatTimeAgo(search.timestamp)}</span>
                  {search.resultCount !== undefined && (
                    <>
                      <span>•</span>
                      <span>{search.resultCount} results</span>
                    </>
                  )}
                </div>
              </div>

              <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleRemove(search.query);
                  }}
                  className="p-1 rounded hover:bg-red-500/10 text-textTertiary hover:text-red-500 transition-colors"
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
    <div className="bg-surface rounded-lg border border-border overflow-hidden">
      <div className="px-4 py-2.5 bg-surface-elevated border-b border-border">
        <div className="flex items-center gap-2 text-sm text-textSecondary">
          <Search className="w-4 h-4" />
          <span className="font-medium">Suggested Searches</span>
        </div>
      </div>
      
      <div className="grid grid-cols-2 lg:grid-cols-3 gap-2 p-3">
        {SUGGESTED_SEARCHES.map((suggestion) => (
          <button
            key={suggestion.query}
            onClick={() => onSelect(suggestion.query)}
            className="flex flex-col items-start p-3 rounded-lg border border-border hover:border-primary/50 hover:bg-primary/5 transition-colors text-left group"
          >
            <span className="text-sm font-medium text-textPrimary group-hover:text-primary transition-colors">
              {suggestion.query}
            </span>
            <span className="text-xs text-textTertiary mt-0.5">
              {suggestion.description}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
