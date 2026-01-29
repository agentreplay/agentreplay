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
 * EnhancedMetricCard - Advanced metric card with confidence intervals and statistical details
 * 
 * Features:
 * - Confidence interval visualization
 * - Reliability badges
 * - Distribution sparkline
 * - Trend indicators with statistical significance
 * - Expandable statistical details
 */

import React, { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { 
  LucideIcon, 
  TrendingUp, 
  TrendingDown, 
  Minus,
  Info,
  ChevronDown,
  ChevronUp,
  AlertTriangle,
  CheckCircle,
  BarChart3
} from 'lucide-react';
import { ConfidenceBar, ReliabilityBadge } from './ConfidenceInterval';

// =============================================================================
// TYPES
// =============================================================================

export interface EnhancedMetricCardProps {
  /** Metric title */
  title: string;
  /** Main value */
  value: number;
  /** Unit (e.g., '%', 'ms', 'USD') */
  unit?: string;
  /** Format as percentage (value 0-1 displayed as 0-100%) */
  asPercentage?: boolean;
  /** 95% Confidence interval [lower, upper] */
  confidenceInterval?: [number, number];
  /** Trend direction */
  trend?: 'up' | 'down' | 'stable';
  /** Change from previous period */
  change?: number;
  /** Is the change statistically significant? */
  isSignificant?: boolean;
  /** Sample size */
  sampleSize?: number;
  /** Pass/fail threshold */
  threshold?: number;
  /** Sparkline data */
  sparklineData?: number[];
  /** Distribution data (histogram) */
  distributionData?: number[];
  /** Icon */
  icon?: LucideIcon;
  /** Description / tooltip */
  description?: string;
  /** Statistical details */
  statisticalDetails?: {
    mean: number;
    median: number;
    stdDev: number;
    min: number;
    max: number;
    p25?: number;
    p75?: number;
    p90?: number;
    p95?: number;
    p99?: number;
  };
  /** Click handler */
  onClick?: () => void;
  /** Additional classes */
  className?: string;
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

function formatValue(value: number, asPercentage: boolean, unit?: string): string {
  if (asPercentage) {
    return `${(value * 100).toFixed(1)}%`;
  }
  if (unit === '$') {
    return `$${value.toFixed(2)}`;
  }
  if (value >= 1000) {
    return `${(value / 1000).toFixed(1)}k${unit || ''}`;
  }
  return `${value.toFixed(value < 10 ? 3 : 1)}${unit || ''}`;
}

function getReliabilityLevel(ci: [number, number] | undefined, value: number): 'high' | 'medium' | 'low' {
  if (!ci) return 'medium';
  const width = ci[1] - ci[0];
  const relativeWidth = width / Math.max(Math.abs(value), 0.001);
  if (relativeWidth < 0.1) return 'high';
  if (relativeWidth < 0.25) return 'medium';
  return 'low';
}

// =============================================================================
// MINI SPARKLINE
// =============================================================================

interface MiniSparklineProps {
  data: number[];
  color?: string;
  height?: number;
}

const MiniSparkline: React.FC<MiniSparklineProps> = ({
  data,
  color = '#3b82f6',
  height = 32,
}) => {
  if (data.length < 2) return null;
  
  const width = 120;
  const padding = 2;
  
  const min = Math.min(...data);
  const max = Math.max(...data);
  const range = max - min || 1;
  
  const points = data.map((v, i) => {
    const x = padding + (i / (data.length - 1)) * (width - padding * 2);
    const y = height - padding - ((v - min) / range) * (height - padding * 2);
    return `${x},${y}`;
  });
  
  const pathData = `M ${points.join(' L ')}`;
  const areaData = `${pathData} L ${width - padding},${height} L ${padding},${height} Z`;
  
  return (
    <svg width={width} height={height} className="overflow-visible">
      <path d={areaData} fill={`${color}22`} />
      <path d={pathData} fill="none" stroke={color} strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
};

// =============================================================================
// MINI HISTOGRAM
// =============================================================================

interface MiniHistogramProps {
  data: number[];
  height?: number;
  bins?: number;
}

const MiniHistogram: React.FC<MiniHistogramProps> = ({
  data,
  height = 40,
  bins = 10,
}) => {
  if (data.length < 2) return null;
  
  const width = 120;
  const padding = 2;
  
  // Calculate histogram bins
  const min = Math.min(...data);
  const max = Math.max(...data);
  const binWidth = (max - min) / bins || 1;
  
  const binCounts = new Array(bins).fill(0);
  data.forEach(v => {
    const binIndex = Math.min(Math.floor((v - min) / binWidth), bins - 1);
    binCounts[binIndex]++;
  });
  
  const maxCount = Math.max(...binCounts);
  const barWidth = (width - padding * 2) / bins - 1;
  
  return (
    <svg width={width} height={height} className="overflow-visible">
      {binCounts.map((count, i) => {
        const barHeight = (count / maxCount) * (height - padding * 2);
        const x = padding + i * (barWidth + 1);
        const y = height - padding - barHeight;
        
        return (
          <rect
            key={i}
            x={x}
            y={y}
            width={barWidth}
            height={barHeight}
            fill="#3b82f6"
            opacity={0.7}
            rx={1}
          />
        );
      })}
    </svg>
  );
};

// =============================================================================
// STATISTICAL DETAILS PANEL
// =============================================================================

interface StatDetailsProps {
  details: NonNullable<EnhancedMetricCardProps['statisticalDetails']>;
  asPercentage: boolean;
  unit?: string;
}

const StatDetails: React.FC<StatDetailsProps> = ({ details, asPercentage, unit }) => {
  const format = (v: number) => formatValue(v, asPercentage, unit);
  
  return (
    <motion.div
      initial={{ height: 0, opacity: 0 }}
      animate={{ height: 'auto', opacity: 1 }}
      exit={{ height: 0, opacity: 0 }}
      transition={{ duration: 0.2 }}
      className="overflow-hidden"
    >
      <div className="pt-4 mt-4 border-t border-border space-y-3">
        {/* Summary stats */}
        <div className="grid grid-cols-3 gap-3 text-sm">
          <div>
            <div className="text-textTertiary text-xs">Mean</div>
            <div className="font-mono font-semibold text-textPrimary">{format(details.mean)}</div>
          </div>
          <div>
            <div className="text-textTertiary text-xs">Median</div>
            <div className="font-mono font-semibold text-textPrimary">{format(details.median)}</div>
          </div>
          <div>
            <div className="text-textTertiary text-xs">Std Dev</div>
            <div className="font-mono font-semibold text-textPrimary">{format(details.stdDev)}</div>
          </div>
        </div>
        
        {/* Range */}
        <div className="grid grid-cols-2 gap-3 text-sm">
          <div>
            <div className="text-textTertiary text-xs">Min</div>
            <div className="font-mono text-textSecondary">{format(details.min)}</div>
          </div>
          <div>
            <div className="text-textTertiary text-xs">Max</div>
            <div className="font-mono text-textSecondary">{format(details.max)}</div>
          </div>
        </div>
        
        {/* Percentiles */}
        {(details.p25 !== undefined || details.p75 !== undefined) && (
          <div>
            <div className="text-textTertiary text-xs mb-1">Percentiles</div>
            <div className="flex gap-2 text-xs">
              {details.p25 !== undefined && (
                <span className="px-2 py-0.5 bg-surface-hover rounded font-mono">
                  P25: {format(details.p25)}
                </span>
              )}
              {details.p75 !== undefined && (
                <span className="px-2 py-0.5 bg-surface-hover rounded font-mono">
                  P75: {format(details.p75)}
                </span>
              )}
              {details.p90 !== undefined && (
                <span className="px-2 py-0.5 bg-surface-hover rounded font-mono">
                  P90: {format(details.p90)}
                </span>
              )}
              {details.p95 !== undefined && (
                <span className="px-2 py-0.5 bg-surface-hover rounded font-mono">
                  P95: {format(details.p95)}
                </span>
              )}
            </div>
          </div>
        )}
      </div>
    </motion.div>
  );
};

// =============================================================================
// MAIN COMPONENT
// =============================================================================

export const EnhancedMetricCard: React.FC<EnhancedMetricCardProps> = ({
  title,
  value,
  unit,
  asPercentage = false,
  confidenceInterval,
  trend = 'stable',
  change,
  isSignificant,
  sampleSize,
  threshold,
  sparklineData,
  distributionData,
  icon: Icon,
  description,
  statisticalDetails,
  onClick,
  className = '',
}) => {
  const [expanded, setExpanded] = useState(false);
  
  const reliability = getReliabilityLevel(confidenceInterval, value);
  const formattedValue = formatValue(value, asPercentage, unit);
  
  const passesThreshold = threshold !== undefined ? value >= threshold : null;
  
  const trendColor = trend === 'up' 
    ? 'text-green-600 dark:text-green-400' 
    : trend === 'down' 
      ? 'text-red-600 dark:text-red-400' 
      : 'text-gray-500';
  
  const TrendIcon = trend === 'up' ? TrendingUp : trend === 'down' ? TrendingDown : Minus;
  
  return (
    <motion.div
      whileHover={{ y: -2 }}
      transition={{ duration: 0.2 }}
      onClick={onClick}
      className={`
        relative overflow-hidden rounded-xl p-5 border shadow-sm
        bg-surface-elevated border-border
        ${onClick ? 'cursor-pointer' : ''}
        ${className}
      `}
    >
      {/* Header */}
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-2">
          {Icon && (
            <div className="p-2 rounded-lg bg-primary/10 text-primary">
              <Icon size={18} />
            </div>
          )}
          <div>
            <h3 className="text-sm font-medium text-textSecondary">{title}</h3>
            {sampleSize !== undefined && (
              <span className="text-xs text-textTertiary">n = {sampleSize.toLocaleString()}</span>
            )}
          </div>
        </div>
        
        {description && (
          <div className="group relative">
            <Info className="w-4 h-4 text-textTertiary cursor-help" />
            <div className="absolute right-0 top-6 z-20 hidden group-hover:block w-48 p-2 bg-gray-900 text-white text-xs rounded shadow-lg">
              {description}
            </div>
          </div>
        )}
      </div>
      
      {/* Main value */}
      <div className="flex items-baseline gap-2 mb-2">
        <span className="text-3xl font-bold text-textPrimary">
          {formattedValue}
        </span>
        
        {passesThreshold !== null && (
          passesThreshold ? (
            <CheckCircle className="w-5 h-5 text-green-500" />
          ) : (
            <AlertTriangle className="w-5 h-5 text-yellow-500" />
          )
        )}
      </div>
      
      {/* Confidence interval */}
      {confidenceInterval && (
        <div className="mb-3">
          <div className="text-xs text-textTertiary mb-1">
            95% CI: [{formatValue(confidenceInterval[0], asPercentage, unit)}, {formatValue(confidenceInterval[1], asPercentage, unit)}]
          </div>
          <ConfidenceBar
            value={value}
            lower={confidenceInterval[0]}
            upper={confidenceInterval[1]}
            threshold={threshold}
            min={asPercentage ? 0 : Math.min(confidenceInterval[0] * 0.9, 0)}
            max={asPercentage ? 1 : confidenceInterval[1] * 1.1}
            height={6}
          />
        </div>
      )}
      
      {/* Reliability badge */}
      {confidenceInterval && (
        <div className="mb-3">
          <ReliabilityBadge ciWidth={(confidenceInterval[1] - confidenceInterval[0])} />
        </div>
      )}
      
      {/* Change indicator */}
      {change !== undefined && Number.isFinite(change) && (
        <div className="flex items-center gap-2 mb-3">
          <div className={`flex items-center gap-1 px-2 py-1 rounded-full text-xs font-semibold ${
            trend === 'up' 
              ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400'
              : trend === 'down'
                ? 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-400'
                : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400'
          }`}>
            <TrendIcon size={12} />
            {change >= 0 ? '+' : ''}{change.toFixed(1)}%
          </div>
          
          {isSignificant !== undefined && (
            <span className={`text-xs ${isSignificant ? 'text-green-600' : 'text-textTertiary'}`}>
              {isSignificant ? '✓ Significant' : 'Not significant'}
            </span>
          )}
        </div>
      )}
      
      {/* Sparkline or histogram */}
      {(sparklineData || distributionData) && (
        <div className="mb-3">
          {distributionData ? (
            <MiniHistogram data={distributionData} />
          ) : sparklineData ? (
            <MiniSparkline 
              data={sparklineData} 
              color={trend === 'up' ? '#22c55e' : trend === 'down' ? '#ef4444' : '#3b82f6'} 
            />
          ) : null}
        </div>
      )}
      
      {/* Expand button for details */}
      {statisticalDetails && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            setExpanded(!expanded);
          }}
          className="flex items-center gap-1 text-xs text-primary hover:text-primary/80 transition-colors"
        >
          <BarChart3 size={12} />
          {expanded ? 'Hide' : 'View'} Statistical Analysis
          {expanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
        </button>
      )}
      
      {/* Expandable details */}
      <AnimatePresence>
        {expanded && statisticalDetails && (
          <StatDetails 
            details={statisticalDetails} 
            asPercentage={asPercentage} 
            unit={unit} 
          />
        )}
      </AnimatePresence>
    </motion.div>
  );
};

export default EnhancedMetricCard;
