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

import { useState, useEffect, useCallback } from 'react';
import { API_BASE_URL } from '../../lib/api-config';
import { VideoHelpButton } from '../components/VideoHelpButton';
import {
  Plus,
  Trash2,
  Save,
  RefreshCw,
  Loader2,
  DollarSign,
  Search,
  Edit2,
  X,
  Check,
  Download,
  Upload,
  AlertCircle,
  CheckCircle,
  Cpu,
  Cloud
} from 'lucide-react';

interface ModelPricing {
  model_id: string;
  provider?: string;
  input_cost_per_1m: number;
  output_cost_per_1m: number;
  context_window?: number;
  supports_vision?: boolean;
  supports_function_calling?: boolean;
  source?: string;
  priority?: string;
}

interface CustomPricingEntry {
  model_id: string;
  provider: string;
  input_cost_per_token: number;
  output_cost_per_token: number;
  max_tokens?: number;
}

export default function CostManagementPage() {
  const [models, setModels] = useState<ModelPricing[]>([]);
  const [customPricing, setCustomPricing] = useState<CustomPricingEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [showAddModal, setShowAddModal] = useState(false);
  const [editingModel, setEditingModel] = useState<string | null>(null);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [lastSyncTime, setLastSyncTime] = useState<number | null>(null);
  
  // New model form state
  const [newModel, setNewModel] = useState<CustomPricingEntry>({
    model_id: '',
    provider: 'openai',
    input_cost_per_token: 0,
    output_cost_per_token: 0,
  });

  // Fetch all pricing data
  const fetchPricing = useCallback(async () => {
    setLoading(true);
    try {
      // Fetch all models from pricing registry
      const response = await fetch(`${API_BASE_URL}/api/v1/pricing/models/all`);
      if (response.ok) {
        const data = await response.json();
        setModels(data.models || []);
        setLastSyncTime(data.last_sync || null);
      }
      
      // Fetch custom pricing entries
      const customResponse = await fetch(`${API_BASE_URL}/api/v1/pricing/custom`);
      if (customResponse.ok) {
        const customData = await customResponse.json();
        setCustomPricing(customData.entries || []);
      }
    } catch (error) {
      console.error('Failed to fetch pricing:', error);
      setMessage({ type: 'error', text: 'Failed to load pricing data' });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchPricing();
  }, [fetchPricing]);

  // Sync from LiteLLM
  const handleSync = async () => {
    setSyncing(true);
    setMessage(null);
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/pricing/sync`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
      });
      const data = await response.json();
      if (data.success) {
        setMessage({ type: 'success', text: `Synced ${data.models_synced} models from LiteLLM` });
        await fetchPricing();
      } else {
        setMessage({ type: 'error', text: data.message || 'Sync failed' });
      }
    } catch (error) {
      setMessage({ type: 'error', text: 'Failed to sync pricing' });
    } finally {
      setSyncing(false);
    }
  };

  // Add custom pricing
  const handleAddCustom = async () => {
    if (!newModel.model_id.trim()) {
      setMessage({ type: 'error', text: 'Model ID is required' });
      return;
    }
    
    setSaving(true);
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/pricing/custom`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(newModel),
      });
      
      if (response.ok) {
        setMessage({ type: 'success', text: `Added custom pricing for ${newModel.model_id}` });
        setShowAddModal(false);
        setNewModel({
          model_id: '',
          provider: 'openai',
          input_cost_per_token: 0,
          output_cost_per_token: 0,
        });
        await fetchPricing();
      } else {
        const error = await response.json();
        setMessage({ type: 'error', text: error.message || 'Failed to add pricing' });
      }
    } catch (error) {
      setMessage({ type: 'error', text: 'Failed to add custom pricing' });
    } finally {
      setSaving(false);
    }
  };

  // Delete custom pricing
  const handleDeleteCustom = async (modelId: string) => {
    if (!confirm(`Delete custom pricing for ${modelId}?`)) return;
    
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/pricing/custom/${encodeURIComponent(modelId)}`, {
        method: 'DELETE',
      });
      
      if (response.ok) {
        setMessage({ type: 'success', text: `Deleted custom pricing for ${modelId}` });
        await fetchPricing();
      } else {
        setMessage({ type: 'error', text: 'Failed to delete pricing' });
      }
    } catch (error) {
      setMessage({ type: 'error', text: 'Failed to delete pricing' });
    }
  };

  // Update custom pricing inline
  const handleUpdateCustom = async (entry: CustomPricingEntry) => {
    setSaving(true);
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/pricing/custom`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(entry),
      });
      
      if (response.ok) {
        setMessage({ type: 'success', text: `Updated pricing for ${entry.model_id}` });
        setEditingModel(null);
        await fetchPricing();
      } else {
        setMessage({ type: 'error', text: 'Failed to update pricing' });
      }
    } catch (error) {
      setMessage({ type: 'error', text: 'Failed to update pricing' });
    } finally {
      setSaving(false);
    }
  };

  // Filter models by search query
  const filteredModels = models.filter(m => 
    m.model_id.toLowerCase().includes(searchQuery.toLowerCase()) ||
    (m.provider?.toLowerCase().includes(searchQuery.toLowerCase()))
  );

  const filteredCustom = customPricing.filter(m =>
    m.model_id.toLowerCase().includes(searchQuery.toLowerCase()) ||
    m.provider.toLowerCase().includes(searchQuery.toLowerCase())
  );

  // Format cost for display
  const formatCost = (costPer1M: number) => {
    if (costPer1M === 0) return 'Free';
    if (costPer1M < 0.01) return `$${costPer1M.toFixed(6)}`;
    if (costPer1M < 1) return `$${costPer1M.toFixed(4)}`;
    return `$${costPer1M.toFixed(2)}`;
  };

  return (
    <div className="min-h-screen bg-background">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-2xl font-bold text-textPrimary mb-1">Cost Management</h1>
            <p className="text-textSecondary text-sm">
              Manage model pricing for accurate cost tracking
              {lastSyncTime && (
                <span className="ml-2 text-textTertiary">
                  Last synced: {new Date(lastSyncTime * 1000).toLocaleDateString()}
                </span>
              )}
            </p>
          </div>
          <div className="flex items-center gap-3">
            <VideoHelpButton pageId="cost-management" />
            <button
              onClick={handleSync}
              disabled={syncing}
              className="flex items-center gap-2 px-4 py-2 bg-surface border border-border rounded-lg text-textSecondary hover:text-textPrimary hover:bg-surface-elevated transition-colors disabled:opacity-50"
            >
              {syncing ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <RefreshCw className="w-4 h-4" />
              )}
              Sync from LiteLLM
            </button>
            <button
              onClick={() => setShowAddModal(true)}
              className="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-hover transition-colors"
            >
              <Plus className="w-4 h-4" />
              Add Custom Pricing
            </button>
          </div>
        </div>

        {/* Status Message */}
        {message && (
          <div className={`mb-4 p-4 rounded-lg flex items-center gap-2 ${
            message.type === 'success' 
              ? 'bg-green-500/10 text-green-500 border border-green-500/20' 
              : 'bg-red-500/10 text-red-500 border border-red-500/20'
          }`}>
            {message.type === 'success' ? (
              <CheckCircle className="w-5 h-5" />
            ) : (
              <AlertCircle className="w-5 h-5" />
            )}
            <span>{message.text}</span>
            <button 
              onClick={() => setMessage(null)}
              className="ml-auto hover:opacity-70"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        )}

        {/* Search */}
        <div className="relative mb-6">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-textTertiary" />
          <input
            type="text"
            placeholder="Search models..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-10 pr-4 py-3 bg-surface border border-border rounded-lg text-textPrimary placeholder-textTertiary focus:outline-none focus:ring-2 focus:ring-primary"
          />
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <Loader2 className="w-8 h-8 animate-spin text-primary" />
          </div>
        ) : (
          <div className="space-y-8">
            {/* Custom Pricing Section */}
            <div>
              <h2 className="text-lg font-semibold text-textPrimary mb-4 flex items-center gap-2">
                <DollarSign className="w-5 h-5 text-green-500" />
                Custom Pricing Overrides
                <span className="text-sm font-normal text-textTertiary">
                  ({filteredCustom.length} models)
                </span>
              </h2>
              
              {filteredCustom.length === 0 ? (
                <div className="bg-surface border border-border rounded-lg p-8 text-center">
                  <DollarSign className="w-12 h-12 text-textTertiary mx-auto mb-3" />
                  <p className="text-textSecondary">No custom pricing configured</p>
                  <p className="text-textTertiary text-sm mt-1">
                    Add custom pricing for models not in the registry or override existing prices
                  </p>
                </div>
              ) : (
                <div className="bg-surface border border-border rounded-lg overflow-hidden">
                  <table className="w-full">
                    <thead className="bg-background">
                      <tr>
                        <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase tracking-wider">Model</th>
                        <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase tracking-wider">Provider</th>
                        <th className="px-4 py-3 text-right text-xs font-medium text-textTertiary uppercase tracking-wider">Input (per 1M)</th>
                        <th className="px-4 py-3 text-right text-xs font-medium text-textTertiary uppercase tracking-wider">Output (per 1M)</th>
                        <th className="px-4 py-3 text-right text-xs font-medium text-textTertiary uppercase tracking-wider">Actions</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-border">
                      {filteredCustom.map((entry) => (
                        <tr key={entry.model_id} className="hover:bg-background/50">
                          <td className="px-4 py-3">
                            <div className="flex items-center gap-2">
                              <Cpu className="w-4 h-4 text-primary" />
                              <span className="font-medium text-textPrimary">{entry.model_id}</span>
                            </div>
                          </td>
                          <td className="px-4 py-3 text-textSecondary">{entry.provider}</td>
                          <td className="px-4 py-3 text-right font-mono text-green-500">
                            {formatCost(entry.input_cost_per_token * 1_000_000)}
                          </td>
                          <td className="px-4 py-3 text-right font-mono text-green-500">
                            {formatCost(entry.output_cost_per_token * 1_000_000)}
                          </td>
                          <td className="px-4 py-3 text-right">
                            <div className="flex items-center justify-end gap-2">
                              <button
                                onClick={() => setEditingModel(entry.model_id)}
                                className="p-1.5 text-textTertiary hover:text-primary rounded-lg hover:bg-primary/10 transition-colors"
                              >
                                <Edit2 className="w-4 h-4" />
                              </button>
                              <button
                                onClick={() => handleDeleteCustom(entry.model_id)}
                                className="p-1.5 text-textTertiary hover:text-red-500 rounded-lg hover:bg-red-500/10 transition-colors"
                              >
                                <Trash2 className="w-4 h-4" />
                              </button>
                            </div>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>

            {/* Registry Pricing Section */}
            <div>
              <h2 className="text-lg font-semibold text-textPrimary mb-4 flex items-center gap-2">
                <Cloud className="w-5 h-5 text-blue-500" />
                Registry Pricing (from LiteLLM)
                <span className="text-sm font-normal text-textTertiary">
                  ({filteredModels.length} models)
                </span>
              </h2>
              
              <div className="bg-surface border border-border rounded-lg overflow-hidden">
                <table className="w-full">
                  <thead className="bg-background">
                    <tr>
                      <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase tracking-wider">Model</th>
                      <th className="px-4 py-3 text-left text-xs font-medium text-textTertiary uppercase tracking-wider">Provider</th>
                      <th className="px-4 py-3 text-right text-xs font-medium text-textTertiary uppercase tracking-wider">Input (per 1M)</th>
                      <th className="px-4 py-3 text-right text-xs font-medium text-textTertiary uppercase tracking-wider">Output (per 1M)</th>
                      <th className="px-4 py-3 text-center text-xs font-medium text-textTertiary uppercase tracking-wider">Context</th>
                      <th className="px-4 py-3 text-center text-xs font-medium text-textTertiary uppercase tracking-wider">Features</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {filteredModels.slice(0, 50).map((model) => (
                      <tr key={model.model_id} className="hover:bg-background/50">
                        <td className="px-4 py-3">
                          <span className="font-medium text-textPrimary">{model.model_id}</span>
                        </td>
                        <td className="px-4 py-3 text-textSecondary">{model.provider || 'unknown'}</td>
                        <td className="px-4 py-3 text-right font-mono text-textSecondary">
                          {formatCost(model.input_cost_per_1m)}
                        </td>
                        <td className="px-4 py-3 text-right font-mono text-textSecondary">
                          {formatCost(model.output_cost_per_1m)}
                        </td>
                        <td className="px-4 py-3 text-center text-textTertiary text-sm">
                          {model.context_window ? `${(model.context_window / 1000).toFixed(0)}K` : '-'}
                        </td>
                        <td className="px-4 py-3 text-center">
                          <div className="flex items-center justify-center gap-1">
                            {model.supports_vision && (
                              <span className="px-1.5 py-0.5 text-xs bg-purple-500/10 text-purple-500 rounded">Vision</span>
                            )}
                            {model.supports_function_calling && (
                              <span className="px-1.5 py-0.5 text-xs bg-blue-500/10 text-blue-500 rounded">Functions</span>
                            )}
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                {filteredModels.length > 50 && (
                  <div className="px-4 py-3 bg-background text-center text-textTertiary text-sm">
                    Showing 50 of {filteredModels.length} models. Use search to filter.
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Add Custom Pricing Modal */}
        {showAddModal && (
          <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <div className="bg-surface border border-border rounded-xl shadow-xl w-full max-w-md p-6">
              <div className="flex items-center justify-between mb-6">
                <h3 className="text-lg font-semibold text-textPrimary">Add Custom Pricing</h3>
                <button
                  onClick={() => setShowAddModal(false)}
                  className="text-textTertiary hover:text-textPrimary"
                >
                  <X className="w-5 h-5" />
                </button>
              </div>

              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-textSecondary mb-1">
                    Model ID *
                  </label>
                  <input
                    type="text"
                    value={newModel.model_id}
                    onChange={(e) => setNewModel({ ...newModel, model_id: e.target.value })}
                    placeholder="e.g., gpt-4o, claude-3-opus"
                    className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary placeholder-textTertiary focus:outline-none focus:ring-2 focus:ring-primary"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium text-textSecondary mb-1">
                    Provider
                  </label>
                  <select
                    value={newModel.provider}
                    onChange={(e) => setNewModel({ ...newModel, provider: e.target.value })}
                    className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary focus:outline-none focus:ring-2 focus:ring-primary"
                  >
                    <option value="openai">OpenAI</option>
                    <option value="anthropic">Anthropic</option>
                    <option value="google">Google</option>
                    <option value="mistral">Mistral</option>
                    <option value="deepseek">DeepSeek</option>
                    <option value="ollama">Ollama (Local)</option>
                    <option value="custom">Custom</option>
                  </select>
                </div>

                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium text-textSecondary mb-1">
                      Input Cost (per 1M tokens)
                    </label>
                    <div className="relative">
                      <span className="absolute left-3 top-1/2 -translate-y-1/2 text-textTertiary">$</span>
                      <input
                        type="number"
                        step="0.0001"
                        min="0"
                        value={newModel.input_cost_per_token * 1_000_000}
                        onChange={(e) => setNewModel({ 
                          ...newModel, 
                          input_cost_per_token: parseFloat(e.target.value) / 1_000_000 || 0 
                        })}
                        className="w-full pl-8 pr-3 py-2 bg-background border border-border rounded-lg text-textPrimary focus:outline-none focus:ring-2 focus:ring-primary"
                      />
                    </div>
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-textSecondary mb-1">
                      Output Cost (per 1M tokens)
                    </label>
                    <div className="relative">
                      <span className="absolute left-3 top-1/2 -translate-y-1/2 text-textTertiary">$</span>
                      <input
                        type="number"
                        step="0.0001"
                        min="0"
                        value={newModel.output_cost_per_token * 1_000_000}
                        onChange={(e) => setNewModel({ 
                          ...newModel, 
                          output_cost_per_token: parseFloat(e.target.value) / 1_000_000 || 0 
                        })}
                        className="w-full pl-8 pr-3 py-2 bg-background border border-border rounded-lg text-textPrimary focus:outline-none focus:ring-2 focus:ring-primary"
                      />
                    </div>
                  </div>
                </div>

                <div className="pt-4 flex justify-end gap-3">
                  <button
                    onClick={() => setShowAddModal(false)}
                    className="px-4 py-2 text-textSecondary hover:text-textPrimary transition-colors"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleAddCustom}
                    disabled={saving || !newModel.model_id.trim()}
                    className="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-hover transition-colors disabled:opacity-50"
                  >
                    {saving ? (
                      <Loader2 className="w-4 h-4 animate-spin" />
                    ) : (
                      <Save className="w-4 h-4" />
                    )}
                    Add Pricing
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
