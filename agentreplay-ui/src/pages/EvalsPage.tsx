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

import { useState, useEffect, useRef } from 'react';
import { Link } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { agentreplayClient, EvalDataset as ApiDataset, EvalRun as ApiEvalRun, EvalExample } from '../lib/agentreplay-api';
import { DatasetFlywheel } from '../components/DatasetFlywheel';
import { VideoHelpButton } from '../components/VideoHelpButton';
import { GoldenTestCaseEditor, GoldenTestCase } from '../components/GoldenTestCaseEditor';
import { 
  StatisticalComparison,
  EnhancedMetricCard,
  ConfidenceInterval,
  type VariantStats,
  type StatisticalTestResult
} from '../../components/metrics';
import { 
  Database, 
  Plus, 
  Play, 
  GitCompare, 
  Download,
  Upload,
  Trash2,
  TrendingUp,
  Table,
  PlayCircle,
  Columns2,
  X,
  FileJson,
  AlertCircle,
  CheckCircle,
  Eye,
  ChevronLeft,
  Target,
  Edit
} from 'lucide-react';

interface Dataset {
  id: string;
  name: string;
  description?: string;
  size: number;
  created_at: string;
  source: 'manual' | 'production' | 'imported';
}

interface TestCase {
  id: string;
  input: string;
  expected_output: string;
  metadata?: Record<string, any>;
}

interface EvalRun {
  id: string;
  name: string;
  dataset_id: string;
  dataset_name: string;
  status: 'running' | 'completed' | 'failed';
  created_at: string;
  completed_at?: string;
  metrics: {
    total_cases: number;
    passed: number;
    failed: number;
    avg_latency_ms: number;
    avg_cost: number;
    groundedness?: number;
    context_relevance?: number;
    answer_relevance?: number;
  };
}

