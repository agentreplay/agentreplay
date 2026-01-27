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

/**
 * MetricsGrid - Grid layout for multiple metrics
 * 
 * Features:
 * - Automatic grouping by category
 * - Responsive column layout
 * - Category headers with icons
 */

import React from 'react';
import { MetricGauge, MetricGaugeProps } from './MetricGauge';
import { 
  BookOpen, 
  Bot, 
  Shield, 
  Star, 
  Zap, 
  DollarSign,
  Settings 
} from 'lucide-react';

// =============================================================================
// TYPES
// =============================================================================

export type MetricCategory = 
  | 'rag' 
  | 'agent' 
  | 'safety' 
  | 'quality' 
  | 'performance' 
  | 'cost' 
  | 'custom';

export interface MetricData extends Omit<MetricGaugeProps, 'size' | 'showLabel'> {
  category: MetricCategory;
}

export interface MetricsGridProps {
  /** Array of metrics to display */
  metrics: MetricData[];
  /** Whether to group metrics by category */
  groupByCategory?: boolean;
  /** Number of columns (2-6) */
  columns?: 2 | 3 | 4 | 5 | 6;
  /** Size of each gauge */
  gaugeSize?: number;
  /** Additional CSS classes */
  className?: string;
}

// =============================================================================
// CATEGORY CONFIG
// =============================================================================

const categoryConfig: Record<MetricCategory, { 
  label: string; 
  icon: React.FC<{ className?: string }>; 
  order: number;
}> = {
  rag: { label: 'RAG Quality', icon: BookOpen, order: 1 },
  agent: { label: 'Agent Behavior', icon: Bot, order: 2 },
  safety: { label: 'Safety', icon: Shield, order: 3 },
  quality: { label: 'Response Quality', icon: Star, order: 4 },
  performance: { label: 'Performance', icon: Zap, order: 5 },
  cost: { label: 'Cost', icon: DollarSign, order: 6 },
  custom: { label: 'Custom Metrics', icon: Settings, order: 7 },
};

// =============================================================================
// COMPONENT
// =============================================================================

export const MetricsGrid: React.FC<MetricsGridProps> = ({
  metrics,
  groupByCategory = true,
  columns = 4,
  gaugeSize = 100,
  className = '',
}) => {
  // Group metrics by category
  const groupedMetrics = React.useMemo(() => {
    if (!groupByCategory) {
      return { all: metrics };
    }
    
    const groups: Record<string, MetricData[]> = {};
    
    for (const metric of metrics) {
      const category = metric.category || 'custom';
      if (!groups[category]) {
        groups[category] = [];
      }
      groups[category].push(metric);
    }
    
    return groups;
  }, [metrics, groupByCategory]);
  
  // Sort categories by order
  const sortedCategories = Object.keys(groupedMetrics).sort((a, b) => {
    const orderA = categoryConfig[a as MetricCategory]?.order ?? 99;
    const orderB = categoryConfig[b as MetricCategory]?.order ?? 99;
    return orderA - orderB;
  });
  
  // Column class
  const columnClass = {
    2: 'grid-cols-2',
    3: 'grid-cols-3',
    4: 'grid-cols-4',
    5: 'grid-cols-5',
    6: 'grid-cols-6',
  }[columns];
  
  return (
    <div className={`space-y-6 ${className}`}>
      {sortedCategories.map((category) => {
        const config = categoryConfig[category as MetricCategory];
        const Icon = config?.icon || Settings;
        
        return (
          <div key={category}>
            {/* Category header */}
            {groupByCategory && (
              <div className="flex items-center gap-2 mb-4 pb-2 border-b border-gray-200 dark:border-gray-700">
                <Icon className="w-5 h-5 text-gray-600 dark:text-gray-400" />
                <h3 className="text-lg font-semibold text-gray-800 dark:text-gray-200">
                  {config?.label || category}
                </h3>
                <span className="text-sm text-gray-500 dark:text-gray-400">
                  ({groupedMetrics[category].length} metrics)
                </span>
              </div>
            )}
            
            {/* Metrics grid */}
            <div className={`grid ${columnClass} gap-4`}>
              {groupedMetrics[category].map((metric) => (
                <MetricGauge
                  key={metric.name}
                  {...metric}
                  size={gaugeSize}
                  showLabel={true}
                />
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
};

export default MetricsGrid;
