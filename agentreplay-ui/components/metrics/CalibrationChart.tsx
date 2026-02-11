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
 * CalibrationChart - Reliability diagram for model calibration
 * 
 * Features:
 * - Reliability diagram with perfect calibration line
 * - Bin-by-bin breakdown with gap indicators
 * - ECE/MCE/Brier score display
 * - Interactive bin tooltips
 */

import React, { useMemo } from 'react';
import { Info, AlertTriangle, CheckCircle } from 'lucide-react';

// =============================================================================
// TYPES
// =============================================================================

export interface CalibrationBin {
  /** Bin index (0-9 for 10 bins) */
  binIndex: number;
  /** Average predicted probability in this bin */
  predicted: number;
  /** Actual observed frequency in this bin */
  observed: number;
  /** Number of samples in this bin */
  count: number;
  /** Calibration gap |predicted - observed| */
  gap: number;
}

export interface CalibrationMetrics {
  /** Brier Score (lower is better, 0 = perfect) */
  brierScore: number;
  /** Expected Calibration Error */
  ece: number;
  /** Maximum Calibration Error */
  mce: number;
}

export interface CalibrationChartProps {
  /** Calibration bins data */
  bins: CalibrationBin[];
  /** Calibration metrics */
  metrics: CalibrationMetrics;
  /** Chart dimensions */
  width?: number;
  height?: number;
  /** Show bin details table */
  showTable?: boolean;
  /** Additional classes */
  className?: string;
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

function getCalibrationStatus(ece: number): 'well-calibrated' | 'moderate' | 'poor' {
  if (ece < 0.05) return 'well-calibrated';
  if (ece < 0.10) return 'moderate';
  return 'poor';
}

function getBinStatus(gap: number): 'calibrated' | 'overconfident' | 'underconfident' {
  if (Math.abs(gap) < 0.05) return 'calibrated';
  if (gap > 0) return 'underconfident'; // predicted < observed
  return 'overconfident'; // predicted > observed
}

// =============================================================================
// CALIBRATION METRIC CARDS
// =============================================================================

interface MetricCardProps {
  title: string;
  value: number;
  description: string;
  status: 'good' | 'moderate' | 'poor';
  lowerIsBetter?: boolean;
}

const CalibrationMetricCard: React.FC<MetricCardProps> = ({
  title,
  value,
  description,
  status,
  lowerIsBetter = true,
}) => {
  const statusColors = {
    good: 'text-green-600 dark:text-green-400',
    moderate: 'text-yellow-600 dark:text-yellow-400',
    poor: 'text-red-600 dark:text-red-400',
  };
  
  const statusBadges = {
    good: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400',
    moderate: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400',
    poor: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400',
  };
  
  const statusLabels = {
    good: 'Well Calibrated',
    moderate: 'Moderately Calibrated',
    poor: 'Poorly Calibrated',
  };
  
  return (
    <div className="bg-surface-elevated border border-border rounded-lg p-4">
      <div className="flex items-center justify-between mb-2">
        <h4 className="text-sm font-medium text-textSecondary">{title}</h4>
        <div className="group relative">
          <Info className="w-4 h-4 text-textTertiary cursor-help" />
          <div className="absolute right-0 top-6 z-10 hidden group-hover:block w-48 p-2 bg-gray-900 text-white text-xs rounded shadow-lg">
            {description}
          </div>
        </div>
      </div>
      <div className={`text-2xl font-bold ${statusColors[status]}`}>
        {value.toFixed(3)}
      </div>
      <span className={`inline-block mt-2 px-2 py-0.5 rounded-full text-xs font-medium ${statusBadges[status]}`}>
        {statusLabels[status]}
      </span>
    </div>
  );
};

// =============================================================================
// RELIABILITY DIAGRAM (SVG)
// =============================================================================

interface ReliabilityDiagramProps {
  bins: CalibrationBin[];
  width: number;
  height: number;
}

const ReliabilityDiagram: React.FC<ReliabilityDiagramProps> = ({
  bins,
  width,
  height,
}) => {
  const padding = { top: 20, right: 20, bottom: 40, left: 50 };
  const chartWidth = width - padding.left - padding.right;
  const chartHeight = height - padding.top - padding.bottom;
  
  // Scale functions
  const scaleX = (v: number) => padding.left + v * chartWidth;
  const scaleY = (v: number) => padding.top + (1 - v) * chartHeight;
  
  // Perfect calibration line points
  const perfectLine = `M ${scaleX(0)} ${scaleY(0)} L ${scaleX(1)} ${scaleY(1)}`;
  
  // Calibration curve
  const sortedBins = [...bins].sort((a, b) => a.predicted - b.predicted);
  const curvePath = sortedBins.length > 0
    ? `M ${scaleX(sortedBins[0].predicted)} ${scaleY(sortedBins[0].observed)} ` +
      sortedBins.slice(1).map(b => `L ${scaleX(b.predicted)} ${scaleY(b.observed)}`).join(' ')
    : '';
  
  // Grid lines
  const gridLines = [0, 0.25, 0.5, 0.75, 1];
  
  return (
    <svg width={width} height={height} className="overflow-visible">
      {/* Grid lines */}
      {gridLines.map(v => (
        <g key={v}>
          {/* Horizontal */}
          <line
            x1={scaleX(0)}
            y1={scaleY(v)}
            x2={scaleX(1)}
            y2={scaleY(v)}
            stroke="currentColor"
            strokeOpacity={0.1}
            className="text-muted-foreground"
          />
          {/* Vertical */}
          <line
            x1={scaleX(v)}
            y1={scaleY(0)}
            x2={scaleX(v)}
            y2={scaleY(1)}
            stroke="currentColor"
            strokeOpacity={0.1}
            className="text-muted-foreground"
          />
          {/* Y-axis labels */}
          <text
            x={scaleX(0) - 8}
            y={scaleY(v)}
            textAnchor="end"
            dominantBaseline="middle"
            className="text-xs fill-gray-500"
          >
            {v.toFixed(1)}
          </text>
          {/* X-axis labels */}
          <text
            x={scaleX(v)}
            y={scaleY(0) + 20}
            textAnchor="middle"
            className="text-xs fill-gray-500"
          >
            {v.toFixed(1)}
          </text>
        </g>
      ))}
      
      {/* Axis labels */}
      <text
        x={scaleX(0.5)}
        y={height - 5}
        textAnchor="middle"
        className="text-sm fill-gray-600 dark:fill-gray-400"
      >
        Predicted Probability
      </text>
      <text
        x={15}
        y={scaleY(0.5)}
        textAnchor="middle"
        dominantBaseline="middle"
        transform={`rotate(-90, 15, ${scaleY(0.5)})`}
        className="text-sm fill-gray-600 dark:fill-gray-400"
      >
        Observed Frequency
      </text>
      
      {/* Perfect calibration line (diagonal) */}
      <path
        d={perfectLine}
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
        strokeDasharray="6,4"
        className="text-gray-400 dark:text-gray-500"
      />
      
      {/* Calibration curve */}
      {curvePath && (
        <path
          d={curvePath}
          fill="none"
          stroke="#3b82f6"
          strokeWidth={3}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      )}
      
      {/* Bin circles (sized by count) */}
      {sortedBins.map((bin, i) => {
        const maxRadius = 20;
        const minRadius = 6;
        const maxCount = Math.max(...bins.map(b => b.count), 1);
        const radius = minRadius + (bin.count / maxCount) * (maxRadius - minRadius);
        const status = getBinStatus(bin.observed - bin.predicted);
        const color = status === 'calibrated' ? '#22c55e' : 
                      status === 'overconfident' ? '#ef4444' : '#eab308';
        
        return (
          <g key={i} className="group cursor-pointer">
            <circle
              cx={scaleX(bin.predicted)}
              cy={scaleY(bin.observed)}
              r={radius}
              fill={color}
              fillOpacity={0.6}
              stroke={color}
              strokeWidth={2}
              className="transition-all hover:fill-opacity-80"
            />
            {/* Tooltip */}
            <title>
              {`Predicted: ${bin.predicted.toFixed(2)}\nObserved: ${bin.observed.toFixed(2)}\nCount: ${bin.count}\nGap: ${bin.gap.toFixed(3)}`}
            </title>
          </g>
        );
      })}
      
      {/* Legend */}
      <g transform={`translate(${width - 120}, 10)`}>
        <line x1={0} y1={8} x2={20} y2={8} stroke="#888" strokeWidth={2} strokeDasharray="6,4" />
        <text x={25} y={12} className="text-xs fill-gray-600 dark:fill-gray-400">Perfect</text>
        
        <line x1={0} y1={28} x2={20} y2={28} stroke="#3b82f6" strokeWidth={3} />
        <text x={25} y={32} className="text-xs fill-gray-600 dark:fill-gray-400">Model</text>
      </g>
    </svg>
  );
};

// =============================================================================
// CALIBRATION TABLE
// =============================================================================

interface CalibrationTableProps {
  bins: CalibrationBin[];
}

const CalibrationTable: React.FC<CalibrationTableProps> = ({ bins }) => {
  const sortedBins = [...bins].sort((a, b) => a.predicted - b.predicted);
  
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border">
            <th className="px-3 py-2 text-left text-textSecondary font-medium">Confidence Bin</th>
            <th className="px-3 py-2 text-right text-textSecondary font-medium">Predicted</th>
            <th className="px-3 py-2 text-right text-textSecondary font-medium">Observed</th>
            <th className="px-3 py-2 text-right text-textSecondary font-medium">Count</th>
            <th className="px-3 py-2 text-right text-textSecondary font-medium">|Difference|</th>
            <th className="px-3 py-2 text-center text-textSecondary font-medium">Status</th>
          </tr>
        </thead>
        <tbody>
          {sortedBins.map((bin, i) => {
            const binStart = (bin.binIndex * 0.1).toFixed(1);
            const binEnd = ((bin.binIndex + 1) * 0.1).toFixed(1);
            const status = getBinStatus(bin.observed - bin.predicted);
            
            const statusConfig = {
              calibrated: { 
                badge: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400',
                icon: CheckCircle,
                label: 'Calibrated'
              },
              overconfident: {
                badge: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400',
                icon: AlertTriangle,
                label: 'Overconfident'
              },
              underconfident: {
                badge: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400',
                icon: AlertTriangle,
                label: 'Underconfident'
              },
            };
            
            const { badge, icon: Icon, label } = statusConfig[status];
            
            return (
              <tr 
                key={i} 
                className={`border-b border-border/50 ${
                  i % 2 === 0 ? 'bg-surface' : 'bg-surface-elevated'
                }`}
              >
                <td className="px-3 py-2 font-mono text-textPrimary">
                  {binStart} - {binEnd}
                </td>
                <td className="px-3 py-2 text-right font-mono text-textSecondary">
                  {bin.predicted.toFixed(3)}
                </td>
                <td className="px-3 py-2 text-right font-mono text-textSecondary">
                  {bin.observed.toFixed(3)}
                </td>
                <td className="px-3 py-2 text-right font-mono text-textSecondary">
                  {bin.count}
                </td>
                <td className="px-3 py-2 text-right font-mono text-textSecondary">
                  {bin.gap.toFixed(3)}
                </td>
                <td className="px-3 py-2 text-center">
                  <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${badge}`}>
                    <Icon className="w-3 h-3" />
                    {label}
                  </span>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
};

// =============================================================================
// MAIN COMPONENT
// =============================================================================

export const CalibrationChart: React.FC<CalibrationChartProps> = ({
  bins,
  metrics,
  width = 400,
  height = 350,
  showTable = true,
  className = '',
}) => {
  const status = getCalibrationStatus(metrics.ece);
  
  const brierStatus = metrics.brierScore < 0.10 ? 'good' : metrics.brierScore < 0.20 ? 'moderate' : 'poor';
  const eceStatus = status === 'well-calibrated' ? 'good' : status === 'moderate' ? 'moderate' : 'poor';
  const mceStatus = metrics.mce < 0.10 ? 'good' : metrics.mce < 0.20 ? 'moderate' : 'poor';
  
  return (
    <div className={`space-y-6 ${className}`}>
      {/* Metric cards */}
      <div className="grid grid-cols-3 gap-4">
        <CalibrationMetricCard
          title="Brier Score"
          value={metrics.brierScore}
          description="Mean squared error between predicted probabilities and actual outcomes. Lower is better (0 = perfect)."
          status={brierStatus}
        />
        <CalibrationMetricCard
          title="Expected Calibration Error (ECE)"
          value={metrics.ece}
          description="Weighted average of calibration gaps across bins. Measures how well confidence matches accuracy."
          status={eceStatus}
        />
        <CalibrationMetricCard
          title="Maximum Calibration Error (MCE)"
          value={metrics.mce}
          description="Largest calibration gap across all bins. Shows worst-case miscalibration."
          status={mceStatus}
        />
      </div>
      
      {/* Reliability diagram */}
      <div className="bg-surface-elevated border border-border rounded-lg p-4">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h3 className="text-lg font-semibold text-textPrimary">Reliability Diagram</h3>
            <p className="text-sm text-textSecondary">
              Perfect calibration = predictions fall on diagonal
            </p>
          </div>
        </div>
        <ReliabilityDiagram bins={bins} width={width} height={height} />
      </div>
      
      {/* Bin-by-bin table */}
      {showTable && (
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <h3 className="text-lg font-semibold text-textPrimary mb-4">Bin-by-Bin Breakdown</h3>
          <CalibrationTable bins={bins} />
        </div>
      )}
    </div>
  );
};

export default CalibrationChart;
