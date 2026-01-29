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
 * Advanced Metrics Demo Page
 * 
 * Showcases all the new advanced metric visualization components
 * with sample data and interactive features.
 */

import React, { useState } from 'react';
import { 
  BarChart3, 
  Activity, 
  Target, 
  Users, 
  TrendingUp,
  Gauge,
  LineChart
} from 'lucide-react';

import { EnhancedMetricCard } from '../../components/metrics/EnhancedMetricCard';
import { CalibrationChart, CalibrationBin, CalibrationMetrics } from '../../components/metrics/CalibrationChart';
import { ROCPRCurves, CurvePoint } from '../../components/metrics/ROCPRCurves';
import { StatisticalComparison, VariantStats, StatisticalTestResult } from '../../components/metrics/StatisticalComparison';
import { AnomalyTimeSeries, TimeSeriesPoint, Anomaly, ControlLimits } from '../../components/metrics/AnomalyTimeSeries';
import { AnnotationQuality, AnnotatorStats, PairwiseAgreement, AgreementMetrics } from '../../components/metrics/AnnotationQuality';

// =============================================================================
// SAMPLE DATA
// =============================================================================

// Sample enhanced metric cards
const sampleMetrics = [
  {
    title: 'RAGAS Score',
    value: 0.847,
    confidenceInterval: [0.812, 0.881] as [number, number],
    trend: 'up' as const,
    change: 8.3,
    isSignificant: true,
    sampleSize: 156,
    threshold: 0.7,
    description: 'Harmonic mean of precision, recall, faithfulness, and relevance',
    sparklineData: [0.72, 0.75, 0.78, 0.81, 0.79, 0.82, 0.85, 0.84, 0.847],
    statisticalDetails: {
      mean: 0.847,
      median: 0.853,
      stdDev: 0.089,
      min: 0.612,
      max: 0.982,
      p25: 0.792,
      p75: 0.901,
      p90: 0.934,
      p95: 0.957,
    },
  },
  {
    title: 'Faithfulness',
    value: 0.912,
    confidenceInterval: [0.891, 0.933] as [number, number],
    trend: 'stable' as const,
    change: 1.2,
    isSignificant: false,
    sampleSize: 156,
    threshold: 0.8,
    description: 'Measures factual consistency with retrieved context',
    sparklineData: [0.89, 0.90, 0.91, 0.90, 0.91, 0.92, 0.91, 0.912],
    statisticalDetails: {
      mean: 0.912,
      median: 0.921,
      stdDev: 0.054,
      min: 0.756,
      max: 0.998,
    },
  },
  {
    title: 'Latency P95',
    value: 1245,
    unit: 'ms',
    confidenceInterval: [1180, 1310] as [number, number],
    trend: 'down' as const,
    change: -12.5,
    isSignificant: true,
    sampleSize: 1024,
    threshold: 2000,
    description: '95th percentile response latency',
    asPercentage: false,
    sparklineData: [1420, 1380, 1350, 1290, 1320, 1280, 1260, 1245],
    statisticalDetails: {
      mean: 1145,
      median: 1089,
      stdDev: 312,
      min: 423,
      max: 2891,
      p90: 1180,
      p95: 1245,
      p99: 1567,
    },
  },
];

// Sample calibration data
const sampleCalibrationBins: CalibrationBin[] = [
  { binIndex: 0, predicted: 0.05, observed: 0.062, count: 128, gap: 0.012 },
  { binIndex: 1, predicted: 0.15, observed: 0.148, count: 95, gap: 0.002 },
  { binIndex: 2, predicted: 0.25, observed: 0.231, count: 112, gap: 0.019 },
  { binIndex: 3, predicted: 0.35, observed: 0.372, count: 87, gap: 0.022 },
  { binIndex: 4, predicted: 0.45, observed: 0.438, count: 103, gap: 0.012 },
  { binIndex: 5, predicted: 0.55, observed: 0.567, count: 98, gap: 0.017 },
  { binIndex: 6, predicted: 0.65, observed: 0.712, count: 76, gap: 0.062 },
  { binIndex: 7, predicted: 0.75, observed: 0.839, count: 89, gap: 0.089 },
  { binIndex: 8, predicted: 0.85, observed: 0.867, count: 134, gap: 0.017 },
  { binIndex: 9, predicted: 0.95, observed: 0.923, count: 78, gap: 0.027 },
];

const sampleCalibrationMetrics: CalibrationMetrics = {
  brierScore: 0.089,
  ece: 0.043,
  mce: 0.089,
};

// Sample ROC/PR curve data
const sampleCurvePoints: CurvePoint[] = Array.from({ length: 20 }, (_, i) => {
  const threshold = i / 19;
  const tpr = Math.pow(1 - threshold, 0.7);
  const fpr = Math.pow(1 - threshold, 2.5);
  const precision = tpr / (tpr + fpr * 2 + 0.01);
  const recall = tpr;
  const f1 = 2 * precision * recall / (precision + recall + 0.001);
  return { threshold, tpr, fpr, precision, recall, f1 };
});

