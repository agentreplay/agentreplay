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

import { useState, useEffect, useCallback, useRef } from 'react';
import { Sparkles, Play, Loader2, Copy, Check, AlertCircle, ChevronRight, Lightbulb, Zap, RefreshCcw, Settings, AlertTriangle, TrendingUp, Clock, DollarSign } from 'lucide-react';
import { Link } from 'react-router-dom';
import { API_BASE_URL } from '../../src/lib/agentreplay-api';

interface AnalysisResult {
  mermaid_code: string;
  summary: string;
  insights: string[];
  suggestions: string[];
}

interface AITraceAnalysisProps {
  traceId: string;
  tenantId?: number;
  projectId?: number;
  observations?: any[];
  autoRun?: boolean; // Auto-run analysis on mount
}

// Check if AI models are configured
function getConfiguredProvider(): { hasProvider: boolean; providerName?: string } {
    try {
        const savedSettings = localStorage.getItem('agentreplay_settings');
        if (!savedSettings) return { hasProvider: false };
        const settings = JSON.parse(savedSettings);
        const providers = settings?.models?.providers || [];
        const validProvider = providers.find((p: any) => p.apiKey && p.apiKey.length > 0);
        if (validProvider) {
            return { hasProvider: true, providerName: validProvider.name || validProvider.provider };
        }
        return { hasProvider: false };
    } catch {
        return { hasProvider: false };
    }
}

// Extract quick stats from observations
function extractQuickStats(observations: any[]): { totalTokens: number; totalCost: number; avgLatency: number; modelCount: number } {
    let totalTokens = 0;
    let totalCost = 0;
    let totalLatency = 0;
    let latencyCount = 0;
    const models = new Set<string>();
    
    observations.forEach(obs => {
        if (obs.usage) {
            totalTokens += (obs.usage.input || 0) + (obs.usage.output || 0);
            totalCost += obs.usage.total_cost || 0;
        }
        if (obs.latency) {
            totalLatency += obs.latency;
            latencyCount++;
        }
        if (obs.model) {
            models.add(obs.model);
        }
    });
    
    return {
        totalTokens,
        totalCost,
        avgLatency: latencyCount > 0 ? totalLatency / latencyCount : 0,
        modelCount: models.size,
    };
}

