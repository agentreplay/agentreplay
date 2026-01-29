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

import React, { useState, useEffect } from 'react';

// Mock Recharts since we don't have it installed
// We'll just render basic divs
const ResponsiveContainer = ({ children, height }: any) => <div style={{ height }} className="bg-gray-100 flex items-center justify-center border rounded">{children}</div>;
const RadarChart = () => <div>Radar Chart Placeholder</div>;
const Award = () => <span>🏆</span>;
const CheckCircle = () => <span>✅</span>;
const AlertCircle = () => <span>⚠️</span>;
const TrendingUp = () => <span>📈</span>;
const TrendingDown = () => <span>📉</span>;

// Mock invoke
const invoke = async <T,>(cmd: string, args: any): Promise<T> => {
    console.log(`Invoke ${cmd}`, args);
    return {
        experiment_name: "Comparison of 2 runs",
        compared_runs: ["run_123", "run_456"],
        metrics_compared: ["accuracy", "latency"],
        statistical_tests: [
            {
                metric: "accuracy",
                baseline_run_id: "run_123",
                treatment_run_id: "run_456",
                baseline_stats: { mean: 0.8, std_dev: 0.1, min: 0.6, max: 0.9, median: 0.82, p25: 0.75, p75: 0.85, sample_size: 100 },
                treatment_stats: { mean: 0.85, std_dev: 0.08, min: 0.7, max: 0.95, median: 0.86, p25: 0.8, p75: 0.9, sample_size: 100 },
                test_result: { p_value: 0.04, statistically_significant: true, significant_at_01: false, t_statistic: 2.1, confidence_interval: [0.01, 0.09] as [number, number] },
                effect_size: { percentage_improvement: 6.25, cohens_d: 0.55, interpretation: "Medium" },
                interpretation: "Treatment improved accuracy by 6.25%."
            },
             {
                metric: "latency",
                baseline_run_id: "run_123",
                treatment_run_id: "run_456",
                baseline_stats: { mean: 200, std_dev: 50, min: 100, max: 300, median: 190, p25: 150, p75: 240, sample_size: 100 },
                treatment_stats: { mean: 210, std_dev: 55, min: 110, max: 320, median: 200, p25: 160, p75: 250, sample_size: 100 },
                test_result: { p_value: 0.2, statistically_significant: false, significant_at_01: false, t_statistic: 1.1, confidence_interval: [-10, 30] as [number, number] },
                effect_size: { percentage_improvement: 5.0, cohens_d: 0.2, interpretation: "Small" },
                interpretation: "Treatment latency increased by 5.0% (not significant)."
            }
        ],
        winner: "run_456",
        recommendation: "Recommendation: Deploy run run_456. It shows 1 significant improvements."
    } as T;
};

interface ComparisonReport {
  experiment_name: string;
  compared_runs: string[];
  metrics_compared: string[];
  statistical_tests: StatisticalTest[];
  winner?: string;
  recommendation: string;
}

interface StatisticalTest {
  metric: string;
  baseline_run_id: string;
  treatment_run_id: string;
  baseline_stats: DescriptiveStats;
  treatment_stats: DescriptiveStats;
  test_result: TestResult;
  effect_size: EffectSize;
  interpretation: string;
}

interface DescriptiveStats {
  mean: number;
  std_dev: number;
  min: number;
  max: number;
  median: number;
  p25: number;
  p75: number;
  sample_size: number;
}

interface TestResult {
  t_statistic: number;
  p_value: number;
  confidence_interval: [number, number];
  statistically_significant: boolean;
  significant_at_01: boolean;
}

interface EffectSize {
  cohens_d: number;
  interpretation: 'Negligible' | 'Small' | 'Medium' | 'Large';
  percentage_improvement: number;
}

