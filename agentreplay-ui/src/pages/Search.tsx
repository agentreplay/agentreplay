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
                        <h1 className="text-lg font-bold tracking-tight text-foreground">Search</h1>
                        <p className="text-xs text-muted-foreground">Find traces by keywords, models, or natural language</p>
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
                        <SearchIcon className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                        <input
                            type="text"
                            value={query}
                            onChange={(e) => setQuery(e.target.value)}
                            placeholder={embeddingSettings.enabled ? "Describe what you're looking for…" : "Search by keywords, model, status…"}
                            className="w-full pl-9 pr-3 py-2.5 rounded-xl text-[13px] focus:outline-none transition-all bg-card border border-border text-foreground"
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
                                : { backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--muted-foreground))' }
                            )
                        }}
                        title="Toggle filters"
                    >
                        <Filter className="w-4 h-4" />
                    </button>
                    <button
                        type="submit"
                        disabled={loading || !query.trim()}
                        className="flex items-center gap-1.5 px-4 py-2.5 rounded-xl font-semibold text-[13px] disabled:cursor-not-allowed disabled:opacity-50 transition-all flex-shrink-0 bg-blue-600 hover:bg-blue-700 text-white"
                    >
                        {loading ? <Loader2 className="w-4 h-4 animate-spin" /> : <><SearchIcon className="w-3.5 h-3.5" /> Search</>}
                    </button>
                </div>
                {/* Tip line */}
                {!searched && (
                    <div className="mt-2.5 text-[11px] flex items-center gap-1.5 pl-5 text-muted-foreground">
                        <Zap className="w-3 h-3 flex-shrink-0" style={{ color: '#0080FF' }} />
                        <span className="animate-fade-in">{SEARCH_TIPS[currentTip]}</span>
                    </div>
                )}
            </form>

            {showFilters && (
                <div
                    className="rounded-xl p-5 animate-slide-in mb-4 bg-secondary border border-border"
                >
                    <div className="flex items-center gap-2.5 mb-5">
                        <div
                            className="flex items-center justify-center w-7 h-7 rounded-lg"
                            style={{ backgroundColor: 'rgba(0, 128, 255, 0.12)' }}
                        >
                            <Filter className="w-3.5 h-3.5" style={{ color: '#0080FF' }} />
                        </div>
                        <span className="text-sm font-bold text-foreground">Search Filters</span>
                    </div>

                    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                        {/* Search Type */}
                        <div>
                            <label className="block text-[11px] font-semibold uppercase tracking-wider mb-2 text-muted-foreground">Search Type</label>
                            <div className="flex rounded-lg overflow-hidden" style={{ border: '1px solid hsl(var(--border))' }}>
                                <button
                                    type="button"
                                    onClick={() => setFilters(f => ({ ...f, searchType: 'semantic' }))}
                                    className="flex-1 px-3 py-2 text-xs font-semibold flex items-center justify-center gap-1.5 transition-all"
                                    style={filters.searchType === 'semantic'
                                        ? { backgroundColor: '#0080FF', color: '#ffffff' }
                                        : { backgroundColor: 'hsl(var(--secondary))', color: 'hsl(var(--muted-foreground))' }
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
                                        : { backgroundColor: 'hsl(var(--secondary))', color: 'hsl(var(--muted-foreground))', borderLeft: '1px solid hsl(var(--border))' }
                                    }
                                >
                                    <Hash className="w-3 h-3" />
                                    Exact
                                </button>
                            </div>
                        </div>

                        {/* Time Range */}
                        <div>
                            <label className="block text-[11px] font-semibold uppercase tracking-wider mb-2 text-muted-foreground">Time Range</label>
                            <div className="relative">
                                <select
                                    value={filters.timeRange}
                                    onChange={(e) => setFilters(f => ({ ...f, timeRange: e.target.value as SearchFilters['timeRange'] }))}
                                    className="w-full px-3 py-2 rounded-lg text-sm appearance-none cursor-pointer focus:outline-none transition-all bg-card border border-border text-foreground"
                                >
                                    {TIME_RANGE_OPTIONS.map(opt => (
                                        <option key={opt.value} value={opt.value}>{opt.label}</option>
                                    ))}
                                </select>
                                <ChevronDown className="absolute right-3 top-1/2 -translate-y-1/2 w-3.5 h-3.5 pointer-events-none text-muted-foreground" />
                            </div>
                        </div>

                        {/* Min Tokens */}
                        <div>
                            <label className="block text-[11px] font-semibold uppercase tracking-wider mb-2 text-muted-foreground">Min Tokens</label>
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
                                className="w-full px-3 py-2 rounded-lg text-sm focus:outline-none transition-all bg-card border border-border text-foreground"
                            />
                        </div>

                        {/* Model Filter */}
                        <div>
                            <label className="block text-[11px] font-semibold uppercase tracking-wider mb-2 text-muted-foreground">Model</label>
                            <input
                                type="text"
                                value={filters.model || ''}
                                onChange={(e) => setFilters(f => ({ ...f, model: e.target.value || undefined }))}
                                placeholder="e.g., gpt-4"
                                className="w-full px-3 py-2 rounded-lg text-sm focus:outline-none transition-all bg-card border border-border text-foreground"
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
                        <label htmlFor="only-errors" className="text-xs flex items-center gap-1.5 font-medium" style={{ color: 'hsl(var(--muted-foreground))' }}>
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
                                            Configure embeddings →
                                        </Link>
                                    </div>
                                )}
                            </div>
                        }
                    />
                </div>
            )}

            {/* Empty State — Feature Guide */}
            {!searched && (
                <div className="flex-1 flex flex-col gap-4 overflow-auto">

                    {/* How It Works — step cards */}
                    <div className="grid grid-cols-3 gap-3">
                        {[
                            {
                                step: '1', title: 'Enter a query',
                                cardClass: 'bg-blue-50 dark:bg-blue-950/30 border border-blue-200 dark:border-blue-800/40',
                                badgeClass: 'bg-blue-100 dark:bg-blue-900/50 text-blue-600 dark:text-blue-400',
                                desc: 'Type keywords, model names, or describe what you\'re looking for in plain language.',
                            },
                            {
                                step: '2', title: 'Apply filters',
                                cardClass: 'bg-violet-50 dark:bg-violet-950/30 border border-violet-200 dark:border-violet-800/40',
                                badgeClass: 'bg-violet-100 dark:bg-violet-900/50 text-violet-600 dark:text-violet-400',
                                desc: 'Narrow results by time range, token count, model, or error status.',
                            },
                            {
                                step: '3', title: 'Explore traces',
                                cardClass: 'bg-emerald-50 dark:bg-emerald-950/30 border border-emerald-200 dark:border-emerald-800/40',
                                badgeClass: 'bg-emerald-100 dark:bg-emerald-900/50 text-emerald-600 dark:text-emerald-400',
                                desc: 'Click any result to view conversations, spans, latency, and metadata.',
                            },
                        ].map((s) => (
                            <div
                                key={s.step}
                                className={`rounded-xl p-4 ${s.cardClass}`}
                            >
                                <div className="flex items-center gap-2.5 mb-2.5">
                                    <div
                                        className={`flex items-center justify-center flex-shrink-0 w-7 h-7 rounded-lg text-xs font-extrabold ${s.badgeClass}`}
                                    >
                                        {s.step}
                                    </div>
                                    <span className="text-sm font-bold text-foreground">{s.title}</span>
                                </div>
                                <p className="text-xs leading-relaxed text-muted-foreground">{s.desc}</p>
                            </div>
                        ))}
                    </div>

                    {/* Search Capabilities — icon cards grid */}
                    <div className="grid grid-cols-3 gap-3">
                        {[
                            { icon: <SearchIcon className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" />, label: 'Prompts & Completions', detail: 'Full-text search across inputs and outputs', iconBgClass: 'bg-blue-100 dark:bg-blue-900/40' },
                            { icon: <Hash className="w-3.5 h-3.5 text-emerald-600 dark:text-emerald-400" />, label: 'Model Names', detail: 'gpt-4, claude-3, llama, gemini, etc.', iconBgClass: 'bg-emerald-100 dark:bg-emerald-900/40' },
                            { icon: <Zap className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" />, label: 'Tool & Function Calls', detail: 'Names, arguments, and return values', iconBgClass: 'bg-amber-100 dark:bg-amber-900/40' },
                            { icon: <AlertCircle className="w-3.5 h-3.5 text-red-600 dark:text-red-400" />, label: 'Errors & Exceptions', detail: 'Timeouts, rate limits, failures', iconBgClass: 'bg-red-100 dark:bg-red-900/40' },
                            { icon: <Info className="w-3.5 h-3.5 text-indigo-600 dark:text-indigo-400" />, label: 'Tags & Metadata', detail: 'Session IDs, user identifiers, labels', iconBgClass: 'bg-indigo-100 dark:bg-indigo-900/40' },
                            { icon: <Brain className="w-3.5 h-3.5 text-violet-600 dark:text-violet-400" />, label: 'Semantic Meaning', detail: 'Natural language queries via AI embeddings', iconBgClass: 'bg-violet-100 dark:bg-violet-900/40' },
                        ].map((c) => (
                            <div
                                key={c.label}
                                className="flex items-start gap-3 rounded-xl p-3.5 bg-card border border-border"
                            >
                                <div
                                    className={`flex items-center justify-center flex-shrink-0 rounded-lg w-[30px] h-[30px] ${c.iconBgClass}`}
                                >
                                    {c.icon}
                                </div>
                                <div className="min-w-0">
                                    <p className="text-xs font-semibold text-foreground">{c.label}</p>
                                    <p className="text-[11px] mt-0.5 text-muted-foreground">{c.detail}</p>
                                </div>
                            </div>
                        ))}
                    </div>

                    {/* Section 3: Try Example Searches */}
                    <div className="rounded-lg p-4 border border-border">
                        <div className="flex items-center justify-between mb-3">
                            <div className="flex items-center gap-2">
                                <Lightbulb className="w-4 h-4 text-amber-500" />
                                <span className="text-xs font-bold text-foreground">Try Example Searches</span>
                            </div>
                            <div className="flex items-center gap-2">
                                <button
                                    onClick={() => applyQuickFilter('slow')}
                                    className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[11px] font-medium transition-all cursor-pointer bg-amber-500/10 border border-amber-500/20 text-amber-600 dark:text-amber-400"
                                >
                                    <TrendingUp className="w-3 h-3" />
                                    Slow traces
                                </button>
                                <button
                                    onClick={() => applyQuickFilter('error')}
                                    className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[11px] font-medium transition-all cursor-pointer bg-red-500/10 border border-red-500/20 text-red-600 dark:text-red-400"
                                >
                                    <AlertCircle className="w-3 h-3" />
                                    Errors only
                                </button>
                            </div>
                        </div>
                        <div className="flex flex-wrap gap-2">
                            {(embeddingSettings.enabled
                                ? ['weather in San Francisco', 'Spanish translation', 'math calculation', 'code explanation', 'summarize this document']
                                : ['gpt-4', 'claude', 'tools', 'error', 'assistant', 'function_call', 'timeout']
                            ).map((example) => (
                                <button
                                    key={example}
                                    onClick={() => applyQuickFilter(example)}
                                    className="px-3 py-1.5 rounded-md text-[11px] font-medium transition-all cursor-pointer bg-card border border-border text-foreground hover:bg-secondary"
                                >
                                    {example}
                                </button>
                            ))}
                        </div>
                    </div>

                    {/* Section 4: Available Filters */}
                    <div className="rounded-lg p-4 border border-border">
                        <div className="flex items-center gap-2 mb-3">
                            <Filter className="w-4 h-4 text-violet-500" />
                            <span className="text-xs font-bold text-foreground">Available Filters</span>
                        </div>
                        <div className="grid grid-cols-4 gap-3">
                            {[
                                { icon: <Sparkles className="w-3.5 h-3.5 text-blue-500" />, name: 'Search Type', detail: 'Semantic AI or exact keyword matching' },
                                { icon: <Clock className="w-3.5 h-3.5 text-amber-500" />, name: 'Time Range', detail: 'Last hour, day, week, or all time' },
                                { icon: <Hash className="w-3.5 h-3.5 text-emerald-500" />, name: 'Token Count', detail: 'Minimum token threshold' },
                                { icon: <AlertCircle className="w-3.5 h-3.5 text-red-500" />, name: 'Error Status', detail: 'Filter to error traces only' },
                            ].map((f) => (
                                <div
                                    key={f.name}
                                    className="flex items-start gap-2.5 p-2.5 rounded-lg bg-card border border-border/50"
                                >
                                    <div className="mt-0.5 flex-shrink-0">{f.icon}</div>
                                    <div>
                                        <p className="text-[11px] font-semibold text-foreground">{f.name}</p>
                                        <p className="text-[10px] text-muted-foreground">{f.detail}</p>
                                    </div>
                                </div>
                            ))}
                        </div>
                    </div>

                    {/* Semantic Search Upsell */}
                    {!embeddingSettings.enabled && (
                        <div className="rounded-lg p-4 flex items-center gap-4 bg-violet-500/8 border border-border">
                            <Brain className="w-6 h-6 flex-shrink-0 text-violet-500" />
                            <div className="flex-1 min-w-0">
                                <p className="text-xs font-semibold text-foreground">Unlock Semantic Search</p>
                                <p className="text-[11px] mt-0.5 text-muted-foreground">
                                    Enable AI embeddings to search traces by meaning, not just keywords.
                                    Find "slow API calls" even when traces don't contain those exact words.
                                </p>
                            </div>
                            <Link
                                to="/settings?tab=embedding"
                                className="flex items-center gap-1.5 flex-shrink-0 px-4 py-2 rounded-lg text-xs font-semibold transition-all bg-violet-600 hover:bg-violet-700 text-white"
                            >
                                Enable <ArrowRight className="w-3.5 h-3.5" />
                            </Link>
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
