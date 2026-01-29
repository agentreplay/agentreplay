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

import React, { useState, useEffect } from 'react';

// Mock Lucide icons since we don't have the library installed
const ThumbsUp = () => <span>👍</span>;
const ThumbsDown = () => <span>👎</span>;
const Star = ({ className }: { className: string }) => <span className={className}>★</span>;
const SkipForward = () => <span>Skip</span>;
const ArrowRight = () => <span>→</span>;
const MessageSquare = ({ className }: { className: string }) => <span className={className}>MSG</span>;

// Mock invoke
const invoke = async (cmd: string, args: any) => {
    console.log(`Invoke ${cmd}`, args);
    if (cmd === 'get_eval_run') {
        return {
            results: [
                {
                    test_case_id: "1",
                    input: "How to bake a cake?",
                    output: "Mix flour, sugar...",
                    expected_output: "Recipe for cake...",
                    eval_metrics: { accuracy: 0.8 },
                }
            ]
        };
    }
    if (cmd === 'get_annotation_stats') {
        return {
            unique_cases_annotated: 5,
            avg_time_per_annotation_secs: 30,
            thumbs_up_count: 3,
            thumbs_down_count: 2
        };
    }
    return {};
};

interface EvalResult {
  test_case_id: string;
  input: string;
  output: string;
  expected_output?: string;
  eval_metrics: Record<string, number>;
  trace_id?: string;
}

interface Annotation {
  id: string;
  eval_run_id: string;
  test_case_id: string;
  annotator: string;
  ratings: Record<string, number>;
  thumbs?: 'Up' | 'Down' | 'Neutral';
  stars?: number;
  tags: string[];
  comment?: string;
  corrected_output?: string;
  time_spent_secs: number;
}

interface AnnotationCampaign {
  id: string;
  name: string;
  eval_run_id: string;
  dimensions: AnnotationDimension[];
  total_cases: number;
  annotated_cases: number;
}

interface AnnotationDimension {
  name: string;
  description: string;
  scale_type: 'Continuous' | 'Discrete' | 'Binary';
  min_value: number;
  max_value: number;
  required: boolean;
}

