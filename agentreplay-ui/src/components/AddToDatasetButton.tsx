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

import { useState, useEffect } from 'react';
import { Database, Plus, X, CheckCircle, AlertCircle, ChevronDown } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { agentreplayClient, EvalDataset } from '../lib/agentreplay-api';

interface AddToDatasetButtonProps {
  traceId: string;
  traceMetadata?: Record<string, any>;
  className?: string;
}

interface DatasetOption {
  id: string;
  name: string;
  size: number;
}

export function AddToDatasetButton({ traceId, traceMetadata, className = '' }: AddToDatasetButtonProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [datasets, setDatasets] = useState<DatasetOption[]>([]);
  const [loading, setLoading] = useState(false);
  const [adding, setAdding] = useState(false);
  const [success, setSuccess] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showNewDataset, setShowNewDataset] = useState(false);
  const [newDatasetName, setNewDatasetName] = useState('');

  useEffect(() => {
    if (isOpen) {
      fetchDatasets();
    }
  }, [isOpen]);

  const fetchDatasets = async () => {
    setLoading(true);
    try {
      const response = await agentreplayClient.listDatasets();
      const normalizedDatasets = (response.datasets || []).map((d: any) => ({
        id: d.dataset_id || d.id,
        name: d.name,
        size: d.examples?.length || d.test_case_count || d.test_cases?.length || 0,
      }));
      setDatasets(normalizedDatasets);
    } catch (err) {
      console.error('Failed to fetch datasets:', err);
      setError('Failed to load datasets');
    } finally {
      setLoading(false);
    }
  };

  const extractTestCaseFromTrace = () => {
    // Extract input from gen_ai.prompt.* attributes
    const prompts: { role: string; content: string }[] = [];
    if (traceMetadata) {
      // Find all prompt entries
      const promptIndices = new Set<number>();
      Object.keys(traceMetadata).forEach(key => {
        const match = key.match(/^gen_ai\.prompt\.(\d+)\./);
        if (match) promptIndices.add(parseInt(match[1]));
      });

      // Sort and extract
      Array.from(promptIndices).sort((a, b) => a - b).forEach(idx => {
        const role = traceMetadata[`gen_ai.prompt.${idx}.role`] || 'user';
        const content = traceMetadata[`gen_ai.prompt.${idx}.content`] || '';
        if (content) {
          prompts.push({ role, content: parseContent(content) });
        }
      });
    }

    // Extract output/completion
    const completion = traceMetadata?.['gen_ai.completion.0.content'] ||
      traceMetadata?.output ||
      '';

    // Build input JSON
    const userPrompts = prompts.filter(p => p.role === 'user');
    const systemPrompts = prompts.filter(p => p.role === 'system');

    const input = JSON.stringify({
      query: userPrompts.length > 0 ? userPrompts[userPrompts.length - 1].content : '',
      system_prompt: systemPrompts.length > 0 ? systemPrompts[0].content : undefined,
      context: traceMetadata?.context || traceMetadata?.retrieved_context || undefined,
    });

    return {
      input,
      expected_output: parseContent(completion),
      metadata: {
        source_trace_id: traceId,
        original_model: traceMetadata?.['gen_ai.request.model'] || traceMetadata?.model || 'unknown',
        original_latency_ms: String(traceMetadata?.duration_ms || traceMetadata?.latency_ms || 0),
        original_cost: String(traceMetadata?.cost || 0),
        original_tokens: String(traceMetadata?.token_count || traceMetadata?.['gen_ai.usage.total_tokens'] || 0),
        imported_at: new Date().toISOString(),
      },
    };
  };

  const parseContent = (content: any): string => {
    if (typeof content !== 'string') return String(content || '');
    try {
      // Try to parse as JSON array (Claude format)
      const parsed = JSON.parse(content);
      if (Array.isArray(parsed) && parsed[0]?.text) {
        return parsed[0].text;
      }
      return content;
    } catch {
      return content;
    }
  };

  const checkDuplicate = async (datasetId: string): Promise<boolean> => {
    try {
      const response = await agentreplayClient.getDataset(datasetId);
      const dataset = response as any;
      const testCases = dataset.test_cases || dataset.examples || [];
      return testCases.some((tc: any) =>
        tc.metadata?.source_trace_id === traceId
      );
    } catch {
      return false;
    }
  };

  const handleAddToDataset = async (datasetId: string) => {
    setAdding(true);
    setError(null);
    try {
      // Check for duplicates first
      const isDuplicate = await checkDuplicate(datasetId);
      if (isDuplicate) {
        setError('This trace is already in the dataset');
        setAdding(false);
        return;
      }

      const testCase = extractTestCaseFromTrace();

      await agentreplayClient.addExamples(datasetId, [{
        example_id: `ex_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`,
        input: testCase.input,
        expected_output: testCase.expected_output,
        metadata: testCase.metadata,
      }]);

      const dataset = datasets.find(d => d.id === datasetId);
      setSuccess(`Added to "${dataset?.name || 'dataset'}"`);
      setTimeout(() => {
        setSuccess(null);
        setIsOpen(false);
      }, 2000);
    } catch (err) {
      console.error('Failed to add to dataset:', err);
      setError(err instanceof Error ? err.message : 'Failed to add to dataset');
    } finally {
      setAdding(false);
    }
  };

  const handleCreateAndAdd = async () => {
    if (!newDatasetName.trim()) return;

    setAdding(true);
    setError(null);
    try {
      // Create new dataset
      const response = await agentreplayClient.createDataset(
        newDatasetName.trim(),
        `Created from trace ${traceId}`
      );

      const datasetId = response.dataset_id;

      // Add the trace as a test case
      const testCase = extractTestCaseFromTrace();
      await agentreplayClient.addExamples(datasetId, [{
        example_id: `ex_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`,
        input: testCase.input,
        expected_output: testCase.expected_output,
        metadata: testCase.metadata,
      }]);

      setSuccess(`Created "${newDatasetName}" and added trace`);
      setNewDatasetName('');
      setShowNewDataset(false);

      // Refresh list to show new dataset
      fetchDatasets();

      setTimeout(() => {
        setSuccess(null);
        setIsOpen(false);
      }, 2000);
    } catch (err) {
      console.error('Failed to create dataset:', err);
      setError(err instanceof Error ? err.message : 'Failed to create dataset');
    } finally {
      setAdding(false);
    }
  };

  return (
    <div className={`relative ${className}`}>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-2 px-3 py-2 bg-primary/10 border border-primary/30 text-primary rounded-lg hover:bg-primary/20 transition-colors text-sm font-medium"
        title="Add to test dataset"
      >
        <Database className="w-4 h-4" />
        Add to Dataset
        <ChevronDown className={`w-3 h-3 transition-transform ${isOpen ? 'rotate-180' : ''}`} />
      </button>

      <AnimatePresence>
        {isOpen && (
          <motion.div
            initial={{ opacity: 0, y: -10, scale: 0.95 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -10, scale: 0.95 }}
            transition={{ duration: 0.15 }}
            className="absolute right-0 top-full mt-2 w-80 bg-surface border border-border rounded-xl shadow-xl z-50 overflow-hidden"
          >
            {/* Header */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-border bg-surface-elevated">
              <div className="flex items-center gap-2">
                <Database className="w-4 h-4 text-primary" />
                <span className="font-medium text-textPrimary">Add to Dataset</span>
              </div>
              <button
                onClick={() => setIsOpen(false)}
                className="p-1 hover:bg-surface-hover rounded transition-colors"
              >
                <X className="w-4 h-4 text-textSecondary" />
              </button>
            </div>

            {/* Content */}
            <div className="max-h-64 overflow-y-auto">
              {loading ? (
                <div className="p-4 text-center text-textSecondary">
                  Loading datasets...
                </div>
              ) : success ? (
                <div className="p-4 flex items-center gap-3 text-success">
                  <CheckCircle className="w-5 h-5" />
                  <span>{success}</span>
                </div>
              ) : (
                <>
                  {datasets.length === 0 ? (
                    <div className="p-4 text-center text-textSecondary">
                      <p className="mb-2">No datasets yet</p>
                      <p className="text-xs text-textTertiary">Create one below</p>
                    </div>
                  ) : (
                    <div className="py-1">
                      {datasets.map((dataset) => (
                        <button
                          key={dataset.id}
                          onClick={() => handleAddToDataset(dataset.id)}
                          disabled={adding}
                          className="w-full px-4 py-3 flex items-center justify-between hover:bg-surface-hover transition-colors disabled:opacity-50"
                        >
                          <div className="text-left">
                            <div className="font-medium text-textPrimary">{dataset.name}</div>
                            <div className="text-xs text-textTertiary">{dataset.size} test cases</div>
                          </div>
                          <Plus className="w-4 h-4 text-textTertiary" />
                        </button>
                      ))}
                    </div>
                  )}

                  {/* Create new dataset */}
                  <div className="border-t border-border p-3">
                    {showNewDataset ? (
                      <div className="space-y-2">
                        <input
                          type="text"
                          value={newDatasetName}
                          onChange={(e) => setNewDatasetName(e.target.value)}
                          placeholder="Dataset name..."
                          className="w-full px-3 py-2 bg-background border border-border rounded-lg text-sm text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-primary"
                          autoFocus
                        />
                        <div className="flex gap-2">
                          <button
                            onClick={() => setShowNewDataset(false)}
                            className="flex-1 px-3 py-1.5 text-sm border border-border rounded-lg text-textSecondary hover:bg-surface-hover"
                          >
                            Cancel
                          </button>
                          <button
                            onClick={handleCreateAndAdd}
                            disabled={!newDatasetName.trim() || adding}
                            className="flex-1 px-3 py-1.5 text-sm bg-primary text-white rounded-lg hover:bg-primary-hover disabled:opacity-50"
                          >
                            {adding ? 'Creating...' : 'Create & Add'}
                          </button>
                        </div>
                      </div>
                    ) : (
                      <button
                        onClick={() => setShowNewDataset(true)}
                        className="w-full flex items-center justify-center gap-2 px-3 py-2 text-sm text-primary hover:bg-primary/10 rounded-lg transition-colors"
                      >
                        <Plus className="w-4 h-4" />
                        Create New Dataset
                      </button>
                    )}
                  </div>
                </>
              )}

              {error && (
                <div className="px-4 py-2 flex items-center gap-2 text-error text-sm border-t border-border">
                  <AlertCircle className="w-4 h-4" />
                  {error}
                </div>
              )}
            </div>

            {/* Preview */}
            {!success && !loading && (
              <div className="px-4 py-3 border-t border-border bg-background/50">
                <div className="text-xs text-textTertiary mb-1">Will extract from this trace:</div>
                <div className="text-xs text-textSecondary">
                  • Input query from prompts<br />
                  • Expected output from completion<br />
                  • Metadata: model, latency, cost, tokens
                </div>
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
