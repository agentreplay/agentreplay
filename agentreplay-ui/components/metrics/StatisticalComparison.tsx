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
 * StatisticalComparison - Advanced A/B test comparison with statistical tests
 * 
 * Features:
 * - Welch's t-test results
 * - Confidence interval for difference
 * - Effect size (Cohen's d) with interpretation
 * - Power analysis
 * - Practical significance check
 * - Winner declaration
 */

import React, { useMemo } from 'react';
import { 
  Trophy, 
  ArrowUp, 
  ArrowDown, 
  Minus, 
  CheckCircle, 
  AlertTriangle,
  Info,
  Zap,
  Target
} from 'lucide-react';
import { DifferenceCI } from './ConfidenceInterval';

// =============================================================================
// TYPES
// =============================================================================

export interface VariantStats {
  name: string;
  mean: number;
  stdDev: number;
  n: number;
  ci95: [number, number];
  median?: number;
  passRate?: number;
}

export interface StatisticalTestResult {
  tStatistic: number;
  degreesOfFreedom: number;
  pValue: number;
  difference: number;
  differenceCI: [number, number];
  cohensD: number;
  achievedPower: number;
}

export interface StatisticalComparisonProps {
  /** Baseline variant statistics */
  baseline: VariantStats;
  /** Treatment variant statistics */
  treatment: VariantStats;
  /** Statistical test results */
  testResult: StatisticalTestResult;
  /** Significance level (default: 0.05) */
  alpha?: number;
  /** Minimum Detectable Effect Size for practical significance */
  mdes?: number;
  /** Additional classes */
  className?: string;
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

function interpretEffectSize(d: number): { label: string; color: string } {
  const absD = Math.abs(d);
  if (absD < 0.2) return { label: 'Negligible', color: 'text-gray-500' };
  if (absD < 0.5) return { label: 'Small', color: 'text-yellow-600 dark:text-yellow-400' };
  if (absD < 0.8) return { label: 'Medium', color: 'text-blue-600 dark:text-blue-400' };
  return { label: 'Large', color: 'text-green-600 dark:text-green-400' };
}

function formatPValue(p: number): string {
  if (p < 0.001) return '< 0.001';
  if (p < 0.01) return p.toFixed(4);
  return p.toFixed(3);
}

// =============================================================================
// VARIANT CARD
// =============================================================================

interface VariantCardProps {
  variant: VariantStats;
  isBaseline?: boolean;
  isWinner?: boolean;
}

const VariantCard: React.FC<VariantCardProps> = ({ variant, isBaseline, isWinner }) => {
  return (
    <div className={`relative p-4 rounded-lg border ${
      isWinner 
        ? 'bg-green-50 border-green-200 dark:bg-green-900/20 dark:border-green-800' 
        : 'bg-surface-elevated border-border'
    }`}>
      {isWinner && (
        <div className="absolute -top-3 left-4 flex items-center gap-1 px-2 py-0.5 bg-green-500 text-white text-xs font-medium rounded-full">
          <Trophy className="w-3 h-3" />
          Winner
        </div>
      )}
      
      <div className="flex items-center gap-2 mb-3">
        <h4 className="font-semibold text-textPrimary">{variant.name}</h4>
        {isBaseline && (
          <span className="px-2 py-0.5 text-xs bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400 rounded">
            Baseline
          </span>
        )}
      </div>
      
      <div className="space-y-2">
        <div className="flex justify-between">
          <span className="text-sm text-textSecondary">Mean ± SD</span>
          <span className="font-mono text-textPrimary">
            {variant.mean.toFixed(3)} ± {variant.stdDev.toFixed(3)}
          </span>
        </div>
        <div className="flex justify-between">
          <span className="text-sm text-textSecondary">Sample Size</span>
          <span className="font-mono text-textPrimary">n = {variant.n}</span>
        </div>
        <div className="flex justify-between">
          <span className="text-sm text-textSecondary">95% CI</span>
          <span className="font-mono text-textPrimary">
            [{variant.ci95[0].toFixed(3)}, {variant.ci95[1].toFixed(3)}]
          </span>
        </div>
        {variant.passRate !== undefined && (
          <div className="flex justify-between">
            <span className="text-sm text-textSecondary">Pass Rate</span>
            <span className="font-mono text-textPrimary">
              {(variant.passRate * 100).toFixed(1)}%
            </span>
          </div>
        )}
      </div>
    </div>
  );
};

// =============================================================================
// EFFECT SIZE VISUALIZATION
// =============================================================================

interface EffectSizeDisplayProps {
  d: number;
}

const EffectSizeDisplay: React.FC<EffectSizeDisplayProps> = ({ d }) => {
  const { label, color } = interpretEffectSize(d);
  
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <span className="text-sm text-textSecondary">Cohen's d</span>
        <span className={`text-2xl font-bold ${color}`}>{d.toFixed(2)}</span>
      </div>
      
      {/* Effect size scale */}
      <div className="relative h-3 bg-gray-100 dark:bg-gray-800 rounded-full overflow-hidden">
        <div className="absolute inset-y-0 left-0 bg-gray-300 dark:bg-gray-600" style={{ width: '20%' }} />
        <div className="absolute inset-y-0 bg-yellow-400" style={{ left: '20%', width: '30%' }} />
        <div className="absolute inset-y-0 bg-blue-400" style={{ left: '50%', width: '30%' }} />
        <div className="absolute inset-y-0 right-0 bg-green-400" style={{ width: '20%' }} />
        
        {/* Marker */}
        <div 
          className="absolute top-1/2 -translate-y-1/2 w-3 h-3 bg-gray-900 dark:bg-white rounded-full border-2 border-white dark:border-gray-900"
          style={{ left: `${Math.min(Math.abs(d) / 1.2 * 100, 100)}%` }}
        />
      </div>
      
      <div className="flex justify-between text-xs text-textTertiary">
        <span>Negligible</span>
        <span>Small (0.2)</span>
        <span>Medium (0.5)</span>
        <span>Large (0.8+)</span>
      </div>
      
      <div className={`text-center font-semibold ${color}`}>
        {label} Effect
      </div>
    </div>
  );
};

// =============================================================================
// POWER METER
// =============================================================================

interface PowerMeterProps {
  power: number;
}

const PowerMeter: React.FC<PowerMeterProps> = ({ power }) => {
  const powerPercent = power * 100;
  const isAdequate = power >= 0.8;
  
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Zap className={`w-4 h-4 ${isAdequate ? 'text-green-500' : 'text-yellow-500'}`} />
          <span className="text-sm font-medium text-textSecondary">Achieved Power</span>
        </div>
        <span className={`text-lg font-bold ${isAdequate ? 'text-green-600' : 'text-yellow-600'}`}>
          {powerPercent.toFixed(1)}%
        </span>
      </div>
      
      <div className="relative h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
        <div 
          className={`absolute inset-y-0 left-0 rounded-full transition-all ${
            isAdequate ? 'bg-green-500' : 'bg-yellow-500'
          }`}
          style={{ width: `${powerPercent}%` }}
        />
        {/* 80% threshold marker */}
        <div 
          className="absolute top-0 bottom-0 w-0.5 bg-gray-400"
          style={{ left: '80%' }}
        />
      </div>
      
      <div className="flex justify-between text-xs">
        <span className="text-textTertiary">0%</span>
        <span className="text-textTertiary">80% recommended →</span>
        <span className="text-textTertiary">100%</span>
      </div>
    </div>
  );
};

