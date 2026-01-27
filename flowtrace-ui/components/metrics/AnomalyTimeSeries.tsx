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
 * AnomalyTimeSeries - Time-series visualization with anomaly detection
 * 
 * Features:
 * - STL decomposition view (trend, seasonal, residual)
 * - Control chart limits (EWMA, CUSUM)
 * - Interactive anomaly markers
 * - Configurable detection sensitivity
 */

import React, { useState, useMemo } from 'react';
import { 
  AlertTriangle, 
  CheckCircle, 
  Settings, 
  Search,
  XCircle,
  Activity,
  TrendingUp
} from 'lucide-react';

// =============================================================================
// TYPES
// =============================================================================

export interface TimeSeriesPoint {
  timestamp: number;
  value: number;
  trend?: number;
  seasonal?: number;
  residual?: number;
}

export interface Anomaly {
  id: string;
  timestamp: number;
  value: number;
  expected: number;
  zScore: number;
  type: 'point' | 'contextual' | 'collective';
  severity: 'critical' | 'warning' | 'info';
  investigated?: boolean;
  falsePositive?: boolean;
}

export interface ControlLimits {
  upperLimit: number;
  centerLine: number;
  lowerLimit: number;
}

export interface AnomalyTimeSeriesProps {
  /** Time series data points */
  data: TimeSeriesPoint[];
  /** Detected anomalies */
  anomalies: Anomaly[];
  /** Control chart limits */
  controlLimits: ControlLimits;
  /** Metric name */
  metricName?: string;
  /** Chart dimensions */
  width?: number;
  height?: number;
  /** Show STL decomposition */
  showDecomposition?: boolean;
  /** Callback when anomaly is investigated */
  onInvestigate?: (anomaly: Anomaly) => void;
  /** Callback when anomaly is marked as false positive */
  onMarkFalsePositive?: (anomaly: Anomaly) => void;
  /** Additional classes */
  className?: string;
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

function formatTimestamp(ts: number): string {
  return new Date(ts).toLocaleString();
}

function getSeverityColor(severity: Anomaly['severity']): string {
  switch (severity) {
    case 'critical': return '#ef4444';
    case 'warning': return '#f59e0b';
    case 'info': return '#3b82f6';
  }
}

function getSeverityBadge(severity: Anomaly['severity']): { bg: string; text: string } {
  switch (severity) {
    case 'critical': 
      return { bg: 'bg-red-100 dark:bg-red-900/30', text: 'text-red-800 dark:text-red-400' };
    case 'warning': 
      return { bg: 'bg-yellow-100 dark:bg-yellow-900/30', text: 'text-yellow-800 dark:text-yellow-400' };
    case 'info': 
      return { bg: 'bg-blue-100 dark:bg-blue-900/30', text: 'text-blue-800 dark:text-blue-400' };
  }
}

// =============================================================================
// TIME SERIES CHART
// =============================================================================

interface TimeSeriesChartProps {
  data: TimeSeriesPoint[];
  anomalies: Anomaly[];
  controlLimits: ControlLimits;
  width: number;
  height: number;
  dataKey?: 'value' | 'trend' | 'seasonal' | 'residual';
  title?: string;
  showControlLimits?: boolean;
  showAnomalies?: boolean;
  onAnomalyClick?: (anomaly: Anomaly) => void;
}

const TimeSeriesChart: React.FC<TimeSeriesChartProps> = ({
  data,
  anomalies,
  controlLimits,
  width,
  height,
  dataKey = 'value',
  title,
  showControlLimits = true,
  showAnomalies = true,
  onAnomalyClick,
}) => {
  const padding = { top: 30, right: 20, bottom: 30, left: 60 };
  const chartWidth = width - padding.left - padding.right;
  const chartHeight = height - padding.top - padding.bottom;
  
  // Calculate scales
  const { minX, maxX, minY, maxY } = useMemo(() => {
    const values = data.map(d => d[dataKey] ?? 0);
    const timestamps = data.map(d => d.timestamp);
    
    let minY = Math.min(...values);
    let maxY = Math.max(...values);
    
    // Include control limits in y range
    if (showControlLimits) {
      minY = Math.min(minY, controlLimits.lowerLimit);
      maxY = Math.max(maxY, controlLimits.upperLimit);
    }
    
    // Add padding
    const yPadding = (maxY - minY) * 0.1;
    
    return {
      minX: Math.min(...timestamps),
      maxX: Math.max(...timestamps),
      minY: minY - yPadding,
      maxY: maxY + yPadding,
    };
  }, [data, dataKey, controlLimits, showControlLimits]);
  
  const scaleX = (ts: number) => padding.left + ((ts - minX) / (maxX - minX)) * chartWidth;
  const scaleY = (v: number) => padding.top + (1 - (v - minY) / (maxY - minY)) * chartHeight;
  
  // Build line path
  const linePath = data.length > 0
    ? `M ${data.map(d => `${scaleX(d.timestamp)} ${scaleY(d[dataKey] ?? 0)}`).join(' L ')}`
    : '';
  
  // Y-axis ticks
  const yTicks = [0, 0.25, 0.5, 0.75, 1].map(p => minY + p * (maxY - minY));
  
  return (
    <svg width={width} height={height} className="overflow-visible">
      {/* Title */}
      {title && (
        <text
          x={padding.left}
          y={15}
          className="text-sm font-medium fill-gray-700 dark:fill-gray-300"
        >
          {title}
        </text>
      )}
      
      {/* Grid and Y-axis */}
      {yTicks.map((tick, i) => (
        <g key={i}>
          <line
            x1={padding.left}
            y1={scaleY(tick)}
            x2={width - padding.right}
            y2={scaleY(tick)}
            stroke="currentColor"
            strokeOpacity={0.1}
            className="text-gray-400"
          />
          <text
            x={padding.left - 8}
            y={scaleY(tick)}
            textAnchor="end"
            dominantBaseline="middle"
            className="text-xs fill-gray-500"
          >
            {tick.toFixed(2)}
          </text>
        </g>
      ))}
      
      {/* Control limits */}
      {showControlLimits && (
        <>
          <line
            x1={padding.left}
            y1={scaleY(controlLimits.upperLimit)}
            x2={width - padding.right}
            y2={scaleY(controlLimits.upperLimit)}
            stroke="#ef4444"
            strokeWidth={1}
            strokeDasharray="4,4"
          />
          <text
            x={width - padding.right + 5}
            y={scaleY(controlLimits.upperLimit)}
            dominantBaseline="middle"
            className="text-xs fill-red-500"
          >
            UCL
          </text>
          
          <line
            x1={padding.left}
            y1={scaleY(controlLimits.centerLine)}
            x2={width - padding.right}
            y2={scaleY(controlLimits.centerLine)}
            stroke="#22c55e"
            strokeWidth={1.5}
          />
          <text
            x={width - padding.right + 5}
            y={scaleY(controlLimits.centerLine)}
            dominantBaseline="middle"
            className="text-xs fill-green-500"
          >
            Mean
          </text>
          
          <line
            x1={padding.left}
            y1={scaleY(controlLimits.lowerLimit)}
            x2={width - padding.right}
            y2={scaleY(controlLimits.lowerLimit)}
            stroke="#ef4444"
            strokeWidth={1}
            strokeDasharray="4,4"
          />
          <text
            x={width - padding.right + 5}
            y={scaleY(controlLimits.lowerLimit)}
            dominantBaseline="middle"
            className="text-xs fill-red-500"
          >
            LCL
          </text>
        </>
      )}
      
      {/* Data line */}
      <path
        d={linePath}
        fill="none"
        stroke="#3b82f6"
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      
      {/* Anomaly points */}
      {showAnomalies && anomalies.map((anomaly, i) => (
        <g 
          key={i} 
          className="cursor-pointer"
          onClick={() => onAnomalyClick?.(anomaly)}
        >
          <circle
            cx={scaleX(anomaly.timestamp)}
            cy={scaleY(anomaly.value)}
            r={8}
            fill={getSeverityColor(anomaly.severity)}
            stroke="white"
            strokeWidth={2}
            className="drop-shadow-md hover:r-10 transition-all"
          />
          <title>
            {`Anomaly: ${anomaly.value.toFixed(3)}\nExpected: ${anomaly.expected.toFixed(3)}\nZ-score: ${anomaly.zScore.toFixed(2)}`}
          </title>
        </g>
      ))}
    </svg>
  );
};

// =============================================================================
// ANOMALY TABLE
// =============================================================================

interface AnomalyTableProps {
  anomalies: Anomaly[];
  onInvestigate?: (anomaly: Anomaly) => void;
  onMarkFalsePositive?: (anomaly: Anomaly) => void;
}

const AnomalyTable: React.FC<AnomalyTableProps> = ({
  anomalies,
  onInvestigate,
  onMarkFalsePositive,
}) => {
  const [typeFilter, setTypeFilter] = useState<string>('all');
  const [severityFilter, setSeverityFilter] = useState<string>('all');
  
  const filteredAnomalies = anomalies.filter(a => {
    if (typeFilter !== 'all' && a.type !== typeFilter) return false;
    if (severityFilter !== 'all' && a.severity !== severityFilter) return false;
    return true;
  });
  
  return (
    <div className="space-y-4">
      {/* Filters */}
      <div className="flex items-center gap-4">
        <div>
          <label className="block text-xs text-textSecondary mb-1">Type</label>
          <select
            value={typeFilter}
            onChange={(e) => setTypeFilter(e.target.value)}
            className="px-3 py-1.5 text-sm rounded border border-border bg-background"
          >
            <option value="all">All Types</option>
            <option value="point">Point</option>
            <option value="contextual">Contextual</option>
            <option value="collective">Collective</option>
          </select>
        </div>
        <div>
          <label className="block text-xs text-textSecondary mb-1">Severity</label>
          <select
            value={severityFilter}
            onChange={(e) => setSeverityFilter(e.target.value)}
            className="px-3 py-1.5 text-sm rounded border border-border bg-background"
          >
            <option value="all">All Severities</option>
            <option value="critical">Critical</option>
            <option value="warning">Warning</option>
            <option value="info">Info</option>
          </select>
        </div>
        <div className="ml-auto text-sm text-textSecondary">
          {filteredAnomalies.length} anomalies
        </div>
      </div>
      
      {/* Table */}
      <div className="overflow-x-auto max-h-80">
        <table className="w-full text-sm">
          <thead className="sticky top-0 bg-surface-elevated">
            <tr className="border-b border-border">
              <th className="px-3 py-2 text-left text-textSecondary font-medium">Timestamp</th>
              <th className="px-3 py-2 text-right text-textSecondary font-medium">Value</th>
              <th className="px-3 py-2 text-right text-textSecondary font-medium">Expected</th>
              <th className="px-3 py-2 text-right text-textSecondary font-medium">Deviation</th>
              <th className="px-3 py-2 text-center text-textSecondary font-medium">Type</th>
              <th className="px-3 py-2 text-center text-textSecondary font-medium">Severity</th>
              <th className="px-3 py-2 text-center text-textSecondary font-medium">Actions</th>
            </tr>
          </thead>
          <tbody>
            {filteredAnomalies.map((anomaly, i) => {
              const { bg, text } = getSeverityBadge(anomaly.severity);
              
              return (
                <tr 
                  key={anomaly.id} 
                  className={`border-b border-border/50 hover:bg-surface-hover ${
                    anomaly.falsePositive ? 'opacity-50' : ''
                  }`}
                >
                  <td className="px-3 py-2 font-mono text-textSecondary">
                    {formatTimestamp(anomaly.timestamp)}
                  </td>
                  <td className="px-3 py-2 text-right font-mono text-textPrimary">
                    {anomaly.value.toFixed(3)}
                  </td>
                  <td className="px-3 py-2 text-right font-mono text-textSecondary">
                    {anomaly.expected.toFixed(3)}
                  </td>
                  <td className="px-3 py-2 text-right">
                    <span className={`font-mono font-semibold ${
                      Math.abs(anomaly.zScore) > 3 ? 'text-red-600' : 'text-yellow-600'
                    }`}>
                      {anomaly.zScore.toFixed(2)}σ
                    </span>
                  </td>
                  <td className="px-3 py-2 text-center">
                    <span className="px-2 py-0.5 text-xs bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 rounded capitalize">
                      {anomaly.type}
                    </span>
                  </td>
                  <td className="px-3 py-2 text-center">
                    <span className={`px-2 py-0.5 text-xs rounded capitalize ${bg} ${text}`}>
                      {anomaly.severity}
                    </span>
                  </td>
                  <td className="px-3 py-2 text-center">
                    <div className="flex items-center justify-center gap-1">
                      <button
                        onClick={() => onInvestigate?.(anomaly)}
                        className="p-1 hover:bg-surface-hover rounded text-textSecondary hover:text-primary"
                        title="Investigate"
                      >
                        <Search className="w-4 h-4" />
                      </button>
                      <button
                        onClick={() => onMarkFalsePositive?.(anomaly)}
                        className="p-1 hover:bg-surface-hover rounded text-textSecondary hover:text-yellow-600"
                        title="Mark as False Positive"
                      >
                        <XCircle className="w-4 h-4" />
                      </button>
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        
        {filteredAnomalies.length === 0 && (
          <div className="text-center py-8 text-textSecondary">
            No anomalies match the current filters
          </div>
        )}
      </div>
    </div>
  );
};

// =============================================================================
// DETECTION SETTINGS
// =============================================================================

interface DetectionSettingsProps {
  sensitivity: number;
  onSensitivityChange: (v: number) => void;
  algorithm: string;
  onAlgorithmChange: (v: string) => void;
}

const DetectionSettings: React.FC<DetectionSettingsProps> = ({
  sensitivity,
  onSensitivityChange,
  algorithm,
  onAlgorithmChange,
}) => {
  return (
    <div className="space-y-4 p-4 bg-surface-elevated rounded-lg border border-border">
      <h4 className="text-sm font-medium text-textPrimary flex items-center gap-2">
        <Settings className="w-4 h-4" />
        Detection Settings
      </h4>
      
      <div>
        <label className="block text-sm text-textSecondary mb-2">Algorithm</label>
        <select
          value={algorithm}
          onChange={(e) => onAlgorithmChange(e.target.value)}
          className="w-full px-3 py-2 text-sm rounded border border-border bg-background"
        >
          <option value="stl-residual">STL + Residual Analysis</option>
          <option value="ewma">EWMA Control Chart</option>
          <option value="cusum">CUSUM Control Chart</option>
          <option value="isolation-forest">Isolation Forest</option>
        </select>
      </div>
      
      <div>
        <label className="block text-sm text-textSecondary mb-2">
          Sensitivity (σ multiplier): {sensitivity.toFixed(1)}
        </label>
        <input
          type="range"
          min={2}
          max={6}
          step={0.5}
          value={sensitivity}
          onChange={(e) => onSensitivityChange(parseFloat(e.target.value))}
          className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer dark:bg-gray-700"
        />
        <div className="flex justify-between text-xs text-textTertiary mt-1">
          <span>More sensitive</span>
          <span>Fewer false positives</span>
        </div>
      </div>
    </div>
  );
};

// =============================================================================
// MAIN COMPONENT
// =============================================================================

export const AnomalyTimeSeries: React.FC<AnomalyTimeSeriesProps> = ({
  data,
  anomalies,
  controlLimits,
  metricName = 'Metric',
  width = 800,
  height = 300,
  showDecomposition = true,
  onInvestigate,
  onMarkFalsePositive,
  className = '',
}) => {
  const [sensitivity, setSensitivity] = useState(3);
  const [algorithm, setAlgorithm] = useState('stl-residual');
  const [selectedAnomaly, setSelectedAnomaly] = useState<Anomaly | null>(null);
  
  const decompositionHeight = 120;
  
  // Check if we have decomposition data
  const hasDecomposition = data.some(d => d.trend !== undefined);
  
  // Summary stats
  const criticalCount = anomalies.filter(a => a.severity === 'critical').length;
  const warningCount = anomalies.filter(a => a.severity === 'warning').length;
  const infoCount = anomalies.filter(a => a.severity === 'info').length;
  
  return (
    <div className={`space-y-6 ${className}`}>
      {/* Summary */}
      <div className="flex items-center gap-6">
        <div className="flex items-center gap-2">
          <Activity className="w-5 h-5 text-primary" />
          <span className="text-lg font-semibold text-textPrimary">{metricName} Over Time</span>
        </div>
        
        <div className="flex items-center gap-4 ml-auto">
          {criticalCount > 0 && (
            <span className="flex items-center gap-1 px-2 py-1 rounded bg-red-100 dark:bg-red-900/30 text-red-800 dark:text-red-400 text-sm">
              <AlertTriangle className="w-4 h-4" />
              {criticalCount} Critical
            </span>
          )}
          {warningCount > 0 && (
            <span className="flex items-center gap-1 px-2 py-1 rounded bg-yellow-100 dark:bg-yellow-900/30 text-yellow-800 dark:text-yellow-400 text-sm">
              <AlertTriangle className="w-4 h-4" />
              {warningCount} Warning
            </span>
          )}
          {criticalCount === 0 && warningCount === 0 && (
            <span className="flex items-center gap-1 px-2 py-1 rounded bg-green-100 dark:bg-green-900/30 text-green-800 dark:text-green-400 text-sm">
              <CheckCircle className="w-4 h-4" />
              No Issues
            </span>
          )}
        </div>
      </div>
      
      {/* Main time series chart */}
      <div className="bg-surface-elevated border border-border rounded-lg p-4">
        <TimeSeriesChart
          data={data}
          anomalies={anomalies}
          controlLimits={controlLimits}
          width={width}
          height={height}
          dataKey="value"
          title="Observed Values"
          onAnomalyClick={setSelectedAnomaly}
        />
      </div>
      
      {/* STL Decomposition */}
      {showDecomposition && hasDecomposition && (
        <div className="bg-surface-elevated border border-border rounded-lg p-4">
          <h4 className="text-sm font-medium text-textPrimary mb-4 flex items-center gap-2">
            <TrendingUp className="w-4 h-4" />
            Seasonal-Trend Decomposition (STL)
          </h4>
          
          <div className="space-y-2">
            <TimeSeriesChart
              data={data}
              anomalies={[]}
              controlLimits={controlLimits}
              width={width}
              height={decompositionHeight}
              dataKey="trend"
              title="Trend"
              showControlLimits={false}
              showAnomalies={false}
            />
            <TimeSeriesChart
              data={data}
              anomalies={[]}
              controlLimits={controlLimits}
              width={width}
              height={decompositionHeight}
              dataKey="seasonal"
              title="Seasonal"
              showControlLimits={false}
              showAnomalies={false}
            />
            <TimeSeriesChart
              data={data}
              anomalies={anomalies}
              controlLimits={controlLimits}
              width={width}
              height={decompositionHeight}
              dataKey="residual"
              title="Residual (Anomalies highlighted)"
              showControlLimits={true}
              showAnomalies={true}
              onAnomalyClick={setSelectedAnomaly}
            />
          </div>
        </div>
      )}
      
      {/* Anomaly table and settings */}
      <div className="grid grid-cols-3 gap-6">
        <div className="col-span-2 bg-surface-elevated border border-border rounded-lg p-4">
          <h4 className="text-sm font-medium text-textPrimary mb-4 flex items-center gap-2">
            <AlertTriangle className="w-4 h-4" />
            Detected Anomalies ({anomalies.length})
          </h4>
          <AnomalyTable
            anomalies={anomalies}
            onInvestigate={onInvestigate}
            onMarkFalsePositive={onMarkFalsePositive}
          />
        </div>
        
        <DetectionSettings
          sensitivity={sensitivity}
          onSensitivityChange={setSensitivity}
          algorithm={algorithm}
          onAlgorithmChange={setAlgorithm}
        />
      </div>
    </div>
  );
};

export default AnomalyTimeSeries;
