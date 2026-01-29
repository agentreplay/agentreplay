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

import { useEffect, useState } from 'react';
import { Clock, Zap, DollarSign } from 'lucide-react';
import { agentreplayClient, ProjectMetricsResponse } from '../lib/agentreplay-api';

interface MetricsCardsProps {
  projectId: number;
}

export default function MetricsCards({ projectId }: MetricsCardsProps) {
  const [metrics, setMetrics] = useState<ProjectMetricsResponse | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!projectId) return;
    
    const fetchMetrics = async () => {
      try {
        setLoading(true);
        const data = await agentreplayClient.getProjectMetrics(projectId);
        setMetrics(data);
      } catch (error) {
        console.error('Failed to fetch metrics:', error);
      } finally {
        setLoading(false);
      }
    };
    
    fetchMetrics();
  }, [projectId]);

  if (loading) {
    return <div className="animate-pulse h-24 bg-surface rounded-lg mb-6"></div>;
  }

  if (!metrics) return null;

  return (
    <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
      {/* Latency Card */}
      <div className="bg-surface border border-border rounded-lg p-4">
        <div className="flex items-center gap-2 text-textSecondary mb-2">
          <Clock className="w-4 h-4" />
          <span className="text-sm font-medium">Latency</span>
        </div>
        <div className="flex flex-col gap-1">
          <div className="flex justify-between items-baseline">
            <span className="text-2xl font-bold text-textPrimary">
              {metrics.latency_ms.p95.toFixed(0)}ms
            </span>
            <span className="text-xs font-medium text-textSecondary">P95</span>
          </div>
          <div className="flex gap-3 text-xs text-textTertiary">
            <span>P50: {metrics.latency_ms.p50.toFixed(0)}ms</span>
            <span>P90: {metrics.latency_ms.p90.toFixed(0)}ms</span>
          </div>
        </div>
      </div>

      {/* Tokens Card */}
      <div className="bg-surface border border-border rounded-lg p-4">
        <div className="flex items-center gap-2 text-textSecondary mb-2">
          <Zap className="w-4 h-4" />
          <span className="text-sm font-medium">Tokens</span>
        </div>
        <div className="flex flex-col gap-1">
          <div className="flex justify-between items-baseline">
            <span className="text-2xl font-bold text-textPrimary">
              {metrics.tokens.p90.toLocaleString()}
            </span>
            <span className="text-xs font-medium text-textSecondary">P90</span>
          </div>
          <div className="flex gap-3 text-xs text-textTertiary">
            <span>P50: {metrics.tokens.p50.toLocaleString()}</span>
            <span>Avg Cost: ${metrics.cost_usd.avg.toFixed(4)}</span>
          </div>
        </div>
      </div>

      {/* Cost Card */}
      <div className="bg-surface border border-border rounded-lg p-4">
        <div className="flex items-center gap-2 text-textSecondary mb-2">
          <DollarSign className="w-4 h-4" />
          <span className="text-sm font-medium">Total Cost (24h)</span>
        </div>
        <div className="flex items-baseline gap-2">
          <span className="text-2xl font-bold text-textPrimary">
            ${metrics.cost_usd.total.toFixed(4)}
          </span>
        </div>
      </div>
    </div>
  );
}
