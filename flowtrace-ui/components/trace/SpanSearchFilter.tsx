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
import { Search, Filter, X, ChevronDown } from 'lucide-react';
import { Span } from './TraceTree';

export interface SpanSearchFilterProps {
  spans: Span[];
  onFilteredSpansChange: (filteredSpans: Span[]) => void;
}

interface FilterCriteria {
  searchText: string;
  useRegex: boolean;
  spanTypes: Set<string>;
  status: Set<string>;
  minDuration?: number;
  maxDuration?: number;
  hasError: boolean | null;
  hasTokens: boolean | null;
}

export function SpanSearchFilter({ spans, onFilteredSpansChange }: SpanSearchFilterProps) {
  const [showFilters, setShowFilters] = useState(false);
  const [filters, setFilters] = useState<FilterCriteria>({
    searchText: '',
    useRegex: false,
    spanTypes: new Set(),
    status: new Set(),
    hasError: null,
    hasTokens: null,
  });

  // Get unique span types and statuses
  const { spanTypes, statuses } = useMemo(() => {
    const types = new Set<string>();
    const stats = new Set<string>();

    const collectValues = (span: Span) => {
      types.add(span.spanType);
      stats.add(span.status);
      if (span.children) {
        span.children.forEach(collectValues);
      }
    };

    spans.forEach(collectValues);

    return {
      spanTypes: Array.from(types),
      statuses: Array.from(stats),
    };
  }, [spans]);

  // Filter spans based on criteria
  const filteredSpans = useMemo(() => {
    if (
      !filters.searchText &&
      filters.spanTypes.size === 0 &&
      filters.status.size === 0 &&
      !filters.minDuration &&
      !filters.maxDuration &&
      filters.hasError === null &&
      filters.hasTokens === null
    ) {
      return spans;
    }

    const matchesSearch = (span: Span): boolean => {
      if (!filters.searchText) return true;

      const searchIn = `${span.name} ${span.spanType} ${JSON.stringify(span.metadata || {})}`;

      if (filters.useRegex) {
        try {
          const regex = new RegExp(filters.searchText, 'i');
          return regex.test(searchIn);
        } catch {
          return false;
        }
      }

      return searchIn.toLowerCase().includes(filters.searchText.toLowerCase());
    };

    const matchesFilters = (span: Span): boolean => {
      // Span type filter
      if (filters.spanTypes.size > 0 && !filters.spanTypes.has(span.spanType)) {
        return false;
      }

      // Status filter
      if (filters.status.size > 0 && !filters.status.has(span.status)) {
        return false;
      }

      // Duration filter
      if (filters.minDuration !== undefined && span.duration < filters.minDuration) {
        return false;
      }
      if (filters.maxDuration !== undefined && span.duration > filters.maxDuration) {
        return false;
      }

      // Error filter
      if (filters.hasError === true && span.status !== 'error') {
        return false;
      }
      if (filters.hasError === false && span.status === 'error') {
        return false;
      }

      // Token filter
      if (filters.hasTokens === true && !span.inputTokens && !span.outputTokens) {
        return false;
      }
      if (filters.hasTokens === false && (span.inputTokens || span.outputTokens)) {
        return false;
      }

      return true;
    };

    const filterSpansRecursive = (spanList: Span[]): Span[] => {
      return spanList
        .filter((span) => matchesSearch(span) && matchesFilters(span))
        .map((span) => ({
          ...span,
          children: span.children ? filterSpansRecursive(span.children) : undefined,
        }));
    };

    return filterSpansRecursive(spans);
  }, [spans, filters]);

  // Notify parent of filtered spans
  React.useEffect(() => {
    onFilteredSpansChange(filteredSpans);
  }, [filteredSpans, onFilteredSpansChange]);

  const handleSearchChange = (value: string) => {
    setFilters((prev) => ({ ...prev, searchText: value }));
  };

  const toggleSpanType = (type: string) => {
    setFilters((prev) => {
      const newTypes = new Set(prev.spanTypes);
      if (newTypes.has(type)) {
        newTypes.delete(type);
      } else {
        newTypes.add(type);
      }
      return { ...prev, spanTypes: newTypes };
    });
  };

  const toggleStatus = (status: string) => {
    setFilters((prev) => {
      const newStatus = new Set(prev.status);
      if (newStatus.has(status)) {
        newStatus.delete(status);
      } else {
        newStatus.add(status);
      }
      return { ...prev, status: newStatus };
    });
  };

  const clearFilters = () => {
    setFilters({
      searchText: '',
      useRegex: false,
      spanTypes: new Set(),
      status: new Set(),
      hasError: null,
      hasTokens: null,
    });
  };

  const activeFilterCount =
    (filters.searchText ? 1 : 0) +
    filters.spanTypes.size +
    filters.status.size +
    (filters.minDuration !== undefined ? 1 : 0) +
    (filters.maxDuration !== undefined ? 1 : 0) +
    (filters.hasError !== null ? 1 : 0) +
    (filters.hasTokens !== null ? 1 : 0);

  return (
    <div className="bg-white rounded-lg border border-gray-200 p-4 space-y-4">
      {/* Search Bar */}
      <div className="flex items-center gap-2">
        <div className="flex-1 relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
          <input
            type="text"
            placeholder="Search spans by name, type, or metadata..."
            value={filters.searchText}
            onChange={(e) => handleSearchChange(e.target.value)}
            className="w-full pl-10 pr-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          />
          {filters.searchText && (
            <button
              onClick={() => handleSearchChange('')}
              className="absolute right-3 top-1/2 -translate-y-1/2 p-1 hover:bg-gray-100 rounded"
            >
              <X className="w-4 h-4 text-gray-400" />
            </button>
          )}
        </div>

        <label className="flex items-center gap-2 text-sm text-gray-600">
          <input
            type="checkbox"
            checked={filters.useRegex}
            onChange={(e) =>
              setFilters((prev) => ({ ...prev, useRegex: e.target.checked }))
            }
            className="rounded border-gray-300"
          />
          Regex
        </label>

        <button
          onClick={() => setShowFilters(!showFilters)}
          className={`flex items-center gap-2 px-4 py-2 rounded-lg border transition-colors ${
            showFilters
              ? 'bg-blue-50 border-blue-300 text-blue-700'
              : 'bg-white border-gray-300 text-gray-700 hover:bg-gray-50'
          }`}
        >
          <Filter className="w-4 h-4" />
          <span>Filters</span>
          {activeFilterCount > 0 && (
            <span className="bg-blue-600 text-white text-xs rounded-full px-2 py-0.5">
              {activeFilterCount}
            </span>
          )}
          <ChevronDown
            className={`w-4 h-4 transition-transform ${showFilters ? 'rotate-180' : ''}`}
          />
        </button>

        {activeFilterCount > 0 && (
          <button
            onClick={clearFilters}
            className="px-3 py-2 text-sm text-red-600 hover:bg-red-50 rounded-lg transition-colors"
          >
            Clear All
          </button>
        )}
      </div>

      {/* Advanced Filters */}
      {showFilters && (
        <div className="grid grid-cols-2 gap-4 pt-4 border-t border-gray-200">
          {/* Span Types */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Span Types
            </label>
            <div className="space-y-2">
              {spanTypes.map((type) => (
                <label key={type} className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={filters.spanTypes.has(type)}
                    onChange={() => toggleSpanType(type)}
                    className="rounded border-gray-300"
                  />
                  <span className="capitalize">{type}</span>
                </label>
              ))}
            </div>
          </div>

          {/* Status */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">Status</label>
            <div className="space-y-2">
              {statuses.map((status) => (
                <label key={status} className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={filters.status.has(status)}
                    onChange={() => toggleStatus(status)}
                    className="rounded border-gray-300"
                  />
                  <span className="capitalize">{status}</span>
                </label>
              ))}
            </div>
          </div>

          {/* Duration */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Duration (ms)
            </label>
            <div className="space-y-2">
              <input
                type="number"
                placeholder="Min duration"
                value={filters.minDuration || ''}
                onChange={(e) =>
                  setFilters((prev) => ({
                    ...prev,
                    minDuration: e.target.value ? Number(e.target.value) : undefined,
                  }))
                }
                className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm"
              />
              <input
                type="number"
                placeholder="Max duration"
                value={filters.maxDuration || ''}
                onChange={(e) =>
                  setFilters((prev) => ({
                    ...prev,
                    maxDuration: e.target.value ? Number(e.target.value) : undefined,
                  }))
                }
                className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm"
              />
            </div>
          </div>

          {/* Additional Filters */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Additional
            </label>
            <div className="space-y-2">
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={filters.hasError === true}
                  onChange={(e) =>
                    setFilters((prev) => ({
                      ...prev,
                      hasError: e.target.checked ? true : null,
                    }))
                  }
                  className="rounded border-gray-300"
                />
                <span>Has Errors</span>
              </label>
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={filters.hasTokens === true}
                  onChange={(e) =>
                    setFilters((prev) => ({
                      ...prev,
                      hasTokens: e.target.checked ? true : null,
                    }))
                  }
                  className="rounded border-gray-300"
                />
                <span>Has Token Usage</span>
              </label>
            </div>
          </div>
        </div>
      )}

      {/* Results Count */}
      <div className="text-sm text-gray-600">
        Showing {filteredSpans.length} of {spans.length} spans
      </div>
    </div>
  );
}

export default SpanSearchFilter;
