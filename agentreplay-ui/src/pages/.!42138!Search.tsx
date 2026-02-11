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

import { useState, useEffect } from 'react';
import { useParams, Link } from 'react-router-dom';
import { Search as SearchIcon, Loader2, Zap, Filter, Clock, AlertCircle, Sparkles, Hash, ChevronDown, Lightbulb, TrendingUp, Info, Brain, ArrowRight, Target } from 'lucide-react';
import { agentreplayClient, TraceMetadata } from '../lib/agentreplay-api';
import { TraceList } from '../components/TraceList';
import { VideoHelpButton } from '../components/VideoHelpButton';
import { saveRecentSearch } from '../../components/search/RecentSearches';

interface SearchFilters {
    searchType: 'semantic' | 'exact';
    timeRange: 'all' | '1h' | '24h' | '7d' | '30d';
    minTokens?: number;
    onlyErrors: boolean;
    model?: string;
    minDuration?: number; // For "slow" queries
}

interface EmbeddingSettings {
    enabled: boolean;
    provider: string;
    model: string;
    apiKey: string | null;
    baseUrl: string | null;
    autoIndexNewTraces: boolean;
}

const TIME_RANGE_OPTIONS = [
    { value: 'all', label: 'All Time' },
    { value: '1h', label: 'Last Hour' },
    { value: '24h', label: 'Last 24 Hours' },
    { value: '7d', label: 'Last 7 Days' },
    { value: '30d', label: 'Last 30 Days' },
];

const SEARCH_TIPS = [
    'Search for content in prompts and responses',
    'Try: "weather" to find weather-related queries',
    'Try: "gpt-4" or "claude" to filter by model',
    'Try: "tools" to find function/tool calls',
    'Search works across input, output, and metadata',
];

// Check embedding configuration
function getEmbeddingSettings(): EmbeddingSettings {
    try {
        const savedSettings = localStorage.getItem('agentreplay_settings');
        if (!savedSettings) return {
            enabled: false,
            provider: 'none',
            model: '',
            apiKey: null,
            baseUrl: null,
            autoIndexNewTraces: false
        };
        const settings = JSON.parse(savedSettings);
        return {
            enabled: settings?.embedding?.enabled || false,
            provider: settings?.embedding?.provider || 'none',
            model: settings?.embedding?.model || 'text-embedding-3-small',
            apiKey: settings?.embedding?.apiKey || null,
            baseUrl: settings?.embedding?.baseUrl || null,
            autoIndexNewTraces: settings?.embedding?.autoIndexNewTraces || false,
        };
    } catch {
        return {
            enabled: false,
            provider: 'none',
            model: '',
            apiKey: null,
            baseUrl: null,
            autoIndexNewTraces: false
        };
    }
}

