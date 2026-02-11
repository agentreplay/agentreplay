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

import { useEffect, useState, useMemo, useCallback } from 'react';
import { useNavigate, useLocation, useParams } from 'react-router-dom';
import { useProjects } from '../context/project-context';
import { API_BASE_URL } from '../../lib/api-config';
import { VideoHelpButton } from '../components/VideoHelpButton';
import {
  AnomalyTimeSeries,
  type TimeSeriesPoint as AnomalyPoint,
  type Anomaly,
  type ControlLimits
} from '../../components/metrics';
import {
  CostByModelChart,
  CostByOperationChart,
  TokenEfficiencyCard,
} from '../../components/costs/CostBreakdown';
import {
  DollarSign,
  Clock,
  Activity,
  AlertCircle,
  BarChart3,
  Bot,
  Server,
  Database,
  Globe,
  Sparkles,
  ZoomIn,
  ZoomOut,
  Maximize,
  RefreshCcw,
  Loader2,
  AlertTriangle,
  Calendar
} from 'lucide-react';
import {
  LineChart,
  Line,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  ScatterChart,
  Scatter,
  ZAxis,
  Cell
} from 'recharts';
import { agentreplayClient, TraceMetadata, Agent } from '../lib/agentreplay-api';
import { TraceList } from '../components/TraceList';
import { format } from 'date-fns';

// ============================================================================
// Types
// ============================================================================

interface TimeSeriesPoint {
  timestamp: number;
  cost: number;
  latency: number;
  requests: number;
  errors: number;
  label: string;
}

interface Node {
  id: string;
  type: 'agent' | 'service' | 'database' | 'external' | 'llm';
  label: string;
  x: number;
  y: number;
  status: 'active' | 'inactive' | 'error';
  metadata?: any;
  calls?: number;
  avgLatency?: number;
}

interface Link {
  source: string;
  target: string;
  value: number;
  label?: string;
}

interface AIAnalysisResult {
  nodes: Array<{
    id: string;
    type: string;
    label: string;
    calls: number;
    avgLatency?: number;
  }>;
  edges: Array<{
    from: string;
    to: string;
    count: number;
    label?: string;
  }>;
  summary: string;
  insights: string[];
}

// ============================================================================
// Main Component
// ============================================================================

