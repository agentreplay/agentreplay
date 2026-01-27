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
 * MetricGauge - Circular gauge for metric visualization
 * 
 * Features:
 * - Circular progress indicator
 * - Color-coded pass/fail status
 * - Threshold indicator
 * - Accessible with screen readers
 */

import React, { useMemo } from 'react';
import { CheckCircle2, XCircle, AlertTriangle } from 'lucide-react';

// =============================================================================
// TYPES
// =============================================================================

export interface MetricGaugeProps {
  /** Metric identifier */
  name: string;
  /** Display label */
  label?: string;
  /** Current value */
  value: number;
  /** Pass/fail threshold */
  threshold: number;
  /** Value direction ('higher' = higher is better, 'lower' = lower is better) */
  direction?: 'higher' | 'lower';
  /** Unit to display (e.g., '%', 'ms', 'USD') */
  unit?: string;
  /** Show label below gauge */
  showLabel?: boolean;
  /** Size in pixels */
  size?: number;
  /** Stroke width */
  strokeWidth?: number;
  /** Custom threshold override for pass/fail calculation */
  customThreshold?: number;
  /** Additional CSS classes */
  className?: string;
}

export interface MiniGaugeProps {
  value: number;
  threshold: number;
  direction?: 'higher' | 'lower';
  size?: number;
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/**
 * Determine if value passes the threshold
 */
function passes(value: number, threshold: number, direction: 'higher' | 'lower'): boolean {
  if (direction === 'higher') {
    return value >= threshold;
  }
  return value <= threshold;
}

/**
 * Get status based on value and threshold
 */
function getStatus(
  value: number, 
  threshold: number, 
  direction: 'higher' | 'lower'
): 'pass' | 'warn' | 'fail' {
  const passed = passes(value, threshold, direction);
  
  if (passed) {
    return 'pass';
  }
  
  // Check if close to threshold (within 10%)
  const diff = Math.abs(value - threshold);
  const tolerance = Math.abs(threshold) * 0.1;
  
  if (diff <= tolerance) {
    return 'warn';
  }
  
  return 'fail';
}

/**
 * Get color based on status
 */
function getStatusColor(status: 'pass' | 'warn' | 'fail'): string {
  switch (status) {
    case 'pass':
      return '#22c55e'; // green-500
    case 'warn':
      return '#eab308'; // yellow-500
    case 'fail':
      return '#ef4444'; // red-500
  }
}

/**
 * Format value for display
 */
function formatValue(value: number, unit?: string): string {
  // Handle percentages (0-1 range)
  if (value >= 0 && value <= 1 && !unit) {
    return `${(value * 100).toFixed(1)}%`;
  }
  
  // Handle large numbers
  if (Math.abs(value) >= 1000) {
    return `${(value / 1000).toFixed(1)}k${unit || ''}`;
  }
  
  // Standard formatting
  const formatted = value.toFixed(value < 10 ? 2 : 1);
  return unit ? `${formatted}${unit}` : formatted;
}

// =============================================================================
// COMPONENT
// =============================================================================

export const MetricGauge: React.FC<MetricGaugeProps> = ({
  name,
  label,
  value,
  threshold,
  direction = 'higher',
  unit,
  showLabel = true,
  size = 120,
  strokeWidth = 8,
  customThreshold,
  className = '',
}) => {
  // Calculate derived values
  const effectiveThreshold = customThreshold ?? threshold;
  const status = useMemo(
    () => getStatus(value, effectiveThreshold, direction),
    [value, effectiveThreshold, direction]
  );
  const statusColor = getStatusColor(status);
  
  // SVG calculations
  const center = size / 2;
  const radius = (size - strokeWidth) / 2;
  const circumference = 2 * Math.PI * radius;
  
  // Normalize value to 0-1 for arc calculation
  const maxValue = direction === 'higher' ? 1 : effectiveThreshold * 2;
  const normalizedValue = Math.min(Math.max(value, 0), maxValue);
  const progress = normalizedValue / maxValue;
  const strokeDashoffset = circumference * (1 - progress);
  
  // Threshold position on arc
  const thresholdProgress = effectiveThreshold / maxValue;
  const thresholdAngle = thresholdProgress * 360 - 90; // Start from top
  const thresholdRadians = (thresholdAngle * Math.PI) / 180;
  const thresholdX = center + (radius - strokeWidth / 2 - 2) * Math.cos(thresholdRadians);
  const thresholdY = center + (radius - strokeWidth / 2 - 2) * Math.sin(thresholdRadians);
  
  return (
    <div className={`flex flex-col items-center ${className}`}>
      <div className="relative" style={{ width: size, height: size }}>
        {/* SVG Gauge */}
        <svg width={size} height={size} className="transform -rotate-90">
          {/* Background circle */}
          <circle
            cx={center}
            cy={center}
            r={radius}
            fill="none"
            stroke="#e5e7eb"
            strokeWidth={strokeWidth}
          />
          
          {/* Progress arc */}
          <circle
            cx={center}
            cy={center}
            r={radius}
            fill="none"
            stroke={statusColor}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={strokeDashoffset}
            className="transition-all duration-500 ease-out"
          />
          
          {/* Threshold marker */}
          <circle
            cx={thresholdX}
            cy={thresholdY}
            r={3}
            fill="#374151"
            className="transform rotate-90"
            style={{ transformOrigin: `${center}px ${center}px` }}
          />
        </svg>
        
        {/* Center content */}
        <div 
          className="absolute inset-0 flex flex-col items-center justify-center"
          style={{ transform: 'rotate(0deg)' }}
        >
          {/* Value */}
          <span 
            className="text-xl font-bold"
            style={{ color: statusColor }}
          >
            {formatValue(value, unit)}
          </span>
          
          {/* Status icon */}
          <div className="mt-1">
            {status === 'pass' && (
              <CheckCircle2 className="w-5 h-5 text-green-500" />
            )}
            {status === 'warn' && (
              <AlertTriangle className="w-5 h-5 text-yellow-500" />
            )}
            {status === 'fail' && (
              <XCircle className="w-5 h-5 text-red-500" />
            )}
          </div>
        </div>
      </div>
      
      {/* Label */}
      {showLabel && (
        <div className="mt-2 text-center">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
            {label || name}
          </span>
          <span className="block text-xs text-gray-500 dark:text-gray-400">
            Threshold: {formatValue(effectiveThreshold, unit)}
          </span>
        </div>
      )}
    </div>
  );
};

// =============================================================================
// VARIANT: MINI GAUGE
// =============================================================================

export const MiniGauge: React.FC<MiniGaugeProps> = ({
  value,
  threshold,
  direction = 'higher',
  size = 24,
}) => {
  const status = getStatus(value, threshold, direction);
  const statusColor = getStatusColor(status);
  
  return (
    <div 
      className="inline-flex items-center justify-center rounded-full"
      style={{ 
        width: size, 
        height: size, 
        backgroundColor: `${statusColor}20`,
        border: `2px solid ${statusColor}`,
      }}
    >
      {status === 'pass' && <CheckCircle2 className="w-3 h-3" style={{ color: statusColor }} />}
      {status === 'warn' && <AlertTriangle className="w-3 h-3" style={{ color: statusColor }} />}
      {status === 'fail' && <XCircle className="w-3 h-3" style={{ color: statusColor }} />}
    </div>
  );
};

export default MetricGauge;
