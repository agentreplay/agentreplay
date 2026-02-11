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
 * ROCPRCurves - Interactive ROC and Precision-Recall curves with threshold selection
 * 
 * Features:
 * - Side-by-side ROC and PR curves
 * - Interactive threshold slider
 * - Live confusion matrix updates
 * - Optimal threshold suggestions
 * - Cost-benefit calculator
 */

import React, { useState, useMemo } from 'react';
import { Info, AlertTriangle, CheckCircle, Target, DollarSign } from 'lucide-react';

// =============================================================================
// TYPES
// =============================================================================

export interface CurvePoint {
  threshold: number;
  tpr: number;  // True Positive Rate (Recall)
  fpr: number;  // False Positive Rate
  precision: number;
  recall: number;  // Same as TPR
  f1: number;
}

export interface ConfusionMatrixData {
  tp: number;  // True Positives
  fp: number;  // False Positives
  tn: number;  // True Negatives
  fn: number;  // False Negatives
}

export interface ROCPRCurvesProps {
  /** Array of curve points at different thresholds */
  curvePoints: CurvePoint[];
  /** Area under ROC curve */
  auroc: number;
  /** Area under PR curve */
  auprc: number;
  /** Base rate (positive class prevalence) */
  baseRate: number;
  /** Matthews Correlation Coefficient at optimal threshold */
  mcc: number;
  /** Total number of samples */
  totalSamples: number;
  /** Width for each chart */
  chartWidth?: number;
  /** Height for each chart */
  chartHeight?: number;
  /** Show cost-benefit calculator */
  showCostBenefit?: boolean;
  /** Additional classes */
  className?: string;
}

