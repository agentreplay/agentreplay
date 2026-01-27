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
 * PassRateBar - Horizontal pass rate visualization
 * 
 * Features:
 * - Horizontal bar showing pass/fail ratio
 * - Color-coded segments
 * - Counts and percentage display
 */

import React from 'react';
import { CheckCircle2, XCircle } from 'lucide-react';

// =============================================================================
// TYPES
// =============================================================================

export interface PassRateBarProps {
  /** Number of passed tests */
  passed: number;
  /** Number of failed tests */
  failed: number;
  /** Label for the bar */
  label?: string;
  /** Show counts below the bar */
  showCounts?: boolean;
  /** Height of the bar in pixels */
  height?: number;
  /** Additional CSS classes */
  className?: string;
}

// =============================================================================
// COMPONENT
// =============================================================================

export const PassRateBar: React.FC<PassRateBarProps> = ({
  passed,
  failed,
  label = 'Pass Rate',
  showCounts = true,
  height = 24,
  className = '',
}) => {
  const total = passed + failed;
  const passRate = total > 0 ? passed / total : 0;
  const passPercent = passRate * 100;
  
  // Determine color based on pass rate
  const getPassRateColor = () => {
    if (passPercent >= 90) return 'text-green-600 dark:text-green-400';
    if (passPercent >= 70) return 'text-yellow-600 dark:text-yellow-400';
    return 'text-red-600 dark:text-red-400';
  };
  
  return (
    <div className={`space-y-1 ${className}`}>
      {/* Label row */}
      <div className="flex items-center justify-between text-sm">
        <span className="font-medium text-gray-700 dark:text-gray-300">{label}</span>
        <span className={`font-semibold ${getPassRateColor()}`}>
          {passPercent.toFixed(1)}%
        </span>
      </div>
      
      {/* Bar */}
      <div 
        className="relative w-full bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden"
        style={{ height }}
      >
        {/* Pass portion */}
        <div
          className="absolute left-0 top-0 bottom-0 bg-green-500 transition-all duration-500 ease-out"
          style={{ width: `${passPercent}%` }}
        />
        
        {/* Fail portion is the background */}
        <div
          className="absolute right-0 top-0 bottom-0 bg-red-500 transition-all duration-500 ease-out"
          style={{ width: `${100 - passPercent}%` }}
        />
      </div>
      
      {/* Counts */}
      {showCounts && (
        <div className="flex items-center justify-between text-xs text-gray-600 dark:text-gray-400">
          <div className="flex items-center gap-1">
            <CheckCircle2 className="w-3 h-3 text-green-500" />
            <span>{passed} passed</span>
          </div>
          <div className="flex items-center gap-1">
            <XCircle className="w-3 h-3 text-red-500" />
            <span>{failed} failed</span>
          </div>
        </div>
      )}
    </div>
  );
};

export default PassRateBar;