// Parse natural language query to extract filters
function parseNaturalLanguageQuery(query: string): Partial<SearchFilters> & { cleanQuery: string } {
    const lower = query.toLowerCase();
    const filters: Partial<SearchFilters> = {};
    let cleanQuery = query;

    // Check for "slow" or latency-related keywords
    if (lower.includes('slow') || lower.includes('latency') || lower.includes('performance')) {
        filters.minDuration = 1000; // 1 second threshold for "slow"
        cleanQuery = cleanQuery.replace(/slow|latency|performance/gi, '').trim();
    }

    // Check for error-related keywords
    if (lower.includes('error') || lower.includes('fail') || lower.includes('exception')) {
        filters.onlyErrors = true;
        cleanQuery = cleanQuery.replace(/errors?|fail(ed|ure)?|exceptions?/gi, '').trim();
    }

    // Check for model names
    const modelPatterns = [
        { pattern: /gpt-?4o?(-mini)?/i, model: 'gpt-4' },
        { pattern: /gpt-?3\.?5/i, model: 'gpt-3.5' },
        { pattern: /claude/i, model: 'claude' },
        { pattern: /gemini/i, model: 'gemini' },
        { pattern: /llama/i, model: 'llama' },
    ];

    for (const { pattern, model } of modelPatterns) {
        if (pattern.test(lower)) {
            filters.model = model;
            cleanQuery = cleanQuery.replace(pattern, '').trim();
            break;
        }
    }

    // Check for token filters
    const tokenMatch = lower.match(/(?:>|more than|over)\s*(\d+)\s*tokens?/i);
    if (tokenMatch) {
        filters.minTokens = parseInt(tokenMatch[1]);
        cleanQuery = cleanQuery.replace(tokenMatch[0], '').trim();
    }

    // Check for "how many" / count queries - these need aggregation
    if (lower.includes('how many') || lower.includes('count') || lower.includes('total')) {
        // Mark as aggregation query - will show all matching traces
        cleanQuery = cleanQuery.replace(/how many|count|total/gi, '').trim();
    }

    // Check for "tool" or "function" calls
    if (lower.includes('tool') || lower.includes('function')) {
        // We'll search for tool/function in trace content
        cleanQuery = cleanQuery.replace(/\b(tool|function)\s*(call)?s?\b/gi, '').trim();
        // Keep "tool" in the query for content matching
        if (!cleanQuery) cleanQuery = 'tool';
    }

    // Remove common filler words
    cleanQuery = cleanQuery.replace(/\b(find|show|get|list|all|the|me|with|api|made|were|there)\b/gi, '').trim();

    return { ...filters, cleanQuery };
}

