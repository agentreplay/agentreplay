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
 * ConfidenceInterval - Visual representation of confidence intervals with uncertainty
 * 
 * Features:
 * - Confidence interval bar visualization
 * - Reliability badges based on CI width
 * - Threshold comparison line
 * - Bootstrap CI support
 */

import React from 'react';
import { Info, AlertTriangle, CheckCircle, AlertCircle } from 'lucide-react';

// =============================================================================
// TYPES
// =============================================================================

export interface ConfidenceIntervalProps {
  /** Point estimate value */
  value: number;
  /** Lower bound of CI */
  lower: number;
  /** Upper bound of CI */
  upper: number;
  /** Confidence level (e.g., 0.95 for 95% CI) */
  confidenceLevel?: number;
  /** Optional threshold line */
  threshold?: number;
  /** Min value for scale (default: 0) */
  min?: number;
  /** Max value for scale (default: 1) */
  max?: number;
  /** Format values as percentages */
  asPercentage?: boolean;
  /** Show numeric values */
  showValues?: boolean;
  /** Size variant */
  size?: 'sm' | 'md' | 'lg';
  /** Additional classes */
  className?: string;
}

export interface ReliabilityBadgeProps {
  /** CI width relative to value */
  ciWidth: number;
  /** Threshold for high reliability */
  highThreshold?: number;
  /** Threshold for medium reliability */
  mediumThreshold?: number;
}