export function ExperimentComparison({ runIds }: { runIds: string[] }) {
  const [report, setReport] = useState<ComparisonReport | null>(null);
  const [selectedMetric, setSelectedMetric] = useState<string>('');
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadComparison();
  }, [runIds]);

  const loadComparison = async () => {
    setLoading(true);
    const metrics = ['accuracy', 'hallucination', 'relevance', 'latency_ms', 'cost_usd'];
    const result = await invoke<ComparisonReport>('compare_eval_runs', {
      runIds,
      metrics
    });
    setReport(result);
    if (result.metrics_compared.length > 0) {
        setSelectedMetric(result.metrics_compared[0]);
    }
    setLoading(false);
  };

  if (loading || !report) {
    return <div className="p-8 text-center text-gray-500">Loading comparison...</div>;
  }

  // Selected metric detailed comparison
  const selectedTests = report.statistical_tests.filter(t => t.metric === selectedMetric);

  return (
    <div className="p-6 space-y-6 bg-gray-50 min-h-screen">
      {/* Header with winner */}
      <div className="bg-white rounded-xl p-6 border border-purple-200 shadow-sm relative overflow-hidden">
        <div className="absolute top-0 left-0 w-2 h-full bg-purple-500"></div>
        <div className="flex items-start justify-between">
          <div>
            <h1 className="text-2xl font-bold text-gray-800 flex items-center gap-3">
              <Award />
              Experiment Comparison
            </h1>
            <p className="text-gray-500 mt-1">
              Comparing {report.compared_runs.length} evaluation runs across {report.metrics_compared.length} metrics
            </p>
          </div>

          {report.winner && (
            <div className="bg-green-50 border border-green-200 rounded-lg px-4 py-2 text-right">
              <div className="text-xs text-green-600 uppercase tracking-wide font-semibold">Winner</div>
              <div className="text-lg font-bold text-green-700">Run {report.winner.substring(0, 8)}</div>
            </div>
          )}
        </div>

        <div className="mt-4 p-4 bg-purple-50 rounded-lg border border-purple-100">
          <div className="text-sm text-purple-900 font-medium">{report.recommendation}</div>
        </div>
      </div>

      {/* Radar chart placeholder */}
      <div className="bg-white rounded-xl border border-gray-200 p-6 shadow-sm">
        <h2 className="text-lg font-semibold text-gray-800 mb-4">Multi-Dimensional Comparison</h2>
        <ResponsiveContainer height={300}>
          <RadarChart />
        </ResponsiveContainer>
      </div>

      {/* Metric selector */}
      <div className="flex gap-2 flex-wrap">
        {report.metrics_compared.map(metric => (
          <button
            key={metric}
            onClick={() => setSelectedMetric(metric)}
            className={`px-4 py-2 rounded-lg transition-all font-medium text-sm ${
              selectedMetric === metric
                ? 'bg-blue-600 text-white shadow-sm'
                : 'bg-white text-gray-600 border border-gray-200 hover:bg-gray-50'
            }`}
          >
            {metric}
          </button>
        ))}
      </div>

      {/* Detailed comparison for selected metric */}
      {selectedTests.map((test, idx) => (
        <div key={idx} className="bg-white rounded-xl border border-gray-200 p-6 space-y-6 shadow-sm">
          <div className="flex items-start justify-between">
            <div>
              <h3 className="text-lg font-bold text-gray-800 capitalize">
                {test.metric} Comparison
              </h3>
              <p className="text-sm text-gray-500 mt-1">{test.interpretation}</p>
            </div>

            {/* Significance badge */}
            <div className={`flex items-center gap-2 px-3 py-1 rounded-full border ${
              test.test_result.statistically_significant
                ? 'bg-green-50 text-green-700 border-green-200'
                : 'bg-yellow-50 text-yellow-700 border-yellow-200'
            }`}>
              {test.test_result.statistically_significant ? <CheckCircle /> : <AlertCircle />}
              <span className="text-xs font-bold uppercase">
                {test.test_result.statistically_significant ? 'Significant' : 'Not Significant'}
              </span>
            </div>
          </div>

          {/* Visual comparison bars */}
          <div className="grid grid-cols-2 gap-6">
            <div>
              <div className="text-xs text-gray-500 uppercase tracking-wide mb-2 font-semibold">Baseline</div>
              <div className="bg-gray-50 rounded-lg p-4 border border-gray-100">
                <div className="text-3xl font-bold text-gray-800">
                  {test.baseline_stats.mean.toFixed(3)}
                </div>
                <div className="text-xs text-gray-500 mt-1">
                  ± {test.baseline_stats.std_dev.toFixed(3)} SD
                </div>
                <div className="text-xs text-gray-400 mt-2">
                  n = {test.baseline_stats.sample_size}
                </div>
              </div>
            </div>

            <div>
              <div className="text-xs text-gray-500 uppercase tracking-wide mb-2 font-semibold">Treatment</div>
              <div className="bg-gray-50 rounded-lg p-4 border border-gray-100">
                <div className="text-3xl font-bold text-gray-800 flex items-center gap-2">
                  {test.treatment_stats.mean.toFixed(3)}
                  {test.effect_size.percentage_improvement > 0 ? (
                    <span className="text-green-500"><TrendingUp /></span>
                  ) : (
                    <span className="text-red-500"><TrendingDown /></span>
                  )}
                </div>
                <div className={`text-sm font-bold mt-1 ${
                  test.effect_size.percentage_improvement > 0 ? 'text-green-600' : 'text-red-600'
                }`}>
                  {test.effect_size.percentage_improvement > 0 ? '+' : ''}
                  {test.effect_size.percentage_improvement.toFixed(1)}%
                </div>
                <div className="text-xs text-gray-400 mt-2">
                  n = {test.treatment_stats.sample_size}
                </div>
              </div>
            </div>
          </div>

          {/* Statistical details */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div className="bg-gray-50 rounded-lg p-3 border border-gray-100">
              <div className="text-xs text-gray-500 font-medium uppercase">p-value</div>
              <div className={`text-lg font-bold ${
                test.test_result.p_value < 0.05 ? 'text-green-600' : 'text-gray-800'
              }`}>
                {test.test_result.p_value.toFixed(4)}
              </div>
              <div className="text-xs text-gray-400 mt-1">
                {test.test_result.significant_at_01 ? 'p < 0.01' : test.test_result.statistically_significant ? 'p < 0.05' : 'p ≥ 0.05'}
              </div>
            </div>

            <div className="bg-gray-50 rounded-lg p-3 border border-gray-100">
              <div className="text-xs text-gray-500 font-medium uppercase">Effect Size</div>
              <div className="text-lg font-bold text-gray-800">
                d = {test.effect_size.cohens_d.toFixed(2)}
              </div>
              <div className="text-xs text-gray-400 mt-1">
                {test.effect_size.interpretation}
              </div>
            </div>

            <div className="bg-gray-50 rounded-lg p-3 border border-gray-100">
              <div className="text-xs text-gray-500 font-medium uppercase">t-statistic</div>
              <div className="text-lg font-bold text-gray-800">
                {test.test_result.t_statistic.toFixed(2)}
              </div>
            </div>

            <div className="bg-gray-50 rounded-lg p-3 border border-gray-100">
              <div className="text-xs text-gray-500 font-medium uppercase">95% CI</div>
              <div className="text-sm font-mono text-gray-800 mt-1">
                [{test.test_result.confidence_interval[0].toFixed(3)}, {test.test_result.confidence_interval[1].toFixed(3)}]
              </div>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

export default ExperimentComparison;
