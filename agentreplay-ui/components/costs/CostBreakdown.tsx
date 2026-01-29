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

'use client';

import { useMemo } from 'react';
import { 
  DollarSign, 
  TrendingUp, 
  TrendingDown, 
  PieChart,
  BarChart3,
  Zap,
  Cpu,
  MessageSquare,
  Code
} from 'lucide-react';
import { 
  PieChart as RechartsPie, 
  Pie, 
  Cell, 
  ResponsiveContainer, 
  Tooltip,
  Legend,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid
} from 'recharts';

interface ModelCost {
  model: string;
  cost: number;
  tokens: number;
  calls: number;
  color: string;
}

interface OperationCost {
  operation: string;
  cost: number;
  percentage: number;
  icon: React.ReactNode;
}

interface CostBreakdownProps {
  modelCosts?: ModelCost[];
  operationCosts?: OperationCost[];
  totalCost?: number;
  previousPeriodCost?: number;
  timeRange?: string;
}

// Default model colors based on provider
const MODEL_COLORS: Record<string, string> = {
  'gpt-4': '#10B981',       // Emerald
  'gpt-4-turbo': '#059669', // Darker emerald
  'gpt-4o': '#34D399',      // Light emerald
  'gpt-3.5-turbo': '#6366F1', // Indigo
  'claude-3-opus': '#F59E0B', // Amber
  'claude-3-sonnet': '#FBBF24', // Yellow
  'claude-3-haiku': '#FCD34D', // Light yellow
  'gemini-pro': '#3B82F6',   // Blue
  'gemini-ultra': '#2563EB', // Darker blue
  'mistral-large': '#8B5CF6', // Purple
  'mistral-medium': '#A78BFA', // Light purple
  'other': '#6B7280',        // Gray
};

