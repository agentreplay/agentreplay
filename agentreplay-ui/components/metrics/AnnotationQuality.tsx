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
 * AnnotationQuality - Inter-rater reliability dashboard for human annotations
 * 
 * Features:
 * - Krippendorff's Alpha / Fleiss' Kappa display
 * - Pairwise annotator agreement heatmap
 * - Individual annotator performance
 * - Agreement recommendations
 */

import React, { useMemo } from 'react';
import { 
  Users, 
  AlertTriangle, 
  CheckCircle, 
  XCircle,
  Info,
  TrendingUp
} from 'lucide-react';

// =============================================================================
// TYPES
// =============================================================================

export interface AnnotatorStats {
  id: string;
  name: string;
  annotationCount: number;
  avgAgreement: number;
  controversialCount: number;
  accuracy?: number;
  precision?: number;
  recall?: number;
}

export interface PairwiseAgreement {
  annotator1: string;
  annotator2: string;
  kappa: number;
  agreement: number;
  count: number;
}

export interface AgreementMetrics {
  krippendorffsAlpha: number;
  alphaCI95?: [number, number];
  fleissKappa: number;
  percentAgreement: number;
}

export interface AnnotationQualityProps {
  /** Overall agreement metrics */
  metrics: AgreementMetrics;
  /** Individual annotator statistics */
  annotators: AnnotatorStats[];
  /** Pairwise agreement data */
  pairwiseAgreements: PairwiseAgreement[];
  /** Whether ground truth is available */
  hasGroundTruth?: boolean;
  /** Additional classes */
  className?: string;
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

function interpretKappa(kappa: number): { label: string; color: string } {
  if (kappa > 0.8) return { label: 'Excellent', color: 'text-green-600 dark:text-green-400' };
  if (kappa > 0.67) return { label: 'Good', color: 'text-blue-600 dark:text-blue-400' };
  if (kappa > 0.4) return { label: 'Moderate', color: 'text-yellow-600 dark:text-yellow-400' };
  if (kappa > 0.2) return { label: 'Fair', color: 'text-orange-600 dark:text-orange-400' };
  return { label: 'Poor', color: 'text-red-600 dark:text-red-400' };
}

function getKappaColor(kappa: number): string {
  if (kappa > 0.8) return '#22c55e';
  if (kappa > 0.67) return '#3b82f6';
  if (kappa > 0.4) return '#eab308';
  if (kappa > 0.2) return '#f97316';
  return '#ef4444';
}

// =============================================================================
// AGREEMENT METRIC CARD
// =============================================================================

interface AgreementMetricCardProps {
  title: string;
  value: number;
  ci95?: [number, number];
  description: string;
}

const AgreementMetricCard: React.FC<AgreementMetricCardProps> = ({
  title,
  value,
  ci95,
  description,
}) => {
  const { label, color } = interpretKappa(value);
  
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
      
      <div className={`text-3xl font-bold ${color}`}>
        {value.toFixed(3)}
      </div>
      
      {ci95 && (
        <div className="text-xs text-textTertiary mt-1">
          95% CI: [{ci95[0].toFixed(3)}, {ci95[1].toFixed(3)}]
        </div>
      )}
      
      <span className={`inline-block mt-2 px-2 py-0.5 rounded-full text-xs font-medium ${
        label === 'Excellent' ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400' :
        label === 'Good' ? 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400' :
        label === 'Moderate' ? 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400' :
        label === 'Fair' ? 'bg-orange-100 text-orange-800 dark:bg-orange-900/30 dark:text-orange-400' :
        'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400'
      }`}>
        {label} Agreement
      </span>
    </div>
  );
};

// =============================================================================
// LANDIS-KOCH SCALE
// =============================================================================

interface LandisKochScaleProps {
  value: number;
}

