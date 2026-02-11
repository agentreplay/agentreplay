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