export function CostByModelChart({ 
  modelCosts,
  totalCost,
  previousPeriodCost = 0,
}: CostBreakdownProps) {
  // Use real data only - no mock fallback
  const displayCosts = modelCosts || [];
  const displayTotal = totalCost || displayCosts.reduce((sum, m) => sum + m.cost, 0);
  const percentChange = previousPeriodCost > 0 
    ? ((displayTotal - previousPeriodCost) / previousPeriodCost) * 100 
    : 0;
  const isIncreasing = percentChange > 0;

  const chartData = useMemo(() => 
    displayCosts.map(m => ({
      name: m.model,
      value: m.cost,
      fill: m.color || MODEL_COLORS[m.model] || MODEL_COLORS['other'],
    })),
    [displayCosts]
  );

  // Show empty state if no data
  if (displayCosts.length === 0) {
    return (
      <div className="bg-surface rounded-lg border border-border p-6">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <PieChart className="w-5 h-5 text-primary" />
            <h3 className="text-lg font-semibold text-textPrimary">Cost by Model</h3>
          </div>
        </div>
        <div className="flex items-center justify-center h-48 text-textTertiary">
          No cost data available for the selected time range
        </div>
      </div>
    );
  }

  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <PieChart className="w-5 h-5 text-primary" />
          <h3 className="text-lg font-semibold text-textPrimary">Cost by Model</h3>
        </div>
        <div className={`flex items-center gap-1 text-sm ${isIncreasing ? 'text-warning' : 'text-success'}`}>
          {isIncreasing ? <TrendingUp className="w-4 h-4" /> : <TrendingDown className="w-4 h-4" />}
          {Math.abs(percentChange).toFixed(1)}%
        </div>
      </div>

      <div className="flex items-center gap-6">
        {/* Pie Chart */}
        <div className="w-48 h-48 flex-shrink-0">
          <ResponsiveContainer width="100%" height="100%">
            <RechartsPie>
              <Pie
                data={chartData}
                cx="50%"
                cy="50%"
                innerRadius={45}
                outerRadius={70}
                paddingAngle={2}
                dataKey="value"
              >
                {chartData.map((entry, index) => (
                  <Cell key={`cell-${index}`} fill={entry.fill} />
                ))}
              </Pie>
              <Tooltip 
                formatter={(value: number) => [`$${value.toFixed(2)}`, 'Cost']}
                contentStyle={{
                  backgroundColor: 'var(--surface)',
                  border: '1px solid var(--border)',
                  borderRadius: '8px',
                  color: 'var(--textPrimary)',
                }}
              />
            </RechartsPie>
          </ResponsiveContainer>
        </div>

        {/* Legend with details */}
        <div className="flex-1 space-y-3">
          {displayCosts.map((model, index) => {
            const percentage = displayTotal > 0 ? (model.cost / displayTotal) * 100 : 0;
            return (
              <div key={model.model} className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <div 
                    className="w-3 h-3 rounded-full"
                    style={{ backgroundColor: model.color || MODEL_COLORS[model.model] || MODEL_COLORS['other'] }}
                  />
                  <span className="text-sm text-textSecondary truncate max-w-[120px]">
                    {model.model}
                  </span>
                </div>
                <div className="text-right">
                  <div className="text-sm font-medium text-textPrimary">
                    ${model.cost.toFixed(2)}
                  </div>
                  <div className="text-xs text-textTertiary">
                    {percentage.toFixed(1)}%
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Token efficiency */}
      <div className="mt-4 pt-4 border-t border-border grid grid-cols-3 gap-4 text-center">
        <div>
          <div className="text-xs text-textTertiary">Total Tokens</div>
          <div className="text-sm font-semibold text-textPrimary">
            {(displayCosts.reduce((sum, m) => sum + m.tokens, 0) / 1_000_000).toFixed(1)}M
          </div>
        </div>
        <div>
          <div className="text-xs text-textTertiary">Avg Cost/1K</div>
          <div className="text-sm font-semibold text-textPrimary">
            ${displayCosts.reduce((sum, m) => sum + m.tokens, 0) > 0 
              ? (displayTotal / (displayCosts.reduce((sum, m) => sum + m.tokens, 0) / 1000)).toFixed(4)
              : '0.0000'}
          </div>
        </div>
        <div>
          <div className="text-xs text-textTertiary">Total Calls</div>
          <div className="text-sm font-semibold text-textPrimary">
            {displayCosts.reduce((sum, m) => sum + m.calls, 0).toLocaleString()}
          </div>
        </div>
      </div>
    </div>
  );
}

export function CostByOperationChart({
  operationCosts,
}: CostBreakdownProps) {
  // Map icon names to components
  const getIcon = (iconName: string | React.ReactNode) => {
    if (typeof iconName !== 'string') return iconName;
    switch (iconName) {
      case 'MessageSquare': return <MessageSquare className="w-4 h-4" />;
      case 'Cpu': return <Cpu className="w-4 h-4" />;
      case 'Code': return <Code className="w-4 h-4" />;
      default: return <MessageSquare className="w-4 h-4" />;
    }
  };
  
  // Use real data only - no mock fallback
  const displayCosts = operationCosts || [];
  
  // Show empty state if no data
  if (displayCosts.length === 0) {
    return (
      <div className="bg-surface rounded-lg border border-border p-6">
        <div className="flex items-center gap-2 mb-4">
          <BarChart3 className="w-5 h-5 text-primary" />
          <h3 className="text-lg font-semibold text-textPrimary">Cost by Operation</h3>
        </div>
        <div className="flex items-center justify-center h-32 text-textTertiary">
          No operation data available
        </div>
      </div>
    );
  }
  
  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <div className="flex items-center gap-2 mb-4">
        <BarChart3 className="w-5 h-5 text-primary" />
        <h3 className="text-lg font-semibold text-textPrimary">Cost by Operation</h3>
      </div>

      <div className="space-y-4">
        {displayCosts.map((op, index) => (
          <div key={op.operation}>
            <div className="flex items-center justify-between mb-1">
              <div className="flex items-center gap-2 text-sm">
                <span className="text-textTertiary">{getIcon(op.icon)}</span>
                <span className="text-textSecondary">{op.operation}</span>
              </div>
              <div className="text-sm font-medium text-textPrimary">
                ${op.cost.toFixed(2)}
              </div>
            </div>
            <div className="flex items-center gap-2">
              <div className="flex-1 h-2 bg-surface-elevated rounded-full overflow-hidden">
                <div 
                  className="h-full bg-primary rounded-full transition-all duration-500"
                  style={{ width: `${op.percentage}%` }}
                />
              </div>
              <span className="text-xs text-textTertiary w-12 text-right">
                {op.percentage.toFixed(1)}%
              </span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export function TokenEfficiencyCard({
  avgCostPer1k = 0,
  cacheHitRate = 0,
  potentialSavings = 0,
}: {
  avgCostPer1k?: number;
  cacheHitRate?: number;
  potentialSavings?: number;
}) {
  // Calculate optimization score based on real metrics
  const getOptimizationScore = () => {
    if (avgCostPer1k === 0 && cacheHitRate === 0) return { score: 0, label: 'No Data', color: 'text-textTertiary' };
    
    let score = 50; // Start at middle
    // Better cache hit rate = better score
    score += cacheHitRate * 0.3;
    // Lower cost per 1k = better score (assume $0.01 is good, $0.1 is bad)
    if (avgCostPer1k > 0) {
      score += Math.max(0, 30 - (avgCostPer1k * 300));
    }
    
    if (score >= 75) return { score, label: 'Good', color: 'text-success' };
    if (score >= 50) return { score, label: 'Fair', color: 'text-warning' };
    return { score, label: 'Needs Work', color: 'text-error' };
  };

  const optimization = getOptimizationScore();

  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <div className="flex items-center gap-2 mb-4">
        <Zap className="w-5 h-5 text-warning" />
        <h3 className="text-lg font-semibold text-textPrimary">Token Efficiency</h3>
      </div>

      <div className="grid grid-cols-3 gap-4">
        <div className="p-3 bg-surface-elevated rounded-lg text-center">
          <div className="text-xs text-textTertiary mb-1">Cost per 1K tokens</div>
          <div className="text-xl font-bold text-textPrimary">
            ${avgCostPer1k.toFixed(4)}
          </div>
        </div>
        <div className="p-3 bg-surface-elevated rounded-lg text-center">
          <div className="text-xs text-textTertiary mb-1">Cache Hit Rate</div>
          <div className={`text-xl font-bold ${cacheHitRate > 30 ? 'text-success' : 'text-textPrimary'}`}>
            {cacheHitRate.toFixed(1)}%
          </div>
        </div>
        <div className="p-3 bg-surface-elevated rounded-lg text-center">
          <div className="text-xs text-textTertiary mb-1">Potential Savings</div>
          <div className="text-xl font-bold text-warning">
            ${potentialSavings.toFixed(2)}
          </div>
        </div>
      </div>

      <div className="mt-4 pt-4 border-t border-border">
        <div className="flex items-center justify-between text-sm">
          <span className="text-textSecondary">Optimization score</span>
          <div className="flex items-center gap-2">
            <div className="w-24 h-2 bg-surface-elevated rounded-full overflow-hidden">
              <div 
                className={`h-full rounded-full ${optimization.color === 'text-success' ? 'bg-success' : optimization.color === 'text-warning' ? 'bg-warning' : 'bg-error'}`}
                style={{ width: `${Math.min(100, Math.max(0, optimization.score))}%` }}
              />
            </div>
            <span className={`font-medium ${optimization.color}`}>{optimization.label}</span>
          </div>
        </div>
      </div>
    </div>
  );
}

export function DailyCostTrend({
  data,
}: {
  data?: Array<{ date: string; cost: number }>;
}) {
  // Use real data only - no mock fallback
  const chartData = data || [];

  // Show empty state if no data
  if (chartData.length === 0) {
    return (
      <div className="bg-surface rounded-lg border border-border p-6">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <DollarSign className="w-5 h-5 text-primary" />
            <h3 className="text-lg font-semibold text-textPrimary">Daily Cost Trend</h3>
          </div>
        </div>
        <div className="flex items-center justify-center h-48 text-textTertiary">
          No cost trend data available
        </div>
      </div>
    );
  }

  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <DollarSign className="w-5 h-5 text-primary" />
          <h3 className="text-lg font-semibold text-textPrimary">Daily Cost Trend</h3>
        </div>
      </div>

      <div className="h-48">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={chartData}>
            <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
            <XAxis 
              dataKey="date" 
              tick={{ fill: 'var(--textTertiary)', fontSize: 10 }}
              tickLine={false}
              axisLine={{ stroke: 'var(--border)' }}
            />
            <YAxis 
              tick={{ fill: 'var(--textTertiary)', fontSize: 10 }}
              tickLine={false}
              axisLine={{ stroke: 'var(--border)' }}
              tickFormatter={(value) => `$${value}`}
            />
            <Tooltip
              contentStyle={{
                backgroundColor: 'var(--surface)',
                border: '1px solid var(--border)',
                borderRadius: '8px',
                color: 'var(--textPrimary)',
              }}
              formatter={(value: number) => [`$${value.toFixed(2)}`, 'Cost']}
              labelFormatter={(label) => `Day ${label}`}
            />
            <Bar 
              dataKey="cost" 
              fill="var(--primary)"
              radius={[2, 2, 0, 0]}
            />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}

// Export all cost components
export { CostAlertsPanel, CostForecast, OptimizationSuggestions } from './CostEnhancements';
