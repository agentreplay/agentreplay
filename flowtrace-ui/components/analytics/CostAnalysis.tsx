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
import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip, Legend } from 'recharts';
import useSWR from 'swr';
import { Loader2, AlertCircle, DollarSign, TrendingDown } from 'lucide-react';

interface CostAnalysisProps {
  sessionId?: string;
}

interface ModelCost {
  cost_usd: number;
  call_count: number;
  input_tokens: number;
  output_tokens: number;
}

interface TokenUsageSummary {
  total_input_tokens: number;
  total_output_tokens: number;
  total_cached_tokens: number;
}

interface CostBreakdownData {
  total_cost_usd: number;
  by_model: Record<string, ModelCost>;
  token_usage: TokenUsageSummary;
}

const MODEL_COLORS = [
  '#8b5cf6', '#3b82f6', '#10b981', '#f59e0b', '#ef4444',
  '#ec4899', '#6366f1', '#14b8a6', '#f97316', '#84cc16'
];

export function CostAnalysis({ sessionId }: { sessionId?: string }) {
  const { data, error, isLoading } = useSWR<CostBreakdownData>(
    `/api/v1/analytics/cost/breakdown?${sessionId ? `session_id=${sessionId}&` : ''}group_by=model`,
    async (url) => {
      const res = await fetch(url);
      if (!res.ok) throw new Error('Failed to fetch cost breakdown');
      const json = await res.json();

      // Transform new API response to match component expectation
      // New API returns { breakdown: [{ group_key: "gpt-4", cost: 0.1, ... }], total_cost: 0.5 }
      // Component expects { total_cost_usd: 0.5, by_model: { "gpt-4": { cost_usd: 0.1 ... } }, token_usage: ... }

      const by_model: Record<string, ModelCost> = {};
      let total_input = 0;
      let total_output = 0;

      json.breakdown.forEach((item: any) => {
        by_model[item.group_key] = {
          cost_usd: item.cost,
          call_count: item.request_count,
          input_tokens: 0, // New API might not return this split per group yet, or we need to adjust API
          output_tokens: 0
        };
        total_input += item.token_count; // Approximation if split not available
      });

      return {
        total_cost_usd: json.total_cost,
        by_model,
        token_usage: {
          total_input_tokens: total_input,
          total_output_tokens: total_output,
          total_cached_tokens: 0 // API needs to provide this
        }
      };
    },
    { refreshInterval: 30000 }
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
                <DollarSign className="h-5 w-5 text-muted-foreground" />
              </div>
              <div>
                <h3 className="text-base font-semibold text-textPrimary mb-1">No Cost Data Available</h3>
                <p className="text-sm text-textSecondary">
                  No span data found for this session. This could mean:
                </p>
                <ul className="text-sm text-textSecondary list-disc list-inside mt-2 space-y-1">
                  <li>The session is very recent and spans haven't been ingested yet</li>
                  <li>Token usage wasn't captured during trace collection</li>
                  <li>The edge storage may not be configured correctly</li>
                </ul>
              </div>
            </>
          ) : (
            <>
              <AlertCircle className="h-5 w-5 text-destructive flex-shrink-0" />
              <div>
                <p className="font-semibold text-destructive">Failed to load cost analysis</p>
                <p className="text-sm text-muted-foreground mt-1">{error.message}</p>
              </div>
            </>
          )}
        </div>
      </Card>
    );
  }

  if (!data) return null;

  // Check if data is empty (no models, no tokens)
  const hasData = Object.keys(data.by_model).length > 0 ||
    data.token_usage.total_input_tokens > 0 ||
    data.token_usage.total_output_tokens > 0;

  if (!hasData) {
    return (
      <Card className="p-6">
        <div className="flex items-start gap-3">
          <div className="p-2 rounded-lg bg-muted">
            <DollarSign className="h-5 w-5 text-muted-foreground" />
          </div>
          <div>
            <h3 className="text-base font-semibold text-textPrimary mb-1">No Cost Data Available</h3>
            <p className="text-sm text-textSecondary">
              No token usage or model costs found for this session.
            </p>
            <ul className="text-sm text-textSecondary list-disc list-inside mt-2 space-y-1">
              <li>No LLM calls were recorded</li>
              <li>Token counts weren't captured in trace metadata</li>
              <li>Span data may not have been ingested yet</li>
            </ul>
          </div>
        </div>
      </Card>
    );
  }

  // Prepare pie chart data
  const pieData = Object.entries(data.by_model).map(([model, cost], index) => ({
    name: model,
    value: cost.cost_usd,
    percentage: Math.round((cost.cost_usd / data.total_cost_usd) * 100),
    calls: cost.call_count,
    color: MODEL_COLORS[index % MODEL_COLORS.length],
  })).sort((a, b) => b.value - a.value);

  // Calculate savings from caching
  const cacheTokens = data.token_usage.total_cached_tokens;
  const estimatedSavings = cacheTokens > 0
    ? (cacheTokens / 1_000_000) * 2.7 * 0.9  // ~90% discount for cached tokens
    : 0;

  return (
    <div className="space-y-4">
      <Card className="p-6">
        <div className="flex items-center gap-2 mb-4">
          <DollarSign className="h-5 w-5 text-green-600" />
          <h3 className="text-lg font-semibold">Cost Analysis</h3>
        </div>

        {/* Summary Stats */}
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
          <div className="p-4 rounded-lg bg-gradient-to-br from-green-50 to-green-100 dark:from-green-950 dark:to-green-900">
            <p className="text-sm text-muted-foreground">Total Cost</p>
            <p className="text-2xl font-bold text-green-600">
              ${data.total_cost_usd.toFixed(4)}
            </p>
          </div>
          <div className="p-4 rounded-lg bg-muted">
            <p className="text-sm text-muted-foreground">Models Used</p>
            <p className="text-2xl font-bold">{Object.keys(data.by_model).length}</p>
          </div>
          <div className="p-4 rounded-lg bg-muted">
            <p className="text-sm text-muted-foreground">Total Calls</p>
            <p className="text-2xl font-bold">
              {Object.values(data.by_model).reduce((sum, m) => sum + m.call_count, 0)}
            </p>
          </div>
          <div className="p-4 rounded-lg bg-gradient-to-br from-blue-50 to-blue-100 dark:from-blue-950 dark:to-blue-900">
            <p className="text-sm text-muted-foreground">Cache Savings</p>
            <p className="text-2xl font-bold text-blue-600">
              ${estimatedSavings.toFixed(4)}
            </p>
          </div>
        </div>

        {/* Cost by Model - Pie Chart */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div>
            <h4 className="font-medium mb-3">Cost Distribution by Model</h4>
            <div className="h-64">
              <ResponsiveContainer width="100%" height="100%">
                <PieChart>
                  <Pie
                    data={pieData}
                    cx="50%"
                    cy="50%"
                    labelLine={false}
                    label={({ name, percentage }) => `${name}: ${percentage}%`}
                    outerRadius={80}
                    fill="#8884d8"
                    dataKey="value"
                  >
                    {pieData.map((entry, index) => (
                      <Cell key={`cell-${index}`} fill={entry.color} />
                    ))}
                  </Pie>
                  <Tooltip
                    content={({ active, payload }) => {
                      if (active && payload && payload.length) {
                        const data = payload[0].payload;
                        return (
                          <div className="bg-popover border rounded-lg p-3 shadow-lg">
                            <p className="font-semibold">{data.name}</p>
                            <p className="text-sm">Cost: ${data.value.toFixed(4)}</p>
                            <p className="text-sm">Calls: {data.calls}</p>
                            <p className="text-sm">Share: {data.percentage}%</p>
                          </div>
                        );
                      }
                      return null;
                    }}
                  />
                </PieChart>
              </ResponsiveContainer>
            </div>
          </div>

          {/* Model Details Table */}
          <div>
            <h4 className="font-medium mb-3">Model Details</h4>
            <div className="overflow-y-auto max-h-64">
              <table className="w-full text-sm">
                <thead className="bg-muted sticky top-0">
                  <tr>
                    <th className="px-3 py-2 text-left">Model</th>
                    <th className="px-3 py-2 text-right">Cost</th>
                    <th className="px-3 py-2 text-right">Calls</th>
                  </tr>
                </thead>
                <tbody className="divide-y">
                  {pieData.map((row, index) => (
                    <tr key={row.name} className="hover:bg-muted/50">
                      <td className="px-3 py-2">
                        <div className="flex items-center gap-2">
                          <div
                            className="w-3 h-3 rounded-full"
                            style={{ backgroundColor: row.color }}
                          />
                          <span className="text-xs truncate max-w-[120px]">{row.name}</span>
                        </div>
                      </td>
                      <td className="px-3 py-2 text-right font-mono">${row.value.toFixed(4)}</td>
                      <td className="px-3 py-2 text-right">{row.calls}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </Card>

      {/* Token Usage */}
      <Card className="p-6">
        <h4 className="font-semibold mb-3">Token Usage</h4>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="p-4 rounded-lg bg-muted">
            <p className="text-sm text-muted-foreground">Input Tokens</p>
            <p className="text-xl font-bold font-mono">
              {data.token_usage.total_input_tokens.toLocaleString()}
            </p>
          </div>
          <div className="p-4 rounded-lg bg-muted">
            <p className="text-sm text-muted-foreground">Output Tokens</p>
            <p className="text-xl font-bold font-mono">
              {data.token_usage.total_output_tokens.toLocaleString()}
            </p>
          </div>
          <div className="p-4 rounded-lg bg-gradient-to-br from-purple-50 to-purple-100 dark:from-purple-950 dark:to-purple-900">
            <p className="text-sm text-muted-foreground">Cached Tokens</p>
            <p className="text-xl font-bold font-mono text-purple-600">
              {data.token_usage.total_cached_tokens.toLocaleString()}
            </p>
          </div>
        </div>
      </Card>

      {/* Cost Optimization Tips */}
      {data.total_cost_usd > 0.01 && (
        <Card className="p-6">
          <div className="flex items-center gap-2 mb-3">
            <TrendingDown className="h-5 w-5 text-blue-600" />
            <h4 className="font-semibold">Cost Optimization Tips</h4>
          </div>
          <ul className="space-y-2">
            {cacheTokens === 0 && (
              <li className="flex items-start gap-2">
                <span className="text-blue-600 mt-0.5">•</span>
                <span className="text-sm text-muted-foreground">
                  Enable prompt caching to save up to 90% on repeated context (Anthropic).
                </span>
              </li>
            )}
            {data.total_cost_usd > 0.05 && (
              <li className="flex items-start gap-2">
                <span className="text-blue-600 mt-0.5">•</span>
                <span className="text-sm text-muted-foreground">
                  Consider using smaller models for simpler tasks (e.g., gpt-4o-mini vs gpt-4o).
                </span>
              </li>
            )}
            <li className="flex items-start gap-2">
              <span className="text-blue-600 mt-0.5">•</span>
              <span className="text-sm text-muted-foreground">
                Implement response caching for common queries to reduce API calls.
              </span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-blue-600 mt-0.5">•</span>
              <span className="text-sm text-muted-foreground">
                Use streaming for better UX without increasing costs.
              </span>
            </li>
          </ul>
        </Card>
      )}
    </div>
  );
}