const LandisKochScale: React.FC<LandisKochScaleProps> = ({ value }) => {
  const scales = [
    { min: 0, max: 0.2, label: 'Poor', color: 'bg-red-500' },
    { min: 0.2, max: 0.4, label: 'Fair', color: 'bg-orange-500' },
    { min: 0.4, max: 0.6, label: 'Moderate', color: 'bg-yellow-500' },
    { min: 0.6, max: 0.8, label: 'Substantial', color: 'bg-blue-500' },
    { min: 0.8, max: 1.0, label: 'Almost Perfect', color: 'bg-green-500' },
  ];
  
  const markerPos = Math.min(Math.max(value, 0), 1) * 100;
  
  return (
    <div className="space-y-2">
      <div className="relative h-4 flex rounded-full overflow-hidden">
        {scales.map((s, i) => (
          <div
            key={i}
            className={`${s.color}`}
            style={{ width: `${(s.max - s.min) * 100}%` }}
          />
        ))}
        {/* Marker */}
        <div 
          className="absolute top-1/2 -translate-y-1/2 w-3 h-3 bg-gray-900 dark:bg-white rounded-full border-2 border-white dark:border-gray-900 z-10"
          style={{ left: `${markerPos}%`, marginLeft: '-6px' }}
        />
      </div>
      <div className="flex justify-between text-xs text-textTertiary">
        {scales.map((s, i) => (
          <span key={i} className="w-1/5 text-center">{s.label}</span>
        ))}
      </div>
    </div>
  );
};

// =============================================================================
// PAIRWISE HEATMAP
// =============================================================================

interface PairwiseHeatmapProps {
  annotators: string[];
  agreements: PairwiseAgreement[];
}

