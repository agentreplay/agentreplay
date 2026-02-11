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

import React, { useMemo } from 'react';
import {
  BarChart,
  Bar,
  LineChart,
  Line,
  PieChart,
  Pie,
  Cell,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
  ScatterChart,
  Scatter,
} from 'recharts';
import {
  TrendingUp,
  Activity,
  Clock,
  AlertTriangle,
  Zap,
  DollarSign,
  Database,
  Network,
} from 'lucide-react';

interface Trace {
  id: string;
  name: string;
  startTime: string;
  endTime: string;
  duration: number;
  spanCount: number;
  status: string;
  cost?: number;
  inputTokens?: number;
  outputTokens?: number;
}

interface Span {
  id: string;
  name: string;
  spanType: string;
  duration: number;
  status: string;
  cost?: number;
  inputTokens?: number;
  outputTokens?: number;
}

interface TraceStatisticsDashboardProps {
  traces: Trace[];
  spans?: Span[];
}

const COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#ec4899'];

export function TraceStatisticsDashboard({ traces, spans = [] }: TraceStatisticsDashboardProps) {
  // Calculate statistics
  const stats = useMemo(() => {
    const totalTraces = traces.length;
    const errorTraces = traces.filter((t) => t.status === 'error').length;
    const avgDuration =
      traces.reduce((sum, t) => sum + t.duration, 0) / (totalTraces || 1);
    const totalCost = traces.reduce((sum, t) => sum + (t.cost || 0), 0);
    const totalTokens = traces.reduce(
      (sum, t) => sum + (t.inputTokens || 0) + (t.outputTokens || 0),
      0
    );
    const avgSpanCount =
      traces.reduce((sum, t) => sum + t.spanCount, 0) / (totalTraces || 1);

    // Percentiles
    const sortedDurations = traces.map((t) => t.duration).sort((a, b) => a - b);
    const p50 = sortedDurations[Math.floor(sortedDurations.length * 0.5)] || 0;
    const p90 = sortedDurations[Math.floor(sortedDurations.length * 0.9)] || 0;
    const p99 = sortedDurations[Math.floor(sortedDurations.length * 0.99)] || 0;

    // Span type distribution
    const spanTypeCount: Record<string, number> = {};
    spans.forEach((span) => {
      spanTypeCount[span.spanType] = (spanTypeCount[span.spanType] || 0) + 1;
    });

    // Error distribution by span type
    const errorsByType: Record<string, number> = {};
    spans.filter((s) => s.status === 'error').forEach((span) => {
      errorsByType[span.spanType] = (errorsByType[span.spanType] || 0) + 1;
    });

    // Duration distribution (histogram buckets)
    const buckets = [0, 100, 500, 1000, 2000, 5000, 10000, Infinity];
    const durationHistogram = buckets.slice(0, -1).map((min, i) => {
      const max = buckets[i + 1];
      const count = traces.filter((t) => t.duration >= min && t.duration < max).length;
      return {
        range: max === Infinity ? `${min}+` : `${min}-${max}ms`,
        count,
      };
    });

    // Span count distribution
    const spanCountBuckets = [0, 10, 50, 100, 500, 1000, Infinity];
    const spanCountHistogram = spanCountBuckets.slice(0, -1).map((min, i) => {
      const max = spanCountBuckets[i + 1];
      const count = traces.filter(
        (t) => t.spanCount >= min && t.spanCount < max
      ).length;
      return {
        range: max === Infinity ? `${min}+` : `${min}-${max}`,
        count,
      };
    });

    // Most expensive operations
    const spanCostMap: Record<string, { count: number; totalCost: number }> = {};
    spans.filter((s) => s.cost).forEach((span) => {
      if (!spanCostMap[span.name]) {
        spanCostMap[span.name] = { count: 0, totalCost: 0 };
      }
      spanCostMap[span.name].count++;
      spanCostMap[span.name].totalCost += span.cost || 0;
    });
    const expensiveOps = Object.entries(spanCostMap)
      .map(([name, data]) => ({
        name,
        avgCost: data.totalCost / data.count,
        totalCost: data.totalCost,
        count: data.count,
      }))
      .sort((a, b) => b.totalCost - a.totalCost)
      .slice(0, 10);

    // Slowest operations
    const spanDurationMap: Record<string, { count: number; totalDuration: number }> = {};
    spans.forEach((span) => {
      if (!spanDurationMap[span.name]) {
        spanDurationMap[span.name] = { count: 0, totalDuration: 0 };
      }
      spanDurationMap[span.name].count++;
      spanDurationMap[span.name].totalDuration += span.duration;
    });
    const slowestOps = Object.entries(spanDurationMap)
      .map(([name, data]) => ({
        name,
        avgDuration: data.totalDuration / data.count,
        totalDuration: data.totalDuration,
        count: data.count,
      }))
      .sort((a, b) => b.avgDuration - a.avgDuration)
      .slice(0, 10);

    return {
      totalTraces,
      errorTraces,
      errorRate: (errorTraces / totalTraces) * 100 || 0,
      avgDuration,
      totalCost,
      totalTokens,
      avgSpanCount,
      p50,
      p90,
      p99,
      spanTypeCount,
      errorsByType,
      durationHistogram,
      spanCountHistogram,
      expensiveOps,
      slowestOps,
    };
  }, [traces, spans]);

  const spanTypeData = Object.entries(stats.spanTypeCount).map(([type, count]) => ({
    name: type,
    value: count,
  }));

  const errorsByTypeData = Object.entries(stats.errorsByType).map(([type, count]) => ({
    name: type,
    errors: count,
  }));

  return (
    <div className="space-y-6 p-6 bg-secondary">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-foreground">Trace Analytics</h2>
        <div className="text-sm text-muted-foreground">
          Analyzing {stats.totalTraces} traces with {spans.length} spans
        </div>
      </div>

      {/* Key Metrics Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <MetricCard
          title="Total Traces"
          value={stats.totalTraces.toLocaleString()}
          icon={<Activity className="w-6 h-6 text-blue-600" />}
          color="blue"
        />
        <MetricCard
          title="Error Rate"
          value={`${stats.errorRate.toFixed(1)}%`}
          subtitle={`${stats.errorTraces} errors`}
          icon={<AlertTriangle className="w-6 h-6 text-red-600" />}
          color="red"
        />
        <MetricCard
          title="Avg Duration"
          value={`${stats.avgDuration.toFixed(0)}ms`}
          subtitle={`P99: ${stats.p99.toFixed(0)}ms`}
          icon={<Clock className="w-6 h-6 text-green-600" />}
          color="green"
        />
        <MetricCard
          title="Total Cost"
          value={`$${stats.totalCost.toFixed(4)}`}
          subtitle={`${stats.totalTokens.toLocaleString()} tokens`}
          icon={<DollarSign className="w-6 h-6 text-purple-600" />}
          color="purple"
        />
      </div>

      {/* Duration Percentiles */}
      <div className="bg-card rounded-lg shadow p-6">
        <h3 className="text-lg font-semibold text-foreground mb-4 flex items-center gap-2">
          <TrendingUp className="w-5 h-5 text-blue-600" />
          Duration Percentiles
        </h3>
        <div className="grid grid-cols-3 gap-4">
          <div className="bg-blue-50 rounded p-4">
            <div className="text-sm text-blue-600 font-medium">P50 (Median)</div>
            <div className="text-2xl font-bold text-blue-900">{stats.p50.toFixed(0)}ms</div>
          </div>
          <div className="bg-orange-50 rounded p-4">
            <div className="text-sm text-orange-600 font-medium">P90</div>
            <div className="text-2xl font-bold text-orange-900">{stats.p90.toFixed(0)}ms</div>
          </div>
          <div className="bg-red-50 rounded p-4">
            <div className="text-sm text-red-600 font-medium">P99</div>
            <div className="text-2xl font-bold text-red-900">{stats.p99.toFixed(0)}ms</div>
          </div>
        </div>
      </div>

      {/* Charts Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Duration Histogram */}
        <div className="bg-card rounded-lg shadow p-6">
          <h3 className="text-lg font-semibold text-foreground mb-4">Duration Distribution</h3>
          <ResponsiveContainer width="100%" height={250}>
            <BarChart data={stats.durationHistogram}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis dataKey="range" />
              <YAxis />
              <Tooltip />
              <Bar dataKey="count" fill="#3b82f6" />
            </BarChart>
          </ResponsiveContainer>
        </div>

        {/* Span Count Histogram */}
        <div className="bg-card rounded-lg shadow p-6">
          <h3 className="text-lg font-semibold text-foreground mb-4">Span Count Distribution</h3>
          <ResponsiveContainer width="100%" height={250}>
            <BarChart data={stats.spanCountHistogram}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis dataKey="range" />
              <YAxis />
              <Tooltip />
              <Bar dataKey="count" fill="#10b981" />
            </BarChart>
          </ResponsiveContainer>
        </div>

        {/* Span Type Distribution */}
        <div className="bg-card rounded-lg shadow p-6">
          <h3 className="text-lg font-semibold text-foreground mb-4">Span Types</h3>
          <ResponsiveContainer width="100%" height={250}>
            <PieChart>
              <Pie
                data={spanTypeData}
                dataKey="value"
                nameKey="name"
                cx="50%"
                cy="50%"
                outerRadius={80}
                label
              >
                {spanTypeData.map((entry, index) => (
                  <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
                ))}
              </Pie>
              <Tooltip />
              <Legend />
            </PieChart>
          </ResponsiveContainer>
        </div>

        {/* Errors by Type */}
        {errorsByTypeData.length > 0 && (
          <div className="bg-card rounded-lg shadow p-6">
            <h3 className="text-lg font-semibold text-foreground mb-4">Errors by Span Type</h3>
            <ResponsiveContainer width="100%" height={250}>
              <BarChart data={errorsByTypeData}>
                <CartesianGrid strokeDasharray="3 3" />
                <XAxis dataKey="name" />
                <YAxis />
                <Tooltip />
                <Bar dataKey="errors" fill="#ef4444" />
              </BarChart>
            </ResponsiveContainer>
          </div>
        )}
      </div>

      {/* Top Operations Tables */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Slowest Operations */}
        <div className="bg-card rounded-lg shadow p-6">
          <h3 className="text-lg font-semibold text-foreground mb-4 flex items-center gap-2">
            <Clock className="w-5 h-5 text-orange-600" />
            Slowest Operations
          </h3>
          <div className="space-y-2">
            {stats.slowestOps.slice(0, 5).map((op, idx) => (
              <div
                key={idx}
                className="flex justify-between items-center p-3 bg-secondary rounded hover:bg-gray-100"
              >
                <div className="flex-1">
                  <div className="text-sm font-medium text-foreground truncate">{op.name}</div>
                  <div className="text-xs text-muted-foreground">{op.count} calls</div>
                </div>
                <div className="text-right">
                  <div className="text-sm font-bold text-orange-600">
                    {op.avgDuration.toFixed(0)}ms
                  </div>
                  <div className="text-xs text-muted-foreground">avg</div>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Most Expensive Operations */}
        {stats.expensiveOps.length > 0 && (
          <div className="bg-card rounded-lg shadow p-6">
            <h3 className="text-lg font-semibold text-foreground mb-4 flex items-center gap-2">
              <DollarSign className="w-5 h-5 text-green-600" />
              Most Expensive Operations
            </h3>
            <div className="space-y-2">
              {stats.expensiveOps.slice(0, 5).map((op, idx) => (
                <div
                  key={idx}
                  className="flex justify-between items-center p-3 bg-secondary rounded hover:bg-gray-100"
                >
                  <div className="flex-1">
                    <div className="text-sm font-medium text-foreground truncate">{op.name}</div>
                    <div className="text-xs text-muted-foreground">{op.count} calls</div>
                  </div>
                  <div className="text-right">
                    <div className="text-sm font-bold text-green-600">
                      ${op.totalCost.toFixed(4)}
                    </div>
                    <div className="text-xs text-muted-foreground">total</div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

interface MetricCardProps {
  title: string;
  value: string;
  subtitle?: string;
  icon: React.ReactNode;
  color: 'blue' | 'red' | 'green' | 'purple';
}

function MetricCard({ title, value, subtitle, icon, color }: MetricCardProps) {
  const colorClasses = {
    blue: 'bg-blue-50 border-blue-200',
    red: 'bg-red-50 border-red-200',
    green: 'bg-green-50 border-green-200',
    purple: 'bg-purple-50 border-purple-200',
  };

  return (
    <div className={`${colorClasses[color]} border rounded-lg p-4`}>
      <div className="flex items-center justify-between mb-2">
        <div className="text-sm font-medium text-muted-foreground">{title}</div>
        {icon}
      </div>
      <div className="text-2xl font-bold text-foreground">{value}</div>
      {subtitle && <div className="text-xs text-muted-foreground mt-1">{subtitle}</div>}
    </div>
  );
}

export default TraceStatisticsDashboard;