// =============================================================================
// MAIN COMPONENT
// =============================================================================

export const StatisticalComparison: React.FC<StatisticalComparisonProps> = ({
  baseline,
  treatment,
  testResult,
  alpha = 0.05,
  mdes = 0.05,
  className = '',
}) => {
  const isSignificant = testResult.pValue < alpha;
  const isImprovement = testResult.difference > 0;
  const isPracticallySignificant = Math.abs(testResult.difference) >= mdes;
  
  const winner = isSignificant && isImprovement ? 'treatment' : 
                 isSignificant && !isImprovement ? 'baseline' : null;
  
  const improvementPercentage = (testResult.difference / baseline.mean) * 100;
  
  return (
    <div className={`space-y-6 ${className}`}>
      {/* Winner declaration */}
      {winner && (
        <div className={`flex items-center gap-3 p-4 rounded-lg ${
          winner === 'treatment' 
            ? 'bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800' 
            : 'bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800'
        }`}>
          <Trophy className={`w-6 h-6 ${
            winner === 'treatment' ? 'text-green-600' : 'text-yellow-600'
          }`} />
          <div>
            <span className="font-semibold text-textPrimary">
              {winner === 'treatment' ? treatment.name : baseline.name} wins
            </span>
            <p className="text-sm text-textSecondary">
              Expected improvement: {improvementPercentage >= 0 ? '+' : ''}{improvementPercentage.toFixed(1)}% 
              (CI: {((testResult.differenceCI[0] / baseline.mean) * 100).toFixed(1)}% to {((testResult.differenceCI[1] / baseline.mean) * 100).toFixed(1)}%)
            </p>
          </div>
        </div>
      )}
      
      {/* Variant cards */}
      <div className="grid grid-cols-2 gap-4">
        <VariantCard 
          variant={baseline} 
          isBaseline 
          isWinner={winner === 'baseline'}
        />
        <VariantCard 
          variant={treatment} 
          isWinner={winner === 'treatment'}
        />
      </div>
      
      {/* Difference visualization */}
      <div className="bg-surface-elevated border border-border rounded-lg p-4">
        <h4 className="text-sm font-medium text-textSecondary mb-4">Difference: Treatment - Baseline</h4>
        
        <div className="flex items-center gap-4 mb-4">
          <div className={`text-3xl font-bold ${
            isImprovement ? 'text-green-600' : testResult.difference < 0 ? 'text-red-600' : 'text-gray-600'
          }`}>
            {testResult.difference >= 0 ? '+' : ''}{testResult.difference.toFixed(4)}
          </div>
          <div className={`text-xl ${
            isImprovement ? 'text-green-600' : testResult.difference < 0 ? 'text-red-600' : 'text-gray-600'
          }`}>
            ({improvementPercentage >= 0 ? '+' : ''}{improvementPercentage.toFixed(1)}%)
          </div>
          {isImprovement ? (
            <ArrowUp className="w-6 h-6 text-green-600" />
          ) : testResult.difference < 0 ? (
            <ArrowDown className="w-6 h-6 text-red-600" />
          ) : (
            <Minus className="w-6 h-6 text-gray-600" />
          )}
        </div>
        
        <DifferenceCI
          difference={testResult.difference}
          lower={testResult.differenceCI[0]}
          upper={testResult.differenceCI[1]}
          asPercentage={false}
        />
      </div>
      
      {/* Statistical test results */}
      <div className="grid grid-cols-2 gap-6">
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <h4 className="text-sm font-medium text-textSecondary mb-4 flex items-center gap-2">
            Welch's t-test Results
            <div className="group relative">
              <Info className="w-4 h-4 text-textTertiary cursor-help" />
              <div className="absolute left-0 top-6 z-10 hidden group-hover:block w-48 p-2 bg-gray-900 text-white text-xs rounded shadow-lg">
                Welch's t-test doesn't assume equal variances between groups
              </div>
            </div>
          </h4>
          
          <table className="w-full text-sm">
            <tbody>
              <tr className="border-b border-border/50">
                <td className="py-2 text-textSecondary">t-statistic</td>
                <td className="py-2 text-right font-mono">{testResult.tStatistic.toFixed(3)}</td>
              </tr>
              <tr className="border-b border-border/50">
                <td className="py-2 text-textSecondary">Degrees of freedom</td>
                <td className="py-2 text-right font-mono">{testResult.degreesOfFreedom.toFixed(1)}</td>
              </tr>
              <tr className="border-b border-border/50 bg-primary/5">
                <td className="py-2 text-textSecondary font-medium">p-value</td>
                <td className="py-2 text-right">
                  <span className="font-mono mr-2">{formatPValue(testResult.pValue)}</span>
                  {isSignificant ? (
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400">
                      <CheckCircle className="w-3 h-3" />
                      Significant
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400">
                      <AlertTriangle className="w-3 h-3" />
                      Not Significant
                    </span>
                  )}
                </td>
              </tr>
              <tr>
                <td className="py-2 text-textSecondary">95% CI for difference</td>
                <td className="py-2 text-right font-mono">
                  [{testResult.differenceCI[0].toFixed(4)}, {testResult.differenceCI[1].toFixed(4)}]
                </td>
              </tr>
            </tbody>
          </table>
        </div>
        
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <h4 className="text-sm font-medium text-textSecondary mb-4">Effect Size</h4>
          <EffectSizeDisplay d={testResult.cohensD} />
        </div>
      </div>
      
      {/* Practical significance & power */}
      <div className="grid grid-cols-2 gap-6">
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <h4 className="text-sm font-medium text-textSecondary mb-4 flex items-center gap-2">
            <Target className="w-4 h-4" />
            Practical Significance Check
          </h4>
          
          {isSignificant && isPracticallySignificant ? (
            <div className="p-3 rounded-lg bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800">
              <div className="flex items-center gap-2 text-green-700 dark:text-green-400 font-medium mb-2">
                <CheckCircle className="w-5 h-5" />
                Statistical AND Practical Significance
              </div>
              <ul className="text-sm text-green-600 dark:text-green-400 space-y-1 ml-7">
                <li>✓ Statistically significant: p &lt; {alpha}</li>
                <li>✓ Effect size: {interpretEffectSize(testResult.cohensD).label.toLowerCase()} (d = {testResult.cohensD.toFixed(2)})</li>
                <li>✓ Difference exceeds MDES: {Math.abs(testResult.difference).toFixed(3)} &gt; {mdes}</li>
              </ul>
            </div>
          ) : isSignificant ? (
            <div className="p-3 rounded-lg bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800">
              <div className="flex items-center gap-2 text-yellow-700 dark:text-yellow-400 font-medium mb-2">
                <AlertTriangle className="w-5 h-5" />
                Statistically Significant, Practically Questionable
              </div>
              <p className="text-sm text-yellow-600 dark:text-yellow-400 ml-7">
                The difference is statistically significant but may not be large enough to matter in practice. 
                Consider if |{Math.abs(testResult.difference).toFixed(3)}| is meaningful for your use case.
              </p>
            </div>
          ) : (
            <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700">
              <div className="flex items-center gap-2 text-gray-700 dark:text-gray-400 font-medium mb-2">
                <Minus className="w-5 h-5" />
                No Significant Difference Detected
              </div>
              <p className="text-sm text-gray-600 dark:text-gray-400 ml-7">
                Cannot conclude that treatment is different from baseline at α = {alpha}.
                Consider increasing sample size for more power.
              </p>
            </div>
          )}
        </div>
        
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <h4 className="text-sm font-medium text-textSecondary mb-4">Study Power</h4>
          <PowerMeter power={testResult.achievedPower} />
          {testResult.achievedPower < 0.8 && (
            <p className="mt-3 text-xs text-yellow-600 dark:text-yellow-400">
              ⚠ Power below 80% may result in missing true effects. Consider larger sample sizes.
            </p>
          )}
        </div>
      </div>
    </div>
  );
};

export default StatisticalComparison;
