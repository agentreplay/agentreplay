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

'use client';

import { useEffect, useState } from 'react';
import { API_BASE_URL } from '../src/lib/flowtrace-api';

interface TraceAttributes {
  [key: string]: string | number;
}

interface TraceDetailModalProps {
  traceId: string | null;
  onClose: () => void;
}

// Evaluation Metric Card Component
interface EvalMetricCardProps {
  label: string;
  value: number;
  description: string;
  colorScheme: 'red' | 'green' | 'blue';
}

const EvalMetricCard: React.FC<EvalMetricCardProps> = ({
  label,
  value,
  description,
  colorScheme,
}) => {
  const normalizedValue = Math.max(0, Math.min(1, value));
  const percentage = (normalizedValue * 100).toFixed(1);

  const colorClasses = {
    red: {
      bg: 'bg-red-900/20',
      border: 'border-red-500/40',
      bar: 'bg-red-500',
      text: 'text-red-400',
    },
    green: {
      bg: 'bg-green-900/20',
      border: 'border-green-500/40',
      bar: 'bg-green-500',
      text: 'text-green-400',
    },
    blue: {
      bg: 'bg-blue-900/20',
      border: 'border-blue-500/40',
      bar: 'bg-blue-500',
      text: 'text-blue-400',
    },
  };

  const colors = colorClasses[colorScheme];

  return (
    <div className={`p-3 ${colors.bg} border ${colors.border} rounded-lg space-y-2`}>
      <div className="flex justify-between items-center">
        <span className="text-sm font-medium text-gray-200">{label}</span>
        <span className={`text-lg font-bold ${colors.text}`}>{percentage}%</span>
      </div>
      <div className="w-full bg-gray-800 rounded-full h-2">
        <div
          className={`${colors.bar} h-2 rounded-full transition-all duration-300`}
          style={{ width: `${percentage}%` }}
        />
      </div>
      <p className="text-xs text-gray-400">{description}</p>
    </div>
  );
};

export function TraceDetailModal({ traceId, onClose }: TraceDetailModalProps) {
  const [attributes, setAttributes] = useState<TraceAttributes | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!traceId) {
      setAttributes(null);
      return;
    }

    setLoading(true);
    setError(null);

    fetch(`${API_BASE_URL}/api/v1/traces/${traceId}/attributes`)
      .then((res) => {
        if (!res.ok) {
          throw new Error(`HTTP ${res.status}: ${res.statusText}`);
        }
        return res.json();
      })
      .then((data) => {
        setAttributes(data);
        setLoading(false);
      })
      .catch((err) => {
        setError(err.message);
        setLoading(false);
      });
  }, [traceId]);

  if (!traceId) {
    return null;
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70"
      onClick={onClose}
    >
      <div
        className="relative w-full max-w-4xl max-h-[80vh] overflow-y-auto bg-gray-900 border border-gray-700 rounded-lg shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="sticky top-0 z-10 bg-gray-900 border-b border-gray-700 p-6">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-xl font-bold text-gray-100">Trace Details</h2>
              <div className="text-sm text-gray-400 font-mono mt-1">{traceId}</div>
            </div>
            <button
              onClick={onClose}
              className="text-gray-400 hover:text-gray-100 text-2xl font-bold"
            >
              ×
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="p-6">
          {loading && (
            <div className="py-8 text-center text-gray-400">
              Loading attributes...
            </div>
          )}

          {error && (
            <div className="p-4 bg-red-900/20 border border-red-500 rounded text-red-300">
              Error: {error}
            </div>
          )}

          {!loading && !error && attributes && Object.keys(attributes).length > 0 && (
            <div className="space-y-6">
              {/* Prompt Section */}
              {attributes.prompt && (
                <div className="space-y-2">
                  <h3 className="text-sm font-semibold text-gray-300 uppercase tracking-wide">
                    Prompt
                  </h3>
                  <div className="p-4 bg-blue-900/10 border border-blue-500/30 rounded-lg">
                    <pre className="text-sm text-gray-200 whitespace-pre-wrap font-sans">
                      {attributes.prompt}
                    </pre>
                  </div>
                </div>
              )}

              {/* Response Section */}
              {attributes.response && (
                <div className="space-y-2">
                  <h3 className="text-sm font-semibold text-gray-300 uppercase tracking-wide">
                    Response
                  </h3>
                  <div className="p-4 bg-green-900/10 border border-green-500/30 rounded-lg">
                    <pre className="text-sm text-gray-200 whitespace-pre-wrap font-sans">
                      {attributes.response}
                    </pre>
                  </div>
                </div>
              )}

              {/* Evaluation Metrics Section */}
              {(attributes.eval_hallucination !== undefined ||
                attributes.eval_relevance !== undefined ||
                attributes.eval_groundedness !== undefined ||
                attributes.eval_toxicity !== undefined) && (
                <div className="space-y-2">
                  <h3 className="text-sm font-semibold text-gray-300 uppercase tracking-wide">
                    Evaluation Metrics
                  </h3>
                  <div className="grid grid-cols-2 gap-3">
                    {attributes.eval_hallucination !== undefined && (
                      <EvalMetricCard
                        label="Hallucination"
                        value={Number(attributes.eval_hallucination)}
                        description="Lower is better"
                        colorScheme="red"
                      />
                    )}
                    {attributes.eval_relevance !== undefined && (
                      <EvalMetricCard
                        label="Relevance"
                        value={Number(attributes.eval_relevance)}
                        description="Higher is better"
                        colorScheme="green"
                      />
                    )}
                    {attributes.eval_groundedness !== undefined && (
                      <EvalMetricCard
                        label="Groundedness"
                        value={Number(attributes.eval_groundedness)}
                        description="Higher is better"
                        colorScheme="blue"
                      />
                    )}
                    {attributes.eval_toxicity !== undefined && (
                      <EvalMetricCard
                        label="Toxicity"
                        value={Number(attributes.eval_toxicity)}
                        description="Lower is better"
                        colorScheme="red"
                      />
                    )}
                  </div>
                </div>
              )}

              {/* Metadata Section */}
              <div className="space-y-2">
                <h3 className="text-sm font-semibold text-gray-300 uppercase tracking-wide">
                  Metadata
                </h3>
                <div className="grid grid-cols-2 gap-3">
                  {Object.entries(attributes)
                    .filter(([key]) => key !== 'prompt' && key !== 'response')
                    .map(([key, value]) => (
                      <div
                        key={key}
                        className="p-3 bg-gray-800/50 border border-gray-700 rounded"
                      >
                        <div className="text-xs text-gray-400 uppercase tracking-wide mb-1">
                          {key.replace(/_/g, ' ')}
                        </div>
                        <div className="text-sm text-gray-100 font-mono">
                          {String(value)}
                        </div>
                      </div>
                    ))}
                </div>
              </div>

              {/* Raw JSON */}
              <details className="mt-4">
                <summary className="text-sm font-semibold text-gray-300 cursor-pointer hover:text-gray-100">
                  View Raw JSON
                </summary>
                <pre className="mt-2 p-4 bg-gray-900 border border-gray-700 rounded text-xs text-gray-300 overflow-x-auto">
                  {JSON.stringify(attributes, null, 2)}
                </pre>
              </details>
            </div>
          )}

          {!loading && !error && attributes && Object.keys(attributes).length === 0 && (
            <div className="py-8 text-center text-gray-400">
              No attributes available for this trace
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
