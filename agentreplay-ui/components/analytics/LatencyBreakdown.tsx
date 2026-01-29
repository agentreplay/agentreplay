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

import { Card } from '../ui/card';
import { BarChart, Bar, XAxis, YAxis, Tooltip, Legend, ResponsiveContainer, Cell } from 'recharts';
import useSWR from 'swr';
import { Loader2, AlertCircle, TrendingUp, Clock } from 'lucide-react';

interface LatencyBreakdownProps {
  sessionId: string;
}

interface LatencyStats {
  total_ms: number;
  count: number;
  avg_ms: number;
  min_ms: number;
  max_ms: number;
}

interface LatencyBreakdownData {
  total_ms: number;
  breakdown: Record<string, LatencyStats>;
  recommendations: string[];
}

const SPAN_TYPE_COLORS: Record<string, string> = {
  'Reasoning': '#8b5cf6',  // Purple for LLM calls
  'ToolCall': '#3b82f6',   // Blue for tools
  'Planning': '#10b981',   // Green for planning
  'Synthesis': '#f59e0b',  // Orange for synthesis
  'Root': '#6b7280',       // Gray for root
};

export function LatencyBreakdown({ sessionId }: LatencyBreakdownProps) {
  const { data, error, isLoading } = useSWR<LatencyBreakdownData>(
    `/api/v1/analytics/latency-breakdown?session_id=${sessionId}`,
    async (url) => {
      const res = await fetch(url);
      if (!res.ok) throw new Error('Failed to fetch latency breakdown');
      return res.json();
    },
    { refreshInterval: 5000 }
  );

  if (isLoading) {
    return (
      <Card className="p-6">
        <div className="flex items-center justify-center h-64">
          <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
        </div>
      </Card>
    );
  }

  if (error) {
    const isNoData = error.message?.includes('No spans found') || 
                     error.message?.includes('404');
    
    return (
      <Card className="p-6">
        <div className="flex items-start gap-3">
          {isNoData ? (
            <>
              <div className="p-2 rounded-lg bg-muted">
                <Clock className="h-5 w-5 text-muted-foreground" />
              </div>
              <div>
                <h3 className="text-base font-semibold text-textPrimary mb-1">No Latency Data Available</h3>
                <p className="text-sm text-textSecondary">
                  No span data found for this session. This could mean:
                </p>
                <ul className="text-sm text-textSecondary list-disc list-inside mt-2 space-y-1">
                  <li>The session is very recent and spans haven't been ingested yet</li>
                  <li>Span data wasn't captured during trace collection</li>
                  <li>The edge storage may not be configured correctly</li>
                </ul>
              </div>
            </>
          ) : (
            <>
              <AlertCircle className="h-5 w-5 text-destructive flex-shrink-0" />
              <div>
                <p className="font-semibold text-destructive">Failed to load latency breakdown</p>
                <p className="text-sm text-muted-foreground mt-1">{error.message}</p>
              </div>
            </>
          )}
        </div>
      </Card>
    );
  }

  if (!data) return null;

  // Prepare chart data
  const chartData = Object.entries(data.breakdown).map(([type, stats]) => ({
    name: type,
    'Avg Latency (ms)': Math.round(stats.avg_ms),
    'Total (ms)': Math.round(stats.total_ms),
    count: stats.count,
    percentage: Math.round((stats.total_ms / data.total_ms) * 100),
  })).sort((a, b) => b['Total (ms)'] - a['Total (ms)']);

  return (
    <div className="space-y-4">
      <Card className="p-6">
        <div className="flex items-center gap-2 mb-4">
          <Clock className="h-5 w-5 text-primary" />
          <h3 className="text-lg font-semibold">Latency Breakdown</h3>
        </div>

        {/* Summary Stats */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
          <div className="p-4 rounded-lg bg-muted">
            <p className="text-sm text-muted-foreground">Total Duration</p>
            <p className="text-2xl font-bold">{Math.round(data.total_ms)}ms</p>
          </div>
          <div className="p-4 rounded-lg bg-muted">
            <p className="text-sm text-muted-foreground">Components</p>
            <p className="text-2xl font-bold">{Object.keys(data.breakdown).length}</p>
          </div>
          <div className="p-4 rounded-lg bg-muted">
            <p className="text-sm text-muted-foreground">Slowest Type</p>
            <p className="text-2xl font-bold">
              {chartData[0]?.name || 'N/A'}
            </p>
          </div>
        </div>

        {/* Bar Chart */}
        <div className="h-64">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={chartData}>
              <XAxis 
                dataKey="name" 
                tick={{ fontSize: 12 }}
                angle={-45}
                textAnchor="end"
                height={80}
              />
              <YAxis 
                label={{ value: 'Time (ms)', angle: -90, position: 'insideLeft' }}
                tick={{ fontSize: 12 }}
              />
              <Tooltip 
                content={({ active, payload }) => {
                  if (active && payload && payload.length) {
                    const data = payload[0].payload;
                    return (
                      <div className="bg-popover border rounded-lg p-3 shadow-lg">
                        <p className="font-semibold">{data.name}</p>
                        <p className="text-sm">Avg: {data['Avg Latency (ms)']}ms</p>
                        <p className="text-sm">Total: {data['Total (ms)']}ms</p>
                        <p className="text-sm">Calls: {data.count}</p>
                        <p className="text-sm">% of Total: {data.percentage}%</p>
                      </div>
                    );
                  }
                  return null;
                }}
              />
              <Bar dataKey="Total (ms)" radius={[8, 8, 0, 0]}>
                {chartData.map((entry, index) => (
                  <Cell 
                    key={`cell-${index}`} 
                    fill={SPAN_TYPE_COLORS[entry.name] || '#6b7280'} 
                  />
                ))}
              </Bar>
            </BarChart>
          </ResponsiveContainer>
        </div>

        {/* Detailed Breakdown Table */}
        <div className="mt-6">
          <h4 className="font-medium mb-3">Detailed Breakdown</h4>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead className="bg-muted">
                <tr>
                  <th className="px-4 py-2 text-left">Component</th>
                  <th className="px-4 py-2 text-right">Calls</th>
                  <th className="px-4 py-2 text-right">Avg (ms)</th>
                  <th className="px-4 py-2 text-right">Total (ms)</th>
                  <th className="px-4 py-2 text-right">% of Total</th>
                </tr>
              </thead>
              <tbody className="divide-y">
                {chartData.map((row) => (
                  <tr key={row.name} className="hover:bg-muted/50">
                    <td className="px-4 py-2">
                      <div className="flex items-center gap-2">
                        <div 
                          className="w-3 h-3 rounded-full" 
                          style={{ backgroundColor: SPAN_TYPE_COLORS[row.name] || '#6b7280' }}
                        />
                        {row.name}
                      </div>
                    </td>
                    <td className="px-4 py-2 text-right">{row.count}</td>
                    <td className="px-4 py-2 text-right font-mono">{row['Avg Latency (ms)']}</td>
                    <td className="px-4 py-2 text-right font-mono">{row['Total (ms)']}</td>
                    <td className="px-4 py-2 text-right">{row.percentage}%</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </Card>

      {/* Recommendations */}
      {data.recommendations && data.recommendations.length > 0 && (
        <Card className="p-6">
          <div className="flex items-center gap-2 mb-3">
            <TrendingUp className="h-5 w-5 text-green-600" />
            <h4 className="font-semibold">Optimization Recommendations</h4>
          </div>
          <ul className="space-y-2">
            {data.recommendations.map((rec, i) => (
              <li key={i} className="flex items-start gap-2">
                <span className="text-green-600 mt-0.5">•</span>
                <span className="text-sm text-muted-foreground">{rec}</span>
              </li>
            ))}
          </ul>
        </Card>
      )}
    </div>
  );
}