// Sample A/B test data
const sampleBaseline: VariantStats = {
  name: 'GPT-4 (Baseline)',
  mean: 0.782,
  stdDev: 0.089,
  n: 156,
  ci95: [0.768, 0.796],
  passRate: 0.71,
};

const sampleTreatment: VariantStats = {
  name: 'GPT-4 + RAG',
  mean: 0.847,
  stdDev: 0.076,
  n: 143,
  ci95: [0.834, 0.860],
  passRate: 0.85,
};

const sampleTestResult: StatisticalTestResult = {
  tStatistic: 6.82,
  degreesOfFreedom: 285.4,
  pValue: 0.00001,
  difference: 0.065,
  differenceCI: [0.042, 0.088],
  cohensD: 0.81,
  achievedPower: 0.997,
};

// Sample time series data
const now = Date.now();
const sampleTimeSeriesData: TimeSeriesPoint[] = Array.from({ length: 50 }, (_, i) => {
  const timestamp = now - (49 - i) * 3600000;
  const trend = 0.8 + i * 0.002;
  const seasonal = 0.05 * Math.sin(i * Math.PI / 12);
  const noise = (Math.random() - 0.5) * 0.1;
  const value = trend + seasonal + noise;
  const residual = noise;
  return { timestamp, value, trend, seasonal, residual };
});

const sampleAnomalies: Anomaly[] = [
  { id: '1', timestamp: now - 35 * 3600000, value: 0.52, expected: 0.82, zScore: -3.2, type: 'point', severity: 'critical', investigated: false, falsePositive: false },
  { id: '2', timestamp: now - 22 * 3600000, value: 0.95, expected: 0.84, zScore: 2.8, type: 'point', severity: 'warning', investigated: false, falsePositive: false },
  { id: '3', timestamp: now - 8 * 3600000, value: 0.98, expected: 0.86, zScore: 2.5, type: 'contextual', severity: 'info', investigated: true, falsePositive: false },
];

const sampleControlLimits: ControlLimits = {
  upperLimit: 0.95,
  centerLine: 0.82,
  lowerLimit: 0.65,
};

// Sample annotation data
const sampleAnnotators: AnnotatorStats[] = [
  { id: '1', name: 'Alice', annotationCount: 245, avgAgreement: 0.89, controversialCount: 5, accuracy: 0.92 },
  { id: '2', name: 'Bob', annotationCount: 198, avgAgreement: 0.84, controversialCount: 8, accuracy: 0.88 },
  { id: '3', name: 'Carol', annotationCount: 267, avgAgreement: 0.91, controversialCount: 3, accuracy: 0.94 },
  { id: '4', name: 'David', annotationCount: 156, avgAgreement: 0.52, controversialCount: 24, accuracy: 0.71 },
  { id: '5', name: 'Eve', annotationCount: 212, avgAgreement: 0.87, controversialCount: 6, accuracy: 0.89 },
];

const samplePairwiseAgreements: PairwiseAgreement[] = [
  { annotator1: 'Alice', annotator2: 'Bob', kappa: 0.82, agreement: 0.89, count: 145 },
  { annotator1: 'Alice', annotator2: 'Carol', kappa: 0.91, agreement: 0.94, count: 178 },
  { annotator1: 'Alice', annotator2: 'David', kappa: 0.48, agreement: 0.67, count: 102 },
  { annotator1: 'Alice', annotator2: 'Eve', kappa: 0.85, agreement: 0.90, count: 156 },
  { annotator1: 'Bob', annotator2: 'Carol', kappa: 0.79, agreement: 0.86, count: 134 },
  { annotator1: 'Bob', annotator2: 'David', kappa: 0.45, agreement: 0.64, count: 98 },
  { annotator1: 'Bob', annotator2: 'Eve', kappa: 0.81, agreement: 0.88, count: 142 },
  { annotator1: 'Carol', annotator2: 'David', kappa: 0.52, agreement: 0.69, count: 112 },
  { annotator1: 'Carol', annotator2: 'Eve', kappa: 0.88, agreement: 0.92, count: 167 },
  { annotator1: 'David', annotator2: 'Eve', kappa: 0.49, agreement: 0.66, count: 89 },
];

const sampleAgreementMetrics: AgreementMetrics = {
  krippendorffsAlpha: 0.723,
  alphaCI95: [0.691, 0.755],
  fleissKappa: 0.698,
  percentAgreement: 0.81,
};

// =============================================================================
// MAIN PAGE COMPONENT
// =============================================================================

