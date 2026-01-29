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

import { AlertTriangle, TrendingUp, DollarSign, Bell } from 'lucide-react';
import { useState } from 'react';

interface CostAlert {
  id: string;
  type: 'budget' | 'spike' | 'forecast';
  severity: 'warning' | 'critical';
  message: string;
  threshold: number;
  current: number;
}

interface Budget {
  monthly: number;
  daily: number;
  alertThreshold: number; // percentage
}

export function CostAlertsPanel() {
  const [budget, setBudget] = useState<Budget>({
    monthly: 1000,
    daily: 50,
    alertThreshold: 80,
  });
  
  const [showBudgetForm, setShowBudgetForm] = useState(false);
  
  // Mock current spending
  const currentMonthly = 850;
  const currentDaily = 45;
  const forecastedMonthly = 1050;
  
  const alerts: CostAlert[] = [];
  
  // Check budget alerts
  if (currentMonthly / budget.monthly >= budget.alertThreshold / 100) {
    alerts.push({
      id: '1',
      type: 'budget',
      severity: currentMonthly > budget.monthly ? 'critical' : 'warning',
      message: `Monthly budget at ${((currentMonthly / budget.monthly) * 100).toFixed(0)}%`,
      threshold: budget.monthly,
      current: currentMonthly,
    });
  }
  
  // Check forecast
  if (forecastedMonthly > budget.monthly) {
    alerts.push({
      id: '2',
      type: 'forecast',
      severity: 'warning',
      message: `Forecasted to exceed monthly budget by $${(forecastedMonthly - budget.monthly).toFixed(2)}`,
      threshold: budget.monthly,
      current: forecastedMonthly,
    });
  }
  
  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <Bell className="w-5 h-5 text-primary" />
          <h3 className="text-lg font-semibold text-textPrimary">Budget & Alerts</h3>
        </div>
        <button
          onClick={() => setShowBudgetForm(!showBudgetForm)}
          className="text-sm text-primary hover:underline"
        >
          {showBudgetForm ? 'Cancel' : 'Configure Budget'}
        </button>
      </div>
      
      {showBudgetForm && (
        <div className="mb-4 p-4 bg-surface-elevated rounded-lg border border-border space-y-3">
          <div>
            <label className="block text-sm font-medium text-textSecondary mb-1">
              Monthly Budget ($)
            </label>
            <input
              type="number"
              value={budget.monthly}
              onChange={e => setBudget({ ...budget, monthly: parseFloat(e.target.value) })}
              className="w-full px-3 py-2 bg-surface border border-border rounded-lg text-textPrimary"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-textSecondary mb-1">
              Daily Budget ($)
            </label>
            <input
              type="number"
              value={budget.daily}
              onChange={e => setBudget({ ...budget, daily: parseFloat(e.target.value) })}
              className="w-full px-3 py-2 bg-surface border border-border rounded-lg text-textPrimary"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-textSecondary mb-1">
              Alert Threshold (%)
            </label>
            <input
              type="number"
              value={budget.alertThreshold}
              onChange={e => setBudget({ ...budget, alertThreshold: parseFloat(e.target.value) })}
              className="w-full px-3 py-2 bg-surface border border-border rounded-lg text-textPrimary"
            />
          </div>
          <button
            onClick={() => setShowBudgetForm(false)}
            className="w-full px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary/90 transition-colors"
          >
            Save Budget
          </button>
        </div>
      )}
      
      {/* Budget Overview */}
      <div className="grid grid-cols-2 gap-4 mb-4">
        <div className="p-3 bg-surface-elevated rounded-lg">
          <div className="text-xs text-textTertiary mb-1">Monthly</div>
          <div className="flex items-baseline gap-2">
            <span className="text-lg font-bold text-textPrimary">
              ${currentMonthly.toFixed(2)}
            </span>
            <span className="text-xs text-textSecondary">
              / ${budget.monthly}
            </span>
          </div>
          <div className="mt-2 h-2 bg-surface rounded-full overflow-hidden">
            <div 
              className={`h-full ${
                currentMonthly > budget.monthly 
                  ? 'bg-destructive' 
                  : currentMonthly / budget.monthly > 0.8 
                    ? 'bg-warning' 
                    : 'bg-success'
              }`}
              style={{ width: `${Math.min((currentMonthly / budget.monthly) * 100, 100)}%` }}
            />
          </div>
        </div>
        
        <div className="p-3 bg-surface-elevated rounded-lg">
          <div className="text-xs text-textTertiary mb-1">Daily</div>
          <div className="flex items-baseline gap-2">
            <span className="text-lg font-bold text-textPrimary">
              ${currentDaily.toFixed(2)}
            </span>
            <span className="text-xs text-textSecondary">
              / ${budget.daily}
            </span>
          </div>
          <div className="mt-2 h-2 bg-surface rounded-full overflow-hidden">
            <div 
              className={`h-full ${
                currentDaily > budget.daily 
                  ? 'bg-destructive' 
                  : currentDaily / budget.daily > 0.8 
                    ? 'bg-warning' 
                    : 'bg-success'
              }`}
              style={{ width: `${Math.min((currentDaily / budget.daily) * 100, 100)}%` }}
            />
          </div>
        </div>
      </div>
      
      {/* Active Alerts */}
      {alerts.length > 0 ? (
        <div className="space-y-2">
          {alerts.map(alert => (
            <div 
              key={alert.id}
              className={`p-3 rounded-lg border flex items-start gap-3 ${
                alert.severity === 'critical' 
                  ? 'bg-destructive/10 border-destructive/20' 
                  : 'bg-warning/10 border-warning/20'
              }`}
            >
              <AlertTriangle className={`w-5 h-5 flex-shrink-0 mt-0.5 ${
                alert.severity === 'critical' ? 'text-destructive' : 'text-warning'
              }`} />
              <div className="flex-1 min-w-0">
                <div className={`text-sm font-medium ${
                  alert.severity === 'critical' ? 'text-destructive' : 'text-warning'
                }`}>
                  {alert.message}
                </div>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="text-center py-4 text-textTertiary text-sm">
          No active alerts
        </div>
      )}
    </div>
  );
}

export function CostForecast() {
  // Mock forecast data
  const forecast = {
    endOfMonth: 1050,
    trend: 'increasing',
    confidence: 0.85,
  };
  
  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <div className="flex items-center gap-2 mb-4">
        <TrendingUp className="w-5 h-5 text-primary" />
        <h3 className="text-lg font-semibold text-textPrimary">Cost Forecast</h3>
      </div>
      
      <div className="space-y-4">
        <div>
          <div className="text-sm text-textSecondary mb-1">End of Month Projection</div>
          <div className="text-3xl font-bold text-textPrimary">
            ${forecast.endOfMonth.toFixed(2)}
          </div>
          <div className="text-xs text-textTertiary mt-1">
            {(forecast.confidence * 100).toFixed(0)}% confidence
          </div>
        </div>
        
        <div className="p-3 bg-surface-elevated rounded-lg">
          <div className="flex items-center justify-between text-sm">
            <span className="text-textSecondary">7-day average</span>
            <span className="font-medium text-textPrimary">$42.50/day</span>
          </div>
          <div className="flex items-center justify-between text-sm mt-2">
            <span className="text-textSecondary">30-day trend</span>
            <span className="font-medium text-warning flex items-center gap-1">
              <TrendingUp className="w-3 h-3" />
              +12%
            </span>
          </div>
        </div>
        
        {/* Simple forecast chart placeholder */}
        <div className="h-32 bg-surface-elevated rounded-lg flex items-end justify-between p-3 gap-1">
          {[0.3, 0.4, 0.35, 0.5, 0.45, 0.6, 0.55, 0.7, 0.75, 0.8, 0.85, 0.9].map((height, i) => (
            <div key={i} className="flex-1 bg-primary/20 rounded-t" style={{ height: `${height * 100}%` }} />
          ))}
        </div>
      </div>
    </div>
  );
}

export function OptimizationSuggestions() {
  const suggestions = [
    {
      id: '1',
      title: 'Switch to GPT-3.5 for simple queries',
      description: '45% of your GPT-4 calls could use GPT-3.5',
      savings: 120,
    },
    {
      id: '2',
      title: 'Implement caching for repeated queries',
      description: '23% of queries are duplicates',
      savings: 85,
    },
    {
      id: '3',
      title: 'Reduce max_tokens for summarization',
      description: 'Average output is 30% shorter than max_tokens',
      savings: 45,
    },
  ];
  
  return (
    <div className="bg-surface rounded-lg border border-border p-6">
      <div className="flex items-center gap-2 mb-4">
        <DollarSign className="w-5 h-5 text-success" />
        <h3 className="text-lg font-semibold text-textPrimary">Optimization Tips</h3>
      </div>
      
      <div className="space-y-3">
        {suggestions.map(suggestion => (
          <div key={suggestion.id} className="p-4 bg-success/10 rounded-lg border border-success/20">
            <div className="flex items-start justify-between mb-2">
              <div className="font-medium text-textPrimary">{suggestion.title}</div>
              <div className="text-success font-bold text-sm">
                -${suggestion.savings}/mo
              </div>
            </div>
            <div className="text-sm text-textSecondary">
              {suggestion.description}
            </div>
          </div>
        ))}
        
        <div className="pt-3 border-t border-border">
          <div className="flex items-center justify-between text-sm">
            <span className="text-textSecondary">Total potential savings</span>
            <span className="text-lg font-bold text-success">
              $250/month
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
