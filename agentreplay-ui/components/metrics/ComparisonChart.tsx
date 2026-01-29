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
 * ComparisonChart - A/B test comparison visualization
 * 
 * Features:
 * - Side-by-side metric comparison
 * - Statistical significance indicators
 * - Effect size visualization
 * - Winner declaration
 */

import React from 'react';
import { 
  ArrowUp, 
  ArrowDown, 
  Minus, 
  Trophy,
  CheckCircle2,
  AlertTriangle,
  Info
} from 'lucide-react';

// =============================================================================
// TYPES
// =============================================================================

export interface MetricComparison {
  /** Metric identifier */
  metric: string;
  /** Display name */
  label: string;
  /** Baseline value */
  baseline: number;
  /** Treatment value */
  treatment: number;
  /** p-value from statistical test */
  pValue: number;
  /** Cohen's d effect size */
  effectSize: number;
  /** Direction: 'higher' = higher is better */
  direction: 'higher' | 'lower';
  /** Unit for display */
  unit?: string;
}

export interface ComparisonChartProps {
  /** Name/label for baseline */
  baselineLabel: string;
  /** Name/label for treatment */
  treatmentLabel: string;
  /** Array of metric comparisons */
  comparisons: MetricComparison[];
  /** Significance threshold (default: 0.05) */
  alpha?: number;
  /** Show statistical details */
  showStatistics?: boolean;
  /** Additional CSS classes */
  className?: string;
}

// =============================================================================
// HELPER COMPONENTS
// =============================================================================

interface SignificanceBadgeProps {
  pValue: number;
  alpha: number;
}

const SignificanceBadge: React.FC<SignificanceBadgeProps> = ({ pValue, alpha }) => {
  const significant = pValue < alpha;
  
  return (
    <span
      className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${
        significant 
          ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400' 
          : 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400'
      }`}
    >
      {significant ? (
        <>
          <CheckCircle2 className="w-3 h-3" />
          p={pValue.toFixed(3)}
        </>
      ) : (
        <>
          <AlertTriangle className="w-3 h-3" />
          p={pValue.toFixed(3)}
        </>
      )}
    </span>
  );
};

interface EffectSizeBadgeProps {
  d: number;
}

const EffectSizeBadge: React.FC<EffectSizeBadgeProps> = ({ d }) => {
  const absD = Math.abs(d);
  
  let magnitude: string;
  let colorClass: string;
  
  if (absD < 0.2) {
    magnitude = 'negligible';
    colorClass = 'bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-300';
  } else if (absD < 0.5) {
    magnitude = 'small';
    colorClass = 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400';
  } else if (absD < 0.8) {
    magnitude = 'medium';
    colorClass = 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400';
  } else {
    magnitude = 'large';
    colorClass = 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400';
  }
  
  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${colorClass}`}>
      {magnitude} (d={d.toFixed(2)})
    </span>
  );
};

interface DeltaDisplayProps {
  baseline: number;
  treatment: number;
  direction: 'higher' | 'lower';
  unit?: string;
}

const DeltaDisplay: React.FC<DeltaDisplayProps> = ({ 
  baseline, 
  treatment, 
  direction,
}) => {
  const delta = treatment - baseline;
  const percentChange = baseline !== 0 ? (delta / baseline) * 100 : 0;
  
  // Determine if change is good or bad
  const isImprovement = direction === 'higher' ? delta > 0 : delta < 0;
  const isRegression = direction === 'higher' ? delta < 0 : delta > 0;
  
  const colorClass = isImprovement 
    ? 'text-green-600 dark:text-green-400' 
    : isRegression 
      ? 'text-red-600 dark:text-red-400' 
      : 'text-gray-600 dark:text-gray-400';
  
  const Icon = delta > 0 ? ArrowUp : delta < 0 ? ArrowDown : Minus;
  
  return (
    <div className={`flex items-center gap-1 ${colorClass}`}>
      <Icon className="w-4 h-4" />
      <span className="font-medium">
        {percentChange >= 0 ? '+' : ''}{percentChange.toFixed(1)}%
      </span>
    </div>
  );
};

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

function formatValue(value: number, unit?: string): string {
  // Handle percentages (0-1 range)
  if (value >= 0 && value <= 1 && !unit) {
    return `${(value * 100).toFixed(1)}%`;
  }
  
  // Handle large numbers
  if (Math.abs(value) >= 1000) {
    return `${(value / 1000).toFixed(2)}k${unit || ''}`;
  }
  
  // Standard formatting
  const formatted = value.toFixed(value < 10 ? 3 : 1);
  return unit ? `${formatted} ${unit}` : formatted;
}

// =============================================================================
// MAIN COMPONENT
// =============================================================================