export interface OptimalThreshold {
  name: string;
  threshold: number;
  precision: number;
  recall: number;
  f1: number;
  description: string;
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

function findOptimalThresholds(points: CurvePoint[]): OptimalThreshold[] {
  if (points.length === 0) return [];
  
  // Youden's J statistic (max TPR - FPR)
  const youdenPoint = [...points].sort((a, b) => (b.tpr - b.fpr) - (a.tpr - a.fpr))[0];
  
  // Max F1
  const f1Point = [...points].sort((a, b) => b.f1 - a.f1)[0];
  
  // High precision (≥0.9 with max recall)
  const highPrecPoints = points.filter(p => p.precision >= 0.9);
  const highPrecPoint = highPrecPoints.length > 0 
    ? [...highPrecPoints].sort((a, b) => b.recall - a.recall)[0]
    : points[0];
  
  // High recall (≥0.9 with max precision)
  const highRecallPoints = points.filter(p => p.recall >= 0.9);
  const highRecallPoint = highRecallPoints.length > 0
    ? [...highRecallPoints].sort((a, b) => b.precision - a.precision)[0]
    : points[0];
  
  return [
    {
      name: "Youden's J",
      threshold: youdenPoint.threshold,
      precision: youdenPoint.precision,
      recall: youdenPoint.recall,
      f1: youdenPoint.f1,
      description: 'Maximizes TPR - FPR (balanced)',
    },
    {
      name: 'Max F1',
      threshold: f1Point.threshold,
      precision: f1Point.precision,
      recall: f1Point.recall,
      f1: f1Point.f1,
      description: 'Best precision-recall balance',
    },
    {
      name: 'High Precision',
      threshold: highPrecPoint.threshold,
      precision: highPrecPoint.precision,
      recall: highPrecPoint.recall,
      f1: highPrecPoint.f1,
      description: 'Minimize false alarms',
    },
    {
      name: 'High Recall',
      threshold: highRecallPoint.threshold,
      precision: highRecallPoint.precision,
      recall: highRecallPoint.recall,
      f1: highRecallPoint.f1,
      description: 'Catch all positives',
    },
  ];
}

function getConfusionMatrix(point: CurvePoint, total: number, baseRate: number): ConfusionMatrixData {
  const positives = Math.round(total * baseRate);
  const negatives = total - positives;
  
  const tp = Math.round(point.tpr * positives);
  const fn = positives - tp;
  const fp = Math.round(point.fpr * negatives);
  const tn = negatives - fp;
  
  return { tp, fp, tn, fn };
}

// =============================================================================
// CURVE SVG COMPONENT
// =============================================================================

interface CurveSVGProps {
  points: CurvePoint[];
  type: 'roc' | 'pr';
  width: number;
  height: number;
  currentThreshold: number;
  baseline?: number;
  auc: number;
}

const CurveSVG: React.FC<CurveSVGProps> = ({
  points,
  type,
  width,
  height,
  currentThreshold,
  baseline,
  auc,
}) => {
  const padding = { top: 20, right: 20, bottom: 40, left: 50 };
  const chartWidth = width - padding.left - padding.right;
  const chartHeight = height - padding.top - padding.bottom;
  
  const scaleX = (v: number) => padding.left + v * chartWidth;
  const scaleY = (v: number) => padding.top + (1 - v) * chartHeight;
  
  // Sort points for curve
  const sortedPoints = type === 'roc'
    ? [...points].sort((a, b) => a.fpr - b.fpr)
    : [...points].sort((a, b) => a.recall - b.recall);
  
  // Build curve path
  const curvePath = sortedPoints.length > 0
    ? `M ${sortedPoints.map(p => 
        type === 'roc' 
          ? `${scaleX(p.fpr)} ${scaleY(p.tpr)}`
          : `${scaleX(p.recall)} ${scaleY(p.precision)}`
      ).join(' L ')}`
    : '';
  
  // Current operating point
  const currentPoint = points.find(p => Math.abs(p.threshold - currentThreshold) < 0.01) || points[0];
  
  // Grid lines
  const gridLines = [0, 0.25, 0.5, 0.75, 1];
  
  return (
    <svg width={width} height={height} className="overflow-visible">
      {/* Grid */}
      {gridLines.map(v => (
        <g key={v}>
          <line
            x1={scaleX(0)} y1={scaleY(v)}
            x2={scaleX(1)} y2={scaleY(v)}
            stroke="currentColor" strokeOpacity={0.1}
            className="text-muted-foreground"
          />
          <line
            x1={scaleX(v)} y1={scaleY(0)}
            x2={scaleX(v)} y2={scaleY(1)}
            stroke="currentColor" strokeOpacity={0.1}
            className="text-muted-foreground"
          />
          <text x={scaleX(0) - 8} y={scaleY(v)} textAnchor="end" dominantBaseline="middle"
            className="text-xs fill-gray-500">{v.toFixed(1)}</text>
          <text x={scaleX(v)} y={scaleY(0) + 20} textAnchor="middle"
            className="text-xs fill-gray-500">{v.toFixed(1)}</text>
        </g>
      ))}
      
      {/* Axis labels */}
      <text x={scaleX(0.5)} y={height - 5} textAnchor="middle"
        className="text-sm fill-gray-600 dark:fill-gray-400">
        {type === 'roc' ? 'False Positive Rate' : 'Recall'}
      </text>
      <text x={15} y={scaleY(0.5)} textAnchor="middle" dominantBaseline="middle"
        transform={`rotate(-90, 15, ${scaleY(0.5)})`}
        className="text-sm fill-gray-600 dark:fill-gray-400">
        {type === 'roc' ? 'True Positive Rate' : 'Precision'}
      </text>
      
      {/* Baseline (diagonal for ROC, horizontal for PR) */}
      {type === 'roc' ? (
        <path
          d={`M ${scaleX(0)} ${scaleY(0)} L ${scaleX(1)} ${scaleY(1)}`}
          fill="none" stroke="currentColor" strokeWidth={2} strokeDasharray="6,4"
          className="text-muted-foreground"
        />
      ) : baseline !== undefined && (
        <path
          d={`M ${scaleX(0)} ${scaleY(baseline)} L ${scaleX(1)} ${scaleY(baseline)}`}
          fill="none" stroke="currentColor" strokeWidth={2} strokeDasharray="6,4"
          className="text-muted-foreground"
        />
      )}
      
      {/* Area under curve (shaded) */}
      {curvePath && (
        <path
          d={`${curvePath} L ${scaleX(type === 'roc' ? 1 : 1)} ${scaleY(0)} L ${scaleX(0)} ${scaleY(0)} Z`}
          fill="#3b82f6"
          fillOpacity={0.1}
        />
      )}
      
      {/* Curve */}
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
      
      {/* Current operating point */}
      {currentPoint && (
        <g className="cursor-pointer">
          <circle
            cx={scaleX(type === 'roc' ? currentPoint.fpr : currentPoint.recall)}
            cy={scaleY(type === 'roc' ? currentPoint.tpr : currentPoint.precision)}
            r={10}
            fill="#ef4444"
            stroke="white"
            strokeWidth={2}
            className="drop-shadow-md"
          />
          <text
            x={scaleX(type === 'roc' ? currentPoint.fpr : currentPoint.recall)}
            y={scaleY(type === 'roc' ? currentPoint.tpr : currentPoint.precision) - 15}
            textAnchor="middle"
            className="text-xs font-medium fill-gray-700 dark:fill-gray-300"
          >
            τ={currentThreshold.toFixed(2)}
          </text>
        </g>
      )}
      
      {/* AUC label */}
      <text
        x={type === 'roc' ? scaleX(0.7) : scaleX(0.3)}
        y={type === 'roc' ? scaleY(0.3) : scaleY(0.3)}
        className="text-sm font-semibold fill-blue-600 dark:fill-blue-400"
      >
        AUC = {auc.toFixed(3)}
      </text>
    </svg>
  );
};

// =============================================================================
// CONFUSION MATRIX COMPONENT
// =============================================================================

interface ConfusionMatrixProps {
  data: ConfusionMatrixData;
}

const ConfusionMatrix: React.FC<ConfusionMatrixProps> = ({ data }) => {
  const total = data.tp + data.fp + data.tn + data.fn;
  const accuracy = (data.tp + data.tn) / total;
  
  return (
    <div className="space-y-3">
      <h4 className="text-sm font-medium text-textSecondary">Confusion Matrix</h4>
      <div className="grid grid-cols-3 gap-1 text-center text-sm">
        {/* Header row */}
        <div></div>
        <div className="font-medium text-textSecondary py-1">Pred -</div>
        <div className="font-medium text-textSecondary py-1">Pred +</div>
        
        {/* Actual Negative row */}
        <div className="font-medium text-textSecondary py-2">Act -</div>
        <div className="bg-green-100 dark:bg-green-900/30 text-green-800 dark:text-green-400 py-2 rounded">
          TN: {data.tn}
        </div>
        <div className="bg-red-100 dark:bg-red-900/30 text-red-800 dark:text-red-400 py-2 rounded">
          FP: {data.fp}
        </div>
        
        {/* Actual Positive row */}
        <div className="font-medium text-textSecondary py-2">Act +</div>
        <div className="bg-red-100 dark:bg-red-900/30 text-red-800 dark:text-red-400 py-2 rounded">
          FN: {data.fn}
        </div>
        <div className="bg-green-100 dark:bg-green-900/30 text-green-800 dark:text-green-400 py-2 rounded">
          TP: {data.tp}
        </div>
      </div>
      <div className="text-xs text-textSecondary text-center">
        Accuracy: {(accuracy * 100).toFixed(1)}%
      </div>
    </div>
  );
};

// =============================================================================
// PERFORMANCE METRICS TABLE
// =============================================================================

interface PerformanceTableProps {
  point: CurvePoint;
  mcc: number;
}

const PerformanceTable: React.FC<PerformanceTableProps> = ({ point, mcc }) => {
  const metrics = [
    { name: 'Precision', value: point.precision, formula: 'TP / (TP + FP)' },
    { name: 'Recall (TPR)', value: point.recall, formula: 'TP / (TP + FN)' },
    { name: 'Specificity (TNR)', value: 1 - point.fpr, formula: 'TN / (TN + FP)' },
    { name: 'F1 Score', value: point.f1, formula: '2 × P × R / (P + R)' },
    { name: 'MCC', value: mcc, formula: 'Balanced metric', highlight: true },
  ];
  
  return (
    <table className="w-full text-sm">
      <tbody>
        {metrics.map((m, i) => (
          <tr key={i} className={`border-b border-border/50 ${m.highlight ? 'bg-primary/5' : ''}`}>
            <td className="py-2 text-textSecondary">{m.name}</td>
            <td className="py-2 text-right font-mono font-semibold text-textPrimary">
              {m.value.toFixed(3)}
            </td>
            <td className="py-2 pl-2 text-xs text-textTertiary">{m.formula}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
};

// =============================================================================
// THRESHOLD SELECTOR
// =============================================================================

interface ThresholdSelectorProps {
  value: number;
  onChange: (v: number) => void;
  optimalThresholds: OptimalThreshold[];
}

const ThresholdSelector: React.FC<ThresholdSelectorProps> = ({
  value,
  onChange,
  optimalThresholds,
}) => {
  return (
    <div className="space-y-4">
      <div>
        <label className="block text-sm font-medium text-textSecondary mb-2">
          Decision Threshold
        </label>
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={value}
          onChange={(e) => onChange(parseFloat(e.target.value))}
          className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer dark:bg-gray-700"
        />
        <div className="flex justify-between text-xs text-textTertiary mt-1">
          <span>0</span>
          <span className="font-mono font-semibold text-textPrimary">{value.toFixed(2)}</span>
          <span>1</span>
        </div>
      </div>
      
      {/* Suggested thresholds */}
      <div>
        <h5 className="text-sm font-medium text-textSecondary mb-2">Suggested Operating Points</h5>
        <div className="space-y-1">
          {optimalThresholds.map((opt, i) => (
            <button
              key={i}
              onClick={() => onChange(opt.threshold)}
              className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-left text-sm transition-colors ${
                Math.abs(value - opt.threshold) < 0.01
                  ? 'bg-primary/10 text-primary border border-primary/30'
                  : 'bg-surface-elevated hover:bg-surface-hover border border-border'
              }`}
            >
              <div>
                <span className="font-medium">{opt.name}</span>
                <span className="text-textTertiary ml-2">τ={opt.threshold.toFixed(2)}</span>
              </div>
              <div className="text-xs text-textSecondary">
                P:{opt.precision.toFixed(2)} R:{opt.recall.toFixed(2)}
              </div>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
};

// =============================================================================
// COST-BENEFIT CALCULATOR
// =============================================================================

interface CostBenefitProps {
  confusionMatrix: ConfusionMatrixData;
}

const CostBenefitCalculator: React.FC<CostBenefitProps> = ({ confusionMatrix }) => {
  const [costFP, setCostFP] = useState(100);
  const [costFN, setCostFN] = useState(100);
  
  const expectedCost = (confusionMatrix.fp * costFP + confusionMatrix.fn * costFN) / 1000;
  
  return (
    <div className="space-y-4 p-4 bg-surface-elevated rounded-lg border border-border">
      <h4 className="text-sm font-medium text-textPrimary flex items-center gap-2">
        <DollarSign className="w-4 h-4" />
        Cost-Benefit Calculator
      </h4>
      
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="block text-xs text-textSecondary mb-1">Cost of False Positive</label>
          <input
            type="number"
            value={costFP}
            onChange={(e) => setCostFP(Number(e.target.value))}
            className="w-full px-3 py-1.5 text-sm rounded border border-border bg-background"
          />
        </div>
        <div>
          <label className="block text-xs text-textSecondary mb-1">Cost of False Negative</label>
          <input
            type="number"
            value={costFN}
            onChange={(e) => setCostFN(Number(e.target.value))}
            className="w-full px-3 py-1.5 text-sm rounded border border-border bg-background"
          />
        </div>
      </div>
      
      <div className="pt-2 border-t border-border">
        <div className="flex justify-between items-center">
          <span className="text-sm text-textSecondary">Expected Cost per 1000 samples:</span>
          <span className="text-lg font-bold text-primary">${expectedCost.toFixed(2)}</span>
        </div>
      </div>
    </div>
  );
};

// =============================================================================
// MAIN COMPONENT
// =============================================================================

export const ROCPRCurves: React.FC<ROCPRCurvesProps> = ({
  curvePoints,
  auroc,
  auprc,
  baseRate,
  mcc,
  totalSamples,
  chartWidth = 350,
  chartHeight = 300,
  showCostBenefit = true,
  className = '',
}) => {
  const [threshold, setThreshold] = useState(0.5);
  
  const optimalThresholds = useMemo(
    () => findOptimalThresholds(curvePoints),
    [curvePoints]
  );
  
  const currentPoint = useMemo(
    () => curvePoints.find(p => Math.abs(p.threshold - threshold) < 0.01) || curvePoints[0],
    [curvePoints, threshold]
  );
  
  const confusionMatrix = useMemo(
    () => getConfusionMatrix(currentPoint, totalSamples, baseRate),
    [currentPoint, totalSamples, baseRate]
  );
  
  return (
    <div className={`space-y-6 ${className}`}>
      {/* Summary cards */}
      <div className="grid grid-cols-4 gap-4">
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <div className="text-sm text-textSecondary">AUROC</div>
          <div className="text-2xl font-bold text-blue-600 dark:text-blue-400">{auroc.toFixed(3)}</div>
          <div className="text-xs text-textTertiary">Area under ROC</div>
        </div>
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <div className="text-sm text-textSecondary">AUPRC</div>
          <div className="text-2xl font-bold text-green-600 dark:text-green-400">{auprc.toFixed(3)}</div>
          <div className="text-xs text-textTertiary">Area under PR</div>
        </div>
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <div className="text-sm text-textSecondary">MCC</div>
          <div className="text-2xl font-bold text-purple-600 dark:text-purple-400">{mcc.toFixed(3)}</div>
          <div className="text-xs text-textTertiary">Matthews Correlation</div>
        </div>
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <div className="text-sm text-textSecondary">Base Rate</div>
          <div className="text-2xl font-bold text-textPrimary">{(baseRate * 100).toFixed(1)}%</div>
          <div className="text-xs text-textTertiary">Positive prevalence</div>
        </div>
      </div>
      
      {/* Charts */}
      <div className="grid grid-cols-2 gap-6">
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <h3 className="text-lg font-semibold text-textPrimary mb-2">ROC Curve</h3>
          <p className="text-sm text-textSecondary mb-4">Receiver Operating Characteristic</p>
          <CurveSVG
            points={curvePoints}
            type="roc"
            width={chartWidth}
            height={chartHeight}
            currentThreshold={threshold}
            auc={auroc}
          />
        </div>
        
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <div className="flex items-start justify-between mb-2">
            <div>
              <h3 className="text-lg font-semibold text-textPrimary">Precision-Recall Curve</h3>
              <p className="text-sm text-textSecondary">Better for imbalanced datasets</p>
            </div>
            <span className="px-2 py-0.5 text-xs bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400 rounded">
              Recommended
            </span>
          </div>
          <CurveSVG
            points={curvePoints}
            type="pr"
            width={chartWidth}
            height={chartHeight}
            currentThreshold={threshold}
            baseline={baseRate}
            auc={auprc}
          />
        </div>
      </div>
      
      {/* Threshold controls and metrics */}
      <div className="grid grid-cols-3 gap-6">
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <h3 className="text-lg font-semibold text-textPrimary mb-4 flex items-center gap-2">
            <Target className="w-5 h-5" />
            Threshold Selection
          </h3>
          <ThresholdSelector
            value={threshold}
            onChange={setThreshold}
            optimalThresholds={optimalThresholds}
          />
        </div>
        
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <h3 className="text-lg font-semibold text-textPrimary mb-4">
            Performance at τ={threshold.toFixed(2)}
          </h3>
          <ConfusionMatrix data={confusionMatrix} />
          <div className="mt-4">
            <PerformanceTable point={currentPoint} mcc={mcc} />
          </div>
        </div>
        
        {showCostBenefit && (
          <CostBenefitCalculator confusionMatrix={confusionMatrix} />
        )}
      </div>
    </div>
  );
};

export default ROCPRCurves;