export default function EvalsPage() {
  const [activeTab, setActiveTab] = useState<'datasets' | 'runs' | 'compare'>('datasets');
  const [datasets, setDatasets] = useState<Dataset[]>([]);
  const [evalRuns, setEvalRuns] = useState<EvalRun[]>([]);
  const [selectedRuns, setSelectedRuns] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchData = async () => {
    try {
      const datasetsResponse = await agentreplayClient.listDatasets();
      const normalizedDatasets = normaliseDatasets(datasetsResponse.datasets || []);
      setDatasets(normalizedDatasets);

      const runsResponse = await agentreplayClient.listEvalRuns();
      const datasetNameMap = new Map(normalizedDatasets.map((dataset) => [dataset.id, dataset.name]));
      const normalizedRuns = normaliseRuns(runsResponse.runs || [], datasetNameMap);
      setEvalRuns(normalizedRuns);
    } catch (error) {
      console.error('Failed to fetch evals data:', error);
      setDatasets([]);
      setEvalRuns([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
  }, []);

  // Auto-refresh runs every 5 seconds if there are running evals
  useEffect(() => {
    const hasRunningEvals = evalRuns.some(r => r.status === 'running');
    if (!hasRunningEvals) return;

    const interval = setInterval(async () => {
      try {
        const runsResponse = await agentreplayClient.listEvalRuns();
        const datasetNameMap = new Map(datasets.map((dataset) => [dataset.id, dataset.name]));
        const normalizedRuns = normaliseRuns(runsResponse.runs || [], datasetNameMap);
        setEvalRuns(normalizedRuns);
      } catch (error) {
        console.error('Failed to refresh runs:', error);
      }
    }, 5000);

    return () => clearInterval(interval);
  }, [evalRuns, datasets]);

  return (
    <div className="min-h-screen bg-surface">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {/* Header */}
        <div className="mb-8 flex items-start justify-between">
          <div>
            <h1 className="text-3xl font-bold text-textPrimary mb-2">Evals & Testing</h1>
            <p className="text-textSecondary">
              Close the loop from production to development
            </p>
          </div>
          <VideoHelpButton pageId="evals" />
        </div>

        {/* Tabs */}
        <div className="inline-flex bg-surface-elevated border border-border rounded-xl p-1 mb-8">
          <button
            onClick={() => setActiveTab('datasets')}
            className={`px-6 py-3 rounded-lg font-semibold transition-all ${
              activeTab === 'datasets'
                ? 'bg-primary text-primary-foreground shadow-md'
                : 'bg-transparent text-textSecondary hover:bg-surface-hover hover:text-textPrimary'
            }`}
          >
            <div className="flex items-center gap-2">
              <Table className="w-5 h-5" />
              Datasets
            </div>
          </button>
          <button
            onClick={() => setActiveTab('runs')}
            className={`px-6 py-3 rounded-lg font-semibold transition-all ${
              activeTab === 'runs'
                ? 'bg-primary text-primary-foreground shadow-md'
                : 'bg-transparent text-textSecondary hover:bg-surface-hover hover:text-textPrimary'
            }`}
          >
            <div className="flex items-center gap-2">
              <PlayCircle className="w-5 h-5" />
              Runs
            </div>
          </button>
          <button
            onClick={() => setActiveTab('compare')}
            className={`px-6 py-3 rounded-lg font-semibold transition-all ${
              activeTab === 'compare'
                ? 'bg-primary text-primary-foreground shadow-md'
                : 'bg-transparent text-textSecondary hover:bg-surface-hover hover:text-textPrimary'
            }`}
          >
            <div className="flex items-center gap-2">
              <Columns2 className="w-5 h-5" />
              Compare
            </div>
          </button>
        </div>

        {/* Content */}
        {activeTab === 'datasets' ? (
          <DatasetsTab 
            datasets={datasets} 
            setDatasets={setDatasets} 
            onRunCreated={(newRun) => {
              setEvalRuns([newRun, ...evalRuns]);
              setActiveTab('runs');
            }}
          />
        ) : activeTab === 'runs' ? (
          <EvalRunsTab evalRuns={evalRuns} datasets={datasets} selectedRuns={selectedRuns} setSelectedRuns={setSelectedRuns} setEvalRuns={setEvalRuns} />
        ) : (
          <CompareTab evalRuns={evalRuns.filter(r => selectedRuns.includes(r.id))} />
        )}
      </div>
    </div>
  );
}

function normaliseDatasets(datasets: ApiDataset[]): Dataset[] {
  return datasets.map((dataset) => ({
    // Handle both 'id' (from server) and 'dataset_id' (from interface) 
    id: (dataset as any).id || dataset.dataset_id,
    name: dataset.name,
    description: dataset.description,
    size: dataset.examples?.length || (dataset as any).test_case_count || 0,
    created_at: new Date(dataset.created_at / 1000).toISOString(), // Convert microseconds to milliseconds
    source: 'manual',
  }));
}

function normaliseRuns(runs: ApiEvalRun[], datasetNameMap: Map<string, string>): EvalRun[] {
  return runs.map((run) => {
    const totalCases = run.results?.length || 0;
    const passed = run.results?.filter((result) => result.passed).length || 0;
    // Extract latency from eval_metrics or metadata
    const avgLatency =
      totalCases === 0
        ? 0
        : run.results.reduce((sum, result) => {
            const latency = result.eval_metrics?.latency_ms || (result as any).metadata?.latency_ms || 0;
            return sum + Number(latency);
          }, 0) / totalCases;
    const avgCost =
      totalCases === 0
        ? 0
        : run.results.reduce((sum, result) => {
            const cost = result.eval_metrics?.cost || (result as any).metadata?.cost || 0;
            return sum + Number(cost);
          }, 0) / totalCases;
    const groundedness = averageEvalMetric(run, 'groundedness');
    const contextRelevance = averageEvalMetric(run, 'context_relevance');
    const answerRelevance = averageEvalMetric(run, 'answer_relevance');

    // Handle both started_at (new) and created_at (old) timestamps
    const timestamp = run.started_at || run.created_at || Date.now() * 1000;
    const runId = run.run_id || (run as any).id || '';
    
    // Use status from response or derive from results
    const status = run.status || (totalCases === 0 ? 'running' : 'completed');

    return {
      id: runId,
      name: run.name,
      dataset_id: run.dataset_id,
      dataset_name: datasetNameMap.get(run.dataset_id) || run.dataset_id,
      status: status as 'running' | 'completed' | 'failed',
      created_at: new Date(timestamp / 1000).toISOString(), // Convert microseconds to milliseconds
      completed_at: run.completed_at ? new Date(run.completed_at / 1000).toISOString() : undefined,
      metrics: {
        total_cases: totalCases,
        passed,
        failed: totalCases - passed,
        avg_latency_ms: avgLatency,
        avg_cost: avgCost,
        groundedness,
        context_relevance: contextRelevance,
        answer_relevance: answerRelevance,
      },
    };
  });
}

function averageEvalMetric(run: ApiEvalRun, key: string) {
  if (!run.results?.length) return undefined;
  const sum = run.results.reduce((acc, result) => {
    const value = result.eval_metrics?.[key] || (result as any).metadata?.[key] || 0;
    return acc + Number(value);
  }, 0);
  if (!sum) return undefined;
  return sum / run.results.length;
}

// Expanded details component for eval runs
function EvalRunDetails({ runId }: { runId: string }) {
  const [run, setRun] = useState<ApiEvalRun | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchDetails = async () => {
      try {
        const result = await agentreplayClient.getEvalRun(runId);
        setRun(result);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load details');
      } finally {
        setLoading(false);
      }
    };
    fetchDetails();
  }, [runId]);

  if (loading) {
    return (
      <div className="mt-4 pt-4 border-t border-border text-center text-textSecondary text-sm">
        Loading detailed results...
      </div>
    );
  }

  if (error || !run?.results?.length) {
    return (
      <div className="mt-4 pt-4 border-t border-border text-center text-textSecondary text-sm">
        {error || 'No detailed results available yet'}
      </div>
    );
  }

  return (
    <div className="mt-4 pt-4 border-t border-border">
      <h4 className="text-sm font-medium text-textPrimary mb-3">📋 Individual Test Case Results</h4>
      <div className="max-h-80 overflow-auto space-y-2">
        {run.results.map((result, idx) => {
          const metrics = result.eval_metrics || {};
          const coherence = metrics.coherence || metrics.Coherence || 0;
          const relevance = metrics.relevance || metrics.Relevance || 0;
          const fluency = metrics.fluency || metrics.Fluency || 0;
          const helpfulness = metrics.helpfulness || metrics.Helpfulness || 0;
          const overall = metrics.overall || metrics.Overall || ((coherence + relevance + fluency + helpfulness) / 4);
          
          return (
            <div 
              key={result.test_case_id || idx}
              className={`p-3 rounded-lg border ${result.passed ? 'border-success/30 bg-success/5' : 'border-error/30 bg-error/5'}`}
            >
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs font-mono text-textTertiary">
                  Test #{idx + 1} • {result.test_case_id?.substring(0, 8) || 'N/A'}
                </span>
                <span className={`text-xs font-medium px-2 py-0.5 rounded ${result.passed ? 'bg-success/20 text-success' : 'bg-error/20 text-error'}`}>
                  {result.passed ? '✓ Passed' : '✗ Failed'}
                </span>
              </div>
              <div className="grid grid-cols-5 gap-2 text-xs">
                <div className="text-center">
                  <div className="text-textTertiary">Coherence</div>
                  <div className={`font-semibold ${coherence >= 3.5 ? 'text-success' : coherence >= 2.5 ? 'text-warning' : 'text-error'}`}>
                    {coherence.toFixed(1)}
                  </div>
                </div>
                <div className="text-center">
                  <div className="text-textTertiary">Relevance</div>
                  <div className={`font-semibold ${relevance >= 3.5 ? 'text-success' : relevance >= 2.5 ? 'text-warning' : 'text-error'}`}>
                    {relevance.toFixed(1)}
                  </div>
                </div>
                <div className="text-center">
                  <div className="text-textTertiary">Fluency</div>
                  <div className={`font-semibold ${fluency >= 3.5 ? 'text-success' : fluency >= 2.5 ? 'text-warning' : 'text-error'}`}>
                    {fluency.toFixed(1)}
                  </div>
                </div>
                <div className="text-center">
                  <div className="text-textTertiary">Helpful</div>
                  <div className={`font-semibold ${helpfulness >= 3.5 ? 'text-success' : helpfulness >= 2.5 ? 'text-warning' : 'text-error'}`}>
                    {helpfulness.toFixed(1)}
                  </div>
                </div>
                <div className="text-center">
                  <div className="text-textTertiary font-medium">Overall</div>
                  <div className={`font-bold ${overall >= 3.5 ? 'text-success' : overall >= 2.5 ? 'text-warning' : 'text-error'}`}>
                    {overall.toFixed(1)}/5
                  </div>
                </div>
              </div>
              {result.error && (
                <div className="mt-2 text-xs text-error bg-error/10 rounded px-2 py-1">
                  {result.error}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

// Test Case interface for the form
interface TestCaseForm {
  id: string;
  input: string;
  expected_output: string;
  metadata: { key: string; value: string }[];
}

// Create Dataset Modal with Test Cases support
function CreateDatasetModal({ 
  isOpen, 
  onClose, 
  onCreated 
}: { 
  isOpen: boolean; 
  onClose: () => void; 
  onCreated: (dataset: Dataset) => void;
}) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [testCases, setTestCases] = useState<TestCaseForm[]>([]);
  const [showTestCaseForm, setShowTestCaseForm] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'basic' | 'testcases'>('basic');

  // Current test case being edited
  const [currentTestCase, setCurrentTestCase] = useState<TestCaseForm>({
    id: '',
    input: '',
    expected_output: '',
    metadata: [],
  });

  const resetForm = () => {
    setName('');
    setDescription('');
    setTestCases([]);
    setShowTestCaseForm(false);
    setCurrentTestCase({ id: '', input: '', expected_output: '', metadata: [] });
    setActiveTab('basic');
    setError(null);
  };

  const addTestCase = () => {
    if (!currentTestCase.input.trim()) {
      setError('Test case input is required');
      return;
    }

    const newTestCase: TestCaseForm = {
      id: `tc-${Date.now()}`,
      input: currentTestCase.input.trim(),
      expected_output: currentTestCase.expected_output.trim(),
      metadata: currentTestCase.metadata.filter(m => m.key.trim() && m.value.trim()),
    };

    setTestCases([...testCases, newTestCase]);
    setCurrentTestCase({ id: '', input: '', expected_output: '', metadata: [] });
    setShowTestCaseForm(false);
    setError(null);
  };

  const removeTestCase = (id: string) => {
    setTestCases(testCases.filter(tc => tc.id !== id));
  };

  const addMetadataField = () => {
    setCurrentTestCase({
      ...currentTestCase,
      metadata: [...currentTestCase.metadata, { key: '', value: '' }],
    });
  };

  const updateMetadataField = (index: number, field: 'key' | 'value', value: string) => {
    const newMetadata = [...currentTestCase.metadata];
    newMetadata[index][field] = value;
    setCurrentTestCase({ ...currentTestCase, metadata: newMetadata });
  };

  const removeMetadataField = (index: number) => {
    setCurrentTestCase({
      ...currentTestCase,
      metadata: currentTestCase.metadata.filter((_, i) => i !== index),
    });
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      setError('Dataset name is required');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const result = await agentreplayClient.createDataset(name.trim(), description.trim() || undefined);
      
      // If we have test cases, add them
      if (testCases.length > 0) {
        const examples: EvalExample[] = testCases.map(tc => ({
          example_id: tc.id,
          input: tc.input,
          expected_output: tc.expected_output || undefined,
          metadata: tc.metadata.reduce((acc, m) => {
            if (m.key.trim() && m.value.trim()) {
              acc[m.key.trim()] = m.value.trim();
            }
            return acc;
          }, {} as Record<string, string>),
        }));
        
        await agentreplayClient.addExamples(result.dataset_id, examples);
      }

      onCreated({
        id: result.dataset_id,
        name: name.trim(),
        description: description.trim() || undefined,
        size: testCases.length,
        created_at: new Date().toISOString(),
        source: 'manual',
      });
      resetForm();
      onClose();
    } catch (err) {
      console.error('Failed to create dataset:', err);
      setError(err instanceof Error ? err.message : 'Failed to create dataset');
    } finally {
      setLoading(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={onClose}>
      <motion.div
        initial={{ opacity: 0, scale: 0.95 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.95 }}
        className="bg-surface rounded-xl border border-border p-6 w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-bold text-textPrimary">Create Dataset</h2>
          <button onClick={() => { resetForm(); onClose(); }} className="text-textTertiary hover:text-textPrimary transition-colors">
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Tab Navigation */}
        <div className="flex gap-2 mb-6 border-b border-border">
          <button
            onClick={() => setActiveTab('basic')}
            className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
              activeTab === 'basic'
                ? 'border-primary text-primary'
                : 'border-transparent text-textSecondary hover:text-textPrimary'
            }`}
          >
            Basic Info
          </button>
          <button
            onClick={() => setActiveTab('testcases')}
            className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
              activeTab === 'testcases'
                ? 'border-primary text-primary'
                : 'border-transparent text-textSecondary hover:text-textPrimary'
            }`}
          >
            Test Cases ({testCases.length})
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          {activeTab === 'basic' && (
            <>
              <div>
                <label className="block text-sm font-medium text-textSecondary mb-2">
                  Dataset Name *
                </label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="e.g., Customer Support Golden Set"
                  className="w-full px-4 py-3 bg-background border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-primary"
                  autoFocus
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-textSecondary mb-2">
                  Description
                </label>
                <textarea
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="Describe the purpose of this dataset..."
                  rows={3}
                  className="w-full px-4 py-3 bg-background border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-primary resize-none"
                />
              </div>

              <div className="p-4 bg-info/10 border border-info/20 rounded-lg">
                <p className="text-sm text-textSecondary">
                  <strong className="text-info">💡 Tip:</strong> You can add test cases now or import them later. 
                  Switch to the "Test Cases" tab to add input/output pairs for evaluation.
                </p>
              </div>
            </>
          )}

          {activeTab === 'testcases' && (
            <div className="space-y-4">
              {/* Existing Test Cases */}
              {testCases.length > 0 && (
                <div className="space-y-2">
                  <label className="block text-sm font-medium text-textSecondary">
                    Added Test Cases
                  </label>
                  {testCases.map((tc, index) => (
                    <div key={tc.id} className="flex items-start gap-3 p-3 bg-background rounded-lg border border-border">
                      <div className="flex-1 min-w-0">
                        <div className="text-sm font-medium text-textPrimary truncate">
                          #{index + 1}: {tc.input.substring(0, 60)}{tc.input.length > 60 ? '...' : ''}
                        </div>
                        {tc.expected_output && (
                          <div className="text-xs text-textTertiary mt-1 truncate">
                            Expected: {tc.expected_output.substring(0, 50)}{tc.expected_output.length > 50 ? '...' : ''}
                          </div>
                        )}
                        {tc.metadata.length > 0 && (
                          <div className="flex gap-1 mt-1 flex-wrap">
                            {tc.metadata.map((m, i) => (
                              <span key={i} className="text-xs px-2 py-0.5 bg-surface-elevated rounded text-textSecondary">
                                {m.key}: {m.value}
                              </span>
                            ))}
                          </div>
                        )}
                      </div>
                      <button
                        type="button"
                        onClick={() => removeTestCase(tc.id)}
                        className="text-textTertiary hover:text-error transition-colors p-1"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  ))}
                </div>
              )}

              {/* Add Test Case Form */}
              {showTestCaseForm ? (
                <div className="p-4 bg-background rounded-lg border border-border space-y-4">
                  <div className="flex items-center justify-between">
                    <h4 className="text-sm font-medium text-textPrimary">New Test Case</h4>
                    <button
                      type="button"
                      onClick={() => setShowTestCaseForm(false)}
                      className="text-textTertiary hover:text-textPrimary"
                    >
                      <X className="w-4 h-4" />
                    </button>
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-textSecondary mb-1">
                      Input (Query/Prompt) *
                    </label>
                    <textarea
                      value={currentTestCase.input}
                      onChange={(e) => setCurrentTestCase({ ...currentTestCase, input: e.target.value })}
                      placeholder='e.g., {"query": "How do I reset my password?", "context": ["Users can reset passwords via Settings"]}'
                      rows={3}
                      className="w-full px-3 py-2 bg-surface border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-primary text-sm font-mono resize-none"
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-textSecondary mb-1">
                      Expected Output (Ground Truth)
                    </label>
                    <textarea
                      value={currentTestCase.expected_output}
                      onChange={(e) => setCurrentTestCase({ ...currentTestCase, expected_output: e.target.value })}
                      placeholder="e.g., Go to Settings > Security and click 'Reset Password'"
                      rows={2}
                      className="w-full px-3 py-2 bg-surface border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-primary text-sm resize-none"
                    />
                  </div>

                  <div>
                    <div className="flex items-center justify-between mb-1">
                      <label className="block text-xs font-medium text-textSecondary">
                        Metadata (Optional)
                      </label>
                      <button
                        type="button"
                        onClick={addMetadataField}
                        className="text-xs text-primary hover:text-primary-hover"
                      >
                        + Add Field
                      </button>
                    </div>
                    {currentTestCase.metadata.map((m, index) => (
                      <div key={index} className="flex gap-2 mb-2">
                        <input
                          type="text"
                          value={m.key}
                          onChange={(e) => updateMetadataField(index, 'key', e.target.value)}
                          placeholder="Key (e.g., category)"
                          className="flex-1 px-3 py-2 bg-surface border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-primary text-sm"
                        />
                        <input
                          type="text"
                          value={m.value}
                          onChange={(e) => updateMetadataField(index, 'value', e.target.value)}
                          placeholder="Value (e.g., account)"
                          className="flex-1 px-3 py-2 bg-surface border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-primary text-sm"
                        />
                        <button
                          type="button"
                          onClick={() => removeMetadataField(index)}
                          className="text-textTertiary hover:text-error p-2"
                        >
                          <X className="w-4 h-4" />
                        </button>
                      </div>
                    ))}
                    {currentTestCase.metadata.length === 0 && (
                      <p className="text-xs text-textTertiary">
                        Add metadata like category, difficulty, source, etc.
                      </p>
                    )}
                  </div>

                  <div className="flex justify-end gap-2">
                    <button
                      type="button"
                      onClick={() => setShowTestCaseForm(false)}
                      className="px-3 py-1.5 text-sm rounded-lg bg-surface-hover border border-border hover:bg-surface-elevated text-textPrimary transition-colors"
                    >
                      Cancel
                    </button>
                    <button
                      type="button"
                      onClick={addTestCase}
                      style={{ backgroundColor: '#2563eb', color: 'white' }}
                      className="px-3 py-1.5 text-sm rounded-lg hover:bg-blue-700 transition-colors shadow-sm"
                    >
                      Add Test Case
                    </button>
                  </div>
                </div>
              ) : (
                <button
                  type="button"
                  onClick={() => setShowTestCaseForm(true)}
                  className="w-full py-3 border-2 border-dashed border-border rounded-lg text-textSecondary hover:border-primary hover:text-primary transition-colors flex items-center justify-center gap-2"
                >
                  <Plus className="w-4 h-4" />
                  Add Test Case
                </button>
              )}

              {testCases.length === 0 && !showTestCaseForm && (
                <div className="p-4 bg-surface-elevated rounded-lg text-center">
                  <p className="text-sm text-textSecondary">
                    No test cases added yet. You can add them now or import later.
                  </p>
                </div>
              )}
            </div>
          )}

          {error && (
            <div className="flex items-center gap-2 text-error text-sm">
              <AlertCircle className="w-4 h-4" />
              {error}
            </div>
          )}

          <div className="flex justify-end gap-3 pt-4 border-t border-border">
            <button
              type="button"
              onClick={() => { resetForm(); onClose(); }}
              className="px-4 py-2 rounded-lg bg-surface-hover border border-border hover:bg-surface-elevated text-textPrimary transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading || !name.trim()}
              style={{ backgroundColor: '#2563eb', color: 'white' }}
              className="px-4 py-2 rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed shadow-lg"
            >
              {loading ? 'Creating...' : `Create Dataset${testCases.length > 0 ? ` (${testCases.length} cases)` : ''}`}
            </button>
          </div>
        </form>
      </motion.div>
    </div>
  );
}

// Import Dataset Modal
function ImportDatasetModal({ 
  isOpen, 
  onClose, 
  onImported 
}: { 
  isOpen: boolean; 
  onClose: () => void; 
  onImported: (dataset: Dataset) => void;
}) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [file, setFile] = useState<File | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [parsePreview, setParsePreview] = useState<{ count: number; sample?: any } | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFile = e.target.files?.[0];
    if (!selectedFile) return;

    setFile(selectedFile);
    setError(null);
    setParsePreview(null);

    // Try to parse and preview the file
    try {
      const text = await selectedFile.text();
      const data = JSON.parse(text);
      
      let examples: any[] = [];
      if (Array.isArray(data)) {
        examples = data;
      } else if (data.examples && Array.isArray(data.examples)) {
        examples = data.examples;
        if (data.name && !name) setName(data.name);
        if (data.description && !description) setDescription(data.description);
      } else if (data.test_cases && Array.isArray(data.test_cases)) {
        examples = data.test_cases;
      }

      if (examples.length === 0) {
        setError('No examples found in file. Expected an array or object with "examples" or "test_cases" field.');
        return;
      }

      setParsePreview({
        count: examples.length,
        sample: examples[0],
      });
    } catch (err) {
      setError('Invalid JSON file. Please upload a valid JSON file.');
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      setError('Dataset name is required');
      return;
    }
    if (!file) {
      setError('Please select a file to import');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      // Parse file content
      const text = await file.text();
      const data = JSON.parse(text);
      
      let examples: any[] = [];
      if (Array.isArray(data)) {
        examples = data;
      } else if (data.examples && Array.isArray(data.examples)) {
        examples = data.examples;
      } else if (data.test_cases && Array.isArray(data.test_cases)) {
        examples = data.test_cases;
      }

      // Create the dataset first
      const result = await agentreplayClient.createDataset(name.trim(), description.trim() || undefined);
      
      // Convert examples to EvalExample format
      const evalExamples: EvalExample[] = examples.map((ex, idx) => ({
        example_id: ex.id || `example-${idx}`,
        input: typeof ex.input === 'string' ? ex.input : JSON.stringify(ex.input || ex.prompt || ex.question || ''),
        expected_output: typeof ex.expected_output === 'string' 
          ? ex.expected_output 
          : (ex.expected_output ? JSON.stringify(ex.expected_output) : undefined) 
            || (ex.expected ? (typeof ex.expected === 'string' ? ex.expected : JSON.stringify(ex.expected)) : undefined)
            || (ex.answer ? (typeof ex.answer === 'string' ? ex.answer : JSON.stringify(ex.answer)) : undefined),
        context: ex.context,
        metadata: ex.metadata || {},
      }));

      // Add examples to the dataset
      if (evalExamples.length > 0) {
        await agentreplayClient.addExamples(result.dataset_id, evalExamples);
      }

      onImported({
        id: result.dataset_id,
        name: name.trim(),
        description: description.trim() || undefined,
        size: evalExamples.length,
        created_at: new Date().toISOString(),
        source: 'imported',
      });
      
      setName('');
      setDescription('');
      setFile(null);
      setParsePreview(null);
      onClose();
    } catch (err) {
      console.error('Failed to import dataset:', err);
      setError(err instanceof Error ? err.message : 'Failed to import dataset');
    } finally {
      setLoading(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={onClose}>
      <motion.div
        initial={{ opacity: 0, scale: 0.95 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.95 }}
        className="bg-surface rounded-xl border border-border p-6 w-full max-w-lg mx-4"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-bold text-textPrimary">Import Dataset</h2>
          <button onClick={onClose} className="text-textTertiary hover:text-textPrimary transition-colors">
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          {/* File Upload */}
          <div>
            <label className="block text-sm font-medium text-textSecondary mb-2">
              JSON File *
            </label>
            <input
              type="file"
              ref={fileInputRef}
              accept=".json"
              onChange={handleFileChange}
              className="hidden"
            />
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              className="w-full px-4 py-8 border-2 border-dashed border-border rounded-lg hover:border-primary transition-colors flex flex-col items-center gap-2"
            >
              <FileJson className="w-8 h-8 text-textTertiary" />
              <span className="text-textSecondary">
                {file ? file.name : 'Click to select a JSON file'}
              </span>
              <span className="text-xs text-textTertiary">
                Supports arrays or objects with "examples" field
              </span>
            </button>
          </div>

          {/* Parse Preview */}
          {parsePreview && (
            <div className="p-3 bg-success/10 border border-success/20 rounded-lg">
              <div className="flex items-center gap-2 text-success text-sm font-medium">
                <CheckCircle className="w-4 h-4" />
                Found {parsePreview.count} example{parsePreview.count !== 1 ? 's' : ''}
              </div>
              {parsePreview.sample && (
                <div className="mt-2 text-xs text-textTertiary">
                  Sample fields: {Object.keys(parsePreview.sample).join(', ')}
                </div>
              )}
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-textSecondary mb-2">
              Dataset Name *
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g., Imported Test Cases"
              className="w-full px-4 py-3 bg-background border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-primary"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-textSecondary mb-2">
              Description (optional)
            </label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Describe the purpose of this dataset..."
              rows={2}
              className="w-full px-4 py-3 bg-background border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-primary resize-none"
            />
          </div>

          {error && (
            <div className="flex items-center gap-2 text-error text-sm">
              <AlertCircle className="w-4 h-4" />
              {error}
            </div>
          )}

          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 rounded-lg bg-surface-hover border border-border hover:bg-surface-elevated text-textPrimary transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading || !name.trim() || !file || !parsePreview}
              style={{ backgroundColor: '#2563eb', color: 'white' }}
              className="px-4 py-2 rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed shadow-lg"
            >
              {loading ? 'Importing...' : 'Import Dataset'}
            </button>
          </div>
        </form>
      </motion.div>
    </div>
  );
}

// Create Run Modal
function CreateRunModal({ 
  isOpen, 
  onClose, 
  datasets,
  onCreated 
}: { 
  isOpen: boolean; 
  onClose: () => void;
  datasets: Dataset[];
  onCreated: (run: EvalRun) => void;
}) {
  const [name, setName] = useState('');
  const [selectedDatasetId, setSelectedDatasetId] = useState('');
  const [agentId, setAgentId] = useState('');
  const [model, setModel] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !selectedDatasetId) return;

    setLoading(true);
    setError('');

    try {
      const response = await agentreplayClient.createEvalRun({
        dataset_id: selectedDatasetId,
        name: name.trim(),
        agent_id: agentId.trim() || undefined,
        model: model.trim() || undefined,
      });

      const selectedDataset = datasets.find(d => d.id === selectedDatasetId);
      
      // Create a new run object for the UI
      const newRun: EvalRun = {
        id: response.run_id,
        name: name.trim(),
        dataset_id: selectedDatasetId,
        dataset_name: selectedDataset?.name || selectedDatasetId,
        status: 'running',
        created_at: new Date().toISOString(),
        metrics: {
          total_cases: 0,
          passed: 0,
          failed: 0,
          avg_latency_ms: 0,
          avg_cost: 0,
        },
      };

      onCreated(newRun);
      onClose();
      setName('');
      setSelectedDatasetId('');
      setAgentId('');
      setModel('');
    } catch (err) {
      console.error('Failed to create eval run:', err);
      setError(err instanceof Error ? err.message : 'Failed to create eval run');
    } finally {
      setLoading(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <motion.div
        initial={{ opacity: 0, scale: 0.95 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.95 }}
        className="bg-surface rounded-xl border border-border p-6 w-full max-w-lg mx-4"
      >
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-semibold text-textPrimary">Create Eval Run</h2>
          <button
            onClick={onClose}
            className="p-2 hover:bg-surface-hover rounded-lg transition-colors"
          >
            <X className="w-5 h-5 text-textSecondary" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-textSecondary mb-2">
              Dataset *
            </label>
            <select
              value={selectedDatasetId}
              onChange={(e) => setSelectedDatasetId(e.target.value)}
              className="w-full px-4 py-3 bg-background border border-border rounded-lg text-textPrimary focus:outline-none focus:border-primary"
              required
            >
              <option value="">Select a dataset...</option>
              {datasets.map((dataset) => (
                <option key={dataset.id} value={dataset.id}>
                  {dataset.name} ({dataset.size} test cases)
                </option>
              ))}
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-textSecondary mb-2">
              Run Name *
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g., GPT-4 Evaluation v1"
              className="w-full px-4 py-3 bg-background border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-primary"
              required
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-textSecondary mb-2">
              Agent ID (optional)
            </label>
            <input
              type="text"
              value={agentId}
              onChange={(e) => setAgentId(e.target.value)}
              placeholder="e.g., customer-support-agent"
              className="w-full px-4 py-3 bg-background border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-primary"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-textSecondary mb-2">
              Model (optional)
            </label>
            <input
              type="text"
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder="e.g., gpt-4o, claude-3-opus"
              className="w-full px-4 py-3 bg-background border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:border-primary"
            />
          </div>

          {error && (
            <div className="flex items-center gap-2 text-error text-sm">
              <AlertCircle className="w-4 h-4" />
              {error}
            </div>
          )}

          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 rounded-lg bg-surface-hover border border-border hover:bg-surface-elevated text-textPrimary transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading || !name.trim() || !selectedDatasetId}
              style={{ backgroundColor: '#2563eb', color: 'white' }}
              className="px-4 py-2 rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed shadow-lg"
            >
              {loading ? 'Creating...' : 'Create Run'}
            </button>
          </div>
        </form>
      </motion.div>
    </div>
  );
}

function DatasetsTab({ 
  datasets, 
  setDatasets,
  onRunCreated
}: { 
  datasets: Dataset[];
  setDatasets: (datasets: Dataset[]) => void;
  onRunCreated?: (run: EvalRun) => void;
}) {
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showImportModal, setShowImportModal] = useState(false);
  const [viewingDataset, setViewingDataset] = useState<{ id: string; name: string; description?: string; testCases: TestCase[] } | null>(null);
  const [loadingDataset, setLoadingDataset] = useState(false);
  const [runningDatasetId, setRunningDatasetId] = useState<string | null>(null);

  const handleDatasetCreated = (dataset: Dataset) => {
    setDatasets([dataset, ...datasets]);
  };

  const handleDatasetImported = (dataset: Dataset) => {
    setDatasets([dataset, ...datasets]);
  };

  const handleDeleteDataset = async (datasetId: string) => {
    if (!confirm('Are you sure you want to delete this dataset?')) return;
    try {
      await agentreplayClient.deleteDataset(datasetId);
      setDatasets(datasets.filter(d => d.id !== datasetId));
    } catch (error) {
      console.error('Failed to delete dataset:', error);
    }
  };

  const handleRunDataset = async (dataset: Dataset) => {
    setRunningDatasetId(dataset.id);
    try {
      const runName = `${dataset.name} - ${new Date().toLocaleDateString()} ${new Date().toLocaleTimeString()}`;
      const response = await agentreplayClient.createEvalRun({
        dataset_id: dataset.id,
        name: runName,
      });
      
      // Create a new run object for the UI
      const newRun: EvalRun = {
        id: response.run_id,
        name: runName,
        dataset_id: dataset.id,
        dataset_name: dataset.name,
        status: 'running',
        created_at: new Date().toISOString(),
        metrics: {
          total_cases: dataset.size,
          passed: 0,
          failed: 0,
          avg_latency_ms: 0,
          avg_cost: 0,
        },
      };
      
      if (onRunCreated) {
        onRunCreated(newRun);
      }
      
      // Show success toast or switch to runs tab
      alert(`Evaluation run "${runName}" started! Check the Runs tab for progress.`);
    } catch (error) {
      console.error('Failed to start eval run:', error);
      alert('Failed to start evaluation run. Please try again.');
    } finally {
      setRunningDatasetId(null);
    }
  };

  const handleViewDataset = async (datasetId: string) => {
    setLoadingDataset(true);
    try {
      const response = await agentreplayClient.getDataset(datasetId);
      const dataset = response as any;
      setViewingDataset({
        id: dataset.id || dataset.dataset_id,
        name: dataset.name,
        description: dataset.description,
        testCases: (dataset.test_cases || dataset.examples || []).map((tc: any) => ({
          id: tc.id || tc.example_id,
          input: typeof tc.input === 'string' ? tc.input : JSON.stringify(tc.input),
          expected_output: typeof tc.expected_output === 'string' ? tc.expected_output : JSON.stringify(tc.expected_output),
          metadata: tc.metadata,
        })),
      });
    } catch (error) {
      console.error('Failed to load dataset:', error);
    } finally {
      setLoadingDataset(false);
    }
  };

  const handleExportDataset = async (datasetId: string, name: string) => {
    try {
      const response = await agentreplayClient.getDataset(datasetId);
      const dataset = response as any;
      const exportData = {
        name: dataset.name,
        description: dataset.description,
        test_cases: dataset.test_cases || dataset.examples || [],
      };
      const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${name.replace(/[^a-z0-9]/gi, '_')}_dataset.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (error) {
      console.error('Failed to export dataset:', error);
    }
  };

  // If viewing a dataset, show the detail view
  if (viewingDataset) {
    return (
      <DatasetDetailView
        dataset={viewingDataset}
        onBack={() => setViewingDataset(null)}
        onRefresh={() => handleViewDataset(viewingDataset.id)}
      />
    );
  }

  return (
    <div>
      {/* Modals */}
      <AnimatePresence>
        {showCreateModal && (
          <CreateDatasetModal
            isOpen={showCreateModal}
            onClose={() => setShowCreateModal(false)}
            onCreated={handleDatasetCreated}
          />
        )}
        {showImportModal && (
          <ImportDatasetModal
            isOpen={showImportModal}
            onClose={() => setShowImportModal(false)}
            onImported={handleDatasetImported}
          />
        )}
      </AnimatePresence>

      {/* Actions */}
      <div className="flex items-center justify-between mb-6">
        <div className="text-sm text-textSecondary">
          {datasets.length} dataset{datasets.length !== 1 ? 's' : ''}
        </div>
        <div className="flex items-center gap-3">
          <button 
            onClick={() => setShowImportModal(true)}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-surface border border-border text-textPrimary hover:bg-surface-hover transition-colors"
          >
            <Upload className="w-4 h-4" />
            Import Dataset
          </button>
          <button 
            onClick={() => setShowCreateModal(true)}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-primary text-primary-foreground hover:bg-primary-hover transition-colors"
          >
            <Plus className="w-4 h-4" />
            Create Dataset
          </button>
        </div>
      </div>

      {/* Info Box - Golden Dataset Workflow */}
      <div className="mb-6 p-4 bg-gradient-to-r from-blue-500/10 to-purple-500/10 border border-blue-500/20 rounded-xl">
        <h3 className="font-semibold text-blue-500 mb-3 flex items-center gap-2">
          ✨ Building a Golden Evaluation Dataset
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-sm">
          <div className="bg-surface/80 rounded-lg p-3">
            <div className="font-medium text-textPrimary mb-1">1️⃣ Collect & Sample</div>
            <p className="text-textSecondary text-xs">
              Go to <Link to="/traces" className="text-primary hover:underline">Traces</Link> → Find interesting cases → Click "Add to Dataset"
            </p>
          </div>
          <div className="bg-surface/80 rounded-lg p-3">
            <div className="font-medium text-textPrimary mb-1">2️⃣ Add Ground Truth</div>
            <p className="text-textSecondary text-xs">
              Define expected tool calls, required keywords, and ideal responses for each test case
            </p>
          </div>
          <div className="bg-surface/80 rounded-lg p-3">
            <div className="font-medium text-textPrimary mb-1">3️⃣ Run Evaluations</div>
            <p className="text-textSecondary text-xs">
              Test your agent against the dataset and get automatic scoring with LLM-as-judge
            </p>
          </div>
        </div>
        <div className="mt-3 text-xs text-textTertiary">
          💡 <strong>Pro tip:</strong> Sample across different categories (happy path, edge cases, safety) for comprehensive coverage.
        </div>
      </div>

      {/* Dataset Flywheel - Auto-curate fine-tuning data */}
      <div className="mb-8">
        <DatasetFlywheel />
      </div>

      {/* Datasets Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {datasets.length === 0 ? (
          <div className="col-span-full text-center py-12 bg-surface rounded-xl border border-border">
            <Database className="w-12 h-12 text-textTertiary mx-auto mb-4 opacity-50" />
            <p className="text-textSecondary mb-2">No datasets yet</p>
            <p className="text-sm text-textTertiary mb-4">
              Create your first dataset to start testing
            </p>
            <button 
              onClick={() => setShowCreateModal(true)}
              className="px-6 py-2 bg-primary text-primary-foreground rounded-lg hover:bg-primary-hover transition-colors"
            >
              <Plus className="w-4 h-4 inline mr-2" />
              Create Dataset
            </button>
          </div>
        ) : (
          datasets.map((dataset, index) => (
            <motion.div
              key={dataset.id}
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: index * 0.1 }}
              onClick={() => handleViewDataset(dataset.id)}
              className="bg-surface rounded-xl border border-border p-6 hover:border-primary transition-all group cursor-pointer"
            >
              <div className="flex items-start justify-between mb-4">
                <div className="flex-1">
                  <h3 className="text-lg font-semibold text-textPrimary group-hover:text-primary transition-colors mb-1">
                    {dataset.name}
                  </h3>
                  {dataset.description && (
                    <p className="text-sm text-textSecondary">{dataset.description}</p>
                  )}
                </div>
                <button 
                  onClick={(e) => {
                    e.stopPropagation();
                    handleDeleteDataset(dataset.id);
                  }}
                  className="text-textTertiary hover:text-error transition-colors"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>

              <div className="flex items-center justify-between text-sm">
                <div className="text-textSecondary">
                  {dataset.size} test cases
                </div>
                <div className="text-textTertiary">
                  {new Date(dataset.created_at).toLocaleDateString()}
                </div>
              </div>

              <div className="mt-4 pt-4 border-t border-border flex gap-2">
                <button 
                  onClick={(e) => {
                    e.stopPropagation();
                    handleViewDataset(dataset.id);
                  }}
                  className="flex-1 px-3 py-2 rounded-lg bg-primary hover:bg-primary-hover text-primary-foreground text-sm font-medium transition-colors"
                >
                  <Eye className="w-3 h-3 inline mr-1" />
                  View
                </button>
                <button 
                  onClick={(e) => {
                    e.stopPropagation();
                    handleRunDataset(dataset);
                  }}
                  disabled={runningDatasetId === dataset.id}
                  className="flex-1 px-3 py-2 rounded-lg bg-success hover:bg-success/80 text-primary-foreground text-sm font-medium transition-colors disabled:opacity-50"
                >
                  <Play className="w-3 h-3 inline mr-1" />
                  {runningDatasetId === dataset.id ? 'Starting...' : 'Run'}
                </button>
                <button 
                  onClick={(e) => {
                    e.stopPropagation();
                    handleExportDataset(dataset.id, dataset.name);
                  }}
                  className="px-3 py-2 rounded-lg bg-background hover:bg-surface border border-border text-textPrimary text-sm font-medium transition-colors"
                >
                  <Download className="w-3 h-3 inline mr-1" />
                </button>
              </div>
            </motion.div>
          ))
        )}
      </div>
    </div>
  );
}

// Dataset Detail View Component
function DatasetDetailView({
  dataset,
  onBack,
  onRefresh,
}: {
  dataset: { id: string; name: string; description?: string; testCases: TestCase[] };
  onBack: () => void;
  onRefresh: () => void;
}) {
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [showGoldenEditor, setShowGoldenEditor] = useState(false);
  const [addingTestCase, setAddingTestCase] = useState(false);

  const handleDeleteTestCase = async (testCaseId: string) => {
    if (!confirm('Delete this test case?')) return;
    setDeletingId(testCaseId);
    try {
      await agentreplayClient.deleteExample(dataset.id, testCaseId);
      onRefresh();
    } catch (error) {
      console.error('Failed to delete test case:', error);
    } finally {
      setDeletingId(null);
    }
  };

  const parseInputDisplay = (input: string) => {
    try {
      const parsed = JSON.parse(input);
      if (parsed.query) return parsed.query;
      if (typeof parsed === 'string') return parsed;
      return input;
    } catch {
      return input;
    }
  };

  const handleSaveGoldenTestCase = async (goldenTestCase: GoldenTestCase) => {
    setAddingTestCase(true);
    try {
      // Convert golden test case to the format expected by the API
      const example: EvalExample = {
        example_id: goldenTestCase.id,
        input: JSON.stringify({
          system_prompt: goldenTestCase.input.system_prompt,
          query: goldenTestCase.input.user_query,
          context: goldenTestCase.input.context,
        }),
        expected_output: goldenTestCase.expected_outputs.ground_truth_answer,
        metadata: {
          category: goldenTestCase.category,
          complexity: goldenTestCase.complexity,
          expected_tool_calls: JSON.stringify(goldenTestCase.expected_outputs.expected_tool_calls),
          expected_contains: goldenTestCase.expected_outputs.expected_response_contains.join(','),
          expected_not_contains: goldenTestCase.expected_outputs.expected_response_not_contains.join(','),
          evaluation_criteria: JSON.stringify(goldenTestCase.evaluation_criteria),
          source_trace_id: goldenTestCase.metadata.source_trace_id,
          notes: goldenTestCase.metadata.notes,
        },
      };

      await agentreplayClient.addExamples(dataset.id, [example]);
      setShowGoldenEditor(false);
      onRefresh();
    } catch (error) {
      console.error('Failed to save test case:', error);
      alert('Failed to save test case. Please try again.');
    } finally {
      setAddingTestCase(false);
    }
  };

  // Show Golden Test Case Editor Modal
  if (showGoldenEditor) {
    return (
      <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
        <motion.div
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.95 }}
          className="bg-surface rounded-xl border border-border p-6 w-full max-w-3xl max-h-[90vh] overflow-y-auto"
        >
          <div className="flex items-center justify-between mb-6">
            <div>
              <h2 className="text-xl font-bold text-textPrimary">Add Golden Test Case</h2>
              <p className="text-sm text-textSecondary">Create a comprehensive test case with ground truth</p>
            </div>
            <button
              onClick={() => setShowGoldenEditor(false)}
              className="p-2 hover:bg-surface-hover rounded-lg transition-colors"
            >
              <X className="w-5 h-5 text-textSecondary" />
            </button>
          </div>
          
          <GoldenTestCaseEditor
            onSave={handleSaveGoldenTestCase}
            onCancel={() => setShowGoldenEditor(false)}
          />
        </motion.div>
      </div>
    );
  }

  return (
    <div>
      {/* Header */}
      <div className="flex items-center gap-4 mb-6">
        <button
          onClick={onBack}
          className="p-2 rounded-lg hover:bg-surface-hover transition-colors"
        >
          <ChevronLeft className="w-5 h-5 text-textSecondary" />
        </button>
        <div className="flex-1">
          <h2 className="text-xl font-bold text-textPrimary">{dataset.name}</h2>
          {dataset.description && (
            <p className="text-sm text-textSecondary">{dataset.description}</p>
          )}
        </div>
        <div className="flex items-center gap-3">
          <span className="text-sm text-textTertiary">
            {dataset.testCases.length} test case{dataset.testCases.length !== 1 ? 's' : ''}
          </span>
          <button
            onClick={() => setShowGoldenEditor(true)}
            className="flex items-center gap-2 px-4 py-2 bg-primary text-primary-foreground rounded-lg hover:bg-primary/90 transition-colors text-sm font-medium"
          >
            <Target className="w-4 h-4" />
            Add Golden Test Case
          </button>
        </div>
      </div>

      {/* Category Filter Info */}
      <div className="mb-4 p-3 bg-surface-elevated rounded-lg border border-border">
        <div className="flex items-center gap-4 text-xs text-textSecondary">
          <span className="font-medium">Categories:</span>
          <span className="px-2 py-0.5 bg-blue-500/10 text-blue-500 rounded">Component</span>
          <span className="px-2 py-0.5 bg-green-500/10 text-green-500 rounded">E2E Happy Path</span>
          <span className="px-2 py-0.5 bg-orange-500/10 text-orange-500 rounded">Edge Cases</span>
          <span className="px-2 py-0.5 bg-red-500/10 text-red-500 rounded">Safety</span>
        </div>
      </div>

      {/* Test Cases Table */}
      <div className="bg-surface rounded-xl border border-border overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-background border-b border-border">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase w-12">#</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase w-24">Category</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase">Input</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase">Expected Output</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase w-24">Source</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase w-16"></th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              {dataset.testCases.length === 0 ? (
                <tr>
                  <td colSpan={6} className="px-4 py-8 text-center text-textSecondary">
                    No test cases yet. Click "Add Golden Test Case" to get started.
                  </td>
                </tr>
              ) : (
                dataset.testCases.map((tc, index) => {
                  const category = tc.metadata?.category || 'uncategorized';
                  const getCategoryStyle = (cat: string) => {
                    if (cat.startsWith('component')) return 'bg-blue-500/10 text-blue-500';
                    if (cat.startsWith('e2e_happy')) return 'bg-green-500/10 text-green-500';
                    if (cat.startsWith('e2e_edge') || cat.startsWith('e2e_adversarial')) return 'bg-orange-500/10 text-orange-500';
                    if (cat.startsWith('safety')) return 'bg-red-500/10 text-red-500';
                    return 'bg-surface-elevated text-textTertiary';
                  };
                  const getCategoryLabel = (cat: string) => {
                    const labels: Record<string, string> = {
                      'component_router': 'Router',
                      'component_tool': 'Tool',
                      'component_response': 'Response',
                      'e2e_happy': 'Happy',
                      'e2e_edge': 'Edge',
                      'e2e_adversarial': 'Adversarial',
                      'safety_injection': 'Injection',
                      'safety_pii': 'PII',
                      'safety_offtopic': 'Off-topic',
                    };
                    return labels[cat] || cat;
                  };
                  
                  return (
                    <tr key={tc.id} className="hover:bg-surface-hover">
                      <td className="px-4 py-3 text-sm text-textTertiary">{index + 1}</td>
                      <td className="px-4 py-3">
                        <span className={`text-xs px-2 py-0.5 rounded ${getCategoryStyle(category)}`}>
                          {getCategoryLabel(category)}
                        </span>
                      </td>
                      <td className="px-4 py-3">
                        <div className="text-sm text-textPrimary max-w-md truncate" title={tc.input}>
                          {parseInputDisplay(tc.input)}
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <div className="text-sm text-textSecondary max-w-md truncate" title={tc.expected_output}>
                          {tc.expected_output.substring(0, 100)}{tc.expected_output.length > 100 ? '...' : ''}
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        {tc.metadata?.source_trace_id ? (
                          <Link
                            to={`/traces/${tc.metadata.source_trace_id}`}
                            className="text-xs text-primary hover:underline"
                          >
                            Trace
                          </Link>
                        ) : (
                          <span className="text-xs text-textTertiary">Manual</span>
                        )}
                      </td>
                      <td className="px-4 py-3">
                        <button
                          onClick={() => handleDeleteTestCase(tc.id)}
                          disabled={deletingId === tc.id}
                          className="text-textTertiary hover:text-error transition-colors disabled:opacity-50"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function EvalRunsTab({ 
  evalRuns,
  datasets,
  selectedRuns,
  setSelectedRuns,
  setEvalRuns
}: { 
  evalRuns: EvalRun[];
  datasets: Dataset[];
  selectedRuns: string[];
  setSelectedRuns: (runs: string[]) => void;
  setEvalRuns: (runs: EvalRun[]) => void;
}) {
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [expandedRunId, setExpandedRunId] = useState<string | null>(null);
  
  const toggleRunSelection = (runId: string) => {
    setSelectedRuns(
      selectedRuns.includes(runId)
        ? selectedRuns.filter(id => id !== runId)
        : [...selectedRuns, runId]
    );
  };

  const handleRunCreated = (newRun: EvalRun) => {
    setEvalRuns([newRun, ...evalRuns]);
  };

  return (
    <div className="space-y-4">
      {/* Explanation Banner */}
      <div className="bg-primary/5 border border-primary/20 rounded-xl p-4">
        <h3 className="font-medium text-primary mb-2">📊 How Eval Runs Work</h3>
        <p className="text-sm text-textSecondary mb-2">
          Each test case in your dataset is evaluated on 4 LLM-as-judge criteria (1-5 scale):
        </p>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-2 text-xs">
          <div className="bg-background rounded-lg p-2">
            <span className="font-medium text-textPrimary">Coherence</span>
            <p className="text-textTertiary">Logical structure</p>
          </div>
          <div className="bg-background rounded-lg p-2">
            <span className="font-medium text-textPrimary">Relevance</span>
            <p className="text-textTertiary">Addresses the input</p>
          </div>
          <div className="bg-background rounded-lg p-2">
            <span className="font-medium text-textPrimary">Fluency</span>
            <p className="text-textTertiary">Grammar &amp; clarity</p>
          </div>
          <div className="bg-background rounded-lg p-2">
            <span className="font-medium text-textPrimary">Helpfulness</span>
            <p className="text-textTertiary">Useful information</p>
          </div>
        </div>
        <p className="text-xs text-textTertiary mt-2">
          Pass threshold: Overall score ≥ 70%. Click any run to see detailed results.
        </p>
      </div>

      {/* Header with Create Button */}
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold text-textPrimary">Evaluation Runs</h2>
        <button
          onClick={() => setShowCreateModal(true)}
          disabled={datasets.length === 0}
          className="flex items-center gap-2 px-4 py-2 rounded-lg bg-primary text-primary-foreground hover:bg-primary-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <PlayCircle className="w-4 h-4" />
          New Eval Run
        </button>
      </div>

      {evalRuns.length === 0 ? (
        <div className="text-center py-12 bg-surface rounded-xl border border-border">
          <Play className="w-12 h-12 text-textTertiary mx-auto mb-4 opacity-50" />
          <p className="text-textSecondary mb-2">No eval runs yet</p>
          <p className="text-sm text-textTertiary">
            Run your first evaluation to see results here
          </p>
        </div>
      ) : (
        evalRuns.map((run) => (
          <motion.div
            key={run.id}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className={`bg-surface rounded-xl border p-6 transition-all ${
              selectedRuns.includes(run.id)
                ? 'border-primary'
                : 'border-border hover:border-border-hover'
            }`}
          >
            <div 
              className="flex items-start justify-between mb-4 cursor-pointer"
              onClick={() => setExpandedRunId(expandedRunId === run.id ? null : run.id)}
            >
              <div className="flex-1">
                <div className="flex items-center gap-3 mb-2">
                  <h3 className="text-lg font-semibold text-textPrimary">{run.name}</h3>
                  <span className={`px-2 py-1 rounded text-xs font-medium ${
                    run.status === 'completed' ? 'bg-success/20 text-success' :
                    run.status === 'running' ? 'bg-warning/20 text-warning' :
                    'bg-error/20 text-error'
                  }`}>
                    {run.status}
                  </span>
                  <span className="text-xs text-textTertiary">
                    {expandedRunId === run.id ? '▼ Click to collapse' : '▶ Click for details'}
                  </span>
                </div>
                <p className="text-sm text-textSecondary">Dataset: {run.dataset_name}</p>
              </div>
              <input
                type="checkbox"
                checked={selectedRuns.includes(run.id)}
                onChange={(e) => {
                  e.stopPropagation();
                  toggleRunSelection(run.id);
                }}
                className="w-5 h-5 rounded border-border cursor-pointer"
              />
            </div>

            {/* Metrics Grid */}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-4">
              <div>
                <div className="text-xs text-textTertiary mb-1">Total Cases</div>
                <div className="text-xl font-semibold text-textPrimary">{run.metrics.total_cases}</div>
              </div>
              <div>
                <div className="text-xs text-textTertiary mb-1">Pass Rate</div>
                <div className="text-xl font-semibold text-success">
                  {run.metrics.total_cases > 0 ? ((run.metrics.passed / run.metrics.total_cases) * 100).toFixed(1) : 0}%
                </div>
              </div>
              <div>
                <div className="text-xs text-textTertiary mb-1">Avg Latency</div>
                <div className="text-xl font-semibold text-textPrimary">
                  {run.metrics.avg_latency_ms.toFixed(0)}ms
                </div>
              </div>
              <div>
                <div className="text-xs text-textTertiary mb-1">Avg Cost</div>
                <div className="text-xl font-semibold text-textPrimary">
                  ${run.metrics.avg_cost.toFixed(4)}
                </div>
              </div>
            </div>

            {/* RAG Metrics */}
            {(run.metrics.groundedness || run.metrics.context_relevance || run.metrics.answer_relevance) && (
              <div className="pt-4 border-t border-border">
                <div className="text-xs text-textTertiary mb-2">RAG Evaluation Scores</div>
                <div className="grid grid-cols-3 gap-4">
                  {run.metrics.groundedness && (
                    <div>
                      <div className="text-sm text-textSecondary">Groundedness</div>
                      <div className="text-lg font-semibold text-textPrimary">
                        {(run.metrics.groundedness * 100).toFixed(1)}%
                      </div>
                    </div>
                  )}
                  {run.metrics.context_relevance && (
                    <div>
                      <div className="text-sm text-textSecondary">Context</div>
                      <div className="text-lg font-semibold text-textPrimary">
                        {(run.metrics.context_relevance * 100).toFixed(1)}%
                      </div>
                    </div>
                  )}
                  {run.metrics.answer_relevance && (
                    <div>
                      <div className="text-sm text-textSecondary">Answer</div>
                      <div className="text-lg font-semibold text-textPrimary">
                        {(run.metrics.answer_relevance * 100).toFixed(1)}%
                      </div>
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* Expanded Details Section */}
            {expandedRunId === run.id && (
              <EvalRunDetails runId={run.id} />
            )}

            <div className="mt-4 text-xs text-textTertiary">
              {new Date(run.created_at).toLocaleString()}
              {run.completed_at && ` • Completed ${new Date(run.completed_at).toLocaleString()}`}
            </div>
          </motion.div>
        ))
      )}

      {/* Create Run Modal */}
      <CreateRunModal
        isOpen={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        datasets={datasets}
        onCreated={handleRunCreated}
      />
    </div>
  );
}

function CompareTab({ evalRuns }: { evalRuns: EvalRun[] }) {
  const [comparisonResult, setComparisonResult] = useState<any | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Transform API comparison result into StatisticalComparison component props
  const transformMetricForComparison = (metric: any): { 
    baseline: VariantStats; 
    treatment: VariantStats; 
    testResult: StatisticalTestResult;
  } | null => {
    if (!metric) return null;
    
    const baseline: VariantStats = {
      name: evalRuns[0]?.name || 'Baseline',
      mean: metric.baseline?.mean ?? 0,
      stdDev: metric.baseline?.std_dev ?? 0,
      n: metric.baseline?.n ?? 0,
      ci95: [
        (metric.baseline?.mean ?? 0) - 1.96 * (metric.baseline?.std_dev ?? 0) / Math.sqrt(metric.baseline?.n ?? 1),
        (metric.baseline?.mean ?? 0) + 1.96 * (metric.baseline?.std_dev ?? 0) / Math.sqrt(metric.baseline?.n ?? 1)
      ],
      passRate: evalRuns[0]?.metrics ? (evalRuns[0].metrics.passed / evalRuns[0].metrics.total_cases) : undefined
    };

    const treatment: VariantStats = {
      name: evalRuns[1]?.name || 'Treatment',
      mean: metric.treatment?.mean ?? 0,
      stdDev: metric.treatment?.std_dev ?? 0,
      n: metric.treatment?.n ?? 0,
      ci95: [
        (metric.treatment?.mean ?? 0) - 1.96 * (metric.treatment?.std_dev ?? 0) / Math.sqrt(metric.treatment?.n ?? 1),
        (metric.treatment?.mean ?? 0) + 1.96 * (metric.treatment?.std_dev ?? 0) / Math.sqrt(metric.treatment?.n ?? 1)
      ],
      passRate: evalRuns[1]?.metrics ? (evalRuns[1].metrics.passed / evalRuns[1].metrics.total_cases) : undefined
    };

    const testResult: StatisticalTestResult = {
      tStatistic: metric.t_statistic ?? 0,
      degreesOfFreedom: metric.degrees_of_freedom ?? (baseline.n + treatment.n - 2),
      pValue: metric.p_value ?? 1,
      difference: (metric.treatment?.mean ?? 0) - (metric.baseline?.mean ?? 0),
      differenceCI: [
        (metric.difference_ci_low ?? ((metric.treatment?.mean ?? 0) - (metric.baseline?.mean ?? 0) - 0.1)),
        (metric.difference_ci_high ?? ((metric.treatment?.mean ?? 0) - (metric.baseline?.mean ?? 0) + 0.1))
      ],
      cohensD: metric.cohens_d ?? 0,
      achievedPower: metric.achieved_power ?? 0.8
    };

    return { baseline, treatment, testResult };
  };

  useEffect(() => {
    if (evalRuns.length === 2) {
      runComparison();
    } else {
      setComparisonResult(null);
    }
  }, [evalRuns]);

  const runComparison = async () => {
    if (evalRuns.length !== 2) return;
    
    setLoading(true);
    setError(null);
    try {
      const result = await agentreplayClient.compareEvalRuns(evalRuns[0].id, evalRuns[1].id);
      setComparisonResult(result);
    } catch (err) {
      console.error('Failed to compare runs:', err);
      setError(err instanceof Error ? err.message : 'Failed to compare runs');
    } finally {
      setLoading(false);
    }
  };

  if (evalRuns.length < 2) {
    return (
      <div className="text-center py-12 bg-surface rounded-xl border border-border">
        <GitCompare className="w-12 h-12 text-textTertiary mx-auto mb-4 opacity-50" />
        <p className="text-textSecondary mb-2">Select 2 runs to compare</p>
        <p className="text-sm text-textTertiary">
          Use the checkboxes in the Eval Runs tab to select runs for A/B comparison
        </p>
      </div>
    );
  }

  if (evalRuns.length > 2) {
    return (
      <div className="text-center py-12 bg-surface rounded-xl border border-border">
        <GitCompare className="w-12 h-12 text-textTertiary mx-auto mb-4 opacity-50" />
        <p className="text-textSecondary mb-2">Select exactly 2 runs for statistical comparison</p>
        <p className="text-sm text-textTertiary">
          Currently selected: {evalRuns.length} runs. Deselect {evalRuns.length - 2} to compare.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <TrendingUp className="w-6 h-6 text-primary" />
          <h2 className="text-xl font-bold text-textPrimary">
            A/B Comparison: Statistical Analysis
          </h2>
        </div>
        <button
          onClick={runComparison}
          disabled={loading}
          className="flex items-center gap-2 px-4 py-2 bg-primary text-primary-foreground rounded-lg hover:bg-primary-hover disabled:opacity-50"
        >
          {loading ? 'Analyzing...' : 'Re-analyze'}
        </button>
      </div>

      {/* Run Summary with Enhanced Metric Cards */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Baseline Run */}
        <div className="space-y-3">
          <div className="bg-surface rounded-xl border border-border p-4">
            <div className="text-xs text-textTertiary uppercase mb-1 flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-blue-500"></span>
              Baseline
            </div>
            <div className="text-lg font-semibold text-textPrimary">{evalRuns[0].name}</div>
            <div className="text-sm text-textSecondary mt-1">
              {evalRuns[0].metrics.total_cases} test cases
            </div>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <EnhancedMetricCard
              title="Pass Rate"
              value={evalRuns[0].metrics.passed / evalRuns[0].metrics.total_cases}
              asPercentage={true}
              confidenceInterval={[
                Math.max(0, (evalRuns[0].metrics.passed / evalRuns[0].metrics.total_cases) - 0.05),
                Math.min(1, (evalRuns[0].metrics.passed / evalRuns[0].metrics.total_cases) + 0.05)
              ]}
              sampleSize={evalRuns[0].metrics.total_cases}
              trend={evalRuns[0].metrics.passed > evalRuns[0].metrics.total_cases / 2 ? 'up' : 'down'}
            />
            <EnhancedMetricCard
              title="Avg Latency"
              value={evalRuns[0].metrics.avg_latency_ms}
              unit="ms"
              sampleSize={evalRuns[0].metrics.total_cases}
            />
          </div>
        </div>
        
        {/* Treatment Run */}
        <div className="space-y-3">
          <div className="bg-surface rounded-xl border border-border p-4">
            <div className="text-xs text-textTertiary uppercase mb-1 flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-green-500"></span>
              Treatment
            </div>
            <div className="text-lg font-semibold text-textPrimary">{evalRuns[1].name}</div>
            <div className="text-sm text-textSecondary mt-1">
              {evalRuns[1].metrics.total_cases} test cases
            </div>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <EnhancedMetricCard
              title="Pass Rate"
              value={evalRuns[1].metrics.passed / evalRuns[1].metrics.total_cases}
              asPercentage={true}
              confidenceInterval={[
                Math.max(0, (evalRuns[1].metrics.passed / evalRuns[1].metrics.total_cases) - 0.05),
                Math.min(1, (evalRuns[1].metrics.passed / evalRuns[1].metrics.total_cases) + 0.05)
              ]}
              sampleSize={evalRuns[1].metrics.total_cases}
              trend={evalRuns[1].metrics.passed > evalRuns[1].metrics.total_cases / 2 ? 'up' : 'down'}
            />
            <EnhancedMetricCard
              title="Avg Latency"
              value={evalRuns[1].metrics.avg_latency_ms}
              unit="ms"
              sampleSize={evalRuns[1].metrics.total_cases}
            />
          </div>
        </div>
      </div>

      {error && (
        <div className="p-4 bg-error/10 border border-error/20 rounded-lg text-error">
          {error}
        </div>
      )}

      {loading && (
        <div className="text-center py-8 text-textSecondary">
          Running statistical analysis...
        </div>
      )}

      {comparisonResult && (
        <>
          {/* Recommendation Banner */}
          <div className={`p-4 rounded-xl border ${
            comparisonResult.recommendation.action === 'deploy_treatment' 
              ? 'bg-success/10 border-success/30' 
              : comparisonResult.recommendation.action === 'keep_baseline'
              ? 'bg-warning/10 border-warning/30'
              : 'bg-surface border-border'
          }`}>
            <div className="flex items-start gap-3">
              <div className={`p-2 rounded-lg ${
                comparisonResult.recommendation.action === 'deploy_treatment' 
                  ? 'bg-success/20' 
                  : comparisonResult.recommendation.action === 'keep_baseline'
                  ? 'bg-warning/20'
                  : 'bg-surface-hover'
              }`}>
                {comparisonResult.recommendation.action === 'deploy_treatment' ? (
                  <CheckCircle className="w-5 h-5 text-success" />
                ) : comparisonResult.recommendation.action === 'keep_baseline' ? (
                  <AlertCircle className="w-5 h-5 text-warning" />
                ) : (
                  <GitCompare className="w-5 h-5 text-textSecondary" />
                )}
              </div>
              <div>
                <div className="font-semibold text-textPrimary">
                  {comparisonResult.recommendation.action === 'deploy_treatment' 
                    ? '✅ Deploy Treatment' 
                    : comparisonResult.recommendation.action === 'keep_baseline'
                    ? '⚠️ Keep Baseline'
                    : '🤔 Inconclusive'}
                </div>
                <div className="text-sm text-textSecondary mt-1">
                  {comparisonResult.recommendation.explanation}
                </div>
                <div className="text-xs text-textTertiary mt-2">
                  Confidence: {(comparisonResult.recommendation.confidence * 100).toFixed(0)}%
                </div>
              </div>
            </div>
          </div>

          {/* Summary Stats */}
          <div className="grid grid-cols-3 gap-4">
            <div className="bg-surface rounded-xl border border-border p-4 text-center">
              <div className="text-2xl font-bold text-success">{comparisonResult.summary.significant_improvements}</div>
              <div className="text-sm text-textSecondary">Significant Improvements</div>
            </div>
            <div className="bg-surface rounded-xl border border-border p-4 text-center">
              <div className="text-2xl font-bold text-error">{comparisonResult.summary.significant_regressions}</div>
              <div className="text-sm text-textSecondary">Significant Regressions</div>
            </div>
            <div className="bg-surface rounded-xl border border-border p-4 text-center">
              <div className="text-2xl font-bold text-textSecondary">{comparisonResult.summary.no_significant_change}</div>
              <div className="text-sm text-textSecondary">No Significant Change</div>
            </div>
          </div>

          {/* Enhanced Statistical Comparison Cards */}
          {comparisonResult.metrics && comparisonResult.metrics.length > 0 && (
            <div className="space-y-4">
              <div className="flex items-center gap-2">
                <TrendingUp className="w-5 h-5 text-primary" />
                <h3 className="font-semibold text-textPrimary">Visual Statistical Analysis</h3>
              </div>
              <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                {comparisonResult.metrics.slice(0, 4).map((metric: any) => {
                  const transformed = transformMetricForComparison(metric);
                  if (!transformed) return null;
                  return (
                    <div key={metric.metric_name} className="bg-surface rounded-xl border border-border p-4">
                      <h4 className="font-medium text-textPrimary mb-3 flex items-center gap-2">
                        {metric.metric_name}
                        {metric.is_significant && (
                          <span className="px-2 py-0.5 text-xs bg-primary/20 text-primary rounded-full">
                            Significant
                          </span>
                        )}
                      </h4>
                      <StatisticalComparison
                        baseline={transformed.baseline}
                        treatment={transformed.treatment}
                        testResult={transformed.testResult}
                        mdes={0.05}
                        className="!p-0 !border-0"
                      />
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Metrics Comparison Table */}
          <div className="bg-surface rounded-xl border border-border overflow-hidden">
            <div className="px-4 py-3 border-b border-border">
              <h3 className="font-semibold text-textPrimary">Metric-by-Metric Analysis</h3>
              <p className="text-xs text-textTertiary">Welch's t-test with Cohen's d effect size</p>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead className="bg-background border-b border-border">
                  <tr>
                    <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase">Metric</th>
                    <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase">Baseline</th>
                    <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase">Treatment</th>
                    <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase">Δ Change</th>
                    <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase">p-value</th>
                    <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase">Effect Size</th>
                    <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase">Winner</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border">
                  {comparisonResult.metrics.map((metric: any) => (
                    <tr key={metric.metric_name} className={metric.is_significant ? 'bg-primary/5' : ''}>
                      <td className="px-4 py-3">
                        <div className="font-medium text-textPrimary">{metric.metric_name}</div>
                        <div className="text-xs text-textTertiary">
                          {metric.higher_is_better ? '↑ Higher is better' : '↓ Lower is better'}
                        </div>
                      </td>
                      <td className="px-4 py-3 text-sm text-textSecondary">
                        <div>{metric.baseline.mean.toFixed(3)}</div>
                        <div className="text-xs text-textTertiary">±{metric.baseline.std_dev.toFixed(3)} (n={metric.baseline.n})</div>
                      </td>
                      <td className="px-4 py-3 text-sm text-textSecondary">
                        <div>{metric.treatment.mean.toFixed(3)}</div>
                        <div className="text-xs text-textTertiary">±{metric.treatment.std_dev.toFixed(3)} (n={metric.treatment.n})</div>
                      </td>
                      <td className="px-4 py-3">
                        <span className={`font-medium ${
                          metric.percent_change > 0 
                            ? metric.higher_is_better ? 'text-success' : 'text-error'
                            : metric.percent_change < 0 
                            ? metric.higher_is_better ? 'text-error' : 'text-success'
                            : 'text-textSecondary'
                        }`}>
                          {metric.percent_change > 0 ? '+' : ''}{metric.percent_change.toFixed(1)}%
                        </span>
                      </td>
                      <td className="px-4 py-3 text-sm">
                        <span className={metric.is_significant ? 'text-primary font-medium' : 'text-textTertiary'}>
                          {metric.p_value < 0.001 ? '<0.001' : metric.p_value.toFixed(3)}
                          {metric.is_significant && ' *'}
                        </span>
                      </td>
                      <td className="px-4 py-3">
                        <span className={`px-2 py-1 rounded text-xs font-medium ${
                          metric.effect_size === 'large' ? 'bg-primary/20 text-primary' :
                          metric.effect_size === 'medium' ? 'bg-warning/20 text-warning' :
                          metric.effect_size === 'small' ? 'bg-success/20 text-success' :
                          'bg-surface-hover text-textTertiary'
                        }`}>
                          {metric.effect_size} (d={metric.cohens_d.toFixed(2)})
                        </span>
                      </td>
                      <td className="px-4 py-3">
                        {metric.winner === 'treatment' ? (
                          <span className="text-success font-medium">Treatment ✓</span>
                        ) : metric.winner === 'baseline' ? (
                          <span className="text-warning font-medium">Baseline ✓</span>
                        ) : (
                          <span className="text-textTertiary">Tie</span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          {/* Legend */}
          <div className="bg-surface-elevated rounded-lg p-4 text-sm">
            <div className="font-medium text-textPrimary mb-2">Understanding the Results</div>
            <div className="grid grid-cols-2 gap-4 text-textSecondary text-xs">
              <div>
                <strong>p-value:</strong> Probability the difference is due to chance. * indicates p &lt; 0.05 (statistically significant)
              </div>
              <div>
                <strong>Cohen's d:</strong> Effect size. |d| &lt; 0.2 = negligible, 0.2-0.5 = small, 0.5-0.8 = medium, &gt; 0.8 = large
              </div>
            </div>
          </div>
        </>
      )}

      {!comparisonResult && !loading && !error && (
        <div className="bg-surface rounded-xl border border-border overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead className="bg-background border-b border-border">
                <tr>
                  <th className="px-6 py-3 text-left text-xs font-medium text-textTertiary uppercase">Metric</th>
                  {evalRuns.map(run => (
                    <th key={run.id} className="px-6 py-3 text-left text-xs font-medium text-textTertiary uppercase">
                      {run.name}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                <tr>
                  <td className="px-6 py-4 text-sm font-medium text-textPrimary">Pass Rate</td>
                  {evalRuns.map(run => (
                    <td key={run.id} className="px-6 py-4 text-sm text-textSecondary">
                      {((run.metrics.passed / run.metrics.total_cases) * 100).toFixed(1)}%
                    </td>
                  ))}
                </tr>
                <tr>
                  <td className="px-6 py-4 text-sm font-medium text-textPrimary">Avg Latency</td>
                  {evalRuns.map(run => (
                    <td key={run.id} className="px-6 py-4 text-sm text-textSecondary">
                      {run.metrics.avg_latency_ms.toFixed(0)}ms
                    </td>
                  ))}
                </tr>
                <tr>
                  <td className="px-6 py-4 text-sm font-medium text-textPrimary">Avg Cost</td>
                  {evalRuns.map(run => (
                    <td key={run.id} className="px-6 py-4 text-sm text-textSecondary">
                      ${run.metrics.avg_cost.toFixed(4)}
                    </td>
                  ))}
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