export interface ConfidenceBarProps {
  value: number;
  lower: number;
  upper: number;
  threshold?: number;
  min?: number;
  max?: number;
  color?: string;
  height?: number;
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

function getReliability(ciWidth: number, value: number): 'high' | 'medium' | 'low' {
  const relativeWidth = ciWidth / Math.max(Math.abs(value), 0.001);
  if (relativeWidth < 0.1) return 'high';
  if (relativeWidth < 0.25) return 'medium';
  return 'low';
}

function formatValue(value: number, asPercentage: boolean): string {
  if (asPercentage) {
    return `${(value * 100).toFixed(1)}%`;
  }
  return value.toFixed(3);
}

// =============================================================================
// RELIABILITY BADGE
// =============================================================================

export const ReliabilityBadge: React.FC<ReliabilityBadgeProps> = ({
  ciWidth,
  highThreshold = 0.1,
  mediumThreshold = 0.25,
}) => {
  let reliability: 'high' | 'medium' | 'low';
  
  if (ciWidth < highThreshold) {
    reliability = 'high';
  } else if (ciWidth < mediumThreshold) {
    reliability = 'medium';
  } else {
    reliability = 'low';
  }
  
  const config = {
    high: {
      icon: CheckCircle,
      text: 'High Confidence',
      className: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400',
    },
    medium: {
      icon: AlertTriangle,
      text: 'Medium Confidence',
      className: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400',
    },
    low: {
      icon: AlertCircle,
      text: 'Low Confidence - Increase Sample Size',
      className: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400',
    },
  };
  
  const { icon: Icon, text, className } = config[reliability];
  
  return (
    <span className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium ${className}`}>
      <Icon className="w-3.5 h-3.5" />
      {text}
    </span>
  );
};

// =============================================================================
// CONFIDENCE BAR
// =============================================================================

export const ConfidenceBar: React.FC<ConfidenceBarProps> = ({
  value,
  lower,
  upper,
  threshold,
  min = 0,
  max = 1,
  color = '#3b82f6',
  height = 8,
}) => {
  const range = max - min;
  
  // Calculate positions as percentages
  const valuePos = ((value - min) / range) * 100;
  const lowerPos = ((lower - min) / range) * 100;
  const upperPos = ((upper - min) / range) * 100;
  const thresholdPos = threshold !== undefined ? ((threshold - min) / range) * 100 : null;
  
  return (
    <div className="relative w-full" style={{ height: height + 16 }}>
      {/* Background track */}
      <div 
        className="absolute top-1/2 left-0 right-0 -translate-y-1/2 rounded-full bg-gray-200 dark:bg-gray-700"
        style={{ height }}
      />
      
      {/* CI range */}
      <div 
        className="absolute top-1/2 -translate-y-1/2 rounded-full"
        style={{ 
          left: `${lowerPos}%`, 
          width: `${upperPos - lowerPos}%`,
          height,
          backgroundColor: `${color}33`,
        }}
      />
      
      {/* Lower bound marker */}
      <div 
        className="absolute top-1/2 -translate-y-1/2 w-0.5 bg-current"
        style={{ 
          left: `${lowerPos}%`,
          height: height + 8,
          color,
        }}
      />
      
      {/* Upper bound marker */}
      <div 
        className="absolute top-1/2 -translate-y-1/2 w-0.5 bg-current"
        style={{ 
          left: `${upperPos}%`,
          height: height + 8,
          color,
        }}
      />
      
      {/* Point estimate */}
      <div 
        className="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 rounded-full border-2 border-white dark:border-gray-900"
        style={{ 
          left: `${valuePos}%`,
          width: height + 4,
          height: height + 4,
          backgroundColor: color,
        }}
      />
      
      {/* Threshold line */}
      {thresholdPos !== null && (
        <div 
          className="absolute top-1/2 -translate-y-1/2 w-0.5 bg-gray-500 dark:bg-gray-400"
          style={{ 
            left: `${thresholdPos}%`,
            height: height + 12,
          }}
        />
      )}
    </div>
  );
};

// =============================================================================
// MAIN COMPONENT
// =============================================================================

export const ConfidenceInterval: React.FC<ConfidenceIntervalProps> = ({
  value,
  lower,
  upper,
  confidenceLevel = 0.95,
  threshold,
  min = 0,
  max = 1,
  asPercentage = true,
  showValues = true,
  size = 'md',
  className = '',
}) => {
  const ciWidth = upper - lower;
  const reliability = getReliability(ciWidth, value);
  
  const sizeClasses = {
    sm: 'text-xs gap-1',
    md: 'text-sm gap-2',
    lg: 'text-base gap-3',
  };
  
  const confPercent = Math.round(confidenceLevel * 100);
  
  return (
    <div className={`flex flex-col ${sizeClasses[size]} ${className}`}>
      {/* Visual bar */}
      <ConfidenceBar
        value={value}
        lower={lower}
        upper={upper}
        threshold={threshold}
        min={min}
        max={max}
        height={size === 'sm' ? 6 : size === 'lg' ? 10 : 8}
      />
      
      {/* Numeric display */}
      {showValues && (
        <div className="flex items-center justify-between text-gray-600 dark:text-gray-400">
          <span className="font-mono">
            {confPercent}% CI: [{formatValue(lower, asPercentage)}, {formatValue(upper, asPercentage)}]
          </span>
          <ReliabilityBadge ciWidth={ciWidth / (max - min)} />
        </div>
      )}
    </div>
  );
};

// =============================================================================
// DIFFERENCE CI (for A/B tests)
// =============================================================================

export interface DifferenceCIProps {
  difference: number;
  lower: number;
  upper: number;
  confidenceLevel?: number;
  asPercentage?: boolean;
  className?: string;
}

export const DifferenceCI: React.FC<DifferenceCIProps> = ({
  difference,
  lower,
  upper,
  confidenceLevel = 0.95,
  asPercentage = true,
  className = '',
}) => {
  // Check if CI crosses zero
  const crossesZero = lower <= 0 && upper >= 0;
  const isPositive = difference > 0;
  
  // Calculate scale to center on zero
  const maxAbs = Math.max(Math.abs(lower), Math.abs(upper)) * 1.2;
  const min = -maxAbs;
  const max = maxAbs;
  
  const color = crossesZero 
    ? '#6b7280' // gray
    : isPositive 
      ? '#22c55e' // green
      : '#ef4444'; // red
  
  const confPercent = Math.round(confidenceLevel * 100);
  
  return (
    <div className={`flex flex-col gap-2 ${className}`}>
      {/* Zero reference label */}
      <div className="relative h-4">
        <span 
          className="absolute text-xs text-gray-500 dark:text-gray-400 -translate-x-1/2"
          style={{ left: '50%' }}
        >
          No difference
        </span>
      </div>
      
      {/* Visual bar with zero centered */}
      <ConfidenceBar
        value={difference}
        lower={lower}
        upper={upper}
        threshold={0}
        min={min}
        max={max}
        color={color}
        height={10}
      />
      
      {/* Interpretation */}
      <div className="flex items-center justify-between">
        <span className="font-mono text-sm text-gray-600 dark:text-gray-400">
          {confPercent}% CI: [{formatValue(lower, asPercentage)}, {formatValue(upper, asPercentage)}]
        </span>
        {crossesZero ? (
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300">
            <AlertTriangle className="w-3 h-3" />
            Not Significant
          </span>
        ) : (
          <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${
            isPositive 
              ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400'
              : 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400'
          }`}>
            <CheckCircle className="w-3 h-3" />
            {isPositive ? 'Significant Improvement' : 'Significant Decline'}
          </span>
        )}
      </div>
    </div>
  );
};

export default ConfidenceInterval;
