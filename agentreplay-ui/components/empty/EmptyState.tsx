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

"use client";

import React from 'react';
import { 
  FileSearch, 
  Activity, 
  Users, 
  AlertCircle, 
  DollarSign,
  Sparkles,
  ArrowRight,
  Play
} from 'lucide-react';

export interface EmptyStateProps {
  variant: 'dashboard' | 'traces' | 'sessions' | 'errors' | 'costs' | 'search';
  title?: string;
  description?: string;
  action?: {
    label: string;
    onClick: () => void;
  };
  secondaryAction?: {
    label: string;
    onClick: () => void;
  };
}

const VARIANT_CONFIG = {
  dashboard: {
    icon: Activity,
    iconColor: 'text-blue-600',
    iconBg: 'bg-blue-100',
    defaultTitle: 'Welcome to AgentReplay',
    defaultDescription: 'Start observing your LLM applications by sending your first trace. Install the SDK and configure your application to begin.',
    defaultAction: 'View Documentation',
    secondaryActionLabel: 'Quick Start Guide',
  },
  traces: {
    icon: FileSearch,
    iconColor: 'text-gray-600',
    iconBg: 'bg-gray-100',
    defaultTitle: 'No traces found',
    defaultDescription: 'There are no traces matching your filters. Try adjusting your search criteria or time range.',
    defaultAction: 'Clear Filters',
    secondaryActionLabel: 'View All Traces',
  },
  sessions: {
    icon: Users,
    iconColor: 'text-purple-600',
    iconBg: 'bg-purple-100',
    defaultTitle: 'No sessions yet',
    defaultDescription: 'Sessions group related traces together. Start sending traces with session IDs to see them here.',
    defaultAction: 'Learn About Sessions',
    secondaryActionLabel: null,
  },
  errors: {
    icon: AlertCircle,
    iconColor: 'text-red-600',
    iconBg: 'bg-red-100',
    defaultTitle: 'No errors found',
    defaultDescription: "Great news! There are no errors in the selected time range. Your LLM applications are running smoothly.",
    defaultAction: 'View All Traces',
    secondaryActionLabel: null,
  },
  costs: {
    icon: DollarSign,
    iconColor: 'text-green-600',
    iconBg: 'bg-green-100',
    defaultTitle: 'No cost data available',
    defaultDescription: 'Cost tracking requires traces with token usage information. Send traces from your LLM calls to see cost analytics.',
    defaultAction: 'Setup Cost Tracking',
    secondaryActionLabel: 'View Supported Models',
  },
  search: {
    icon: FileSearch,
    iconColor: 'text-gray-600',
    iconBg: 'bg-gray-100',
    defaultTitle: 'No results found',
    defaultDescription: 'We couldn\'t find any traces matching your search query. Try different keywords or filters.',
    defaultAction: 'Clear Search',
    secondaryActionLabel: null,
  },
};

export function EmptyState({
  variant,
  title,
  description,
  action,
  secondaryAction,
}: EmptyStateProps) {
  const config = VARIANT_CONFIG[variant];
  const Icon = config.icon;

  return (
    <div className="flex items-center justify-center min-h-[400px] p-8">
      <div className="max-w-md w-full text-center">
        {/* Icon */}
        <div className={`inline-flex items-center justify-center w-16 h-16 rounded-full ${config.iconBg} mb-4`}>
          <Icon className={`w-8 h-8 ${config.iconColor}`} />
        </div>

        {/* Title */}
        <h3 className="text-xl font-semibold text-gray-900 mb-2">
          {title || config.defaultTitle}
        </h3>

        {/* Description */}
        <p className="text-gray-600 mb-6">
          {description || config.defaultDescription}
        </p>

        {/* Actions */}
        <div className="flex flex-col sm:flex-row gap-3 justify-center">
          {action && (
            <button
              onClick={action.onClick}
              className="inline-flex items-center justify-center gap-2 px-5 py-2.5 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-colors"
            >
              {action.label}
              <ArrowRight className="w-4 h-4" />
            </button>
          )}
          
          {!action && config.defaultAction && (
            <button
              onClick={() => {}}
              className="inline-flex items-center justify-center gap-2 px-5 py-2.5 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-colors"
            >
              {config.defaultAction}
              <ArrowRight className="w-4 h-4" />
            </button>
          )}

          {secondaryAction && (
            <button
              onClick={secondaryAction.onClick}
              className="inline-flex items-center justify-center gap-2 px-5 py-2.5 bg-white hover:bg-gray-50 text-gray-700 border border-gray-300 rounded-lg font-medium transition-colors"
            >
              {secondaryAction.label}
            </button>
          )}
          
          {!secondaryAction && config.secondaryActionLabel && (
            <button
              onClick={() => {}}
              className="inline-flex items-center justify-center gap-2 px-5 py-2.5 bg-white hover:bg-gray-50 text-gray-700 border border-gray-300 rounded-lg font-medium transition-colors"
            >
              {config.secondaryActionLabel}
            </button>
          )}
        </div>

        {/* Getting Started Tips (only for dashboard) */}
        {variant === 'dashboard' && (
          <div className="mt-10 pt-8 border-t border-gray-200">
            <div className="text-left space-y-4">
              <h4 className="text-sm font-semibold text-gray-900 flex items-center gap-2">
                <Sparkles className="w-4 h-4 text-yellow-500" />
                Quick Start
              </h4>
              
              <div className="space-y-3">
                <div className="flex gap-3 items-start">
                  <div className="flex-shrink-0 w-6 h-6 rounded-full bg-blue-100 text-blue-600 flex items-center justify-center text-xs font-semibold">
                    1
                  </div>
                  <div>
                    <p className="text-sm font-medium text-gray-900">Install the SDK</p>
                    <code className="text-xs text-gray-600 bg-gray-100 px-2 py-1 rounded mt-1 inline-block">
                      pip install agentreplay
                    </code>
                  </div>
                </div>

                <div className="flex gap-3 items-start">
                  <div className="flex-shrink-0 w-6 h-6 rounded-full bg-blue-100 text-blue-600 flex items-center justify-center text-xs font-semibold">
                    2
                  </div>
                  <div>
                    <p className="text-sm font-medium text-gray-900">Configure your app</p>
                    <code className="text-xs text-gray-600 bg-gray-100 px-2 py-1 rounded mt-1 inline-block">
                      export AGENTREPLAY_ENABLED=true
                    </code>
                  </div>
                </div>

                <div className="flex gap-3 items-start">
                  <div className="flex-shrink-0 w-6 h-6 rounded-full bg-blue-100 text-blue-600 flex items-center justify-center text-xs font-semibold">
                    3
                  </div>
                  <div>
                    <p className="text-sm font-medium text-gray-900">Send your first trace</p>
                    <p className="text-xs text-gray-600 mt-1">
                      Traces will appear here automatically
                    </p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default EmptyState;
