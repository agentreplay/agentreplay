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