export const ComparisonChart: React.FC<ComparisonChartProps> = ({
  baselineLabel,
  treatmentLabel,
  comparisons,
  alpha = 0.05,
  showStatistics = true,
  className = '',
}) => {
  // Calculate overall winner
  const significantImprovements = comparisons.filter(c => {
    const significant = c.pValue < alpha;
    const improved = c.direction === 'higher' 
      ? c.treatment > c.baseline 
      : c.treatment < c.baseline;
    return significant && improved;
  }).length;
  
  const significantRegressions = comparisons.filter(c => {
    const significant = c.pValue < alpha;
    const regressed = c.direction === 'higher' 
      ? c.treatment < c.baseline 
      : c.treatment > c.baseline;
    return significant && regressed;
  }).length;
  
  const winner = significantImprovements > significantRegressions 
    ? 'treatment' 
    : significantRegressions > significantImprovements 
      ? 'baseline' 
      : null;
  
  return (
    <div className={`bg-white dark:bg-gray-900 rounded-lg shadow ${className}`}>
      {/* Winner declaration */}
      {winner && (
        <div className={`flex items-center gap-2 p-4 rounded-t-lg ${
          winner === 'treatment' 
            ? 'bg-green-50 dark:bg-green-900/20' 
            : 'bg-yellow-50 dark:bg-yellow-900/20'
        }`}>
          <Trophy className={`w-5 h-5 ${
            winner === 'treatment' 
              ? 'text-green-600 dark:text-green-400' 
              : 'text-yellow-600 dark:text-yellow-400'
          }`} />
          <span className="font-semibold text-gray-800 dark:text-gray-200">
            {winner === 'treatment' ? treatmentLabel : baselineLabel} wins
          </span>
          <span className="text-sm text-gray-600 dark:text-gray-400">
            ({Math.max(significantImprovements, significantRegressions)}/{comparisons.length} metrics significantly better)
          </span>
        </div>
      )}
      
      {/* Comparison table */}
      <div className="overflow-x-auto">
        <table className="w-full">
          <thead>
            <tr className="border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800">
              <th className="px-4 py-3 text-left text-sm font-semibold text-gray-700 dark:text-gray-300">
                Metric
              </th>
              <th className="px-4 py-3 text-right text-sm font-semibold text-gray-700 dark:text-gray-300">
                {baselineLabel}
              </th>
              <th className="px-4 py-3 text-right text-sm font-semibold text-gray-700 dark:text-gray-300">
                {treatmentLabel}
              </th>
              <th className="px-4 py-3 text-center text-sm font-semibold text-gray-700 dark:text-gray-300">
                Δ Change
              </th>
              {showStatistics && (
                <>
                  <th className="px-4 py-3 text-center text-sm font-semibold text-gray-700 dark:text-gray-300">
                    Significance
                  </th>
                  <th className="px-4 py-3 text-center text-sm font-semibold text-gray-700 dark:text-gray-300">
                    Effect Size
                  </th>
                </>
              )}
            </tr>
          </thead>
          <tbody>
            {comparisons.map((comp, index) => {
              const significant = comp.pValue < alpha;
              const improved = comp.direction === 'higher' 
                ? comp.treatment > comp.baseline 
                : comp.treatment < comp.baseline;
              
              return (
                <tr 
                  key={comp.metric}
                  className={`border-b border-gray-100 dark:border-gray-800 ${
                    index % 2 === 0 
                      ? 'bg-white dark:bg-gray-900' 
                      : 'bg-gray-50 dark:bg-gray-800/50'
                  } ${significant && improved ? 'bg-green-50/50 dark:bg-green-900/10' : ''}`}
                >
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-gray-900 dark:text-gray-100">
                        {comp.label}
                      </span>
                      {comp.direction === 'lower' && (
                        <span className="text-xs text-gray-500 dark:text-gray-400">(lower is better)</span>
                      )}
                    </div>
                  </td>
                  <td className="px-4 py-3 text-right font-mono text-gray-700 dark:text-gray-300">
                    {formatValue(comp.baseline, comp.unit)}
                  </td>
                  <td className="px-4 py-3 text-right font-mono text-gray-700 dark:text-gray-300">
                    {formatValue(comp.treatment, comp.unit)}
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex justify-center">
                      <DeltaDisplay 
                        baseline={comp.baseline}
                        treatment={comp.treatment}
                        direction={comp.direction}
                        unit={comp.unit}
                      />
                    </div>
                  </td>
                  {showStatistics && (
                    <>
                      <td className="px-4 py-3 text-center">
                        <SignificanceBadge pValue={comp.pValue} alpha={alpha} />
                      </td>
                      <td className="px-4 py-3 text-center">
                        <EffectSizeBadge d={comp.effectSize} />
                      </td>
                    </>
                  )}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      
      {/* Legend */}
      {showStatistics && (
        <div className="flex items-center gap-4 px-4 py-3 bg-gray-50 dark:bg-gray-800/50 rounded-b-lg text-xs text-gray-600 dark:text-gray-400">
          <div className="flex items-center gap-1">
            <Info className="w-3 h-3" />
            <span>Effect size: |d|&lt;0.2 negligible, 0.2-0.5 small, 0.5-0.8 medium, ≥0.8 large</span>
          </div>
        </div>
      )}
    </div>
  );
};

export default ComparisonChart;
