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
import { agentreplayClient, ExperimentResponse, ExperimentStatsResponse, CreateExperimentRequest } from '../lib/agentreplay-api';

// ============================================================================
// Status Badge Component
// ============================================================================

const StatusBadge: React.FC<{ status: string }> = ({ status }) => {
  const colors: Record<string, string> = {
    draft: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300',
    running: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-300',
    paused: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300',
    completed: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300',
    stopped: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300',
  };

  return (
    <span className={`px-2 py-1 rounded-full text-xs font-medium ${colors[status] || colors.draft}`}>
      {status.charAt(0).toUpperCase() + status.slice(1)}
    </span>
  );
};

// ============================================================================
// Create Experiment Modal
// ============================================================================

interface CreateExperimentModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreated: (experiment: ExperimentResponse) => void;
}

const CreateExperimentModal: React.FC<CreateExperimentModalProps> = ({ isOpen, onClose, onCreated }) => {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [variants, setVariants] = useState([
    { name: 'Control', description: 'Control variant', config: {} },
    { name: 'Treatment', description: 'Treatment variant', config: {} },
  ]);
  const [metrics, setMetrics] = useState<string[]>(['latency', 'accuracy', 'cost']);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleAddVariant = () => {
    setVariants([...variants, { name: `Variant ${variants.length + 1}`, description: '', config: {} }]);
  };

  const handleRemoveVariant = (index: number) => {
    if (variants.length > 2) {
      setVariants(variants.filter((_, i) => i !== index));
    }
  };

  const handleVariantChange = (index: number, field: 'name' | 'description', value: string) => {
    const updated = [...variants];
    updated[index] = { ...updated[index], [field]: value };
    setVariants(updated);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);

    try {
      const request: CreateExperimentRequest = {
        name,
        description,
        variants,
        metrics,
      };
      const experiment = await agentreplayClient.createExperiment(request);
      onCreated(experiment);
      onClose();
      setName('');
      setDescription('');
      setVariants([
        { name: 'Control', description: 'Control variant', config: {} },
        { name: 'Treatment', description: 'Treatment variant', config: {} },
      ]);
    } catch (err: any) {
      setError(err.message || 'Failed to create experiment');
    } finally {
      setLoading(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-white dark:bg-gray-800 rounded-lg p-6 w-full max-w-2xl max-h-[90vh] overflow-y-auto">
        <h2 className="text-xl font-bold mb-4">Create New Experiment</h2>
        
        <form onSubmit={handleSubmit}>
          <div className="mb-4">
            <label className="block text-sm font-medium mb-1">Experiment Name</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
              placeholder="e.g., GPT-4 vs Claude Comparison"
              required
            />
          </div>

          <div className="mb-4">
            <label className="block text-sm font-medium mb-1">Description</label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className="w-full p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
              placeholder="What are you testing?"
              rows={3}
            />
          </div>

          <div className="mb-4">
            <div className="flex items-center justify-between mb-2">
              <label className="block text-sm font-medium">Variants</label>
              <button
                type="button"
                onClick={handleAddVariant}
                className="text-sm text-blue-600 hover:text-blue-800"
              >
                + Add Variant
              </button>
            </div>
            {variants.map((variant, index) => (
              <div key={index} className="flex gap-2 mb-2">
                <input
                  type="text"
                  value={variant.name}
                  onChange={(e) => handleVariantChange(index, 'name', e.target.value)}
                  className="flex-1 p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
                  placeholder="Variant name"
                  required
                />
                <input
                  type="text"
                  value={variant.description}
                  onChange={(e) => handleVariantChange(index, 'description', e.target.value)}
                  className="flex-1 p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
                  placeholder="Description"
                />
                {variants.length > 2 && (
                  <button
                    type="button"
                    onClick={() => handleRemoveVariant(index)}
                    className="px-3 py-2 text-red-600 hover:bg-red-100 rounded"
                  >
                    ✕
                  </button>
                )}
              </div>
            ))}
          </div>

          <div className="mb-4">
            <label className="block text-sm font-medium mb-1">Success Metrics</label>
            <div className="flex flex-wrap gap-2">
              {['latency', 'accuracy', 'cost', 'tokens', 'error_rate'].map((metric) => (
                <label key={metric} className="flex items-center gap-1">
                  <input
                    type="checkbox"
                    checked={metrics.includes(metric)}
                    onChange={(e) => {
                      if (e.target.checked) {
                        setMetrics([...metrics, metric]);
                      } else {
                        setMetrics(metrics.filter((m) => m !== metric));
                      }
                    }}
                    className="rounded"
                  />
                  <span className="text-sm">{metric}</span>
                </label>
              ))}
            </div>
          </div>

          {error && (
            <div className="mb-4 p-3 bg-red-100 text-red-800 rounded text-sm">
              {error}
            </div>
          )}

          <div className="flex justify-end gap-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border rounded hover:bg-gray-100 dark:hover:bg-gray-700"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading || !name}
              className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
            >
              {loading ? 'Creating...' : 'Create Experiment'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

// ============================================================================
// Experiment Card Component
// ============================================================================

interface ExperimentCardProps {
  experiment: ExperimentResponse;
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onView: (id: string) => void;
}

const ExperimentCard: React.FC<ExperimentCardProps> = ({ experiment, onStart, onStop, onView }) => {
  const totalTraffic = Object.values(experiment.traffic_split).reduce((a, b) => a + b, 0);

  return (
    <div className="border border-gray-200 dark:border-gray-700 rounded-lg p-4 bg-white dark:bg-gray-800 hover:shadow-md transition-shadow">
      <div className="flex items-start justify-between mb-3">
        <div>
          <h3 className="font-semibold text-lg">{experiment.name}</h3>
          <p className="text-sm text-gray-600 dark:text-gray-400">{experiment.description}</p>
        </div>
        <StatusBadge status={experiment.status} />
      </div>

      <div className="grid grid-cols-2 gap-4 mb-4 text-sm">
        <div>
          <span className="text-gray-500">Variants:</span>
          <span className="ml-2 font-medium">{experiment.variants.length}</span>
        </div>
        <div>
          <span className="text-gray-500">Metrics:</span>
          <span className="ml-2 font-medium">{experiment.metrics.length}</span>
        </div>
        <div>
          <span className="text-gray-500">Created:</span>
          <span className="ml-2 font-medium">
            {new Date(experiment.created_at / 1000).toLocaleDateString()}
          </span>
        </div>
        {totalTraffic > 0 && (
          <div>
            <span className="text-gray-500">Traffic:</span>
            <span className="ml-2 font-medium">{(totalTraffic * 100).toFixed(0)}%</span>
          </div>
        )}
      </div>

      <div className="flex gap-2">
        <button
          onClick={() => onView(experiment.id)}
          className="flex-1 px-3 py-2 border rounded text-sm hover:bg-gray-100 dark:hover:bg-gray-700"
        >
          View Details
        </button>
        {experiment.status === 'draft' && (
          <button
            onClick={() => onStart(experiment.id)}
            className="px-3 py-2 bg-green-600 text-white rounded text-sm hover:bg-green-700"
          >
            Start
          </button>
        )}
        {experiment.status === 'running' && (
          <button
            onClick={() => onStop(experiment.id)}
            className="px-3 py-2 bg-red-600 text-white rounded text-sm hover:bg-red-700"
          >
            Stop
          </button>
        )}
      </div>
    </div>
  );
};

// ============================================================================
// Experiment Detail View
// ============================================================================

interface ExperimentDetailProps {
  experiment: ExperimentResponse;
  stats: ExperimentStatsResponse | null;
  onBack: () => void;
  onStart: (id: string) => void;
  onStop: (id: string) => void;
}

const ExperimentDetail: React.FC<ExperimentDetailProps> = ({ experiment, stats, onBack, onStart, onStop }) => {
  const [trafficSplit, setTrafficSplit] = useState<Record<string, number>>(() => {
    if (Object.keys(experiment.traffic_split).length > 0) {
      return experiment.traffic_split;
    }
    const equalWeight = 1 / experiment.variants.length;
    return Object.fromEntries(experiment.variants.map(v => [v.id, equalWeight]));
  });

  const handleTrafficChange = (variantId: string, value: number) => {
    setTrafficSplit({ ...trafficSplit, [variantId]: value / 100 });
  };

  const handleStartWithTraffic = async () => {
    try {
      await agentreplayClient.startExperiment(experiment.id, trafficSplit);
      window.location.reload();
    } catch (err) {
      console.error('Failed to start experiment:', err);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <button onClick={onBack} className="text-blue-600 hover:text-blue-800 flex items-center gap-1">
          ← Back to Experiments
        </button>
        <div className="flex gap-2">
          {experiment.status === 'draft' && (
            <button
              onClick={handleStartWithTraffic}
              className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700"
            >
              Start Experiment
            </button>
          )}
          {experiment.status === 'running' && (
            <button
              onClick={() => onStop(experiment.id)}
              className="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700"
            >
              Stop Experiment
            </button>
          )}
        </div>
      </div>

      <div className="bg-white dark:bg-gray-800 rounded-lg p-6 border">
        <div className="flex items-start justify-between mb-4">
          <div>
            <h2 className="text-2xl font-bold">{experiment.name}</h2>
            <p className="text-gray-600 dark:text-gray-400">{experiment.description}</p>
          </div>
          <StatusBadge status={experiment.status} />
        </div>

        <div className="grid grid-cols-4 gap-4 text-sm mb-6">
          <div>
            <span className="text-gray-500 block">Created</span>
            <span className="font-medium">{new Date(experiment.created_at / 1000).toLocaleString()}</span>
          </div>
          {experiment.start_time && (
            <div>
              <span className="text-gray-500 block">Started</span>
              <span className="font-medium">{new Date(experiment.start_time / 1000).toLocaleString()}</span>
            </div>
          )}
          {experiment.end_time && (
            <div>
              <span className="text-gray-500 block">Ended</span>
              <span className="font-medium">{new Date(experiment.end_time / 1000).toLocaleString()}</span>
            </div>
          )}
          <div>
            <span className="text-gray-500 block">Metrics</span>
            <span className="font-medium">{experiment.metrics.join(', ')}</span>
          </div>
        </div>

        {/* Traffic Allocation */}
        <div className="mb-6">
          <h3 className="font-semibold mb-3">Traffic Allocation</h3>
          <div className="space-y-3">
            {experiment.variants.map((variant) => (
              <div key={variant.id} className="flex items-center gap-4">
                <div className="w-32 font-medium">{variant.name}</div>
                <input
                  type="range"
                  min="0"
                  max="100"
                  value={(trafficSplit[variant.id] || 0) * 100}
                  onChange={(e) => handleTrafficChange(variant.id, parseInt(e.target.value))}
                  className="flex-1"
                  disabled={experiment.status !== 'draft'}
                />
                <div className="w-16 text-right">
                  {((trafficSplit[variant.id] || 0) * 100).toFixed(0)}%
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Variants */}
        <div className="mb-6">
          <h3 className="font-semibold mb-3">Variants</h3>
          <div className="grid gap-4">
            {experiment.variants.map((variant, index) => (
              <div key={variant.id} className="border rounded p-4">
                <div className="flex items-center gap-2 mb-2">
                  <span className={`px-2 py-0.5 rounded text-xs ${index === 0 ? 'bg-blue-100 text-blue-800' : 'bg-gray-100 text-gray-800'}`}>
                    {index === 0 ? 'Control' : 'Treatment'}
                  </span>
                  <h4 className="font-medium">{variant.name}</h4>
                </div>
                <p className="text-sm text-gray-600 dark:text-gray-400">{variant.description}</p>
                {Object.keys(variant.config).length > 0 && (
                  <pre className="mt-2 text-xs bg-gray-100 dark:bg-gray-700 p-2 rounded overflow-x-auto">
                    {JSON.stringify(variant.config, null, 2)}
                  </pre>
                )}
              </div>
            ))}
          </div>
        </div>

        {/* Statistics */}
        {stats && (
          <div>
            <h3 className="font-semibold mb-3">Results</h3>
            {stats.winner && (
              <div className="mb-4 p-4 bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded">
                <div className="flex items-center gap-2">
                  <span className="text-2xl">🏆</span>
                  <div>
                    <div className="font-semibold text-green-800 dark:text-green-200">
                      Winner: {stats.winner}
                    </div>
                    {stats.confidence && (
                      <div className="text-sm text-green-600 dark:text-green-400">
                        Confidence: {(stats.confidence * 100).toFixed(1)}%
                      </div>
                    )}
                  </div>
                </div>
              </div>
            )}
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2">Variant</th>
                  <th className="text-right py-2">Samples</th>
                  {experiment.metrics.map(metric => (
                    <th key={metric} className="text-right py-2">{metric}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {Object.entries(stats.variant_stats).map(([id, variantStats]) => (
                  <tr key={id} className="border-b">
                    <td className="py-2">{variantStats.variant_name}</td>
                    <td className="text-right py-2">{variantStats.sample_count}</td>
                    {experiment.metrics.map(metric => {
                      const metricStats = variantStats.metrics[metric];
                      return (
                        <td key={metric} className="text-right py-2">
                          {metricStats ? `${metricStats.mean.toFixed(3)} ±${metricStats.std_dev.toFixed(3)}` : '-'}
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
};

// ============================================================================
// Main Experiments Page
// ============================================================================

export default function Experiments() {
  const [experiments, setExperiments] = useState<ExperimentResponse[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [selectedExperiment, setSelectedExperiment] = useState<ExperimentResponse | null>(null);
  const [selectedStats, setSelectedStats] = useState<ExperimentStatsResponse | null>(null);
  const [filter, setFilter] = useState<string>('all');

  const fetchExperiments = async () => {
    setLoading(true);
    try {
      const response = await agentreplayClient.listExperiments(filter === 'all' ? undefined : filter);
      setExperiments(response.experiments);
      setError(null);
    } catch (err: any) {
      console.error('Failed to fetch experiments:', err);
      setError(err.message || 'Failed to load experiments');
      setExperiments([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchExperiments();
  }, [filter]);

  const handleStart = async (id: string) => {
    try {
      const experiment = experiments.find(e => e.id === id);
      if (experiment) {
        const equalWeight = 1 / experiment.variants.length;
        const trafficSplit = Object.fromEntries(experiment.variants.map(v => [v.id, equalWeight]));
        await agentreplayClient.startExperiment(id, trafficSplit);
        fetchExperiments();
      }
    } catch (err) {
      console.error('Failed to start experiment:', err);
    }
  };

  const handleStop = async (id: string) => {
    try {
      await agentreplayClient.stopExperiment(id);
      fetchExperiments();
      if (selectedExperiment?.id === id) {
        const updated = await agentreplayClient.getExperiment(id);
        setSelectedExperiment(updated);
      }
    } catch (err) {
      console.error('Failed to stop experiment:', err);
    }
  };

  const handleView = async (id: string) => {
    try {
      const experiment = await agentreplayClient.getExperiment(id);
      setSelectedExperiment(experiment);
      
      if (experiment.status === 'running' || experiment.status === 'completed') {
        try {
          const stats = await agentreplayClient.getExperimentStats(id);
          setSelectedStats(stats);
        } catch (err) {
          setSelectedStats(null);
        }
      } else {
        setSelectedStats(null);
      }
    } catch (err) {
      console.error('Failed to fetch experiment:', err);
    }
  };

  const handleCreated = (experiment: ExperimentResponse) => {
    setExperiments([experiment, ...experiments]);
  };

  if (selectedExperiment) {
    return (
      <div className="container mx-auto p-6">
        <ExperimentDetail
          experiment={selectedExperiment}
          stats={selectedStats}
          onBack={() => {
            setSelectedExperiment(null);
            setSelectedStats(null);
          }}
          onStart={handleStart}
          onStop={handleStop}
        />
      </div>
    );
  }

  return (
    <div className="container mx-auto p-6">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold">A/B Experiments</h1>
          <p className="text-gray-600 dark:text-gray-400">
            Compare prompts, models, and configurations with statistical rigor
          </p>
        </div>
        <button
          onClick={() => setShowCreateModal(true)}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 flex items-center gap-2"
        >
          <span>+</span> New Experiment
        </button>
      </div>

      <div className="flex gap-2 mb-6">
        {['all', 'draft', 'running', 'completed', 'stopped'].map((status) => (
          <button
            key={status}
            onClick={() => setFilter(status)}
            className={`px-3 py-1 rounded-full text-sm ${
              filter === status
                ? 'bg-blue-600 text-white'
                : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'
            }`}
          >
            {status.charAt(0).toUpperCase() + status.slice(1)}
          </button>
        ))}
      </div>

      {loading ? (
        <div className="flex items-center justify-center h-64">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" />
        </div>
      ) : error ? (
        <div className="text-center py-12">
          <div className="text-red-500 mb-2">⚠️ {error}</div>
          <button
            onClick={fetchExperiments}
            className="text-blue-600 hover:text-blue-800"
          >
            Try again
          </button>
        </div>
      ) : experiments.length === 0 ? (
        <div className="text-center py-12 bg-gray-50 dark:bg-gray-800 rounded-lg">
          <div className="text-4xl mb-4">🧪</div>
          <h3 className="text-lg font-medium mb-2">No experiments yet</h3>
          <p className="text-gray-600 dark:text-gray-400 mb-4">
            Create your first A/B experiment to compare prompts, models, or configurations
          </p>
          <button
            onClick={() => setShowCreateModal(true)}
            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700"
          >
            Create Experiment
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {experiments.map((experiment) => (
            <ExperimentCard
              key={experiment.id}
              experiment={experiment}
              onStart={handleStart}
              onStop={handleStop}
              onView={handleView}
            />
          ))}
        </div>
      )}

      <CreateExperimentModal
        isOpen={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        onCreated={handleCreated}
      />
    </div>
  );
}
