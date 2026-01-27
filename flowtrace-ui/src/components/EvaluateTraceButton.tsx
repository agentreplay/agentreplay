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

import React, { useState } from 'react';
import { flowtraceClient, EvaluationResult, GEvalRequest, RagasRequest } from '../lib/flowtrace-api';

// ============================================================================
// Evaluation Presets
// ============================================================================

interface EvalPreset {
  id: string;
  name: string;
  description: string;
  type: 'geval' | 'ragas';
  criteria?: string[];
  icon: string;
}

const EVAL_PRESETS: EvalPreset[] = [
  {
    id: 'rag',
    name: 'RAG Quality',
    description: 'Context precision, recall, faithfulness & answer relevance',
    type: 'ragas',
    icon: '📚',
  },
  {
    id: 'agent',
    name: 'Agent Performance',
    description: 'Task completion, tool correctness & trajectory efficiency',
    type: 'geval',
    criteria: ['task_completion', 'tool_correctness', 'trajectory_efficiency'],
    icon: '🤖',
  },
  {
    id: 'quality',
    name: 'Output Quality',
    description: 'Coherence, relevance, fluency & helpfulness',
    type: 'geval',
    criteria: ['coherence', 'relevance', 'fluency', 'helpfulness'],
    icon: '✨',
  },
  {
    id: 'safety',
    name: 'Safety & Toxicity',
    description: 'Check for harmful, biased, or inappropriate content',
    type: 'geval',
    criteria: ['toxicity', 'bias', 'appropriateness'],
    icon: '🛡️',
  },
  {
    id: 'code',
    name: 'Code Quality',
    description: 'Correctness, style, security & documentation',
    type: 'geval',
    criteria: ['correctness', 'code_style', 'security', 'documentation'],
    icon: '💻',
  },
  {
    id: 'hallucination',
    name: 'Hallucination Detection (CIP)',
    description: 'Causal Integrity Protocol - counterfactual analysis for hallucinations',
    type: 'geval',
    criteria: ['factual_grounding', 'source_attribution', 'claim_consistency', 'counterfactual_robustness'],
    icon: '🔍',
  },
];

// ============================================================================
// Evaluate Trace Modal Component
// ============================================================================

interface EvaluateTraceModalProps {
  isOpen: boolean;
  onClose: () => void;
  traceId: string;
  traceMetadata?: Record<string, any>;
}