const PairwiseHeatmap: React.FC<PairwiseHeatmapProps> = ({ annotators, agreements }) => {
  // Build lookup map
  const agreementMap = useMemo(() => {
    const map = new Map<string, number>();
    agreements.forEach(a => {
      map.set(`${a.annotator1}-${a.annotator2}`, a.kappa);
      map.set(`${a.annotator2}-${a.annotator1}`, a.kappa);
    });
    return map;
  }, [agreements]);
  
  const getKappa = (a1: string, a2: string): number | null => {
    if (a1 === a2) return null;
    return agreementMap.get(`${a1}-${a2}`) ?? null;
  };
  
  const cellSize = 50;
  const labelWidth = 100;
  
  return (
    <div className="overflow-x-auto">
      <table className="border-collapse">
        <thead>
          <tr>
            <th style={{ width: labelWidth }}></th>
            {annotators.map(a => (
              <th 
                key={a}
                className="text-xs font-medium text-textSecondary p-2 text-center"
                style={{ width: cellSize }}
              >
                {a.slice(0, 8)}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {annotators.map((a1, i) => (
            <tr key={a1}>
              <td className="text-xs font-medium text-textSecondary p-2 text-right">
                {a1}
              </td>
              {annotators.map((a2, j) => {
                const kappa = getKappa(a1, a2);
                
                if (i === j) {
                  return (
                    <td 
                      key={a2}
                      className="border border-border bg-gray-100 dark:bg-gray-800"
                      style={{ width: cellSize, height: cellSize }}
                    />
                  );
                }
                
                if (kappa === null) {
                  return (
                    <td 
                      key={a2}
                      className="border border-border bg-gray-50 dark:bg-gray-900"
                      style={{ width: cellSize, height: cellSize }}
                    />
                  );
                }
                
                const color = getKappaColor(kappa);
                const { label } = interpretKappa(kappa);
                
                return (
                  <td 
                    key={a2}
                    className="border border-border text-center cursor-pointer group relative"
                    style={{ 
                      width: cellSize, 
                      height: cellSize,
                      backgroundColor: `${color}33`,
                    }}
                    title={`${a1} ↔ ${a2}: κ = ${kappa.toFixed(3)} (${label})`}
                  >
                    <span className="text-xs font-mono font-semibold" style={{ color }}>
                      {kappa.toFixed(2)}
                    </span>
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};

// =============================================================================
// ANNOTATOR PERFORMANCE TABLE
// =============================================================================

interface AnnotatorTableProps {
  annotators: AnnotatorStats[];
  hasGroundTruth: boolean;
}

const AnnotatorTable: React.FC<AnnotatorTableProps> = ({ annotators, hasGroundTruth }) => {
  const sortedAnnotators = [...annotators].sort((a, b) => b.avgAgreement - a.avgAgreement);
  
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="border-b border-border">
          <th className="px-3 py-2 text-left text-textSecondary font-medium">Annotator</th>
          <th className="px-3 py-2 text-right text-textSecondary font-medium">Annotations</th>
          <th className="px-3 py-2 text-right text-textSecondary font-medium">Avg Agreement</th>
          <th className="px-3 py-2 text-right text-textSecondary font-medium">Controversial</th>
          {hasGroundTruth && (
            <>
              <th className="px-3 py-2 text-right text-textSecondary font-medium">Accuracy</th>
            </>
          )}
          <th className="px-3 py-2 text-center text-textSecondary font-medium">Status</th>
        </tr>
      </thead>
      <tbody>
        {sortedAnnotators.map((annotator, i) => {
          const isLowAgreement = annotator.avgAgreement < 0.6;
          const isHighControversial = annotator.controversialCount > 10;
          
          return (
            <tr 
              key={annotator.id}
              className={`border-b border-border/50 ${
                isLowAgreement ? 'bg-red-50 dark:bg-red-900/10' : ''
              }`}
            >
              <td className="px-3 py-2 font-medium text-textPrimary">
                {annotator.name}
              </td>
              <td className="px-3 py-2 text-right font-mono text-textSecondary">
                {annotator.annotationCount}
              </td>
              <td className="px-3 py-2 text-right">
                <div className="flex items-center justify-end gap-2">
                  <div className="w-16 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                    <div 
                      className="h-full rounded-full"
                      style={{ 
                        width: `${annotator.avgAgreement * 100}%`,
                        backgroundColor: getKappaColor(annotator.avgAgreement)
                      }}
                    />
                  </div>
                  <span className="font-mono text-textPrimary">
                    {(annotator.avgAgreement * 100).toFixed(1)}%
                  </span>
                </div>
              </td>
              <td className="px-3 py-2 text-right">
                <span className={`font-mono ${isHighControversial ? 'text-yellow-600' : 'text-textSecondary'}`}>
                  {annotator.controversialCount}
                </span>
                {isHighControversial && (
                  <span className="ml-1 px-1.5 py-0.5 text-xs bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400 rounded">
                    High
                  </span>
                )}
              </td>
              {hasGroundTruth && annotator.accuracy !== undefined && (
                <td className="px-3 py-2 text-right font-mono text-textSecondary">
                  {(annotator.accuracy * 100).toFixed(1)}%
                </td>
              )}
              <td className="px-3 py-2 text-center">
                {isLowAgreement ? (
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400">
                    <XCircle className="w-3 h-3" />
                    Low Agreement
                  </span>
                ) : (
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400">
                    <CheckCircle className="w-3 h-3" />
                    Reliable
                  </span>
                )}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
};

// =============================================================================
// RECOMMENDATIONS PANEL
// =============================================================================

interface RecommendationsProps {
  metrics: AgreementMetrics;
  annotators: AnnotatorStats[];
}

const Recommendations: React.FC<RecommendationsProps> = ({ metrics, annotators }) => {
  const lowAgreementAnnotators = annotators.filter(a => a.avgAgreement < 0.6);
  const isLowOverallAgreement = metrics.krippendorffsAlpha < 0.67;
  const isExcellent = metrics.krippendorffsAlpha >= 0.8;
  
  if (isExcellent) {
    return (
      <div className="p-4 rounded-lg bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800">
        <div className="flex items-center gap-2 text-green-700 dark:text-green-400 font-medium mb-2">
          <CheckCircle className="w-5 h-5" />
          Excellent Agreement!
        </div>
        <p className="text-sm text-green-600 dark:text-green-400">
          Your annotations are highly reliable with Krippendorff's α = {metrics.krippendorffsAlpha.toFixed(3)}. 
          This exceeds the recommended threshold of 0.8.
        </p>
      </div>
    );
  }
  
  return (
    <div className={`p-4 rounded-lg border ${
      isLowOverallAgreement 
        ? 'bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800'
        : 'bg-yellow-50 dark:bg-yellow-900/20 border-yellow-200 dark:border-yellow-800'
    }`}>
      <div className={`flex items-center gap-2 font-medium mb-3 ${
        isLowOverallAgreement 
          ? 'text-red-700 dark:text-red-400'
          : 'text-yellow-700 dark:text-yellow-400'
      }`}>
        <AlertTriangle className="w-5 h-5" />
        Recommendations
      </div>
      
      <ul className={`text-sm space-y-2 ${
        isLowOverallAgreement 
          ? 'text-red-600 dark:text-red-400'
          : 'text-yellow-600 dark:text-yellow-400'
      }`}>
        {isLowOverallAgreement && (
          <li>
            ⚠ Agreement is below recommended threshold (0.67). Consider:
            <ul className="ml-4 mt-1 space-y-1 list-disc list-inside">
              <li>Improving annotation guidelines with more examples</li>
              <li>Additional annotator training sessions</li>
              <li>Simplifying the rating scale</li>
              <li>Having annotators discuss and resolve disagreements</li>
            </ul>
          </li>
        )}
        
        {lowAgreementAnnotators.length > 0 && (
          <li>
            ⚠ Annotators with low agreement: {lowAgreementAnnotators.map(a => a.name).join(', ')}. 
            Review their annotations or provide additional training.
          </li>
        )}
        
        {!isLowOverallAgreement && lowAgreementAnnotators.length === 0 && (
          <li>
            Good agreement levels. Continue monitoring and consider increasing sample size 
            to narrow confidence intervals.
          </li>
        )}
      </ul>
    </div>
  );
};

// =============================================================================
// MAIN COMPONENT
// =============================================================================

export const AnnotationQuality: React.FC<AnnotationQualityProps> = ({
  metrics,
  annotators,
  pairwiseAgreements,
  hasGroundTruth = false,
  className = '',
}) => {
  const annotatorNames = annotators.map(a => a.name);
  
  return (
    <div className={`space-y-6 ${className}`}>
      {/* Header */}
      <div className="flex items-center gap-2">
        <Users className="w-5 h-5 text-primary" />
        <h3 className="text-lg font-semibold text-textPrimary">Human Annotation Quality Assessment</h3>
      </div>
      
      {/* Overall metrics */}
      <div className="grid grid-cols-3 gap-4">
        <AgreementMetricCard
          title="Krippendorff's Alpha"
          value={metrics.krippendorffsAlpha}
          ci95={metrics.alphaCI95}
          description="Chance-corrected agreement measure that handles missing data and multiple annotators. Recommended threshold: > 0.67 for tentative conclusions, > 0.8 for definitive conclusions."
        />
        <AgreementMetricCard
          title="Fleiss' Kappa"
          value={metrics.fleissKappa}
          description="Multi-rater agreement measure for categorical data. Extends Cohen's Kappa to more than two raters."
        />
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <h4 className="text-sm font-medium text-textSecondary mb-3">Landis-Koch Scale</h4>
          <LandisKochScale value={metrics.krippendorffsAlpha} />
        </div>
      </div>
      
      {/* Pairwise agreement heatmap */}
      <div className="bg-surface-elevated border border-border rounded-lg p-4">
        <h4 className="text-sm font-medium text-textPrimary mb-4">
          Pairwise Annotator Agreement (Cohen's Kappa)
        </h4>
        <PairwiseHeatmap 
          annotators={annotatorNames} 
          agreements={pairwiseAgreements} 
        />
        <p className="text-xs text-textTertiary mt-3">
          Color intensity indicates agreement strength. Hover over cells for details.
        </p>
      </div>
      
      {/* Annotator performance */}
      <div className="bg-surface-elevated border border-border rounded-lg p-4">
        <h4 className="text-sm font-medium text-textPrimary mb-4">
          Individual Annotator Statistics
        </h4>
        <AnnotatorTable annotators={annotators} hasGroundTruth={hasGroundTruth} />
      </div>
      
      {/* Recommendations */}
      <Recommendations metrics={metrics} annotators={annotators} />
    </div>
  );
};

export default AnnotationQuality;
