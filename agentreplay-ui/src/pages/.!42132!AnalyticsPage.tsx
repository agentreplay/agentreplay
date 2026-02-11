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
              <h1 className="text-[22px] font-bold" style={{ color: '#111827' }}>Analytics</h1>
              <p className="text-[13px]" style={{ color: '#6b7280' }}>
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
                  backgroundColor: isGlobalMode ? 'rgba(16,185,129,0.08)' : '#ffffff',
                  border: isGlobalMode ? '1px solid rgba(16,185,129,0.2)' : '1px solid #e5e7eb',
                  color: isGlobalMode ? '#10b981' : '#6b7280',
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
                className="px-4 py-2 rounded-lg text-[13px]"
                style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb', color: '#111827' }}
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
        <div className="flex gap-1 mb-6 rounded-xl p-1" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
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
                color: activeTab === tab.key ? '#ffffff' : '#6b7280',
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
            <div key={i} className="rounded-2xl p-6 animate-pulse" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
              <div className="flex items-center justify-between mb-4">
                <div className="h-10 w-10 rounded-lg" style={{ backgroundColor: '#f1f5f9' }} />
                <div className="h-4 w-12 rounded" style={{ backgroundColor: '#f1f5f9' }} />
              </div>
              <div className="h-4 w-20 rounded mb-2" style={{ backgroundColor: '#f1f5f9' }} />
              <div className="h-8 w-24 rounded" style={{ backgroundColor: '#f1f5f9' }} />
            </div>
          ))}
        </div>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {[1, 2].map((i) => (
            <div key={i} className="rounded-2xl p-6 animate-pulse" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
              <div className="h-6 w-32 rounded mb-4" style={{ backgroundColor: '#f1f5f9' }} />
              <div className="h-48 rounded" style={{ backgroundColor: '#f1f5f9' }} />
            </div>
          ))}
        </div>
      </>
    );
  }

  if (timeSeriesData.length === 0) {
    return (
      <div className="rounded-2xl overflow-hidden flex flex-col items-center" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb', minHeight: '380px' }}>
        <div className="py-14 px-10 text-center flex-1 flex flex-col items-center justify-center">
          <div className="w-16 h-16 rounded-2xl flex items-center justify-center mx-auto mb-4" style={{ background: 'linear-gradient(135deg, rgba(0,128,255,0.1), rgba(0,200,255,0.06))' }}>
            <BarChart3 className="w-8 h-8" style={{ color: '#0080FF' }} />
          </div>
          <p className="text-[18px] font-bold mb-2" style={{ color: '#111827' }}>No analytics data yet</p>
          <p className="text-[14px] mb-5 max-w-lg mx-auto leading-relaxed" style={{ color: '#6b7280' }}>
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
            <div className="flex-1 rounded-xl p-3 text-left" style={{ backgroundColor: '#f9fafb', border: '1px solid #f1f5f9' }}>
              <div className="flex items-center gap-1.5 mb-1">
                <Clock className="w-3.5 h-3.5" style={{ color: '#8b5cf6' }} />
                <span className="text-[12px] font-bold" style={{ color: '#111827' }}>Timeline</span>
              </div>
              <p className="text-[11px] leading-relaxed" style={{ color: '#9ca3af' }}>Scatter chart of execution durations with recent activity log</p>
            </div>
            <div className="flex-1 rounded-xl p-3 text-left" style={{ backgroundColor: '#f9fafb', border: '1px solid #f1f5f9' }}>
              <div className="flex items-center gap-1.5 mb-1">
                <Activity className="w-3.5 h-3.5" style={{ color: '#f59e0b' }} />
                <span className="text-[12px] font-bold" style={{ color: '#111827' }}>System Map</span>
              </div>
              <p className="text-[11px] leading-relaxed" style={{ color: '#9ca3af' }}>Interactive topology of agents, LLM models, and service connections</p>
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
        <div className="rounded-2xl p-6" style={{ backgroundColor: '#ffffff', border: '1px solid #e5e7eb' }}>
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-[15px] font-bold" style={{ color: '#111827' }}>Cost Over Time</h3>
            <button
              onClick={() => navigate('/traces?sort=cost&order=desc')}
              className="text-[12px] font-medium transition-colors"
              style={{ color: '#0080FF' }}
            >