export const EvaluateTraceModal: React.FC<EvaluateTraceModalProps> = ({
  isOpen,
  onClose,
  traceId,
  traceMetadata,
}) => {
  const [selectedPreset, setSelectedPreset] = useState<EvalPreset | null>(null);
  const [customCriteria, setCustomCriteria] = useState<string[]>([]);
  const [newCriterion, setNewCriterion] = useState('');
  const [loading, setLoading] = useState(false);
  const [loadingStatus, setLoadingStatus] = useState<string>('');
  const [results, setResults] = useState<EvaluationResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  
  // For RAGAS evaluation
  const [ragasInputs, setRagasInputs] = useState({
    question: traceMetadata?.prompt || traceMetadata?.input || '',
    answer: traceMetadata?.response || traceMetadata?.output || '',
    context: [] as string[],
    ground_truth: '',
  });
  const [newContext, setNewContext] = useState('');

  const handleAddCriterion = () => {
    if (newCriterion && !customCriteria.includes(newCriterion)) {
      setCustomCriteria([...customCriteria, newCriterion]);
      setNewCriterion('');
    }
  };

  const handleRemoveCriterion = (criterion: string) => {
    setCustomCriteria(customCriteria.filter(c => c !== criterion));
  };

  const handleAddContext = () => {
    if (newContext && !ragasInputs.context.includes(newContext)) {
      setRagasInputs({ ...ragasInputs, context: [...ragasInputs.context, newContext] });
      setNewContext('');
    }
  };

  const handleRemoveContext = (index: number) => {
    setRagasInputs({
      ...ragasInputs,
      context: ragasInputs.context.filter((_, i) => i !== index),
    });
  };

  const handleRunEvaluation = async () => {
    if (!selectedPreset) return;
    
    setLoading(true);
    setError(null);
    setResults(null);
    setLoadingStatus('Preparing evaluation...');

    try {
      let result: EvaluationResult;

      if (selectedPreset.type === 'ragas') {
        setLoadingStatus('Running RAG evaluation (context, faithfulness, relevance)...');
        const request: RagasRequest = {
          trace_id: traceId,
          question: ragasInputs.question,
          answer: ragasInputs.answer,
          context: ragasInputs.context,
          ground_truth: ragasInputs.ground_truth || undefined,
        };
        result = await flowtraceClient.runRagas(request);
      } else {
        const allCriteria = [...(selectedPreset.criteria || []), ...customCriteria];
        setLoadingStatus(`Calling LLM to evaluate ${allCriteria.length} criteria...`);
        const request: GEvalRequest = {
          trace_id: traceId,
          criteria: allCriteria,
        };
        result = await flowtraceClient.runGEval(request);
      }

      setLoadingStatus('');
      setResults(result);
    } catch (err: any) {
      setError(err.message || 'Failed to run evaluation');
      setLoadingStatus('');
    } finally {
      setLoading(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-white dark:bg-gray-800 rounded-lg w-full max-w-3xl max-h-[90vh] overflow-hidden flex flex-col">
        <div className="p-6 border-b flex items-center justify-between">
          <div>
            <h2 className="text-xl font-bold">Evaluate Trace</h2>
            <p className="text-sm text-gray-600 dark:text-gray-400">
              Run quality evaluations using LLM-as-judge
            </p>
          </div>
          <button onClick={onClose} className="text-gray-500 hover:text-gray-700 text-2xl">
            ×
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-6">
          {/* Loading Progress */}
          {loading && (
            <div className="mb-6 p-4 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg">
              <div className="flex items-center gap-3">
                <div className="animate-spin rounded-full h-5 w-5 border-2 border-blue-600 border-t-transparent" />
                <div>
                  <p className="font-medium text-blue-800 dark:text-blue-200">Running Evaluation...</p>
                  <p className="text-sm text-blue-600 dark:text-blue-300">{loadingStatus}</p>
                </div>
              </div>
              <div className="mt-3 h-1.5 bg-blue-200 dark:bg-blue-800 rounded-full overflow-hidden">
                <div className="h-full bg-blue-600 rounded-full animate-pulse" style={{ width: '60%' }} />
              </div>
            </div>
          )}

          {/* Preset Selection */}
          {!results && !loading && (
            <>
              <div className="mb-6">
                <h3 className="font-medium mb-3">Select Evaluation Type</h3>
                <div className="grid grid-cols-2 gap-3">
                  {EVAL_PRESETS.map((preset) => (
                    <button
                      key={preset.id}
                      onClick={() => setSelectedPreset(preset)}
                      className={`p-4 border rounded-lg text-left hover:shadow-md transition-all ${
                        selectedPreset?.id === preset.id
                          ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
                          : 'border-gray-200 dark:border-gray-700'
                      }`}
                    >
                      <div className="flex items-center gap-2 mb-1">
                        <span className="text-xl">{preset.icon}</span>
                        <span className="font-medium">{preset.name}</span>
                      </div>
                      <p className="text-sm text-gray-600 dark:text-gray-400">
                        {preset.description}
                      </p>
                    </button>
                  ))}
                </div>
              </div>

              {/* Custom Criteria (for G-Eval) */}
              {selectedPreset && selectedPreset.type === 'geval' && (
                <div className="mb-6">
                  <h3 className="font-medium mb-3">Evaluation Criteria</h3>
                  <div className="flex flex-wrap gap-2 mb-2">
                    {selectedPreset.criteria?.map((c) => (
                      <span key={c} className="px-3 py-1 bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200 rounded-full text-sm">
                        {c}
                      </span>
                    ))}
                    {customCriteria.map((c) => (
                      <span key={c} className="px-3 py-1 bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200 rounded-full text-sm flex items-center gap-1">
                        {c}
                        <button onClick={() => handleRemoveCriterion(c)} className="ml-1 text-red-500">×</button>
                      </span>
                    ))}
                  </div>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={newCriterion}
                      onChange={(e) => setNewCriterion(e.target.value)}
                      className="flex-1 p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
                      placeholder="Add custom criterion..."
                      onKeyPress={(e) => e.key === 'Enter' && handleAddCriterion()}
                    />
                    <button
                      onClick={handleAddCriterion}
                      className="px-4 py-2 bg-gray-200 dark:bg-gray-600 rounded hover:bg-gray-300"
                    >
                      Add
                    </button>
                  </div>
                </div>
              )}

              {/* RAGAS Inputs */}
              {selectedPreset && selectedPreset.type === 'ragas' && (
                <div className="mb-6 space-y-4">
                  <h3 className="font-medium mb-3">RAG Evaluation Inputs</h3>
                  
                  <div>
                    <label className="block text-sm font-medium mb-1">Question/Query</label>
                    <textarea
                      value={ragasInputs.question}
                      onChange={(e) => setRagasInputs({ ...ragasInputs, question: e.target.value })}
                      className="w-full p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
                      rows={2}
                      placeholder="The user's question..."
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-1">Answer/Response</label>
                    <textarea
                      value={ragasInputs.answer}
                      onChange={(e) => setRagasInputs({ ...ragasInputs, answer: e.target.value })}
                      className="w-full p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
                      rows={3}
                      placeholder="The LLM's response..."
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-1">Retrieved Context</label>
                    <div className="space-y-2 mb-2">
                      {ragasInputs.context.map((ctx, i) => (
                        <div key={i} className="flex items-start gap-2 p-2 bg-gray-50 dark:bg-gray-700 rounded">
                          <p className="flex-1 text-sm">{ctx.substring(0, 150)}...</p>
                          <button onClick={() => handleRemoveContext(i)} className="text-red-500">×</button>
                        </div>
                      ))}
                    </div>
                    <div className="flex gap-2">
                      <input
                        type="text"
                        value={newContext}
                        onChange={(e) => setNewContext(e.target.value)}
                        className="flex-1 p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
                        placeholder="Add retrieved document/chunk..."
                        onKeyPress={(e) => e.key === 'Enter' && handleAddContext()}
                      />
                      <button
                        onClick={handleAddContext}
                        className="px-4 py-2 bg-gray-200 dark:bg-gray-600 rounded hover:bg-gray-300"
                      >
                        Add
                      </button>
                    </div>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-1">Ground Truth (Optional)</label>
                    <textarea
                      value={ragasInputs.ground_truth}
                      onChange={(e) => setRagasInputs({ ...ragasInputs, ground_truth: e.target.value })}
                      className="w-full p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
                      rows={2}
                      placeholder="Expected correct answer for comparison..."
                    />
                  </div>
                </div>
              )}
            </>
          )}

          {/* Results Display */}
          {results && (
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <h3 className="font-medium">Evaluation Results</h3>
                <button
                  onClick={() => setResults(null)}
                  className="text-sm text-blue-600 hover:text-blue-800"
                >
                  Run Another Evaluation
                </button>
              </div>

              <div className="p-4 bg-gray-50 dark:bg-gray-700 rounded-lg">
                <div className="flex items-center justify-between mb-4">
                  <div>
                    <span className="text-sm text-gray-600 dark:text-gray-400">Overall Score</span>
                    <div className="text-3xl font-bold">
                      {(results.score * 100).toFixed(1)}%
                    </div>
                    {results.passed !== undefined && (
                      <span className={`text-xs px-2 py-0.5 rounded ${results.passed ? 'bg-green-100 text-green-700' : 'bg-red-100 text-red-700'}`}>
                        {results.passed ? '✓ Passed' : '✗ Below Threshold'}
                      </span>
                    )}
                  </div>
                  <div className="text-right">
                    <span className="text-sm text-gray-600 dark:text-gray-400">Evaluator</span>
                    <div className="font-medium">{results.evaluator.toUpperCase()}</div>
                    <div className="text-xs text-gray-500">
                      {results.evaluation_time_ms}ms | {results.model_used}
                    </div>
                    {results.confidence !== undefined && (
                      <div className="text-xs text-gray-500">
                        Confidence: {(results.confidence * 100).toFixed(0)}%
                      </div>
                    )}
                  </div>
                </div>

                <div className="space-y-3">
                  <h4 className="text-sm font-medium">Detailed Scores</h4>
                  {Object.entries(results.details).map(([metric, score]) => (
                    <div key={metric} className="space-y-1">
                      <div className="flex items-center justify-between">
                        <span className="text-sm font-medium">{metric.replace(/_/g, ' ')}</span>
                        <div className="flex items-center gap-2">
                          <div className="w-32 h-2 bg-gray-200 dark:bg-gray-600 rounded-full overflow-hidden">
                            <div
                              className={`h-full ${
                                score >= 0.8 ? 'bg-green-500' : score >= 0.5 ? 'bg-yellow-500' : 'bg-red-500'
                              }`}
                              style={{ width: `${score * 100}%` }}
                            />
                          </div>
                          <span className="text-sm font-medium w-12 text-right">
                            {(score * 100).toFixed(0)}%
                          </span>
                        </div>
                      </div>
                      {/* Show explanation for this metric if available */}
                      {results.detail_explanations?.[metric] && (
                        <p className="text-xs text-gray-500 dark:text-gray-400 ml-1 italic">
                          💡 {results.detail_explanations[metric]}
                        </p>
                      )}
                    </div>
                  ))}
                </div>

                {/* Overall explanation */}
                {results.explanation && (
                  <div className="mt-4 pt-4 border-t border-gray-200 dark:border-gray-600">
                    <h4 className="text-sm font-medium mb-2">🧠 LLM Analysis</h4>
                    <p className="text-sm text-gray-600 dark:text-gray-300 whitespace-pre-wrap">
                      {results.explanation}
                    </p>
                  </div>
                )}

                {/* Cost info */}
                {results.cost_usd !== undefined && results.cost_usd > 0 && (
                  <div className="mt-2 text-xs text-gray-500">
                    Est. cost: ${results.cost_usd.toFixed(4)}
                  </div>
                )}
              </div>
            </div>
          )}

          {/* Error Display */}
          {error && (
            <div className="p-4 bg-red-100 text-red-800 rounded-lg">
              {error}
            </div>
          )}
        </div>

        <div className="p-6 border-t flex justify-end gap-2">
          <button
            onClick={onClose}
            className="px-4 py-2 border rounded hover:bg-gray-100 dark:hover:bg-gray-700"
          >
            Close
          </button>
          {!results && (
            <button
              onClick={handleRunEvaluation}
              disabled={!selectedPreset || loading}
              className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50 flex items-center gap-2"
            >
              {loading ? (
                <>
                  <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white" />
                  Evaluating...
                </>
              ) : (
                <>
                  🧪 Run Evaluation
                </>
              )}
            </button>
          )}
        </div>
      </div>
    </div>
  );
};

// ============================================================================
// Evaluate Trace Button Component
// ============================================================================

interface EvaluateTraceButtonProps {
  traceId: string;
  traceMetadata?: Record<string, any>;
  className?: string;
}

export const EvaluateTraceButton: React.FC<EvaluateTraceButtonProps> = ({
  traceId,
  traceMetadata,
  className = '',
}) => {
  const [showModal, setShowModal] = useState(false);

  return (
    <>
      <button
        onClick={() => setShowModal(true)}
        className={`flex items-center gap-2 px-3 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition-colors text-sm font-medium ${className}`}
        title="Run Evaluation"
      >
        🧪 Evaluate
      </button>

      <EvaluateTraceModal
        isOpen={showModal}
        onClose={() => setShowModal(false)}
        traceId={traceId}
        traceMetadata={traceMetadata}
      />
    </>
  );
};

export default EvaluateTraceButton;