export default function MetricsDemoPage() {
  const [activeTab, setActiveTab] = useState<'overview' | 'calibration' | 'classification' | 'comparison' | 'anomaly' | 'annotation'>('overview');

  const tabs = [
    { id: 'overview', label: 'Overview', icon: Gauge },
    { id: 'calibration', label: 'Calibration', icon: Target },
    { id: 'classification', label: 'Classification', icon: BarChart3 },
    { id: 'comparison', label: 'A/B Comparison', icon: TrendingUp },
    { id: 'anomaly', label: 'Anomaly Detection', icon: Activity },
    { id: 'annotation', label: 'Annotation Quality', icon: Users },
  ];

  return (
    <div className="min-h-screen bg-background">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {/* Header */}
        <div className="mb-8">
          <h1 className="text-3xl font-bold text-textPrimary mb-2">Advanced Metrics Dashboard</h1>
          <p className="text-textSecondary">
            Comprehensive statistical analysis and visualization for LLM evaluations
          </p>
        </div>

        {/* Tabs */}
        <div className="flex flex-wrap gap-2 mb-8 border-b border-border pb-4">
          {tabs.map((tab) => {
            const Icon = tab.icon;
            return (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id as typeof activeTab)}
                className={`flex items-center gap-2 px-4 py-2 rounded-lg font-medium transition-all ${
                  activeTab === tab.id
                    ? 'bg-primary text-white shadow-md'
                    : 'bg-surface-elevated text-textSecondary hover:bg-surface-hover hover:text-textPrimary'
                }`}
              >
                <Icon className="w-4 h-4" />
                {tab.label}
              </button>
            );
          })}
        </div>

        {/* Content */}
        {activeTab === 'overview' && (
          <div className="space-y-8">
            <div>
              <h2 className="text-xl font-semibold text-textPrimary mb-4">
                Enhanced Metric Cards with Confidence Intervals
              </h2>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                {sampleMetrics.map((metric, i) => (
                  <EnhancedMetricCard
                    key={i}
                    {...metric}
                    icon={i === 0 ? BarChart3 : i === 1 ? Target : Activity}
                    asPercentage={i !== 2}
                  />
                ))}
              </div>
            </div>
            
            <div className="bg-surface-elevated border border-border rounded-xl p-6">
              <h3 className="text-lg font-semibold text-textPrimary mb-4">Features Demonstrated</h3>
              <ul className="grid grid-cols-1 md:grid-cols-2 gap-3 text-sm text-textSecondary">
                <li className="flex items-center gap-2">
                  <span className="w-2 h-2 bg-green-500 rounded-full"></span>
                  Bootstrap 95% Confidence Intervals
                </li>
                <li className="flex items-center gap-2">
                  <span className="w-2 h-2 bg-blue-500 rounded-full"></span>
                  Reliability badges based on CI width
                </li>
                <li className="flex items-center gap-2">
                  <span className="w-2 h-2 bg-purple-500 rounded-full"></span>
                  Statistical significance indicators
                </li>
                <li className="flex items-center gap-2">
                  <span className="w-2 h-2 bg-yellow-500 rounded-full"></span>
                  Expandable statistical details
                </li>
                <li className="flex items-center gap-2">
                  <span className="w-2 h-2 bg-red-500 rounded-full"></span>
                  Threshold pass/fail visualization
                </li>
                <li className="flex items-center gap-2">
                  <span className="w-2 h-2 bg-cyan-500 rounded-full"></span>
                  Trend sparklines
                </li>
              </ul>
            </div>
          </div>
        )}

        {activeTab === 'calibration' && (
          <div>
            <h2 className="text-xl font-semibold text-textPrimary mb-4">
              Model Calibration Analysis
            </h2>
            <CalibrationChart
              bins={sampleCalibrationBins}
              metrics={sampleCalibrationMetrics}
              width={500}
              height={400}
              showTable
            />
          </div>
        )}

        {activeTab === 'classification' && (
          <div>
            <h2 className="text-xl font-semibold text-textPrimary mb-4">
              Binary Classification Metrics
            </h2>
            <ROCPRCurves
              curvePoints={sampleCurvePoints}
              auroc={0.912}
              auprc={0.887}
              baseRate={0.35}
              mcc={0.72}
              totalSamples={1000}
              chartWidth={400}
              chartHeight={320}
              showCostBenefit
            />
          </div>
        )}

        {activeTab === 'comparison' && (
          <div>
            <h2 className="text-xl font-semibold text-textPrimary mb-4">
              A/B Test Statistical Comparison
            </h2>
            <StatisticalComparison
              baseline={sampleBaseline}
              treatment={sampleTreatment}
              testResult={sampleTestResult}
              alpha={0.05}
              mdes={0.05}
            />
          </div>
        )}

        {activeTab === 'anomaly' && (
          <div>
            <h2 className="text-xl font-semibold text-textPrimary mb-4">
              Time-Series Anomaly Detection
            </h2>
            <AnomalyTimeSeries
              data={sampleTimeSeriesData}
              anomalies={sampleAnomalies}
              controlLimits={sampleControlLimits}
              metricName="RAGAS Score"
              width={900}
              height={280}
              showDecomposition
              onInvestigate={(a) => console.log('Investigate:', a)}
              onMarkFalsePositive={(a) => console.log('False positive:', a)}
            />
          </div>
        )}

        {activeTab === 'annotation' && (
          <div>
            <h2 className="text-xl font-semibold text-textPrimary mb-4">
              Human Annotation Quality
            </h2>
            <AnnotationQuality
              metrics={sampleAgreementMetrics}
              annotators={sampleAnnotators}
              pairwiseAgreements={samplePairwiseAgreements}
              hasGroundTruth
            />
          </div>
        )}
      </div>
    </div>
  );
}
