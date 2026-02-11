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
import { API_BASE_URL } from '../src/lib/agentreplay-api';

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
      bg: 'bg-red-50 dark:bg-red-900/20',
      border: 'border-red-300 dark:border-red-500/40',
      bar: 'bg-red-500',
      text: 'text-red-600 dark:text-red-400',
    },
    green: {
      bg: 'bg-green-50 dark:bg-green-900/20',
      border: 'border-green-300 dark:border-green-500/40',
      bar: 'bg-green-500',
      text: 'text-green-600 dark:text-green-400',
    },
    blue: {
      bg: 'bg-blue-50 dark:bg-blue-900/20',
      border: 'border-blue-300 dark:border-blue-500/40',
      bar: 'bg-blue-500',
      text: 'text-blue-600 dark:text-blue-400',
    },
  };

  const colors = colorClasses[colorScheme];

  return (
    <div className={`p-3 ${colors.bg} border ${colors.border} rounded-lg space-y-2`}>
      <div className="flex justify-between items-center">
        <span className="text-sm font-medium text-foreground">{label}</span>
        <span className={`text-lg font-bold ${colors.text}`}>{percentage}%</span>
      </div>
      <div className="w-full bg-secondary rounded-full h-2">
        <div
          className={`${colors.bar} h-2 rounded-full transition-all duration-300`}
          style={{ width: `${percentage}%` }}
        />
      </div>
      <p className="text-xs text-muted-foreground">{description}</p>
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
        className="relative w-full max-w-4xl max-h-[80vh] overflow-y-auto bg-card border border-border rounded-lg shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="sticky top-0 z-10 bg-card border-b border-border p-6">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-xl font-bold text-foreground">Trace Details</h2>
              <div className="text-sm text-muted-foreground font-mono mt-1">{traceId}</div>
            </div>
            <button
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground text-2xl font-bold"
            >
