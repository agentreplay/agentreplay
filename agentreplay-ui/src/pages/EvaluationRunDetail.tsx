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

import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { AlertCircle, ArrowLeft, CheckCircle2, Loader2, SquareGantt, XCircle } from 'lucide-react';
import { agentreplayClient, EvalRun } from '../lib/agentreplay-api';
import { Button } from '../../components/ui/button';

interface DisplayMetric {
  label: string;
  value: string;
  detail?: string;
}

export default function EvaluationRunDetail() {
  const { projectId, runId } = useParams<{ projectId: string; runId: string }>();
  const navigate = useNavigate();
  const [run, setRun] = useState<EvalRun | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!runId) return;
    const fetchRun = async () => {
      setLoading(true);
      setError(null);
      try {
        const result = await agentreplayClient.getEvalRun(runId);
        setRun(result);
      } catch (err) {
        console.error('Failed to fetch eval run', err);
        setError(err instanceof Error ? err.message : 'Unknown error');
      } finally {
        setLoading(false);
      }
    };

    fetchRun();
  }, [runId]);

  const metrics = useMemo(() => {
    if (!run) return [] as DisplayMetric[];
    const total = run.results?.length || 1;
    const passed = run.results?.filter((result) => result.passed).length || 0;
    // Get average of first eval metric or default to 0
    const avgScore = run.results?.length 
      ? run.results.reduce((sum, result) => {
          const values = Object.values(result.eval_metrics || {});
          return sum + (values[0] ?? 0);
        }, 0) / total
      : 0;
    return [
      { label: 'Dataset', value: run.dataset_id },
      { label: 'Pass rate', value: `${((passed / total) * 100).toFixed(1)}%`, detail: `${passed}/${total}` },
      { label: 'Avg score', value: avgScore ? avgScore.toFixed(2) : '—', detail: 'LLM-as-judge' },
      { label: 'Duration', value: formatDuration(run.started_at || run.created_at || Date.now() * 1000, run.results) },
    ];
  }, [run]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center text-textSecondary">
        <Loader2 className="mr-2 h-4 w-4 animate-spin" /> Fetching evaluation run…
      </div>
    );
  }

  if (error || !run) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 text-center">
        <AlertCircle className="h-10 w-10 text-red-400" />
        <p className="text-sm text-textSecondary">{error || 'Run not found.'}</p>
        <Button variant="outline" onClick={() => navigate(-1)}>
          Go back
        </Button>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-6">
      <header className="flex items-center justify-between">
        <div>
          <button className="mb-1 flex items-center gap-2 text-xs uppercase tracking-widest text-textTertiary" onClick={() => navigate(`/projects/${projectId}/evaluations`)}>
            <ArrowLeft className="h-3 w-3" /> Eval runs
          </button>
          <h1 className="text-2xl font-semibold text-textPrimary">{run.name}</h1>
          <p className="text-sm text-textSecondary">Run ID · {run.run_id || (run as any).id}</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm">Duplicate run</Button>
          <Button variant="default" size="sm" className="gap-2">
            <SquareGantt className="h-4 w-4" /> Compare
          </Button>
        </div>
      </header>

      <section className="grid gap-4 md:grid-cols-4">
        {metrics.map((metric) => (
          <div key={metric.label} className="rounded-2xl border border-border/60 bg-background/80 p-4">
            <p className="text-xs uppercase tracking-widest text-textTertiary">{metric.label}</p>
            <p className="mt-1 text-2xl font-semibold text-textPrimary">{metric.value}</p>
            {metric.detail && <p className="text-xs text-textSecondary">{metric.detail}</p>}
          </div>
        ))}
      </section>

      <section className="rounded-3xl border border-border/50 bg-background/80">
        <div className="flex items-center justify-between border-b border-border/50 px-4 py-3">
          <div>
            <p className="text-xs uppercase tracking-widest text-textTertiary">Results</p>
            <p className="text-sm text-textSecondary">Dive into individual test cases.</p>
          </div>
          <div className="text-xs text-textTertiary">{run.results?.length || 0} examples</div>
        </div>
        <div className="max-h-[520px] overflow-auto">
          <table className="min-w-full text-sm">
            <thead className="bg-surface/80 text-textTertiary">
              <tr>
                <th className="px-4 py-2 text-left font-semibold">Test Case</th>
                <th className="px-4 py-2 text-left font-semibold">Score</th>
                <th className="px-4 py-2 text-left font-semibold">Status</th>
                <th className="px-4 py-2 text-left font-semibold">Trace ID</th>
              </tr>
            </thead>
            <tbody>
              {run.results?.map((result) => {
                const firstMetric = Object.entries(result.eval_metrics || {})[0];
                const score = firstMetric ? firstMetric[1] : undefined;
                return (
                  <tr key={result.test_case_id} className="border-b border-border/40 text-textSecondary">
                    <td className="px-4 py-3 text-textPrimary">
                      <p className="font-semibold font-mono text-xs">{result.test_case_id.substring(0, 16)}...</p>
                    </td>
                    <td className="px-4 py-3">{typeof score === 'number' ? score.toFixed(2) : '—'}</td>
                    <td className="px-4 py-3">
                      {result.passed ? (
                        <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-1 text-xs font-semibold text-emerald-400">
                          <CheckCircle2 className="h-3 w-3" /> Passed
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 rounded-full bg-red-500/15 px-2 py-1 text-xs font-semibold text-red-400">
                          <XCircle className="h-3 w-3" /> Failed
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-3 text-xs text-textSecondary font-mono">
                      {result.trace_id ? result.trace_id.substring(0, 16) + '...' : '—'}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}

function formatDuration(createdAt: number, results?: EvalRun['results']) {
  if (!results?.length) return '—';
  const lastTimestamp = results.reduce((max, result) => {
    const ts = result.timestamp_us ?? createdAt;
    return Math.max(max, ts);
  }, createdAt);
  const diff = Math.max(0, lastTimestamp - createdAt);
  // Convert from microseconds to milliseconds
  const diffMs = diff / 1000;
  if (diffMs < 1000) return `${diffMs.toFixed(0)} ms`;
  const seconds = diffMs / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)} s`;
  const minutes = Math.floor(seconds / 60);
  const remainder = Math.round(seconds % 60);
  return `${minutes}m ${remainder}s`;
}
