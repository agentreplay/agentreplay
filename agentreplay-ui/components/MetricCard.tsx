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

import { motion } from 'framer-motion';
import { LucideIcon, TrendingDown, TrendingUp } from 'lucide-react';
import { Sparklines, SparklinesLine } from 'react-sparklines';

export interface MetricCardProps {
  title: string;
  value: string | number;
  unit?: string;
  change?: number;
  trend?: 'up' | 'down' | 'neutral';
  icon: LucideIcon;
  sparklineData?: number[];
  onClick?: () => void;
  className?: string;
}

export function MetricCard({
  title,
  value,
  unit,
  change,
  trend = 'neutral',
  icon: Icon,
  sparklineData,
  onClick,
  className = '',
}: MetricCardProps) {
  const isPositive = trend === 'up' && change !== undefined && change > 0;
  const isNegative = trend === 'down' && change !== undefined && change < 0;
  const isBetter =
    (trend === 'up' && change !== undefined && change > 0) ||
    (trend === 'down' && change !== undefined && change < 0);

  return (
    <motion.div
      whileHover={{ y: -4, scale: 1.01 }}
      transition={{ duration: 0.2 }}
      onClick={onClick}
      className={`
        relative overflow-hidden
        rounded-xl
        p-6
        border
        shadow-lg
        cursor-pointer
        group
        ${className}
      `}
      style={{
        backgroundColor: 'var(--color-surface)',
        borderColor: 'var(--color-border)',
      }}
    >
      <div
        className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-300"
        style={{
          background:
            'linear-gradient(135deg, rgba(99, 102, 241, 0.05) 0%, rgba(6, 182, 212, 0.05) 100%)',
        }}
      />

      <div className="relative z-10">
        <div className="flex items-start justify-between mb-4">
          <div
            className="text-sm font-medium"
            style={{ color: 'var(--color-text-secondary)' }}
          >
            {title}
          </div>
          <div
            className="p-2 rounded-lg"
            style={{
              backgroundColor: 'rgba(99, 102, 241, 0.1)',
              color: 'var(--color-primary)',
            }}
          >
            <Icon size={20} />
          </div>
        </div>

        <div className="flex items-baseline gap-2 mb-3">
          <div
            className="text-4xl font-bold"
            style={{ color: 'var(--color-text-primary)' }}
          >
            {value}
          </div>
          {unit && (
            <div
              className="text-xl font-normal"
              style={{ color: 'var(--color-text-tertiary)' }}
            >
              {unit}
            </div>
          )}
        </div>

        {change !== undefined && Number.isFinite(change) && (
          <div className="flex items-center gap-2">
            <div
              className={`
                flex items-center gap-1 px-2 py-1 rounded-full text-xs font-semibold
                ${
                  isBetter
                    ? 'text-green-600 dark:text-green-400'
                    : !isBetter && change !== 0
                    ? 'text-red-600 dark:text-red-400'
                    : 'text-gray-400'
                }
              `}
              style={{
                backgroundColor: isBetter
                  ? 'rgba(16, 185, 129, 0.1)'
                  : !isBetter && change !== 0
                  ? 'rgba(239, 68, 68, 0.1)'
                  : 'rgba(148, 163, 184, 0.1)',
              }}
            >
              {isPositive && <TrendingUp size={12} />}
              {isNegative && <TrendingDown size={12} />}
              {Math.abs(change).toFixed(1)}%
            </div>
            <span
              className="text-xs"
              style={{ color: 'var(--color-text-tertiary)' }}
            >
              vs previous period
            </span>
          </div>
        )}

        {sparklineData && sparklineData.length > 1 && (
          <div className="mt-4">
            <Sparkline
              data={sparklineData}
              color={isBetter ? 'rgb(16, 185, 129)' : 'rgb(99, 102, 241)'}
            />
          </div>
        )}
      </div>
    </motion.div>
  );
}

interface SparklineProps {
  data: number[];
  color?: string;
  fillColor?: string;
}

function Sparkline({
  data,
  color = 'rgb(99, 102, 241)',
  fillColor,
}: SparklineProps) {
  const cleanData = data.filter((value) => Number.isFinite(value));
  if (cleanData.length === 0) {
    return null;
  }

  const fill = fillColor || `${color}33`;

  return (
    <Sparklines data={cleanData} height={48}>
      <SparklinesLine
        color={color}
        style={{
          strokeWidth: 2,
          fill,
        }}
      />
    </Sparklines>
  );
}

export function SimpleSparkline({
  data,
  color = 'rgb(99, 102, 241)',
}: SparklineProps) {
  if (data.length < 2) {
    return null;
  }

  const cleanData = data.filter((value) => Number.isFinite(value));
  if (cleanData.length < 2) {
    return null;
  }

  const width = 200;
  const height = 48;
  const padding = 2;

  const max = Math.max(...cleanData);
  const min = Math.min(...cleanData);
  const range = max - min || 1;

  const points = cleanData.map((value, index) => {
    const x =
      (index / (cleanData.length - 1)) * (width - padding * 2) + padding;
    const y =
      height -
      padding -
      ((value - min) / range) * (height - padding * 2);
    return `${x},${y}`;
  });

  const pathData = `M ${points.join(' L ')}`;
  const areaData = `${pathData} L ${width - padding},${height} L ${padding},${height} Z`;

  return (
    <svg width={width} height={height} style={{ width: '100%', height: '100%' }}>
      <path d={areaData} fill={`${color}33`} stroke="none" />
      <path
        d={pathData}
        fill="none"
        stroke={color}
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