export default function Search() {
    const { projectId } = useParams<{ projectId: string }>();
    const [query, setQuery] = useState('');
    const [results, setResults] = useState<TraceMetadata[]>([]);
    const [loading, setLoading] = useState(false);
    const [searched, setSearched] = useState(false);
    const [showFilters, setShowFilters] = useState(false);
    const [searchTime, setSearchTime] = useState<number | null>(null);
    const [currentTip, setCurrentTip] = useState(0);
    const [searchMode, setSearchMode] = useState<'semantic' | 'smart' | 'text'>('smart');
    const [embeddingSettings] = useState<EmbeddingSettings>(getEmbeddingSettings);
    const [filters, setFilters] = useState<SearchFilters>({
        searchType: 'semantic',
        timeRange: '24h',
        onlyErrors: false,
    });

    // Rotate tips
    useEffect(() => {
        const interval = setInterval(() => {
            setCurrentTip((prev) => (prev + 1) % SEARCH_TIPS.length);
        }, 5000);
        return () => clearInterval(interval);
    }, []);

    const handleSearch = async (e?: React.FormEvent) => {
        e?.preventDefault();
        if (!query.trim() || !projectId) return;

        setLoading(true);
        setSearched(true);
        const startTime = performance.now();

        try {
            // Parse natural language to extract intent
            const parsed = parseNaturalLanguageQuery(query);
            const mergedFilters = { ...filters, ...parsed };

            // Build query string with extracted filters
            let fullQuery = parsed.cleanQuery || query;
            if (mergedFilters.minTokens) {
                fullQuery += ` tokens:>${mergedFilters.minTokens}`;
            }
            if (mergedFilters.onlyErrors) {
                fullQuery += ' status:error';
            }
            if (mergedFilters.model) {
                fullQuery += ` model:${mergedFilters.model}`;
            }

            // Pass embedding config if enabled
            const embeddingConfig = embeddingSettings.enabled ? {
                provider: embeddingSettings.provider,
                model: embeddingSettings.model,
                apiKey: embeddingSettings.apiKey,
                baseUrl: embeddingSettings.baseUrl,
                enabled: true,
            } : undefined;

            // Try semantic search first (with embedding config if available)
            let response = await agentreplayClient.searchTraces(
                fullQuery.trim(),
                parseInt(projectId),
                100,
                embeddingConfig
            );
            let traces = response.traces || [];
            setSearchMode(embeddingSettings.enabled ? 'semantic' : 'smart');

            // If no results, try smart fallback: fetch traces and filter client-side
            if (traces.length === 0) {
                setSearchMode('smart');

                // Fetch recent traces
                const allTraces = await agentreplayClient.listTraces({
                    limit: 500,
                    project_id: parseInt(projectId),
                    start_time: getStartTime(mergedFilters.timeRange),
                });

                traces = (allTraces.traces || []).filter((t: TraceMetadata) => {
                    // Apply filters
                    if (mergedFilters.onlyErrors && t.status !== 'error') return false;
                    if (mergedFilters.model && !t.model?.toLowerCase().includes(mergedFilters.model.toLowerCase())) return false;
                    if (mergedFilters.minTokens && (t.token_count || 0) < mergedFilters.minTokens) return false;
                    if (mergedFilters.minDuration && (t.duration_us || 0) < mergedFilters.minDuration * 1000) return false;

                    // If there's remaining query text, do a simple text match
                    if (parsed.cleanQuery) {
                        const searchText = parsed.cleanQuery.toLowerCase();
                        const meta = (t as any).metadata || {};
                        const traceText = [
                            t.model,
                            t.agent_name,
                            t.operation,
                            t.span_type,
                            (t as any).input_preview,
                            (t as any).output_preview,
                            meta['tool.name'],
                            meta['agent_id'],
                            meta['event.type'],
                        ].filter(Boolean).join(' ').toLowerCase();

                        if (!traceText.includes(searchText)) return false;
                    }

                    return true;
                });

                // Sort by duration if looking for slow traces
                if (mergedFilters.minDuration) {
                    traces.sort((a: TraceMetadata, b: TraceMetadata) => (b.duration_us || 0) - (a.duration_us || 0));
                }
            }

            setResults(traces);
            setSearchTime(performance.now() - startTime);

            // Save to recent searches
            if (projectId) {
                saveRecentSearch(query, projectId, traces.length);
            }
        } catch (error) {
            console.error('Search failed:', error);
            setResults([]);
            setSearchTime(null);
        } finally {
            setLoading(false);
        }
    };

    // Helper to get start time based on time range
    function getStartTime(timeRange: string): number {
        const now = Date.now() * 1000; // microseconds
        switch (timeRange) {
            case '1h': return now - 3600 * 1_000_000;
            case '24h': return now - 86400 * 1_000_000;
            case '7d': return now - 7 * 86400 * 1_000_000;
            case '30d': return now - 30 * 86400 * 1_000_000;
            default: return 0;
        }
    }

    // Quick filter shortcuts
    const applyQuickFilter = (filterQuery: string) => {
        setQuery(filterQuery);
        // Auto-search
        setTimeout(() => {
            const form = document.querySelector('form');
            form?.dispatchEvent(new Event('submit', { cancelable: true, bubbles: true }));
        }, 100);
    };

    return (
        <div className="flex flex-col h-full" style={{ paddingTop: '8px' }}>
            {/* Header */}
            <header className="flex items-center justify-between mb-5">
                <div className="flex items-center gap-3">
                    <div
                        className="w-9 h-9 rounded-lg flex items-center justify-center flex-shrink-0"
                        style={{ backgroundColor: 'rgba(0,128,255,0.1)' }}
                    >
                        <SearchIcon className="w-4 h-4" style={{ color: '#0080FF' }} />
                    </div>
                    <div>
                        <h1 className="text-lg font-bold tracking-tight" style={{ color: '#111827' }}>Search</h1>
                        <p className="text-xs" style={{ color: '#9ca3af' }}>Find traces by keywords, models, or natural language</p>
                    </div>
                </div>
                <div className="flex items-center gap-2">
                    <VideoHelpButton pageId="search" />
                    {embeddingSettings.enabled ? (
                        <span
                            className="flex items-center gap-1.5 px-3 py-1.5 rounded-full text-[11px] font-semibold"
                            style={{ backgroundColor: 'rgba(16,185,129,0.1)', color: '#059669', border: '1px solid rgba(16,185,129,0.2)' }}
                        >
                            <span className="w-1.5 h-1.5 rounded-full animate-pulse" style={{ backgroundColor: '#10b981' }} />
                            Vector search active
                        </span>
                    ) : (
                        <Link
                            to="/settings?tab=embedding"
                            className="flex items-center gap-1.5 px-3 py-1.5 rounded-full text-[11px] font-medium transition-all"
                            style={{ backgroundColor: 'rgba(245,158,11,0.08)', color: '#d97706', border: '1px solid rgba(245,158,11,0.2)' }}
                        >
                            <Sparkles className="w-3 h-3" />
                            Enable AI search
                        </Link>
                    )}
                </div>
            </header>

            {/* Search Bar */}
            <form onSubmit={handleSearch} className="mb-5">
                <div className="flex items-center gap-2">
                    <div className="flex-1 relative">
                        <SearchIcon className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4" style={{ color: '#9ca3af' }} />
                        <input
                            type="text"
                            value={query}
                            onChange={(e) => setQuery(e.target.value)}
                            placeholder={embeddingSettings.enabled ? "Describe what you're looking for…" : "Search by keywords, model, status…"}
                            className="w-full pl-9 pr-3 py-2.5 rounded-xl text-[13px] focus:outline-none transition-all"
                            style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb', color: '#111827' }}
                            autoFocus
                        />
                    </div>
                    <button
                        type="button"
                        onClick={() => setShowFilters(!showFilters)}
                        className="flex items-center justify-center rounded-xl transition-all flex-shrink-0"
                        style={{
                            width: '38px', height: '38px',
                            ...(showFilters
                                ? { backgroundColor: '#0080FF', color: '#ffffff' }
                                : { backgroundColor: '#ffffff', border: '1px solid #e5e7eb', color: '#9ca3af' }
                            )
                        }}
                        title="Toggle filters"
                    >
                        <Filter className="w-4 h-4" />
                    </button>
                    <button
                        type="submit"
                        disabled={loading || !query.trim()}
                        className="flex items-center gap-1.5 px-4 py-2.5 rounded-xl font-semibold text-[13px] disabled:cursor-not-allowed transition-all flex-shrink-0"
                        style={{
                            backgroundColor: loading || !query.trim() ? '#93c5fd' : '#0080FF',
                            color: '#ffffff',
                        }}
                    >
                        {loading ? <Loader2 className="w-4 h-4 animate-spin" /> : <><SearchIcon className="w-3.5 h-3.5" /> Search</>}
                    </button>
                </div>
                {/* Tip line */}
                {!searched && (
                    <div className="mt-2.5 text-[11px] flex items-center gap-1.5 pl-5" style={{ color: '#9ca3af' }}>
                        <Zap className="w-3 h-3 flex-shrink-0" style={{ color: '#0080FF' }} />
                        <span className="animate-fade-in">{SEARCH_TIPS[currentTip]}</span>
                    </div>
                )}
            </form>

            {showFilters && (
                <div
                    className="rounded-xl p-5 animate-slide-in mb-4"
                    style={{ backgroundColor: '#f9fafb', border: '1px solid #e5e7eb' }}
                >
                    <div className="flex items-center gap-2.5 mb-5">
                        <div
                            className="flex items-center justify-center w-7 h-7 rounded-lg"
                            style={{ backgroundColor: 'rgba(0, 128, 255, 0.12)' }}
                        >
                            <Filter className="w-3.5 h-3.5" style={{ color: '#0080FF' }} />
                        </div>
                        <span className="text-sm font-bold" style={{ color: '#111827' }}>Search Filters</span>
                    </div>

                    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                        {/* Search Type */}
                        <div>
                            <label className="block text-[11px] font-semibold uppercase tracking-wider mb-2" style={{ color: '#6b7280' }}>Search Type</label>
                            <div className="flex rounded-lg overflow-hidden" style={{ border: '1px solid #d1d5db' }}>
                                <button
                                    type="button"
                                    onClick={() => setFilters(f => ({ ...f, searchType: 'semantic' }))}
                                    className="flex-1 px-3 py-2 text-xs font-semibold flex items-center justify-center gap-1.5 transition-all"
                                    style={filters.searchType === 'semantic'
                                        ? { backgroundColor: '#0080FF', color: '#ffffff' }
                                        : { backgroundColor: '#f3f4f6', color: '#4b5563' }
                                    }
                                >
                                    <Sparkles className="w-3 h-3" />
                                    Semantic
                                </button>
                                <button
                                    type="button"
                                    onClick={() => setFilters(f => ({ ...f, searchType: 'exact' }))}
                                    className="flex-1 px-3 py-2 text-xs font-semibold flex items-center justify-center gap-1.5 transition-all"
                                    style={filters.searchType === 'exact'
                                        ? { backgroundColor: '#0080FF', color: '#ffffff', borderLeft: '1px solid #0080FF' }
                                        : { backgroundColor: '#f3f4f6', color: '#4b5563', borderLeft: '1px solid #d1d5db' }
                                    }
                                >
                                    <Hash className="w-3 h-3" />
                                    Exact
                                </button>
                            </div>
                        </div>

                        {/* Time Range */}
                        <div>
                            <label className="block text-[11px] font-semibold uppercase tracking-wider mb-2" style={{ color: '#6b7280' }}>Time Range</label>
                            <div className="relative">
                                <select
                                    value={filters.timeRange}
                                    onChange={(e) => setFilters(f => ({ ...f, timeRange: e.target.value as SearchFilters['timeRange'] }))}
                                    className="w-full px-3 py-2 rounded-lg text-sm appearance-none cursor-pointer focus:outline-none transition-all"
                                    style={{ backgroundColor: '#ffffff', border: '1px solid #d1d5db', color: '#1f2937' }}
                                >
                                    {TIME_RANGE_OPTIONS.map(opt => (
                                        <option key={opt.value} value={opt.value}>{opt.label}</option>
                                    ))}
                                </select>
                                <ChevronDown className="absolute right-3 top-1/2 -translate-y-1/2 w-3.5 h-3.5 pointer-events-none" style={{ color: '#6b7280' }} />
                            </div>
                        </div>

                        {/* Min Tokens */}
                        <div>
                            <label className="block text-[11px] font-semibold uppercase tracking-wider mb-2" style={{ color: '#6b7280' }}>Min Tokens</label>
                            <input
                                type="text"
                                inputMode="numeric"
                                pattern="[0-9]*"
                                value={filters.minTokens || ''}
                                onChange={(e) => {
                                    const val = e.target.value.replace(/[^0-9]/g, '');
                                    setFilters(f => ({ ...f, minTokens: val ? parseInt(val) : undefined }));
                                }}
                                placeholder="Any"
                                className="w-full px-3 py-2 rounded-lg text-sm focus:outline-none transition-all"
                                style={{ backgroundColor: '#ffffff', border: '1px solid #d1d5db', color: '#1f2937' }}
                            />
                        </div>

                        {/* Model Filter */}
                        <div>
                            <label className="block text-[11px] font-semibold uppercase tracking-wider mb-2" style={{ color: '#6b7280' }}>Model</label>
                            <input
                                type="text"
                                value={filters.model || ''}
                                onChange={(e) => setFilters(f => ({ ...f, model: e.target.value || undefined }))}
                                placeholder="e.g., gpt-4"
                                className="w-full px-3 py-2 rounded-lg text-sm focus:outline-none transition-all"
                                style={{ backgroundColor: '#ffffff', border: '1px solid #d1d5db', color: '#1f2937' }}
                            />
                        </div>
                    </div>

                    {/* Error Toggle */}
                    <div className="mt-4 flex items-center gap-2.5">
                        <input
                            type="checkbox"
                            id="only-errors"
                            checked={filters.onlyErrors}
                            onChange={(e) => setFilters(f => ({ ...f, onlyErrors: e.target.checked }))}
                            className="w-4 h-4 rounded"
                            style={{ accentColor: '#0080FF', border: '1px solid #9ca3af' }}
                        />
                        <label htmlFor="only-errors" className="text-xs flex items-center gap-1.5 font-medium" style={{ color: '#4b5563' }}>
                            <AlertCircle className="w-3.5 h-3.5" style={{ color: '#ef4444' }} />
                            Show only errors
                        </label>
                    </div>
                </div>
            )}

            {/* Results */}
            {searched && (
                <div className="space-y-4 flex-1">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-3">
                            <h2 className="text-base font-bold text-textPrimary">Results</h2>
                            {searchMode === 'smart' && results.length > 0 && (
                                <span className="px-2.5 py-1 bg-primary/10 text-primary text-[11px] font-semibold rounded-full flex items-center gap-1.5">
                                    <Lightbulb className="w-3 h-3" />
                                    Smart filter applied
                                </span>
                            )}
                        </div>
                        <div className="flex items-center gap-3">
                            {searchTime !== null && (
                                <span className="flex items-center gap-1.5 glass-card rounded-full px-3 py-1.5 text-[11px] font-medium text-textSecondary">
                                    <Clock className="w-3 h-3 text-warning" />
                                    {searchTime.toFixed(0)}ms
                                </span>
                            )}
                            <span className="glass-card rounded-full px-3 py-1.5 text-[11px] font-medium text-textSecondary">
                                {results.length} trace{results.length !== 1 ? 's' : ''} found
                            </span>
                        </div>
                    </div>

                    {/* Active Filters Pills */}
                    {(filters.minTokens || filters.onlyErrors || filters.model || filters.minDuration) && (
                        <div className="flex flex-wrap gap-2">
                            {filters.minTokens && (
                                <span className="px-2.5 py-1.5 bg-primary/10 text-primary text-[11px] font-semibold rounded-full flex items-center gap-1.5 border border-primary/15">
                                    Tokens &gt; {filters.minTokens}
                                    <button onClick={() => setFilters(f => ({ ...f, minTokens: undefined }))} className="ml-0.5 hover:opacity-70 transition-opacity">×</button>
                                </span>
                            )}
                            {filters.minDuration && (
                                <span className="px-2.5 py-1.5 bg-warning/10 text-warning text-[11px] font-semibold rounded-full flex items-center gap-1.5 border border-warning/15">
                                    <TrendingUp className="w-3 h-3" />
                                    Slow (&gt;{filters.minDuration}ms)
                                    <button onClick={() => setFilters(f => ({ ...f, minDuration: undefined }))} className="ml-0.5 hover:opacity-70 transition-opacity">×</button>
                                </span>
                            )}
                            {filters.onlyErrors && (
                                <span className="px-2.5 py-1.5 bg-error/10 text-error text-[11px] font-semibold rounded-full flex items-center gap-1.5 border border-error/15">
                                    Errors only
                                    <button onClick={() => setFilters(f => ({ ...f, onlyErrors: false }))} className="ml-0.5 hover:opacity-70 transition-opacity">×</button>
                                </span>
                            )}
                            {filters.model && (
                                <span className="px-2.5 py-1.5 bg-primary/10 text-primary text-[11px] font-semibold rounded-full flex items-center gap-1.5 border border-primary/15">
                                    Model: {filters.model}
                                    <button onClick={() => setFilters(f => ({ ...f, model: undefined }))} className="ml-0.5 hover:opacity-70 transition-opacity">×</button>
                                </span>
                            )}
                        </div>
                    )}

                    <TraceList
                        traces={results}
                        loading={loading}
                        emptyMessage={
                            <div className="text-center py-12">
                                <div className="flex items-center justify-center w-14 h-14 rounded-2xl bg-primary/5 border border-primary/10 mx-auto mb-4">
                                    <SearchIcon className="w-6 h-6 text-primary/40" />
                                </div>
                                <p className="text-sm font-medium text-textPrimary mb-1">No traces found matching "{query}"</p>
                                {!embeddingSettings.enabled && (
                                    <div className="text-xs text-textTertiary mt-3">
                                        <p className="mb-2">
                                            💡 For complex natural language queries,
                                            enable <strong className="text-textSecondary">AI embeddings</strong> in Settings.
                                        </p>
                                        <Link to="/settings?tab=embedding" className="text-primary hover:underline font-medium">
