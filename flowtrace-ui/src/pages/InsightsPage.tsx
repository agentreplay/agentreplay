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

import { useState, useEffect, useCallback } from 'react';
import { useParams } from 'react-router-dom';
import { flowtraceClient, type InsightsSummary, type InsightView, type InsightsQuery } from '../lib/flowtrace-api';
import { useToast } from '../context/toast-context';
import { VideoHelpButton } from '../components/VideoHelpButton';
import { 
  AlertCircle, 
  AlertTriangle, 
  Info, 
  TrendingUp, 
  Activity,
  Clock,
  RefreshCw,
  ChevronDown,
  ChevronUp,
  Shield,
  Zap,
  DollarSign,
  MessageSquare,
  Target
} from 'lucide-react';

// Severity badge component
function SeverityBadge({ severity }: { severity: string }) {
  const colors = {
    critical: 'bg-red-500/20 text-red-400 border-red-500/30',
    high: 'bg-orange-500/20 text-orange-400 border-orange-500/30',
    medium: 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30',
    low: 'bg-blue-500/20 text-blue-400 border-blue-500/30',
    info: 'bg-gray-500/20 text-gray-400 border-gray-500/30',
  };

  const icons = {
    critical: <AlertCircle className="w-3 h-3" />,
    high: <AlertTriangle className="w-3 h-3" />,
    medium: <AlertTriangle className="w-3 h-3" />,
    low: <Info className="w-3 h-3" />,
    info: <Info className="w-3 h-3" />,
  };

  const colorClass = colors[severity as keyof typeof colors] || colors.info;
  const icon = icons[severity as keyof typeof icons] || icons.info;

  return (
    <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium border ${colorClass}`}>
      {icon}
      {severity.charAt(0).toUpperCase() + severity.slice(1)}
    </span>
  );
}

// Insight type icon
function InsightTypeIcon({ type }: { type: string }) {
  const icons: Record<string, JSX.Element> = {
    latency_anomaly: <Clock className="w-4 h-4 text-yellow-400" />,
    error_rate_anomaly: <AlertCircle className="w-4 h-4 text-red-400" />,
    cost_anomaly: <DollarSign className="w-4 h-4 text-green-400" />,
    semantic_drift: <Target className="w-4 h-4 text-purple-400" />,
    failure_pattern: <AlertTriangle className="w-4 h-4 text-orange-400" />,
    performance_regression: <TrendingUp className="w-4 h-4 text-blue-400" />,
    traffic_anomaly: <Activity className="w-4 h-4 text-cyan-400" />,
    token_usage_spike: <MessageSquare className="w-4 h-4 text-pink-400" />,
  };

  return icons[type] || <Zap className="w-4 h-4 text-gray-400" />;
}

// Health Score Gauge
function HealthScoreGauge({ score }: { score: number }) {
  const getColor = (s: number) => {
    if (s >= 90) return 'text-green-400';
    if (s >= 70) return 'text-yellow-400';
    if (s >= 50) return 'text-orange-400';
    return 'text-red-400';
  };

  const getLabel = (s: number) => {
    if (s >= 90) return 'Excellent';
    if (s >= 70) return 'Good';
    if (s >= 50) return 'Fair';
    return 'Needs Attention';
  };

  return (
    <div className="flex flex-col items-center">
      <div className={`text-6xl font-bold ${getColor(score)}`}>
        {score}
      </div>
      <div className="text-sm text-textSecondary mt-1">
        {getLabel(score)}
      </div>
      <div className="w-full h-2 bg-surface rounded-full mt-3 overflow-hidden">
        <div 
          className={`h-full rounded-full transition-all duration-500 ${
            score >= 90 ? 'bg-green-400' :
            score >= 70 ? 'bg-yellow-400' :
            score >= 50 ? 'bg-orange-400' : 'bg-red-400'
          }`}
          style={{ width: `${score}%` }}
        />
      </div>
    </div>
  );
}

// Insight Card Component
function InsightCard({ insight, isExpanded, onToggle }: { 
  insight: InsightView; 
  isExpanded: boolean; 
  onToggle: () => void;
}) {
  return (
    <div className="bg-surface border border-border rounded-lg overflow-hidden hover:border-primary/50 transition-colors">
      <button 
        onClick={onToggle}
        className="w-full p-4 text-left flex items-start gap-4"
      >
        <InsightTypeIcon type={insight.insight_type} />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <SeverityBadge severity={insight.severity} />
            <span className="text-xs text-textSecondary">
              {new Date(insight.generated_at / 1000).toLocaleString()}
            </span>
          </div>
          <h3 className="text-textPrimary font-medium truncate">
            {insight.summary}
          </h3>
          <p className="text-sm text-textSecondary mt-1 line-clamp-2">
            {insight.description}
          </p>
        </div>
        <div className="flex-shrink-0 text-textSecondary">
          {isExpanded ? <ChevronUp className="w-5 h-5" /> : <ChevronDown className="w-5 h-5" />}
        </div>
      </button>
      
      {isExpanded && (
        <div className="px-4 pb-4 border-t border-border">
          <div className="pt-4">
            <h4 className="text-sm font-medium text-textPrimary mb-2">Suggestions</h4>
            <ul className="space-y-2">
              {insight.suggestions.map((suggestion, i) => (
                <li key={i} className="flex items-start gap-2 text-sm text-textSecondary">
                  <span className="text-primary mt-0.5">•</span>
                  {suggestion}
                </li>
              ))}
            </ul>

            {insight.related_trace_ids.length > 0 && (
              <div className="mt-4">
                <h4 className="text-sm font-medium text-textPrimary mb-2">Related Traces</h4>
                <div className="flex flex-wrap gap-2">
                  {insight.related_trace_ids.slice(0, 5).map((traceId) => (
                    <a
                      key={traceId}
                      href={`/traces/${traceId}`}
                      className="text-xs font-mono bg-background px-2 py-1 rounded text-primary hover:underline"
                    >
                      {traceId.substring(0, 16)}...
                    </a>
                  ))}
                  {insight.related_trace_ids.length > 5 && (
                    <span className="text-xs text-textSecondary">
                      +{insight.related_trace_ids.length - 5} more
                    </span>
                  )}
                </div>
              </div>
            )}

            <div className="mt-4 flex items-center gap-4 text-xs text-textSecondary">
              <span>Confidence: {(insight.confidence * 100).toFixed(0)}%</span>
              <span>Type: {insight.insight_type.replace(/_/g, ' ')}</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default function InsightsPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const numericProjectId = parseInt(projectId || '0');
  
  const [summary, setSummary] = useState<InsightsSummary | null>(null);
  const [insights, setInsights] = useState<InsightView[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [filters, setFilters] = useState<InsightsQuery>({
    project_id: numericProjectId,
    window_seconds: 3600,
    limit: 50,
  });
  const { error: showError, success: showSuccess } = useToast();

  // Update filters when projectId changes
  useEffect(() => {
    setFilters(f => ({ ...f, project_id: numericProjectId }));
  }, [numericProjectId]);

  const loadData = useCallback(async () => {
    if (!numericProjectId) return;
    
    try {
      const [summaryData, insightsData] = await Promise.all([
        flowtraceClient.getInsightsSummary(numericProjectId),
        flowtraceClient.getInsights({ ...filters, project_id: numericProjectId }),
      ]);
      setSummary(summaryData);
      setInsights(insightsData.insights);
    } catch (err) {
      console.error('Failed to load insights:', err);
      showError('Failed to load insights');
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, [filters, numericProjectId, showError]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleRefresh = async () => {
    setRefreshing(true);
    await loadData();
  };

  const handleGenerateInsights = async () => {
    if (!numericProjectId) return;
    
    setGenerating(true);
    try {
      // Call the insights endpoint with force_refresh to regenerate
      const [summaryData, insightsData] = await Promise.all([
        flowtraceClient.getInsightsSummary(numericProjectId),
        flowtraceClient.getInsights({ ...filters, project_id: numericProjectId }),
      ]);
      setSummary(summaryData);
      setInsights(insightsData.insights);
      
      if (insightsData.insights.length > 0) {
        showSuccess(`Generated ${insightsData.insights.length} insights`);
      } else {
        showSuccess('Analysis complete - no issues detected');
      }
    } catch (err) {
      console.error('Failed to generate insights:', err);
      showError('Failed to generate insights');
    } finally {
      setGenerating(false);
    }
  };

  const timeWindows = [
    { label: '1 hour', value: 3600 },
    { label: '6 hours', value: 21600 },
    { label: '24 hours', value: 86400 },
    { label: '7 days', value: 604800 },
  ];

  if (loading) {
    return (
      <div className="min-h-screen bg-background flex items-center justify-center">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary"></div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-background">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-3xl font-bold text-textPrimary mb-2">Insights</h1>
            <p className="text-textSecondary">
              Automatic anomaly detection and pattern recognition
            </p>
          </div>
          <div className="flex items-center gap-3">
            <VideoHelpButton pageId="insights" />
            <button
              onClick={handleRefresh}
              disabled={refreshing}
              className="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary/90 disabled:opacity-50 transition-colors"
            >
              <RefreshCw className={`w-4 h-4 ${refreshing ? 'animate-spin' : ''}`} />
              Refresh
            </button>
          </div>
        </div>

        {/* Summary Cards */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
          {/* Health Score */}
          <div className="bg-surface border border-border rounded-lg p-6">
            <div className="flex items-center gap-2 mb-4">
              <Shield className="w-5 h-5 text-primary" />
              <h3 className="text-sm font-medium text-textSecondary">Health Score</h3>
            </div>
            <HealthScoreGauge score={summary?.health_score || 100} />
          </div>

          {/* Total Insights */}
          <div className="bg-surface border border-border rounded-lg p-6">
            <div className="flex items-center gap-2 mb-4">
              <Activity className="w-5 h-5 text-accent" />
              <h3 className="text-sm font-medium text-textSecondary">Total Insights</h3>
            </div>
            <div className="text-4xl font-bold text-textPrimary">
              {summary?.total_insights || 0}
            </div>
            <div className="mt-4 flex items-center gap-4 text-sm">
              <span className="text-red-400">{summary?.critical_count || 0} critical</span>
              <span className="text-orange-400">{summary?.high_count || 0} high</span>
            </div>
          </div>

          {/* By Severity */}
          <div className="bg-surface border border-border rounded-lg p-6">
            <div className="flex items-center gap-2 mb-4">
              <AlertTriangle className="w-5 h-5 text-warning" />
              <h3 className="text-sm font-medium text-textSecondary">By Severity</h3>
            </div>
            <div className="space-y-2">
              {['critical', 'high', 'medium', 'low'].map((sev) => (
                <div key={sev} className="flex items-center justify-between text-sm">
                  <span className="text-textSecondary capitalize">{sev}</span>
                  <span className="text-textPrimary font-medium">
                    {summary?.by_severity?.[sev] || 0}
                  </span>
                </div>
              ))}
            </div>
          </div>

          {/* By Type */}
          <div className="bg-surface border border-border rounded-lg p-6">
            <div className="flex items-center gap-2 mb-4">
              <TrendingUp className="w-5 h-5 text-success" />
              <h3 className="text-sm font-medium text-textSecondary">By Type</h3>
            </div>
            <div className="space-y-2">
              {Object.entries(summary?.by_type || {}).slice(0, 4).map(([type, count]) => (
                <div key={type} className="flex items-center justify-between text-sm">
                  <span className="text-textSecondary truncate max-w-[120px]">
                    {type.replace(/_/g, ' ')}
                  </span>
                  <span className="text-textPrimary font-medium">{count}</span>
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* Filters */}
        <div className="flex items-center gap-4 mb-6">
          <div className="flex items-center gap-2">
            <Clock className="w-4 h-4 text-textSecondary" />
            <span className="text-sm text-textSecondary">Time window:</span>
            <div className="flex rounded-lg overflow-hidden border border-border">
              {timeWindows.map((tw) => (
                <button
                  key={tw.value}
                  onClick={() => setFilters(f => ({ ...f, window_seconds: tw.value }))}
                  className={`px-3 py-1.5 text-sm transition-colors ${
                    filters.window_seconds === tw.value
                      ? 'bg-primary text-white'
                      : 'bg-surface text-textSecondary hover:text-textPrimary'
                  }`}
                >
                  {tw.label}
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* Top Insights */}
        {summary?.top_insights && summary.top_insights.length > 0 && (
          <div className="mb-8">
            <h2 className="text-xl font-semibold text-textPrimary mb-4">
              Top Insights
            </h2>
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
              {summary.top_insights.map((insight) => (
                <InsightCard
                  key={insight.id}
                  insight={insight}
                  isExpanded={expandedId === insight.id}
                  onToggle={() => setExpandedId(expandedId === insight.id ? null : insight.id)}
                />
              ))}
            </div>
          </div>
        )}

        {/* All Insights */}
        <div>
          <h2 className="text-xl font-semibold text-textPrimary mb-4">
            All Insights ({insights.length})
          </h2>
          
          {insights.length === 0 ? (
            <div className="bg-surface border border-border rounded-lg p-12 text-center">
              <Shield className="w-12 h-12 text-success mx-auto mb-4" />
              <h3 className="text-lg font-medium text-textPrimary mb-2">
                No issues detected
              </h3>
              <p className="text-textSecondary max-w-md mx-auto mb-6">
                Your system is running smoothly. We'll alert you when we detect
                anomalies, performance regressions, or unusual patterns.
              </p>
              <button
                onClick={handleGenerateInsights}
                disabled={generating}
                className="inline-flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary/90 disabled:opacity-50 transition-colors"
              >
                <Activity className={`w-4 h-4 ${generating ? 'animate-pulse' : ''}`} />
                {generating ? 'Analyzing...' : 'Analyze Traces Now'}
              </button>
            </div>
          ) : (
            <div className="space-y-3">
              {insights.map((insight) => (
                <InsightCard
                  key={insight.id}
                  insight={insight}
                  isExpanded={expandedId === insight.id}
                  onToggle={() => setExpandedId(expandedId === insight.id ? null : insight.id)}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