export function AITraceAnalysis({ traceId, tenantId = 1, projectId = 1, observations = [], autoRun = true }: AITraceAnalysisProps) {
  const [analysis, setAnalysis] = useState<AnalysisResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const mermaidRef = useRef<HTMLDivElement>(null);
  const [mermaidLoaded, setMermaidLoaded] = useState(false);
  const hasAutoRun = useRef(false);
  
  const providerInfo = getConfiguredProvider();
  const quickStats = extractQuickStats(observations);

  // Dynamically load Mermaid from CDN
  useEffect(() => {
    if (typeof window !== 'undefined' && !(window as any).mermaid) {
      const script = document.createElement('script');
      script.src = 'https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js';
      script.async = true;
      script.onload = () => {
        (window as any).mermaid.initialize({
          startOnLoad: false,
          theme: 'dark',
          themeVariables: {
            primaryColor: '#3b82f6',
            primaryTextColor: '#fff',
            primaryBorderColor: '#1e40af',
            lineColor: '#6b7280',
            secondaryColor: '#1e293b',
            tertiaryColor: '#0f172a',
          },
          flowchart: {
            useMaxWidth: true,
            htmlLabels: true,
            curve: 'basis',
          },
        });
        setMermaidLoaded(true);
      };
      document.head.appendChild(script);
    } else if ((window as any).mermaid) {
      setMermaidLoaded(true);
    }
  }, []);

  // Render Mermaid diagram when analysis is available
  useEffect(() => {
    if (analysis?.mermaid_code && mermaidRef.current && mermaidLoaded) {
      const renderDiagram = async () => {
        try {
          const mermaid = (window as any).mermaid;
          // Clean up the mermaid code
          const cleanCode = analysis.mermaid_code
            .replace(/\\n/g, '\n')
            .replace(/\\"/g, '"')
            .trim();
          
          mermaidRef.current!.innerHTML = '';
          const { svg } = await mermaid.render(`mermaid-${Date.now()}`, cleanCode);
          mermaidRef.current!.innerHTML = svg;
        } catch (err) {
          console.error('Mermaid render error:', err);
          // Show the raw code if rendering fails
          if (mermaidRef.current) {
            mermaidRef.current.innerHTML = `<pre class="text-xs text-textSecondary overflow-auto p-4 bg-surface rounded-lg">${analysis.mermaid_code}</pre>`;
          }
        }
      };
      renderDiagram();
    }
  }, [analysis, mermaidLoaded]);

  const runAnalysis = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/traces/${traceId}/analyze`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Tenant-ID': tenantId.toString(),
          'X-Project-ID': projectId.toString(),
        },
        body: JSON.stringify({ analysis_type: 'flow_diagram' }),
      });
      
      if (!response.ok) {
        const errData = await response.json().catch(() => ({}));
        throw new Error(errData.error || `Analysis failed: ${response.status}`);
      }
      
      const data = await response.json();
      setAnalysis(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Analysis failed');
    } finally {
      setLoading(false);
    }
  }, [traceId, tenantId, projectId]);

  // Auto-run analysis on mount if enabled and provider is configured
  useEffect(() => {
    if (autoRun && providerInfo.hasProvider && !hasAutoRun.current && !analysis && !loading) {
      hasAutoRun.current = true;
      runAnalysis();
    }
  }, [autoRun, providerInfo.hasProvider, analysis, loading, runAnalysis]);

  const copyMermaidCode = () => {
    if (analysis?.mermaid_code) {
      navigator.clipboard.writeText(analysis.mermaid_code.replace(/\\n/g, '\n'));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  // Show configuration prompt if no AI provider
  if (!providerInfo.hasProvider) {
    return (
      <div className="flex flex-col items-center justify-center h-full py-12 px-6">
        <div className="bg-warning/10 rounded-full p-6 mb-6">
          <AlertTriangle className="w-12 h-12 text-warning" />
        </div>
        <h3 className="text-xl font-semibold text-textPrimary mb-2">Configure AI Model</h3>
        <p className="text-textSecondary text-center mb-6 max-w-md">
          AI Analysis requires a configured AI provider. Add your OpenAI, Anthropic, or Ollama API key to enable intelligent trace analysis.
        </p>
        <Link
          to="/settings?tab=models"
          className="flex items-center gap-2 px-6 py-3 bg-primary text-white rounded-lg hover:bg-primary/90 transition-colors font-medium"
        >
          <Settings className="w-5 h-5" />
          Configure AI Models
        </Link>
      </div>
    );
  }

  if (!analysis && !loading && !error) {
    return (
      <div className="flex flex-col h-full">
        {/* Quick Stats Banner */}
        {observations.length > 0 && (
          <div className="p-4 border-b border-border bg-surface-elevated">
            <div className="grid grid-cols-4 gap-4">
              <div className="text-center">
                <div className="text-2xl font-bold text-textPrimary">{observations.length}</div>
                <div className="text-xs text-textSecondary">Spans</div>
              </div>
              <div className="text-center">
                <div className="text-2xl font-bold text-textPrimary">{quickStats.totalTokens.toLocaleString()}</div>
                <div className="text-xs text-textSecondary">Tokens</div>
              </div>
              <div className="text-center">
                <div className="text-2xl font-bold text-textPrimary">
                  {quickStats.avgLatency >= 1000 ? `${(quickStats.avgLatency / 1000).toFixed(1)}s` : `${Math.round(quickStats.avgLatency)}ms`}
                </div>
                <div className="text-xs text-textSecondary">Avg Latency</div>
              </div>
              <div className="text-center">
                <div className="text-2xl font-bold text-textPrimary">${quickStats.totalCost.toFixed(4)}</div>
                <div className="text-xs text-textSecondary">Cost</div>
              </div>
            </div>
          </div>
        )}
        
        <div className="flex-1 flex flex-col items-center justify-center py-12 px-6">
          <div className="bg-gradient-to-br from-primary/20 to-purple-500/20 rounded-full p-6 mb-6">
            <Sparkles className="w-12 h-12 text-primary" />
          </div>
          <h3 className="text-xl font-semibold text-textPrimary mb-2">AI Trace Analysis</h3>
          <p className="text-textSecondary text-center mb-6 max-w-md">
            Let AI analyze this trace and generate a visual flow diagram with insights and optimization suggestions.
          </p>
          <button
            onClick={runAnalysis}
            className="flex items-center gap-2 px-6 py-3 bg-primary text-white rounded-lg hover:bg-primary/90 transition-colors font-medium"
          >
            <Play className="w-5 h-5" />
            Analyze Trace Flow
          </button>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center h-full py-12">
        <Loader2 className="w-10 h-10 text-primary animate-spin mb-4" />
        <p className="text-textSecondary">Analyzing trace with AI...</p>
        <p className="text-textTertiary text-sm mt-1">This may take a few seconds</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-full py-12 px-6">
        <div className="bg-red-500/20 rounded-full p-4 mb-4">
          <AlertCircle className="w-8 h-8 text-red-400" />
        </div>
        <h3 className="text-lg font-semibold text-textPrimary mb-2">Analysis Failed</h3>
        <p className="text-textSecondary text-center mb-4">{error}</p>
        <button
          onClick={runAnalysis}
          className="flex items-center gap-2 px-4 py-2 bg-surface border border-border rounded-lg hover:bg-surface-hover transition-colors"
        >
          <RefreshCcw className="w-4 h-4" />
          Try Again
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-border bg-surface-elevated">
        <div className="flex items-center gap-2">
          <Sparkles className="w-5 h-5 text-primary" />
          <h3 className="font-semibold text-textPrimary">AI Analysis</h3>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={copyMermaidCode}
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm bg-surface border border-border rounded-lg hover:bg-surface-hover transition-colors"
          >
            {copied ? <Check className="w-4 h-4 text-success" /> : <Copy className="w-4 h-4" />}
            {copied ? 'Copied!' : 'Copy Diagram'}
          </button>
          <button
            onClick={runAnalysis}
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm bg-primary text-white rounded-lg hover:bg-primary/90 transition-colors"
          >
            <RefreshCcw className="w-4 h-4" />
            Re-analyze
          </button>
        </div>
      </div>

      {/* Quick Stats Bar */}
      {observations.length > 0 && (
        <div className="flex items-center gap-6 px-4 py-2 border-b border-border bg-surface text-sm">
          <div className="flex items-center gap-1.5">
            <TrendingUp className="w-4 h-4 text-textSecondary" />
            <span className="text-textSecondary">{observations.length} spans</span>
          </div>
          <div className="flex items-center gap-1.5">
            <Sparkles className="w-4 h-4 text-textSecondary" />
            <span className="text-textSecondary">{quickStats.totalTokens.toLocaleString()} tokens</span>
          </div>
          <div className="flex items-center gap-1.5">
            <Clock className="w-4 h-4 text-textSecondary" />
            <span className={quickStats.avgLatency > 5000 ? 'text-warning' : 'text-textSecondary'}>
              {quickStats.avgLatency >= 1000 ? `${(quickStats.avgLatency / 1000).toFixed(1)}s` : `${Math.round(quickStats.avgLatency)}ms`}
            </span>
          </div>
          <div className="flex items-center gap-1.5">
            <DollarSign className="w-4 h-4 text-textSecondary" />
            <span className="text-textSecondary">${quickStats.totalCost.toFixed(4)}</span>
          </div>
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-auto p-4 space-y-6">
        {/* Summary */}
        {analysis?.summary && (
          <div className="bg-surface border border-border rounded-lg p-4">
            <h4 className="text-sm font-medium text-textSecondary mb-2">Summary</h4>
            <p className="text-textPrimary">{analysis.summary}</p>
          </div>
        )}

        {/* Flow Diagram */}
        <div className="bg-surface border border-border rounded-lg p-4">
          <h4 className="text-sm font-medium text-textSecondary mb-3">Flow Diagram</h4>
          <div 
            ref={mermaidRef} 
            className="w-full overflow-auto bg-background rounded-lg p-4 min-h-[200px] flex items-center justify-center"
          >
            {!mermaidLoaded && (
              <div className="flex items-center gap-2 text-textSecondary">
                <Loader2 className="w-4 h-4 animate-spin" />
                Loading diagram renderer...
              </div>
            )}
          </div>
        </div>

        {/* Insights with severity indicators */}
        {analysis?.insights && analysis.insights.length > 0 && (
          <div className="bg-surface border border-border rounded-lg p-4">
            <div className="flex items-center gap-2 mb-3">
              <Lightbulb className="w-4 h-4 text-yellow-500" />
              <h4 className="text-sm font-medium text-textSecondary">Key Insights</h4>
            </div>
            <ul className="space-y-3">
              {analysis.insights.map((insight, i) => {
                // Detect severity from keywords
                const isWarning = /slow|high|expensive|error|failed|timeout/i.test(insight);
                const isSuccess = /fast|efficient|optimal|good|success/i.test(insight);
                
                return (
                  <li key={i} className={`flex items-start gap-3 p-2 rounded-lg ${isWarning ? 'bg-warning/5' : isSuccess ? 'bg-success/5' : ''}`}>
                    <div className={`w-1.5 h-1.5 rounded-full mt-2 flex-shrink-0 ${isWarning ? 'bg-warning' : isSuccess ? 'bg-success' : 'bg-primary'}`} />
                    <span className="text-textPrimary text-sm">{insight}</span>
                  </li>
                );
              })}
            </ul>
          </div>
        )}

        {/* Actionable Suggestions */}
        {analysis?.suggestions && analysis.suggestions.length > 0 && (
          <div className="bg-surface border border-border rounded-lg p-4">
            <div className="flex items-center gap-2 mb-3">
              <Zap className="w-4 h-4 text-primary" />
              <h4 className="text-sm font-medium text-textSecondary">Optimization Suggestions</h4>
            </div>
            <ul className="space-y-2">
              {analysis.suggestions.map((suggestion, i) => (
                <li key={i} className="flex items-start gap-3 p-3 bg-primary/5 rounded-lg border border-primary/10">
                  <div className="flex-shrink-0 w-6 h-6 rounded-full bg-primary/10 flex items-center justify-center">
                    <span className="text-xs font-medium text-primary">{i + 1}</span>
                  </div>
                  <span className="text-textPrimary text-sm">{suggestion}</span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </div>
  );
}