export function AnnotationQueue({ evalRunId }: { evalRunId: string }) {
  const [results, setResults] = useState<EvalResult[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [campaign, setCampaign] = useState<AnnotationCampaign | null>(null);

  // Current annotation state
  const [ratings, setRatings] = useState<Record<string, number>>({});
  const [thumbs, setThumbs] = useState<'Up' | 'Down' | 'Neutral' | undefined>();
  const [stars, setStars] = useState<number>(0);
  const [tags, setTags] = useState<string[]>([]);
  const [comment, setComment] = useState('');
  const [correctedOutput, setCorrectedOutput] = useState('');
  const [startTime, setStartTime] = useState(Date.now());

  // Stats
  const [stats, setStats] = useState<any>(null);

  useEffect(() => {
    loadResults();
    loadCampaign();
    loadStats();
  }, [evalRunId]);

  const loadResults = async () => {
    const run = await invoke('get_eval_run', { runId: evalRunId }) as { results?: EvalResult[] };
    setResults(run.results || []);
  };

  const loadCampaign = async () => {
    // Load campaign config (dimensions to rate)
    setCampaign({
      id: 'campaign_1',
      name: 'Quality Review',
      eval_run_id: evalRunId,
      dimensions: [
        {
          name: 'accuracy',
          description: 'How accurate is the response?',
          scale_type: 'Continuous',
          min_value: 0.0,
          max_value: 1.0,
          required: true,
        },
        {
          name: 'helpfulness',
          description: 'How helpful is this response?',
          scale_type: 'Continuous',
          min_value: 0.0,
          max_value: 1.0,
          required: true,
        },
      ],
      total_cases: results.length,
      annotated_cases: 0,
    });
  };

  const loadStats = async () => {
    const s = await invoke('get_annotation_stats', { evalRunId });
    setStats(s);
  };

  const currentResult = results[currentIndex];

  const saveAnnotation = async () => {
    if (!currentResult || !campaign) return;

    const timeSpent = Math.floor((Date.now() - startTime) / 1000);

    const annotation: Annotation = {
      id: crypto.randomUUID(),
      eval_run_id: evalRunId,
      test_case_id: currentResult.test_case_id,
      annotator: 'local_user', // Desktop app - single user
      ratings,
      thumbs,
      stars: stars > 0 ? stars : undefined,
      tags,
      comment: comment || undefined,
      corrected_output: correctedOutput || undefined,
      time_spent_secs: timeSpent,
    };

    await invoke('create_annotation', { annotation });

    // Reset for next
    resetForm();
    if (currentIndex < results.length - 1) {
        setCurrentIndex(currentIndex + 1);
        setStartTime(Date.now());
        loadStats();
    } else {
        alert("Campaign completed!");
    }
  };

  const skip = () => {
    resetForm();
    if (currentIndex < results.length - 1) {
        setCurrentIndex(currentIndex + 1);
        setStartTime(Date.now());
    }
  };

  const resetForm = () => {
    setRatings({});
    setThumbs(undefined);
    setStars(0);
    setTags([]);
    setComment('');
    setCorrectedOutput('');
  };

  const progress = results.length > 0 ? (currentIndex / results.length) * 100 : 0;
  const remaining = results.length - currentIndex;

  if (!currentResult || !campaign) {
    return <div className="p-8 text-center">Loading...</div>;
  }

  return (
    <div className="h-screen flex flex-col bg-background">
      {/* Header with progress */}
      <div className="bg-surface border-b border-border p-4">
        <div className="flex items-center justify-between mb-2">
          <h1 className="text-xl font-bold text-textPrimary">Annotation Queue</h1>
          <div className="text-sm text-textSecondary">
            {currentIndex + 1} / {results.length} ({remaining} remaining)
          </div>
        </div>
        <div className="w-full h-2 bg-gray-200 rounded-full overflow-hidden">
          <div
            className="h-full bg-blue-600 transition-all duration-300"
            style={{ width: `${progress}%` }}
          />
        </div>
      </div>

      {/* Main content */}
      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-5xl mx-auto space-y-6">
          {/* Input/Output Display */}
          <div className="bg-white rounded-xl border border-gray-200 p-6 space-y-4 shadow-sm">
            <div>
              <label className="text-sm font-semibold text-gray-500 uppercase tracking-wide mb-2 block">
                Input
              </label>
              <div className="bg-gray-50 rounded-lg p-4 text-gray-800">
                <pre className="whitespace-pre-wrap font-mono text-sm">{currentResult.input}</pre>
              </div>
            </div>

            <div>
              <label className="text-sm font-semibold text-gray-500 uppercase tracking-wide mb-2 block">
                Output
              </label>
              <div className="bg-gray-50 rounded-lg p-4 text-gray-800">
                <pre className="whitespace-pre-wrap font-mono text-sm">{currentResult.output}</pre>
              </div>
            </div>

            {currentResult.expected_output && (
              <div>
                <label className="text-sm font-semibold text-gray-500 uppercase tracking-wide mb-2 block">
                  Expected Output
                </label>
                <div className="bg-green-50 rounded-lg p-4 text-green-700">
                  <pre className="whitespace-pre-wrap font-mono text-sm">{currentResult.expected_output}</pre>
                </div>
              </div>
            )}
          </div>

          {/* Annotation Controls */}
          <div className="bg-white rounded-xl border border-gray-200 p-6 space-y-6 shadow-sm">
            <h2 className="text-lg font-semibold text-gray-800">Your Annotation</h2>

            {/* Quick thumbs rating */}
            <div>
              <label className="text-sm font-semibold text-gray-500 mb-3 block">
                Quick Rating
              </label>
              <div className="flex gap-3">
                <button
                  onClick={() => setThumbs('Up')}
                  className={`flex-1 py-3 rounded-lg border-2 transition-all ${
                    thumbs === 'Up'
                      ? 'border-green-500 bg-green-50 text-green-500'
                      : 'border-gray-200 hover:border-green-500 text-gray-400'
                  }`}
                >
                  <ThumbsUp />
                </button>
                <button
                  onClick={() => setThumbs('Down')}
                  className={`flex-1 py-3 rounded-lg border-2 transition-all ${
                    thumbs === 'Down'
                      ? 'border-red-500 bg-red-50 text-red-500'
                      : 'border-gray-200 hover:border-red-500 text-gray-400'
                  }`}
                >
                  <ThumbsDown />
                </button>
              </div>
            </div>

            {/* Star rating */}
            <div>
              <label className="text-sm font-semibold text-gray-500 mb-3 block">
                Overall Quality (Stars)
              </label>
              <div className="flex gap-2">
                {[1, 2, 3, 4, 5].map(n => (
                  <button
                    key={n}
                    onClick={() => setStars(n)}
                    className="transition-all text-2xl"
                  >
                    <Star
                      className={`${
                        n <= stars
                          ? 'text-yellow-500'
                          : 'text-gray-300'
                      }`}
                    />
                  </button>
                ))}
              </div>
            </div>

            {/* Dimensional ratings */}
            <div className="space-y-4">
              <label className="text-sm font-semibold text-gray-500 block">
                Detailed Ratings
              </label>
              {campaign.dimensions.map(dim => (
                <div key={dim.name}>
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm text-gray-800 capitalize">{dim.name}</span>
                    <span className="text-sm font-mono text-gray-500">
                      {(ratings[dim.name] || 0).toFixed(2)}
                    </span>
                  </div>
                  <input
                    type="range"
                    min={dim.min_value}
                    max={dim.max_value}
                    step={0.01}
                    value={ratings[dim.name] || 0}
                    onChange={(e) => setRatings({
                      ...ratings,
                      [dim.name]: parseFloat(e.target.value)
                    })}
                    className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer"
                  />
                  <div className="text-xs text-gray-400 mt-1">{dim.description}</div>
                </div>
              ))}
            </div>

            {/* Comment */}
            <div>
              <label className="text-sm font-semibold text-gray-500 mb-3 block">
                <MessageSquare className="w-4 h-4 inline mr-2" />
                Comments
              </label>
              <textarea
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                placeholder="Why did you rate this way? What could be improved?"
                className="w-full h-24 px-4 py-3 bg-gray-50 border border-gray-200 rounded-lg text-gray-800 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>
          </div>

          {/* Action buttons */}
          <div className="flex gap-3">
            <button
              onClick={skip}
              className="flex-1 py-3 px-6 bg-gray-100 border border-gray-200 rounded-lg text-gray-600 hover:bg-gray-200 transition-all flex items-center justify-center gap-2"
            >
              <SkipForward />
            </button>
            <button
              onClick={saveAnnotation}
              className="flex-[2] py-3 px-6 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-all flex items-center justify-center gap-2 font-semibold"
            >
              Save & Next
              <ArrowRight />
            </button>
          </div>
        </div>
      </div>

      {/* Stats sidebar (optional) */}
      {stats && (
        <div className="fixed right-4 top-20 w-64 bg-white rounded-xl border border-gray-200 p-4 space-y-3 shadow-sm">
          <h3 className="font-semibold text-gray-800">Session Stats</h3>
          <div className="space-y-2 text-sm">
            <div className="flex justify-between">
              <span className="text-gray-500">Completed</span>
              <span className="font-semibold text-gray-800">{stats.unique_cases_annotated}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-500">Avg Time</span>
              <span className="font-semibold text-gray-800">{stats.avg_time_per_annotation_secs}s</span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-500">Thumbs Up</span>
              <span className="font-semibold text-green-500">{stats.thumbs_up_count}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-500">Thumbs Down</span>
              <span className="font-semibold text-red-500">{stats.thumbs_down_count}</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default AnnotationQueue;