export default function AnalyticsPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { projectId: urlProjectId } = useParams<{ projectId: string }>();
  const { currentProject } = useProjects();
  const [activeTab, setActiveTab] = useState<'dashboard' | 'timeline' | 'system-map'>('dashboard');
  const [timeRange, setTimeRange] = useState('24h');
  const [isGlobalMode, setIsGlobalMode] = useState(false); // Global analytics toggle
  const [loading, setLoading] = useState(true);
  const [metrics, setMetrics] = useState({
    totalCost: 0,
    avgLatency: 0,
    totalRequests: 0,
    errorRate: 0,
  });
  const [timeSeriesData, setTimeSeriesData] = useState<TimeSeriesPoint[]>([]);
  const [costAnalytics, setCostAnalytics] = useState<{
    modelCosts: Array<{ model: string; cost: number; tokens: number; calls: number; color: string }>;
    operationCosts: Array<{ operation: string; cost: number; percentage: number; icon: string }>;
    efficiency: { avgCostPer1k: number; cacheHitRate: number; potentialSavings: number };
  } | null>(null);

  useEffect(() => {
    if (activeTab === 'dashboard') {
      loadAnalytics();
    }
  }, [timeRange, currentProject, urlProjectId, activeTab, isGlobalMode]);

  const loadAnalytics = async () => {
    setLoading(true);
    try {
      const now = Date.now() * 1000;
      const ranges: Record<string, number> = {
        '1h': 60 * 60 * 1_000_000,
        '6h': 6 * 60 * 60 * 1_000_000,
        '24h': 24 * 60 * 60 * 1_000_000,
        '7d': 7 * 24 * 60 * 60 * 1_000_000,
        '30d': 30 * 24 * 60 * 60 * 1_000_000,
      };
      const intervalSeconds: Record<string, number> = {
        '1h': 60,
        '6h': 300,
        '24h': 1800,
        '7d': 7200,
        '30d': 28800,
      };
      const startTime = now - ranges[timeRange];
      const endTime = now;
      const interval = intervalSeconds[timeRange] || 300;

      // In global mode, don't filter by project_id to show all data
      const projectIdStr = currentProject?.project_id || urlProjectId;
      const projectIdNum = !isGlobalMode && projectIdStr ? parseInt(projectIdStr, 10) : null;
      const projectFilter = projectIdNum ? `&project_id=${projectIdNum}` : '';

      let buckets: any[] = [];

      const granularityMap: Record<string, string> = {
        '1h': 'minute',
        '6h': 'minute',
        '24h': 'hour',
        '7d': 'hour',
        '30d': 'day',
      };
      const granularity = granularityMap[timeRange] || 'hour';

      let apiSummary: any = null;

      try {
        const response = await fetch(
          `${API_BASE_URL}/api/v1/analytics/timeseries?metric=request_count&start_time=${startTime}&end_time=${endTime}&granularity=${granularity}${projectFilter}`,
          { headers: { 'Content-Type': 'application/json' } }
        );
        if (response.ok) {
          const data = await response.json();
          const dataPoints = data.data || data.data_points || [];
          apiSummary = data.summary || null;

          if (dataPoints.length > 0) {
            buckets = dataPoints.map((point: any) => ({
              timestamp: point.timestamp,
              total_cost: point.total_cost || 0,
              request_count: point.request_count || 0,
              error_count: point.error_count || 0,
              total_duration: 0,
              total_tokens: point.total_tokens || 0,
              avg_duration: point.avg_duration || 0,
            }));
          }
        }
      } catch (e) {
        console.log('Analytics endpoint not available, falling back to traces:', e);
      }

      if (buckets.length === 0) {
        const tracesUrl = new URL(`${API_BASE_URL}/api/v1/traces`);
        tracesUrl.searchParams.set('limit', '1000');
        tracesUrl.searchParams.set('start_ts', startTime.toString());
        tracesUrl.searchParams.set('end_ts', endTime.toString());
        if (projectIdNum) {
          tracesUrl.searchParams.set('project_id', projectIdNum.toString());
        }

        const tracesResponse = await fetch(tracesUrl.toString(), {
          headers: { 'Content-Type': 'application/json' }
        });

        if (tracesResponse.ok) {
          const tracesData = await tracesResponse.json();
          const traces = tracesData.traces || [];

          const bucketMap = new Map<number, any>();
          const intervalUs = interval * 1_000_000;

          for (const trace of traces) {
            const bucketTs = Math.floor(trace.timestamp_us / intervalUs) * intervalUs;
            if (!bucketMap.has(bucketTs)) {
              bucketMap.set(bucketTs, {
                timestamp: bucketTs,
                total_cost: 0,
                request_count: 0,
                error_count: 0,
                total_duration: 0,
                total_tokens: 0,
              });
            }
            const bucket = bucketMap.get(bucketTs);
            bucket.request_count += 1;
            bucket.total_cost += trace.cost || 0;
            bucket.total_duration += trace.duration_us || 0;
            bucket.total_tokens += trace.tokens || trace.token_count || 0;
            if (trace.status === 'error') bucket.error_count += 1;
          }

          buckets = Array.from(bucketMap.values()).map(b => ({
            ...b,
            avg_duration: b.request_count > 0 ? b.total_duration / b.request_count / 1000 : 0,
          })).sort((a, b) => a.timestamp - b.timestamp);
        }
      }

      // Use API summary if available, otherwise calculate from buckets
      const totalCost = apiSummary?.total_cost ?? buckets.reduce((sum: number, b: any) => sum + (b.total_cost || 0), 0);
      const totalRequests = apiSummary?.total_requests ?? buckets.reduce((sum: number, b: any) => sum + (b.request_count || 0), 0);
      const totalErrors = buckets.reduce((sum: number, b: any) => sum + (b.error_count || 0), 0);
      const avgLatency = apiSummary?.avg_duration_ms ?? (buckets.length > 0
        ? buckets.reduce((sum: number, b: any) => sum + (b.avg_duration || 0), 0) / buckets.length
        : 0);
      const errorRate = apiSummary?.error_rate ?? (totalRequests > 0 ? (totalErrors / totalRequests) * 100 : 0);

      const seriesData: TimeSeriesPoint[] = buckets.map((bucket: any) => ({
        timestamp: bucket.timestamp,
        cost: bucket.total_cost || 0,
        latency: bucket.avg_duration || 0,
        requests: bucket.request_count || 0,
        errors: bucket.error_count || 0,
        label: formatTimestamp(bucket.timestamp, timeRange),
      }));

      setMetrics({ totalCost, avgLatency, totalRequests, errorRate });
      setTimeSeriesData(seriesData);

      // Fetch cost analytics with real data
      try {
        const costUrl = new URL(`${API_BASE_URL}/api/v1/analytics/costs`);
        costUrl.searchParams.set('start_time', startTime.toString());
        costUrl.searchParams.set('end_time', endTime.toString());
        if (projectIdNum) {
          costUrl.searchParams.set('project_id', projectIdNum.toString());
        }

        const costResponse = await fetch(costUrl.toString(), {
          headers: { 'Content-Type': 'application/json' }
        });

        if (costResponse.ok) {
          const costData = await costResponse.json();
          setCostAnalytics({
            modelCosts: (costData.model_breakdown || []).map((m: any) => ({
              model: m.model,
              cost: m.cost,
              tokens: m.tokens,
              calls: m.calls,
              color: m.color,
            })),
            operationCosts: (costData.operation_breakdown || []).map((op: any) => ({
              operation: op.operation,
              cost: op.cost,
              percentage: op.percentage,
              icon: op.icon,
            })),
            efficiency: {
              avgCostPer1k: costData.efficiency?.avg_cost_per_1k || 0,
              cacheHitRate: costData.efficiency?.cache_hit_rate || 0,
              potentialSavings: costData.efficiency?.potential_savings || 0,
            },
          });
        }
      } catch (costError) {
        console.log('Cost analytics not available:', costError);
      }
    } catch (error) {
      console.error('Failed to load analytics:', error);
      setMetrics({ totalCost: 0, avgLatency: 0, totalRequests: 0, errorRate: 0 });
      setTimeSeriesData([]);
    } finally {
      setLoading(false);
    }
  };

  const formatTimestamp = (timestampUs: number, range: string): string => {
    const date = new Date(timestampUs / 1000);
    if (range === '1h' || range === '6h') {
      return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
    } else if (range === '24h') {
      return date.toLocaleTimeString('en-US', { hour: '2-digit' });
    } else {
      return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    }
  };

  const computeAnomalyData = (values: number[], timestamps: number[], metricName: string) => {
    if (values.length < 3) return { points: [], anomalies: [], controlLimits: { upperLimit: 0, centerLine: 0, lowerLimit: 0 } };

    const mean = values.reduce((a, b) => a + b, 0) / values.length;
    const stdDev = Math.sqrt(values.reduce((sum, v) => sum + Math.pow(v - mean, 2), 0) / values.length);

    const points: AnomalyPoint[] = timestamps.map((ts, i) => ({
      timestamp: ts,
      value: values[i],
      trend: mean,
      residual: values[i] - mean
    }));

    const anomalies: Anomaly[] = [];
    values.forEach((v, i) => {
      const zScore = stdDev > 0 ? Math.abs(v - mean) / stdDev : 0;
      if (zScore > 2) {
        anomalies.push({
          id: `${metricName}-${i}`,
          timestamp: timestamps[i],
          value: v,
          expected: mean,
          zScore,
          type: 'point',
          severity: zScore > 3 ? 'critical' : zScore > 2.5 ? 'warning' : 'info',
          investigated: false,
          falsePositive: false
        });
      }
    });

    const controlLimits: ControlLimits = {
      upperLimit: mean + 2 * stdDev,
      centerLine: mean,
      lowerLimit: Math.max(0, mean - 2 * stdDev)
    };

    return { points, anomalies, controlLimits };
  };

  const latencyAnomalyData = useMemo(() => {
    const values = timeSeriesData.map(d => d.latency);
    const timestamps = timeSeriesData.map(d => d.timestamp);
    return computeAnomalyData(values, timestamps, 'latency');
  }, [timeSeriesData]);

  const errorsAnomalyData = useMemo(() => {
    const values = timeSeriesData.map(d => d.errors);
    const timestamps = timeSeriesData.map(d => d.timestamp);
    return computeAnomalyData(values, timestamps, 'errors');
  }, [timeSeriesData]);

  return (
    <div className="flex flex-col h-full">
      <div className="flex-1 px-2 py-4">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: 'linear-gradient(135deg, #0080FF, #00c8ff)' }}>
              <BarChart3 className="w-5 h-5" style={{ color: '#ffffff' }} />
            </div>
            <div>
              <h1 className="text-[22px] font-bold text-foreground">Analytics</h1>
              <p className="text-[13px] text-muted-foreground">
                Monitor performance, timeline, and system topology
                {isGlobalMode ? (
                  <span className="ml-2 px-2 py-0.5 text-[11px] rounded font-medium" style={{ backgroundColor: 'rgba(16,185,129,0.08)', color: '#10b981' }}>
                    Global (All Projects)
                  </span>
                ) : currentProject && (
                  <span className="ml-2 px-2 py-0.5 text-[11px] rounded font-medium" style={{ backgroundColor: 'rgba(0,128,255,0.08)', color: '#0080FF' }}>
                    {currentProject.name}
                  </span>
                )}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <VideoHelpButton pageId="analytics" />
            {/* Global/Project Toggle */}
            {activeTab === 'dashboard' && (
              <button
                onClick={() => setIsGlobalMode(!isGlobalMode)}
                className="flex items-center gap-2 px-3 py-2 rounded-lg text-[13px] font-medium transition-all"
                style={{
                  backgroundColor: isGlobalMode ? 'rgba(16,185,129,0.08)' : 'hsl(var(--card))',
                  border: isGlobalMode ? '1px solid rgba(16,185,129,0.2)' : '1px solid hsl(var(--border))',
                  color: isGlobalMode ? '#10b981' : 'hsl(var(--muted-foreground))',
                }}
              >
                <Globe className="w-4 h-4" />
                {isGlobalMode ? 'Global' : 'Project'}
              </button>
            )}
            {activeTab === 'dashboard' && (
              <select
                value={timeRange}
                onChange={(e) => setTimeRange(e.target.value)}
                className="px-4 py-2 rounded-lg text-[13px] bg-card border border-border text-foreground"
              >
                <option value="1h">Last Hour</option>
                <option value="6h">Last 6 Hours</option>
                <option value="24h">Last 24 Hours</option>
                <option value="7d">Last 7 Days</option>
                <option value="30d">Last 30 Days</option>
              </select>
            )}
          </div>
        </div>

        {/* Tabs */}
        <div className="flex gap-1 mb-6 rounded-xl p-1 bg-card border border-border">
          {[
            { key: 'dashboard' as const, icon: <BarChart3 className="w-4 h-4" />, label: 'Dashboard' },
            { key: 'timeline' as const, icon: <Clock className="w-4 h-4" />, label: 'Timeline' },
            { key: 'system-map' as const, icon: <Activity className="w-4 h-4" />, label: 'System Map' },
          ].map((tab) => (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              className="flex items-center gap-2 px-4 py-2 rounded-lg text-[13px] font-semibold transition-all"
              style={{
                backgroundColor: activeTab === tab.key ? '#0080FF' : 'transparent',
                color: activeTab === tab.key ? '#ffffff' : 'hsl(var(--muted-foreground))',
              }}
            >
              {tab.icon}
              {tab.label}
            </button>
          ))}
        </div>

        {/* Tab Content */}
        {activeTab === 'dashboard' && (
          <DashboardTab
            loading={loading}
            metrics={metrics}
            timeSeriesData={timeSeriesData}
            latencyAnomalyData={latencyAnomalyData}
            errorsAnomalyData={errorsAnomalyData}
            navigate={navigate}
            location={location}
            currentProject={currentProject}
            costAnalytics={costAnalytics}
          />
        )}

        {activeTab === 'timeline' && (
          <TimelineTab currentProject={currentProject} />
        )}

        {activeTab === 'system-map' && (
          <SystemMapTab />
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Dashboard Tab
// ============================================================================

function DashboardTab({
  loading,
  metrics,
  timeSeriesData,
  latencyAnomalyData,
  errorsAnomalyData,
  navigate,
  location,
  currentProject,
  costAnalytics
}: {
  loading: boolean;
  metrics: { totalCost: number; avgLatency: number; totalRequests: number; errorRate: number };
  timeSeriesData: TimeSeriesPoint[];
  latencyAnomalyData: any;
  errorsAnomalyData: any;
  navigate: any;
  location: any;
  currentProject: any;
  costAnalytics: {
    modelCosts: Array<{ model: string; cost: number; tokens: number; calls: number; color: string }>;
    operationCosts: Array<{ operation: string; cost: number; percentage: number; icon: string }>;
    efficiency: { avgCostPer1k: number; cacheHitRate: number; potentialSavings: number };
  } | null;
}) {
  if (loading) {
    return (
      <>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
          {[1, 2, 3, 4].map((i) => (
            <div key={i} className="rounded-2xl p-6 animate-pulse bg-card border border-border">
              <div className="flex items-center justify-between mb-4">
                <div className="h-10 w-10 rounded-lg bg-secondary" />
                <div className="h-4 w-12 rounded bg-secondary" />
              </div>
              <div className="h-4 w-20 rounded mb-2 bg-secondary" />
              <div className="h-8 w-24 rounded bg-secondary" />
            </div>
          ))}
        </div>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {[1, 2].map((i) => (
            <div key={i} className="rounded-2xl p-6 animate-pulse bg-card border border-border">
              <div className="h-6 w-32 rounded mb-4 bg-secondary" />
              <div className="h-48 rounded bg-secondary" />
            </div>
          ))}
        </div>
      </>
    );
  }

  if (timeSeriesData.length === 0) {
    return (
      <div className="rounded-2xl overflow-hidden flex flex-col items-center bg-card border border-border" style={{ minHeight: '380px' }}>
        <div className="py-14 px-10 text-center flex-1 flex flex-col items-center justify-center">
          <div className="w-16 h-16 rounded-2xl flex items-center justify-center mx-auto mb-4" style={{ background: 'linear-gradient(135deg, rgba(0,128,255,0.1), rgba(0,200,255,0.06))' }}>
            <BarChart3 className="w-8 h-8" style={{ color: '#0080FF' }} />
          </div>
          <p className="text-[18px] font-bold mb-2 text-foreground">No analytics data yet</p>
          <p className="text-[14px] mb-5 max-w-lg mx-auto leading-relaxed text-muted-foreground">
            Dashboard gives you a bird's-eye view of your AI agent performance — track costs, latency, request volume, error rates, and detect anomalies across all your traces.
          </p>
          <div className="flex flex-wrap items-center justify-center gap-2 mb-6">
            <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold" style={{ backgroundColor: 'rgba(0,128,255,0.06)', color: '#0080FF', border: '1px solid rgba(0,128,255,0.12)' }}>📊 Cost Tracking</span>
            <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold" style={{ backgroundColor: 'rgba(16,185,129,0.06)', color: '#10b981', border: '1px solid rgba(16,185,129,0.12)' }}>⚡ Latency Monitoring</span>
            <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold" style={{ backgroundColor: 'rgba(139,92,246,0.06)', color: '#8b5cf6', border: '1px solid rgba(139,92,246,0.12)' }}>🔍 Anomaly Detection</span>
            <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold" style={{ backgroundColor: 'rgba(245,158,11,0.06)', color: '#f59e0b', border: '1px solid rgba(245,158,11,0.12)' }}>📈 Request Volume</span>
            <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold" style={{ backgroundColor: 'rgba(239,68,68,0.06)', color: '#ef4444', border: '1px solid rgba(239,68,68,0.12)' }}>🚨 Error Rate</span>
          </div>

          {/* Other Tabs Guide */}
          <div className="flex gap-3 mb-6 max-w-lg mx-auto">
            <div className="flex-1 rounded-xl p-3 text-left bg-secondary border border-border">
              <div className="flex items-center gap-1.5 mb-1">
                <Clock className="w-3.5 h-3.5" style={{ color: '#8b5cf6' }} />
                <span className="text-[12px] font-bold text-foreground">Timeline</span>
              </div>
              <p className="text-[11px] leading-relaxed text-muted-foreground">Scatter chart of execution durations with recent activity log</p>
            </div>
            <div className="flex-1 rounded-xl p-3 text-left bg-secondary border border-border">
              <div className="flex items-center gap-1.5 mb-1">
                <Activity className="w-3.5 h-3.5" style={{ color: '#f59e0b' }} />
                <span className="text-[12px] font-bold text-foreground">System Map</span>
              </div>
              <p className="text-[11px] leading-relaxed text-muted-foreground">Interactive topology of agents, LLM models, and service connections</p>
            </div>
          </div>

          <button
            onClick={() => {
              const projectId = currentProject?.project_id || location.pathname.split('/')[2];
              navigate(`/projects/${projectId}/docs`);
            }}
            className="inline-flex items-center gap-2 px-5 py-2.5 rounded-xl text-[13px] font-semibold transition-all"
            style={{ backgroundColor: '#0080FF', color: '#ffffff' }}
          >
            View SDK Documentation
          </button>
        </div>
      </div>
    );
  }

  return (
    <>
      {/* Metrics Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
        <MetricCard
          title="Total Cost"
          value={`$${metrics.totalCost.toFixed(4)}`}
          icon={<DollarSign className="w-6 h-6" />}
          color="green"
        />
        <MetricCard
          title="Avg. Latency"
          value={`${metrics.avgLatency.toFixed(0)}ms`}
          icon={<Clock className="w-6 h-6" />}
          color="yellow"
        />
        <MetricCard
          title="Total Requests"
          value={metrics.totalRequests.toLocaleString()}
          icon={<Activity className="w-6 h-6" />}
          color="blue"
        />
        <MetricCard
          title="Error Rate"
          value={`${metrics.errorRate.toFixed(2)}%`}
          icon={<AlertCircle className="w-6 h-6" />}
          color="red"
        />
      </div>

      {/* Charts Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
        {/* Cost Over Time */}
        <div className="rounded-2xl p-6 bg-card border border-border">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-[15px] font-bold text-foreground">Cost Over Time</h3>
            <button
              onClick={() => navigate('/traces?sort=cost&order=desc')}
              className="text-[12px] font-medium transition-colors"
              style={{ color: '#0080FF' }}
            >
              Show most expensive →
            </button>
          </div>
          <ResponsiveContainer width="100%" height={250}>
            <LineChart data={timeSeriesData}>
              <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
              <XAxis dataKey="label" stroke="#9ca3af" tick={{ fill: '#9ca3af', fontSize: 11 }} />
              <YAxis stroke="#9ca3af" tick={{ fill: '#9ca3af', fontSize: 11 }} tickFormatter={(value) => `$${value.toFixed(4)}`} />
              <Tooltip
                contentStyle={{ backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: '12px', boxShadow: '0 4px 12px rgba(0,0,0,0.06)' }}
                formatter={(value: number) => [`$${value.toFixed(4)}`, 'Cost']}
              />
              <Line type="monotone" dataKey="cost" stroke="#10b981" strokeWidth={2} dot={{ fill: '#10b981', r: 4 }} activeDot={{ r: 6 }} />
            </LineChart>
          </ResponsiveContainer>
        </div>

        {/* Latency Over Time */}
        <div className="rounded-2xl p-6 bg-card border border-border">
          <h3 className="text-[15px] font-bold mb-4 text-foreground">Latency Over Time</h3>
          <ResponsiveContainer width="100%" height={250}>
            <LineChart data={timeSeriesData}>
              <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
              <XAxis dataKey="label" stroke="#9ca3af" tick={{ fill: '#9ca3af', fontSize: 11 }} />
              <YAxis stroke="#9ca3af" tick={{ fill: '#9ca3af', fontSize: 11 }} tickFormatter={(value) => `${value.toFixed(0)}ms`} />
              <Tooltip
                contentStyle={{ backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: '12px', boxShadow: '0 4px 12px rgba(0,0,0,0.06)' }}
                formatter={(value: number) => [`${value.toFixed(0)}ms`, 'Avg Latency']}
              />
              <Line type="monotone" dataKey="latency" stroke="#eab308" strokeWidth={2} dot={{ fill: '#eab308', r: 4 }} activeDot={{ r: 6 }} />
            </LineChart>
          </ResponsiveContainer>
        </div>

        {/* Requests Over Time */}
        <div className="rounded-2xl p-6 bg-card border border-border">
          <h3 className="text-[15px] font-bold mb-4 text-foreground">Requests Over Time</h3>
          <ResponsiveContainer width="100%" height={250}>
            <BarChart data={timeSeriesData}>
              <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
              <XAxis dataKey="label" stroke="#9ca3af" tick={{ fill: '#9ca3af', fontSize: 11 }} />
              <YAxis stroke="#9ca3af" tick={{ fill: '#9ca3af', fontSize: 11 }} />
              <Tooltip
                contentStyle={{ backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: '12px', boxShadow: '0 4px 12px rgba(0,0,0,0.06)' }}
                formatter={(value: number) => [value, 'Requests']}
              />
              <Bar dataKey="requests" fill="#3b82f6" radius={[8, 8, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>

        {/* Errors Over Time */}
        <div className="rounded-2xl p-6 bg-card border border-border">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-[15px] font-bold text-foreground">Errors Over Time</h3>
            <button
              onClick={() => navigate('/traces?status=error')}
              className="text-[12px] font-medium transition-colors"
              style={{ color: '#0080FF' }}
            >
              View error traces →
            </button>
          </div>
          <ResponsiveContainer width="100%" height={250}>
            <BarChart data={timeSeriesData}>
              <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
              <XAxis dataKey="label" stroke="#9ca3af" tick={{ fill: '#9ca3af', fontSize: 11 }} />
              <YAxis stroke="#9ca3af" tick={{ fill: '#9ca3af', fontSize: 11 }} allowDecimals={false} />
              <Tooltip
                contentStyle={{ backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', borderRadius: '12px', boxShadow: '0 4px 12px rgba(0,0,0,0.06)' }}
                formatter={(value: number) => [value, 'Errors']}
              />
              <Bar dataKey="errors" fill="#ef4444" radius={[8, 8, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* Anomaly Detection Section */}
      {timeSeriesData.length >= 10 && (
        <div className="mb-8">
          <div className="flex items-center gap-2 mb-2">
            <div className="w-7 h-7 rounded-lg flex items-center justify-center" style={{ background: 'linear-gradient(135deg, rgba(139,92,246,0.12), rgba(168,85,247,0.06))' }}>
              <ZoomIn className="w-3.5 h-3.5" style={{ color: '#8b5cf6' }} />
            </div>
            <h3 className="text-[16px] font-bold text-foreground">Anomaly Detection</h3>
          </div>
          <p className="text-[13px] mb-4 text-muted-foreground">
            Statistical analysis using 2σ control limits to detect unusual patterns in your metrics.
          </p>
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <AnomalyTimeSeries
              metricName="Latency"
              data={latencyAnomalyData.points}
              anomalies={latencyAnomalyData.anomalies}
              controlLimits={latencyAnomalyData.controlLimits}
              showDecomposition={false}
            />
            <AnomalyTimeSeries
              metricName="Error Count"
              data={errorsAnomalyData.points}
              anomalies={errorsAnomalyData.anomalies}
              controlLimits={errorsAnomalyData.controlLimits}
              showDecomposition={false}
            />
          </div>
        </div>
      )}

      {/* Cost Analytics Section */}
      <div className="mb-8">
        <div className="flex items-center gap-2 mb-2">
          <div className="w-7 h-7 rounded-lg flex items-center justify-center" style={{ background: 'linear-gradient(135deg, rgba(16,185,129,0.12), rgba(52,211,153,0.06))' }}>
            <DollarSign className="w-3.5 h-3.5" style={{ color: '#10b981' }} />
          </div>
          <h3 className="text-[16px] font-bold text-foreground">Cost Analytics</h3>
        </div>
        <p className="text-[13px] mb-4 text-muted-foreground">
          Breakdown of LLM costs by model and operation type to optimize spending.
        </p>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-6">
          <CostByModelChart
            modelCosts={costAnalytics?.modelCosts}
            totalCost={costAnalytics?.modelCosts?.reduce((sum, m) => sum + m.cost, 0) || metrics.totalCost}
          />
          <CostByOperationChart operationCosts={costAnalytics?.operationCosts} />
        </div>
        <TokenEfficiencyCard
          avgCostPer1k={costAnalytics?.efficiency?.avgCostPer1k}
          cacheHitRate={costAnalytics?.efficiency?.cacheHitRate}
          potentialSavings={costAnalytics?.efficiency?.potentialSavings}
        />
      </div>
    </>
  );
}

// ============================================================================
// Timeline Tab
// ============================================================================

function TimelineTab({ currentProject }: { currentProject: any }) {
  const [traces, setTraces] = useState<TraceMetadata[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchTraces = async () => {
      setLoading(true);
      try {
        const params: any = { limit: 100 };
        if (currentProject) {
          params.project_id = parseInt(currentProject.project_id);
        }
        const response = await agentreplayClient.listTraces(params);
        setTraces(response.traces || []);
      } catch (err) {
        console.error('Failed to fetch timeline data:', err);
        setError('Failed to load timeline data');
      } finally {
        setLoading(false);
      }
    };

    fetchTraces();
  }, [currentProject]);

  const chartData = traces.map(t => ({
    id: t.trace_id,
    time: t.timestamp_us / 1000,
    duration: t.duration_us ? t.duration_us / 1000 : 0,
    status: t.status,
    agent: t.agent_name
  }));

  const CustomTooltip = ({ active, payload }: any) => {
    if (active && payload && payload.length) {
      const data = payload[0].payload;
      return (
        <div className="p-3 rounded-xl text-xs bg-card border border-border shadow-md">
          <p className="font-semibold text-foreground">{data.agent || 'Unknown Agent'}</p>
          <p className="text-muted-foreground">Time: {format(new Date(data.time), 'HH:mm:ss')}</p>
          <p className="text-muted-foreground">Duration: {data.duration.toFixed(0)}ms</p>
          <p className="capitalize" style={{ color: data.status === 'error' ? '#ef4444' : '#10b981' }}>
            {data.status}
          </p>
        </div>
      );
    }
    return null;
  };

  if (error) {
    return (
      <div className="mb-6 p-4 rounded-xl text-[13px]" style={{ backgroundColor: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.12)', color: '#ef4444' }}>
        {error}
      </div>
    );
  }

  const navigate = useNavigate();

  if (!loading && traces.length === 0) {
    return (
      <div className="rounded-2xl overflow-hidden flex flex-col items-center bg-card border border-border" style={{ minHeight: '380px' }}>
        <div className="py-14 px-10 text-center flex-1 flex flex-col items-center justify-center">
          <div className="w-16 h-16 rounded-2xl flex items-center justify-center mx-auto mb-4" style={{ background: 'linear-gradient(135deg, rgba(139,92,246,0.1), rgba(168,85,247,0.06))' }}>
            <Clock className="w-8 h-8" style={{ color: '#8b5cf6' }} />
          </div>
          <p className="text-[18px] font-bold mb-2 text-foreground">No timeline data yet</p>
          <p className="text-[14px] mb-5 max-w-md mx-auto leading-relaxed text-muted-foreground">
            Timeline shows the execution distribution and activity history of your AI agent traces.
          </p>
          <div className="flex items-center justify-center gap-2 mb-6">
            <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold" style={{ backgroundColor: 'rgba(59,130,246,0.06)', color: '#3b82f6', border: '1px solid rgba(59,130,246,0.12)' }}>📊 Scatter Chart</span>
            <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold" style={{ backgroundColor: 'rgba(16,185,129,0.06)', color: '#10b981', border: '1px solid rgba(16,185,129,0.12)' }}>⏱️ Duration Analysis</span>
            <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold" style={{ backgroundColor: 'rgba(139,92,246,0.06)', color: '#8b5cf6', border: '1px solid rgba(139,92,246,0.12)' }}>🤖 Agent Breakdown</span>
          </div>
          <button
            onClick={() => {
              const projectId = currentProject?.project_id || location.pathname.split('/')[2];
              navigate(`/projects/${projectId}/docs`);
            }}
            className="inline-flex items-center gap-2 px-5 py-2.5 rounded-xl text-[13px] font-semibold transition-all"
            style={{ backgroundColor: '#0080FF', color: '#ffffff' }}
          >
            View SDK Documentation
          </button>
        </div>
      </div>
    );
  }

  return (
    <div>
      {/* Timeline Chart */}
      <div className="rounded-2xl p-6 mb-8 h-[300px] bg-card border border-border">
        <h2 className="text-[13px] font-semibold mb-4 flex items-center gap-2" style={{ color: 'hsl(var(--muted-foreground))', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
          <Activity className="w-4 h-4" />
          Execution Distribution (Last 100 Traces)
        </h2>
        {loading ? (
          <div className="h-full flex items-center justify-center">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2" style={{ borderColor: '#0080FF' }}></div>
          </div>
        ) : (
          <ResponsiveContainer width="100%" height="100%">
            <ScatterChart margin={{ top: 20, right: 20, bottom: 20, left: 20 }}>
              <XAxis
                type="number"
                dataKey="time"
                name="Time"
                domain={['auto', 'auto']}
                tickFormatter={(time) => format(new Date(time), 'HH:mm')}
                stroke="#6b7280"
                fontSize={12}
              />
              <YAxis
                type="number"
                dataKey="duration"
                name="Duration"
                unit="ms"
                stroke="#6b7280"
                fontSize={12}
              />
              <ZAxis type="number" range={[50, 400]} />
              <Tooltip content={<CustomTooltip />} cursor={{ strokeDasharray: '3 3' }} />
              <Scatter name="Traces" data={chartData}>
                {chartData.map((entry, index) => (
                  <Cell
                    key={`cell-${index}`}
                    fill={entry.status === 'error' ? '#ef4444' : '#10b981'}
                    fillOpacity={0.6}
                  />
                ))}
              </Scatter>
            </ScatterChart>
          </ResponsiveContainer>
        )}
      </div>

      {/* Recent Traces List */}
      <div>
        <h2 className="text-[16px] font-bold mb-4 text-foreground">Recent Activity</h2>
        <TraceList traces={traces} loading={loading} />
      </div>
    </div>
  );
}

// ============================================================================
// System Map Tab
// ============================================================================

function SystemMapTab() {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedNode, setSelectedNode] = useState<Node | null>(null);
  const [zoom, setZoom] = useState(1);

  const [aiAnalysis, setAiAnalysis] = useState<AIAnalysisResult | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [analysisError, setAnalysisError] = useState<string | null>(null);
  const [lastAnalyzed, setLastAnalyzed] = useState<Date | null>(null);
  const [traceCount, setTraceCount] = useState(0);

  const runAIAnalysis = useCallback(async () => {
    setAnalyzing(true);
    setAnalysisError(null);

    try {
      const response = await agentreplayClient.listTraces({ limit: 100 });
      const traces = response.traces || [];
      setTraceCount(traces.length);

      if (traces.length === 0) {
        setAnalysisError('No traces found. Run some AI agents to see the system map.');
        setAnalyzing(false);
        return;
      }

      const nodeMap = new Map<string, { type: string; calls: number; totalDuration: number }>();
      const edgeMap = new Map<string, { count: number; from: string; to: string }>();

      nodeMap.set('agentreplay', { type: 'service', calls: traces.length, totalDuration: 0 });

      traces.forEach((trace: any) => {
        const model = trace.metadata?.['gen_ai.request.model'] || trace.metadata?.model || trace.model;
        const agentName = trace.agent_name || trace.metadata?.agent_name;

        if (model) {
          const modelKey = `llm:${model}`;
          const existing = nodeMap.get(modelKey) || { type: 'llm', calls: 0, totalDuration: 0 };
          existing.calls++;
          existing.totalDuration += trace.duration_us ? trace.duration_us / 1000 : 0;
          nodeMap.set(modelKey, existing);

          const edgeKey = agentName ? `${agentName}:${model}` : `app:${model}`;
          const edge = edgeMap.get(edgeKey) || { count: 0, from: agentName || 'app', to: model };
          edge.count++;
          edgeMap.set(edgeKey, edge);
        }

        if (agentName) {
          const agentKey = `agent:${agentName}`;
          const existing = nodeMap.get(agentKey) || { type: 'agent', calls: 0, totalDuration: 0 };
          existing.calls++;
          existing.totalDuration += trace.duration_us ? trace.duration_us / 1000 : 0;
          nodeMap.set(agentKey, existing);
        }
      });

      const nodes = Array.from(nodeMap.entries()).map(([key, value]) => {
        const [type, name] = key.includes(':') ? key.split(':') : ['service', key];
        return {
          id: key,
          type: type === 'llm' ? 'llm' : type === 'agent' ? 'agent' : 'service',
          label: name || key,
          calls: value.calls,
          avgLatency: value.calls > 0 ? Math.round(value.totalDuration / value.calls) : 0,
        };
      });

      const edges = Array.from(edgeMap.entries()).map(([, edge]) => ({
        from: edge.from,
        to: edge.to,
        count: edge.count,
        label: `${edge.count} calls`,
      }));

      const insights: string[] = [];
      const llmNodes = nodes.filter(n => n.type === 'llm');
      if (llmNodes.length > 1) {
        insights.push(`Using ${llmNodes.length} different LLM models across traces`);
      }
      const slowestNode = nodes.filter(n => n.avgLatency).sort((a, b) => (b.avgLatency || 0) - (a.avgLatency || 0))[0];
      if (slowestNode && slowestNode.avgLatency && slowestNode.avgLatency > 1000) {
        insights.push(`${slowestNode.label} has highest avg latency: ${slowestNode.avgLatency}ms`);
      }
      const busiestModel = llmNodes.sort((a, b) => b.calls - a.calls)[0];
      if (busiestModel) {
        insights.push(`Most used model: ${busiestModel.label} (${busiestModel.calls} calls)`);
      }

      setAiAnalysis({ nodes, edges, summary: `Analyzed ${traces.length} traces.`, insights });
      setLastAnalyzed(new Date());
    } catch (err) {
      console.error('Analysis failed:', err);
      setAnalysisError(err instanceof Error ? err.message : 'Analysis failed');
    } finally {
      setAnalyzing(false);
    }
  }, []);

  useEffect(() => {
    const fetchAgents = async () => {
      try {
        const response = await agentreplayClient.listAgents();
        setAgents(response.agents || []);
      } catch (err) {
        console.error('Failed to fetch agents:', err);
      } finally {
        setLoading(false);
      }
    };

    fetchAgents();
    runAIAnalysis();
  }, [runAIAnalysis]);

  const { nodes, links } = useMemo(() => {
    const nodes: Node[] = [];
    const links: Link[] = [];

    if (aiAnalysis && aiAnalysis.nodes.length > 0) {
      const centerX = 400;
      const centerY = 200;
      const radius = 160;

      const llmNodes = aiAnalysis.nodes.filter(n => n.type === 'llm');
      const agentNodes = aiAnalysis.nodes.filter(n => n.type === 'agent');
      const serviceNodes = aiAnalysis.nodes.filter(n => n.type === 'service');

      serviceNodes.forEach((node) => {
        nodes.push({
          id: node.id,
          type: 'service',
          label: node.label,
          x: centerX,
          y: centerY,
          status: 'active',
          calls: node.calls,
          avgLatency: node.avgLatency,
        });
      });

      llmNodes.forEach((node, i) => {
        const angle = Math.PI + (i / Math.max(llmNodes.length - 1, 1)) * Math.PI;
        const x = centerX + radius * Math.cos(angle);
        const y = centerY + radius * 0.8 * Math.sin(angle);

        nodes.push({
          id: node.id,
          type: 'external',
          label: node.label,
          x: llmNodes.length === 1 ? centerX : x,
          y: llmNodes.length === 1 ? centerY - radius * 0.8 : y,
          status: 'active',
          calls: node.calls,
          avgLatency: node.avgLatency,
        });
      });

      agentNodes.forEach((node, i) => {
        const angle = (i / Math.max(agentNodes.length - 1, 1)) * Math.PI;
        const x = centerX + radius * Math.cos(angle);
        const y = centerY + radius * 0.8 * Math.sin(angle);

        nodes.push({
          id: node.id,
          type: 'agent',
          label: node.label,
          x: agentNodes.length === 1 ? centerX : x,
          y: agentNodes.length === 1 ? centerY + radius * 0.8 : y,
          status: 'active',
          calls: node.calls,
          avgLatency: node.avgLatency,
        });
      });

      aiAnalysis.edges.forEach(edge => {
        const sourceNode = nodes.find(n => n.label === edge.from || n.id.includes(edge.from));
        const targetNode = nodes.find(n => n.label === edge.to || n.id.includes(edge.to));

        if (sourceNode && targetNode) {
          links.push({
            source: sourceNode.id,
            target: targetNode.id,
            value: edge.count,
            label: edge.label,
          });
        }
      });

      return { nodes, links };
    }

    // Fallback
    nodes.push({ id: 'hub', type: 'service', label: 'AgentReplay Core', x: 400, y: 130, status: 'active' });
    nodes.push({ id: 'db', type: 'database', label: 'Trace Store', x: 400, y: 290, status: 'active' });
    links.push({ source: 'hub', target: 'db', value: 1 });

    const graphRadius = 180;
    agents.forEach((agent, index) => {
      const angle = (index / agents.length) * 2 * Math.PI;
      const x = 400 + graphRadius * Math.cos(angle);
      const y = 200 + graphRadius * Math.sin(angle);

      nodes.push({
        id: agent.agent_id,
        type: 'agent',
        label: agent.name,
        x,
        y,
        status: agent.last_seen && (Date.now() - agent.last_seen < 300000) ? 'active' : 'inactive',
        metadata: agent
      });
      links.push({ source: agent.agent_id, target: 'hub', value: 1 });
    });

    return { nodes, links };
  }, [agents, aiAnalysis]);

  const getNodeIcon = (type: string) => {
    switch (type) {
      case 'agent': return Bot;
      case 'database': return Database;
      case 'external': return Globe;
      case 'llm': return Sparkles;
      default: return Server;
    }
  };

  const getNodeColor = (type: string, status: string) => {
    if (status === 'inactive') return '#6b7280';
    if (status === 'error') return '#ef4444';
    switch (type) {
      case 'agent': return '#3b82f6';
      case 'database': return '#10b981';
      case 'external': return '#8b5cf6';
      case 'llm': return '#8b5cf6';
      default: return '#f59e0b';
    }
  };

  const navigate = useNavigate();

  // Determine if we're showing fallback vs real data
  const hasRealData = aiAnalysis && aiAnalysis.nodes.length > 0;

  return (
    <div className="flex flex-col">
      {/* Controls */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          {analyzing ? (
            <span className="flex items-center gap-2 text-[13px] text-muted-foreground">
              <Loader2 className="w-4 h-4 animate-spin" />
              Analyzing...
            </span>
          ) : lastAnalyzed ? (
            <span className="text-[12px] text-muted-foreground">
              Updated {lastAnalyzed.toLocaleTimeString()}
            </span>
          ) : null}
          {aiAnalysis && (
            <span className="text-[13px] text-muted-foreground">
              AI-analyzed topology from {traceCount} traces
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={runAIAnalysis}
            disabled={analyzing}
            className="flex items-center gap-1.5 px-3 py-1.5 text-[13px] rounded-lg transition-all disabled:opacity-50"
            style={{ backgroundColor: 'rgba(0,128,255,0.08)', color: '#0080FF' }}
          >
            <RefreshCcw className={`w-4 h-4 ${analyzing ? 'animate-spin' : ''}`} />
            Refresh
          </button>
          <div className="flex items-center gap-2 pl-4 ml-2 border-l border-border">
            <button onClick={() => setZoom(z => Math.max(0.5, z - 0.1))} className="p-2 rounded transition-all text-muted-foreground">
              <ZoomOut className="w-4 h-4" />
            </button>
            <span className="text-[12px] w-12 text-center text-muted-foreground">{Math.round(zoom * 100)}%</span>
            <button onClick={() => setZoom(z => Math.min(2, z + 0.1))} className="p-2 rounded transition-all text-muted-foreground">
              <ZoomIn className="w-4 h-4" />
            </button>
            <button onClick={() => setZoom(1)} className="p-2 rounded transition-all text-muted-foreground">
              <Maximize className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>

      {/* AI Insights Panel */}
      {hasRealData && aiAnalysis.insights.length > 0 && (
        <div className="px-4 py-3 rounded-xl mb-4" style={{ backgroundColor: 'rgba(0,128,255,0.04)', border: '1px solid hsl(var(--border))' }}>
          <div className="flex items-center gap-3 overflow-x-auto">
            <Sparkles className="w-4 h-4 flex-shrink-0" style={{ color: '#0080FF' }} />
            {aiAnalysis.insights.map((insight, i) => (
              <span key={i} className="text-[13px] whitespace-nowrap text-muted-foreground">
                {insight}
                {i < aiAnalysis.insights.length - 1 && <span className="mx-3 text-border">•</span>}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Guided info banner for fallback state */}
      {!hasRealData && !loading && !analyzing && (
        <div className="rounded-xl mb-4 px-5 py-4" style={{ backgroundColor: 'rgba(0,128,255,0.03)', border: '1px solid hsl(var(--border))' }}>
          <div className="flex items-start gap-3">
            <div className="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 mt-0.5" style={{ background: 'linear-gradient(135deg, rgba(245,158,11,0.1), rgba(251,191,36,0.06))' }}>
              <Activity className="w-4 h-4" style={{ color: '#f59e0b' }} />
            </div>
            <div className="flex-1">
              <p className="text-[13px] font-semibold mb-1 text-foreground">Showing default system topology</p>
              <p className="text-[12px] mb-2.5 leading-relaxed text-muted-foreground">
                This is a preview of your system architecture. Run AI agent traces to populate the map with real topology data including agents, LLM models, and service connections.
              </p>
              <div className="flex items-center gap-2">
                <span className="px-2 py-0.5 rounded-full text-[10px] font-semibold" style={{ backgroundColor: 'rgba(59,130,246,0.06)', color: '#3b82f6', border: '1px solid rgba(59,130,246,0.12)' }}>🔗 AI Topology</span>
                <span className="px-2 py-0.5 rounded-full text-[10px] font-semibold" style={{ backgroundColor: 'rgba(245,158,11,0.06)', color: '#f59e0b', border: '1px solid rgba(245,158,11,0.12)' }}>🌐 Node Connections</span>
                <span className="px-2 py-0.5 rounded-full text-[10px] font-semibold" style={{ backgroundColor: 'rgba(139,92,246,0.06)', color: '#8b5cf6', border: '1px solid rgba(139,92,246,0.12)' }}>✨ Performance Insights</span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Graph */}
      <div className="flex-1 relative overflow-hidden rounded-2xl" style={{ height: '380px', backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))' }}>
        {loading || analyzing ? (
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="text-center">
              <Loader2 className="w-8 h-8 animate-spin mx-auto mb-2" style={{ color: '#0080FF' }} />
              <p className="text-[13px] text-muted-foreground">
                {analyzing ? 'Analyzing trace patterns...' : 'Loading...'}
              </p>
            </div>
          </div>
        ) : (
          <svg
            width="100%"
            height="100%"
            viewBox="0 0 800 400"
            className="select-none"
            style={{ transform: `scale(${zoom})`, transformOrigin: 'center' }}
          >
            <defs>
              <marker id="arrowhead" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">
                <polygon points="0 0, 10 3.5, 0 7" fill="hsl(var(--border))" />
              </marker>
            </defs>

            {links.map((link, i) => {
              const source = nodes.find(n => n.id === link.source);
              const target = nodes.find(n => n.id === link.target);
              if (!source || !target) return null;
              return (
                <g key={i}>
                  <line
                    x1={source.x}
                    y1={source.y}
                    x2={target.x}
                    y2={target.y}
                    stroke="hsl(var(--border))"
                    strokeWidth={Math.min(3, 1 + link.value * 0.5)}
                    markerEnd="url(#arrowhead)"
                  />
                </g>
              );
            })}

            {nodes.map((node) => {
              const Icon = getNodeIcon(node.type);
              const color = getNodeColor(node.type, node.status);
              return (
                <g
                  key={node.id}
                  transform={`translate(${node.x}, ${node.y})`}
                  className="cursor-pointer"
                  onClick={() => setSelectedNode(node)}
                >
                  <circle r={28} fill={color} opacity={0.1} className="transition-all hover:opacity-25" />
                  <circle r={22} fill="hsl(var(--card))" stroke={color} strokeWidth={2.5} />
                  <foreignObject x={-10} y={-10} width={20} height={20}>
                    <div className="flex items-center justify-center w-full h-full">
                      <Icon className="w-4 h-4" style={{ color }} />
                    </div>
                  </foreignObject>
                  <text y={40} textAnchor="middle" fill="hsl(var(--foreground))" fontSize={11} fontWeight={600}>
                    {node.label}
                  </text>
                  {node.calls && (
                    <text y={54} textAnchor="middle" fill="hsl(var(--muted-foreground))" fontSize={9}>
                      {node.calls} calls
                    </text>
                  )}
                </g>
              );
            })}
          </svg>
        )}

        {/* Selected Node Details */}
        {selectedNode && (
          <div className="absolute bottom-4 left-4 rounded-2xl p-4 max-w-xs" style={{ backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', boxShadow: '0 8px 24px rgba(0,0,0,0.08)' }}>
            <div className="flex items-center gap-2 mb-2">
              {(() => {
                const Icon = getNodeIcon(selectedNode.type);
                return <Icon className="w-5 h-5" style={{ color: getNodeColor(selectedNode.type, selectedNode.status) }} />;
              })()}
              <span className="font-semibold text-[14px] text-foreground">{selectedNode.label}</span>
            </div>
            <div className="text-[13px] space-y-1 text-muted-foreground">
              <p>Type: <span className="capitalize">{selectedNode.type}</span></p>
              <p>Status: <span className={selectedNode.status === 'active' ? 'text-emerald-500' : 'text-muted-foreground'}>{selectedNode.status}</span></p>
              {selectedNode.calls && <p>Calls: {selectedNode.calls}</p>}
              {selectedNode.avgLatency && <p>Avg Latency: {selectedNode.avgLatency}ms</p>}
            </div>
            <button
              onClick={() => setSelectedNode(null)}
              className="mt-3 text-[12px] font-medium"
              style={{ color: '#0080FF' }}
            >
              Close
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Metric Card Component
// ============================================================================

interface MetricCardProps {
  title: string;
  value: string;
  icon: React.ReactNode;
  color: 'green' | 'yellow' | 'blue' | 'red';
}

function MetricCard({ title, value, icon, color }: MetricCardProps) {
  const colorMap = {
    green: { bg: 'linear-gradient(135deg, rgba(16,185,129,0.1), rgba(52,211,153,0.05))', text: '#10b981' },
    yellow: { bg: 'linear-gradient(135deg, rgba(234,179,8,0.1), rgba(250,204,21,0.05))', text: '#eab308' },
    blue: { bg: 'linear-gradient(135deg, rgba(59,130,246,0.1), rgba(96,165,250,0.05))', text: '#3b82f6' },
    red: { bg: 'linear-gradient(135deg, rgba(239,68,68,0.1), rgba(248,113,113,0.05))', text: '#ef4444' },
  };
  const c = colorMap[color];

  return (
    <div className="rounded-2xl p-6 bg-card border border-border">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-[13px] font-semibold" style={{ color: 'hsl(var(--muted-foreground))', textTransform: 'uppercase', letterSpacing: '0.05em' }}>{title}</h3>
        <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: c.bg, color: c.text }}>
          {icon}
        </div>
      </div>
      <div className="text-[28px] font-bold text-foreground">{value}</div>
    </div>
  );
}
