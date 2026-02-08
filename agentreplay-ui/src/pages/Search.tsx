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
import { Search as SearchIcon, Loader2, Zap, Filter, Clock, AlertCircle, Sparkles, Hash, ChevronDown, Lightbulb, TrendingUp, Settings, Info, Brain } from 'lucide-react';
import { agentreplayClient, TraceMetadata } from '../lib/agentreplay-api';
import { TraceList } from '../components/TraceList';
import { VideoHelpButton } from '../components/VideoHelpButton';
import { RecentSearches, SuggestedSearches, saveRecentSearch } from '../../components/search/RecentSearches';

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
        <div className="p-6 max-w-7xl mx-auto">
            {/* Header */}
            <div className="flex items-center justify-between mb-6">
                <div className="flex items-center gap-3">
                    <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-primary/20 to-primary/5 flex items-center justify-center">
                        <Sparkles className="w-6 h-6 text-primary" />
                    </div>
                    <div>
                        <h1 className="text-2xl font-bold text-textPrimary">Semantic Search</h1>
                        <p className="text-textSecondary">Find traces using natural language or filters</p>
                    </div>
                </div>
                
                {/* Video Help & Embedding Status */}
                <div className="flex items-center gap-3">
                    <VideoHelpButton pageId="search" />
                    {embeddingSettings.enabled ? (
                        <span className="flex items-center gap-2 px-3 py-1.5 bg-success/10 text-success rounded-full text-sm">
                            <Brain className="w-4 h-4" />
                            Vector search enabled
                        </span>
                    ) : (
                        <Link
                            to="/settings?tab=embedding"
                            className="flex items-center gap-2 px-3 py-1.5 bg-warning/10 text-warning rounded-full text-sm hover:bg-warning/20 transition-colors"
                        >
                            <Info className="w-4 h-4" />
                            Enable embeddings for semantic search
                        </Link>
                    )}
                </div>
            </div>

            {/* Semantic Search Info Banner - only show if embeddings not enabled */}
            {!embeddingSettings.enabled && !searched && (
                <div className="mb-6 p-4 bg-surface border border-border rounded-xl">
                    <div className="flex items-start gap-3">
                        <div className="p-2 bg-primary/10 rounded-lg">
                            <Brain className="w-5 h-5 text-primary" />
                        </div>
                        <div className="flex-1">
                            <h3 className="font-medium text-textPrimary mb-1">How Semantic Search Works</h3>
                            <p className="text-sm text-textSecondary mb-3">
                                <strong>True semantic search</strong> uses AI embeddings to understand the meaning of your query, 
                                not just keywords. For example, searching "slow API calls" would find traces with high latency, 
                                even if they don't contain those exact words.
                            </p>
                            <div className="flex items-center gap-4 text-sm">
                                <div className="flex items-center gap-2 text-textSecondary">
                                    <span className="w-2 h-2 bg-success rounded-full"></span>
                                    <strong>Currently:</strong> Smart filter search (keywords + patterns)
                                </div>
                                <Link
                                    to="/settings?tab=embedding"
                                    className="flex items-center gap-1 text-primary hover:underline"
                                >
                                    <Settings className="w-4 h-4" />
                                    Enable AI embeddings →
                                </Link>
                            </div>
                        </div>
                    </div>
                </div>
            )}

            {/* Search Box */}
            <div className="mb-6">
                <form onSubmit={handleSearch} className="relative">
                    <SearchIcon className="absolute left-4 top-1/2 transform -translate-y-1/2 w-5 h-5 text-textTertiary" />
                    <input
                        type="text"
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                        placeholder={embeddingSettings.enabled ? "Search using natural language (AI-powered)..." : "Search by keywords, model, status..."}
                        className="w-full pl-12 pr-32 py-4 bg-surface border border-border rounded-xl text-lg text-textPrimary placeholder-textTertiary focus:outline-none focus:ring-2 focus:ring-primary shadow-sm"
                        autoFocus
                    />
                    <div className="absolute right-2 top-1/2 transform -translate-y-1/2 flex items-center gap-2">
                        <button
                            type="button"
                            onClick={() => setShowFilters(!showFilters)}
                            className={`p-2 rounded-lg transition-colors ${showFilters ? 'bg-primary/20 text-primary' : 'hover:bg-surface-hover text-textSecondary'}`}
                        >
                            <Filter className="w-5 h-5" />
                        </button>
                        <button
                            type="submit"
                            disabled={loading || !query.trim()}
                            className="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors font-medium"
                        >
                            {loading ? <Loader2 className="w-5 h-5 animate-spin" /> : 'Search'}
                        </button>
                    </div>
                </form>

                {/* Rotating Tips */}
                {!searched && (
                    <div className="mt-3 text-sm text-textTertiary flex items-center gap-2">
                        <Zap className="w-4 h-4 text-primary" />
                        <span className="animate-fade-in">{SEARCH_TIPS[currentTip]}</span>
                    </div>
                )}
            </div>

            {/* Filters Panel */}
            {showFilters && (
                <div className="mb-6 p-4 bg-surface border border-border rounded-xl">
                    <div className="flex items-center gap-2 mb-4">
                        <Filter className="w-4 h-4 text-textSecondary" />
                        <span className="text-sm font-medium text-textPrimary">Search Filters</span>
                    </div>
                    
                    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                        {/* Search Type */}
                        <div>
                            <label className="block text-xs text-textTertiary mb-1.5">Search Type</label>
                            <div className="flex rounded-lg border border-border overflow-hidden">
                                <button
                                    type="button"
                                    onClick={() => setFilters(f => ({ ...f, searchType: 'semantic' }))}
                                    className={`flex-1 px-3 py-2 text-sm flex items-center justify-center gap-1 transition-colors ${
                                        filters.searchType === 'semantic' 
                                            ? 'bg-primary text-white' 
                                            : 'bg-background hover:bg-surface-hover text-textSecondary'
                                    }`}
                                >
                                    <Sparkles className="w-3 h-3" />
                                    Semantic
                                </button>
                                <button
                                    type="button"
                                    onClick={() => setFilters(f => ({ ...f, searchType: 'exact' }))}
                                    className={`flex-1 px-3 py-2 text-sm flex items-center justify-center gap-1 transition-colors ${
                                        filters.searchType === 'exact' 
                                            ? 'bg-primary text-white' 
                                            : 'bg-background hover:bg-surface-hover text-textSecondary'
                                    }`}
                                >
                                    <Hash className="w-3 h-3" />
                                    Exact
                                </button>
                            </div>
                        </div>

                        {/* Time Range */}
                        <div>
                            <label className="block text-xs text-textTertiary mb-1.5">Time Range</label>
                            <div className="relative">
                                <select
                                    value={filters.timeRange}
                                    onChange={(e) => setFilters(f => ({ ...f, timeRange: e.target.value as SearchFilters['timeRange'] }))}
                                    className="w-full px-3 py-2 bg-background border border-border rounded-lg text-sm text-textPrimary appearance-none cursor-pointer"
                                >
                                    {TIME_RANGE_OPTIONS.map(opt => (
                                        <option key={opt.value} value={opt.value}>{opt.label}</option>
                                    ))}
                                </select>
                                <ChevronDown className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-textTertiary pointer-events-none" />
                            </div>
                        </div>

                        {/* Min Tokens */}
                        <div>
                            <label className="block text-xs text-textTertiary mb-1.5">Min Tokens</label>
                            <input
                                type="number"
                                min={0}
                                value={filters.minTokens || ''}
                                onChange={(e) => setFilters(f => ({ ...f, minTokens: e.target.value ? parseInt(e.target.value) : undefined }))}
                                placeholder="Any"
                                className="w-full px-3 py-2 bg-background border border-border rounded-lg text-sm text-textPrimary placeholder-textTertiary"
                            />
                        </div>

                        {/* Model Filter */}
                        <div>
                            <label className="block text-xs text-textTertiary mb-1.5">Model</label>
                            <input
                                type="text"
                                value={filters.model || ''}
                                onChange={(e) => setFilters(f => ({ ...f, model: e.target.value || undefined }))}
                                placeholder="e.g., gpt-4"
                                className="w-full px-3 py-2 bg-background border border-border rounded-lg text-sm text-textPrimary placeholder-textTertiary"
                            />
                        </div>
                    </div>

                    {/* Error Toggle */}
                    <div className="mt-4 flex items-center gap-2">
                        <input
                            type="checkbox"
                            id="only-errors"
                            checked={filters.onlyErrors}
                            onChange={(e) => setFilters(f => ({ ...f, onlyErrors: e.target.checked }))}
                            className="w-4 h-4 rounded border-border text-primary focus:ring-primary"
                        />
                        <label htmlFor="only-errors" className="text-sm text-textSecondary flex items-center gap-1">
                            <AlertCircle className="w-4 h-4 text-error" />
                            Show only errors
                        </label>
                    </div>
                </div>
            )}

            {/* Results */}
            {searched && (
                <div className="space-y-4">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-3">
                            <h2 className="text-lg font-semibold text-textPrimary">Results</h2>
                            {searchMode === 'smart' && results.length > 0 && (
                                <span className="px-2 py-0.5 bg-primary/10 text-primary text-xs rounded-full flex items-center gap-1">
                                    <Lightbulb className="w-3 h-3" />
                                    Smart filter applied
                                </span>
                            )}
                        </div>
                        <div className="flex items-center gap-4 text-sm text-textSecondary">
                            {searchTime !== null && (
                                <span className="flex items-center gap-1">
                                    <Clock className="w-4 h-4" />
                                    {searchTime.toFixed(0)}ms
                                </span>
                            )}
                            <span>
                                Found {results.length} trace{results.length !== 1 ? 's' : ''}
                            </span>
                        </div>
                    </div>

                    {/* Active Filters Pills */}
                    {(filters.minTokens || filters.onlyErrors || filters.model || filters.minDuration) && (
                        <div className="flex flex-wrap gap-2">
                            {filters.minTokens && (
                                <span className="px-2 py-1 bg-primary/10 text-primary text-xs rounded-full flex items-center gap-1">
                                    Tokens &gt; {filters.minTokens}
                                    <button onClick={() => setFilters(f => ({ ...f, minTokens: undefined }))} className="ml-1 hover:text-primary-hover">×</button>
                                </span>
                            )}
                            {filters.minDuration && (
                                <span className="px-2 py-1 bg-warning/10 text-warning text-xs rounded-full flex items-center gap-1">
                                    <TrendingUp className="w-3 h-3" />
                                    Slow (&gt;{filters.minDuration}ms)
                                    <button onClick={() => setFilters(f => ({ ...f, minDuration: undefined }))} className="ml-1 hover:opacity-70">×</button>
                                </span>
                            )}
                            {filters.onlyErrors && (
                                <span className="px-2 py-1 bg-error/10 text-error text-xs rounded-full flex items-center gap-1">
                                    Errors only
                                    <button onClick={() => setFilters(f => ({ ...f, onlyErrors: false }))} className="ml-1 hover:opacity-70">×</button>
                                </span>
                            )}
                            {filters.model && (
                                <span className="px-2 py-1 bg-primary/10 text-primary text-xs rounded-full flex items-center gap-1">
                                    Model: {filters.model}
                                    <button onClick={() => setFilters(f => ({ ...f, model: undefined }))} className="ml-1 hover:text-primary-hover">×</button>
                                </span>
                            )}
                        </div>
                    )}

                    <TraceList
                        traces={results}
                        loading={loading}
                        emptyMessage={
                            <div className="text-center py-8">
                                <p className="text-textSecondary mb-4">No traces found matching "{query}"</p>
                                {!embeddingSettings.enabled && (
                                    <div className="text-sm text-textTertiary">
                                        <p className="mb-2">
                                            💡 For complex natural language queries like "{query}", 
                                            <br />enable <strong>AI embeddings</strong> in Settings.
                                        </p>
                                        <Link to="/settings?tab=embedding" className="text-primary hover:underline">
                                            Configure embeddings →
                                        </Link>
                                    </div>
                                )}
                            </div>
                        }
                    />
                </div>
            )}

            {/* Empty State */}
            {!searched && (
                <div className="space-y-6">
                    {/* Recent Searches */}
                    {projectId && (
                        <RecentSearches
                            projectId={projectId}
                            onSelect={(selectedQuery) => {
                                setQuery(selectedQuery);
                                setTimeout(() => {
                                    const form = document.querySelector('form');
                                    form?.dispatchEvent(new Event('submit', { cancelable: true, bubbles: true }));
                                }, 100);
                            }}
                            maxItems={5}
                        />
                    )}
                    
                    {/* Suggested Searches */}
                    <SuggestedSearches
                        onSelect={(selectedQuery) => {
                            setQuery(selectedQuery);
                            setTimeout(() => {
                                const form = document.querySelector('form');
                                form?.dispatchEvent(new Event('submit', { cancelable: true, bubbles: true }));
                            }, 100);
                        }}
                    />

                    <div className="text-center py-8">
                        <div className="w-20 h-20 mx-auto mb-6 rounded-2xl bg-gradient-to-br from-primary/10 to-primary/5 flex items-center justify-center">
                            {embeddingSettings.enabled ? (
                                <Brain className="w-10 h-10 text-primary/50" />
                            ) : (
                                <Sparkles className="w-10 h-10 text-primary/50" />
                            )}
                        </div>
                        <h3 className="text-lg font-medium text-textPrimary mb-2">
                            {embeddingSettings.enabled ? 'AI-Powered Search' : 'Smart Trace Search'}
                        </h3>
                        <p className="text-textSecondary max-w-md mx-auto mb-6">
                            {embeddingSettings.enabled 
                                ? 'Use natural language to search. AI understands meaning, not just keywords.'
                                : 'Search by keywords, model names, status, and patterns. Enable embeddings for true semantic search.'
                            }
                        </p>
                        
                        {/* Example queries */}
                        <div className="flex flex-wrap justify-center gap-2 mb-8">
                            {embeddingSettings.enabled ? (
                                // True semantic search examples
                                ['weather in San Francisco', 'Spanish translation', 'math calculation', 'code explanation'].map((example) => (
                                    <button
                                        key={example}
                                        onClick={() => applyQuickFilter(example)}
                                        className="px-3 py-1.5 bg-surface border border-border rounded-full text-sm text-textSecondary hover:text-primary hover:border-primary transition-colors"
                                    >
                                        {example}
                                    </button>
                                ))
                            ) : (
                                // Pattern-based search examples
                                ['gpt-4', 'claude', 'tools', 'assistant', 'user question'].map((example) => (
                                    <button
                                        key={example}
                                        onClick={() => applyQuickFilter(example)}
                                        className="px-3 py-1.5 bg-surface border border-border rounded-full text-sm text-textSecondary hover:text-primary hover:border-primary transition-colors"
                                    >
                                        {example}
                                    </button>
                                ))
                            )}
                        </div>
                        
                        {/* Quick Actions */}
                        <div className="flex justify-center gap-4 text-sm">
                            <button
                                onClick={() => applyQuickFilter('slow')}
                                className="flex items-center gap-2 px-4 py-2 bg-warning/10 text-warning rounded-lg hover:bg-warning/20 transition-colors"
                            >
                                <TrendingUp className="w-4 h-4" />
                                Find slow traces
                            </button>
                            <button
                                onClick={() => applyQuickFilter('error')}
                                className="flex items-center gap-2 px-4 py-2 bg-error/10 text-error rounded-lg hover:bg-error/20 transition-colors"
                            >
                                <AlertCircle className="w-4 h-4" />
                                Find errors
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
