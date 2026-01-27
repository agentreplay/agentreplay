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
 * Metrics Visualization Components
 * 
 * Provides React components for beautiful, intuitive metric visualization:
 * - MetricGauge: Circular gauge for single metrics
 * - MetricsGrid: Grid layout for multiple metrics by category
 * - ComparisonChart: A/B test comparison table
 * - PassRateBar: Horizontal pass rate visualization
 * - EnhancedMetricCard: Advanced metric card with confidence intervals
 * - ConfidenceInterval: Bootstrap CI visualization
 * - CalibrationChart: Reliability diagrams for model calibration
 * - ROCPRCurves: Interactive ROC/PR curves with threshold selection
 * - StatisticalComparison: Advanced A/B test with statistical tests
 * - AnomalyTimeSeries: Time-series with anomaly detection
 * - AnnotationQuality: Inter-rater reliability dashboard
 */

export { MetricGauge, MiniGauge } from './MetricGauge';
export type { MetricGaugeProps, MiniGaugeProps } from './MetricGauge';

export { MetricsGrid } from './MetricsGrid';
export type { MetricsGridProps, MetricData, MetricCategory } from './MetricsGrid';

export { ComparisonChart } from './ComparisonChart';
export type { ComparisonChartProps, MetricComparison } from './ComparisonChart';

export { PassRateBar } from './PassRateBar';
export type { PassRateBarProps } from './PassRateBar';

// New advanced metric components
export { 
  ConfidenceInterval, 
  ConfidenceBar, 
  ReliabilityBadge, 
  DifferenceCI 
} from './ConfidenceInterval';
export type { 
  ConfidenceIntervalProps, 
  ConfidenceBarProps, 
  ReliabilityBadgeProps,
  DifferenceCIProps 
} from './ConfidenceInterval';

export { EnhancedMetricCard } from './EnhancedMetricCard';
export type { EnhancedMetricCardProps } from './EnhancedMetricCard';

export { CalibrationChart } from './CalibrationChart';
export type { 
  CalibrationChartProps, 
  CalibrationBin, 
  CalibrationMetrics 
} from './CalibrationChart';

export { ROCPRCurves } from './ROCPRCurves';
export type { 
  ROCPRCurvesProps, 
  CurvePoint, 
  ConfusionMatrixData,
  OptimalThreshold 
} from './ROCPRCurves';

export { StatisticalComparison } from './StatisticalComparison';
export type { 
  StatisticalComparisonProps, 
  VariantStats, 
  StatisticalTestResult 
} from './StatisticalComparison';

export { AnomalyTimeSeries } from './AnomalyTimeSeries';
export type { 
  AnomalyTimeSeriesProps, 
  TimeSeriesPoint, 
  Anomaly, 
  ControlLimits 
} from './AnomalyTimeSeries';

export { AnnotationQuality } from './AnnotationQuality';
export type { 
  AnnotationQualityProps, 
  AnnotatorStats, 
  PairwiseAgreement, 
  AgreementMetrics 
} from './AnnotationQuality';
