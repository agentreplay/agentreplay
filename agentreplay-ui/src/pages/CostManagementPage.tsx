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
    <div className="flex flex-col h-full">
      <div className="flex-1 px-2 py-4">
        {/* Header */}
        <div className="flex items-center justify-between mb-5">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: 'linear-gradient(135deg, #10b981, #059669)' }}>
              <DollarSign className="w-5 h-5" style={{ color: '#ffffff' }} />
            </div>
            <div>
              <h1 className="text-[22px] font-bold text-foreground">Cost Management</h1>
              <p className="text-[13px] text-muted-foreground">
                Manage model pricing for accurate cost tracking
                {lastSyncTime && (
                  <span className="ml-2" style={{ color: 'hsl(var(--muted-foreground))' }}>
                    · Last synced: {new Date(lastSyncTime * 1000).toLocaleDateString()}
                  </span>
                )}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2.5">
            <VideoHelpButton pageId="cost-management" />
            <button
              onClick={handleSync}
              disabled={syncing}
              className="flex items-center gap-2 px-4 py-2 rounded-xl text-[13px] font-semibold transition-all disabled:opacity-50"
              style={{ backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--foreground))' }}
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
              className="flex items-center gap-2 px-4 py-2 rounded-xl text-[13px] font-semibold transition-all"
              style={{ backgroundColor: '#0080FF', color: '#ffffff' }}
            >
              <Plus className="w-4 h-4" />
              Add Custom Pricing
            </button>
          </div>
        </div>

        {/* Status Message */}
        {message && (
          <div
            className="mb-4 px-4 py-3 rounded-xl flex items-center gap-2 text-[13px]"
            style={{
              backgroundColor: message.type === 'success' ? 'rgba(16,185,129,0.06)' : 'rgba(239,68,68,0.06)',
              border: `1px solid ${message.type === 'success' ? 'rgba(16,185,129,0.15)' : 'rgba(239,68,68,0.15)'}`,
              color: message.type === 'success' ? '#10b981' : '#ef4444',
            }}
          >
            {message.type === 'success' ? (
              <CheckCircle className="w-4 h-4 flex-shrink-0" />
            ) : (
              <AlertCircle className="w-4 h-4 flex-shrink-0" />
            )}
            <span>{message.text}</span>
            <button
              onClick={() => setMessage(null)}
              className="ml-auto hover:opacity-70"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        {/* Search */}
        <div className="relative mb-5">
          <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 w-4 h-4" style={{ color: 'hsl(var(--muted-foreground))' }} />
          <input
            type="text"
            placeholder="Search models..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-10 pr-4 py-2.5 rounded-xl text-[13px] focus:outline-none transition-all"
            style={{
              backgroundColor: 'hsl(var(--card))',
              border: '1px solid hsl(var(--border))',
              color: 'hsl(var(--foreground))',
            }}
          />
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <div className="text-center">
              <Loader2 className="w-8 h-8 animate-spin mx-auto mb-2" style={{ color: '#0080FF' }} />
              <p className="text-[13px] text-muted-foreground">Loading pricing data...</p>
            </div>
          </div>
        ) : (
          <div className="space-y-6">
            {/* Custom Pricing Section */}
            <div>
              <h2 className="text-[13px] font-semibold mb-3 flex items-center gap-2" style={{ color: 'hsl(var(--muted-foreground))', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                <DollarSign className="w-4 h-4" style={{ color: '#10b981' }} />
                Custom Pricing Overrides
                <span className="font-normal text-muted-foreground">
                  ({filteredCustom.length} models)
                </span>
              </h2>

              {filteredCustom.length === 0 ? (
                <div className="rounded-2xl p-8 text-center bg-card border border-border">
                  <div className="w-12 h-12 rounded-xl flex items-center justify-center mx-auto mb-3" style={{ background: 'linear-gradient(135deg, rgba(16,185,129,0.1), rgba(5,150,105,0.06))' }}>
                    <DollarSign className="w-6 h-6" style={{ color: '#10b981' }} />
                  </div>
                  <p className="text-[14px] font-semibold mb-1 text-foreground">No custom pricing configured</p>
                  <p className="text-[12px]" style={{ color: 'hsl(var(--muted-foreground))' }}>
                    Add custom pricing for models not in the registry or override existing prices
                  </p>
                </div>
              ) : (
                <div className="rounded-2xl overflow-hidden bg-card border border-border">
                  <table className="w-full">
                    <thead>
                      <tr className="border-b border-border">
                        <th className="px-4 py-2.5 text-left text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Model</th>
                        <th className="px-4 py-2.5 text-left text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Provider</th>
                        <th className="px-4 py-2.5 text-right text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Input (per 1M)</th>
                        <th className="px-4 py-2.5 text-right text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Output (per 1M)</th>
                        <th className="px-4 py-2.5 text-right text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {filteredCustom.map((entry) => (
                        <tr key={entry.model_id} className="transition-colors border-b border-border/50">
                          <td className="px-4 py-3">
                            <div className="flex items-center gap-2">
                              <Cpu className="w-3.5 h-3.5" style={{ color: '#0080FF' }} />
                              <span className="text-[13px] font-medium text-foreground">{entry.model_id}</span>
                            </div>
                          </td>
                          <td className="px-4 py-3 text-[13px] text-muted-foreground">{entry.provider}</td>
                          <td className="px-4 py-3 text-right text-[13px] font-mono" style={{ color: '#10b981' }}>
                            {formatCost(entry.input_cost_per_token * 1_000_000)}
                          </td>
                          <td className="px-4 py-3 text-right text-[13px] font-mono" style={{ color: '#10b981' }}>
                            {formatCost(entry.output_cost_per_token * 1_000_000)}
                          </td>
                          <td className="px-4 py-3 text-right">
                            <div className="flex items-center justify-end gap-1">
                              <button
                                onClick={() => setEditingModel(entry.model_id)}
                                className="p-1.5 rounded-lg transition-all"
                                style={{ color: 'hsl(var(--muted-foreground))' }}
                              >
                                <Edit2 className="w-3.5 h-3.5" />
                              </button>
                              <button
                                onClick={() => handleDeleteCustom(entry.model_id)}
                                className="p-1.5 rounded-lg transition-all"
                                style={{ color: 'hsl(var(--muted-foreground))' }}
                              >
                                <Trash2 className="w-3.5 h-3.5" />
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
              <h2 className="text-[13px] font-semibold mb-3 flex items-center gap-2" style={{ color: 'hsl(var(--muted-foreground))', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                <Cloud className="w-4 h-4" style={{ color: '#0080FF' }} />
                Registry Pricing (from LiteLLM)
                <span className="font-normal text-muted-foreground">
                  ({filteredModels.length} models)
                </span>
              </h2>

              <div className="rounded-2xl overflow-hidden bg-card border border-border">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-border">
                      <th className="px-4 py-2.5 text-left text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Model</th>
                      <th className="px-4 py-2.5 text-left text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Provider</th>
                      <th className="px-4 py-2.5 text-right text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Input (per 1M)</th>
                      <th className="px-4 py-2.5 text-right text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Output (per 1M)</th>
                      <th className="px-4 py-2.5 text-center text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Context</th>
                      <th className="px-4 py-2.5 text-center text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Features</th>
                    </tr>
                  </thead>
                  <tbody>
                    {filteredModels.slice(0, 50).map((model) => (
                      <tr key={model.model_id} className="transition-colors border-b border-border/50">
                        <td className="px-4 py-2.5">
                          <span className="text-[13px] font-medium text-foreground">{model.model_id}</span>
                        </td>
                        <td className="px-4 py-2.5 text-[13px] text-muted-foreground">{model.provider || 'unknown'}</td>
                        <td className={`px-4 py-2.5 text-right text-[13px] font-mono ${model.input_cost_per_1m === 0 ? 'text-emerald-500' : 'text-foreground'}`}>
                          {formatCost(model.input_cost_per_1m)}
                        </td>
                        <td className={`px-4 py-2.5 text-right text-[13px] font-mono ${model.output_cost_per_1m === 0 ? 'text-emerald-500' : 'text-foreground'}`}>
                          {formatCost(model.output_cost_per_1m)}
                        </td>
                        <td className="px-4 py-2.5 text-center text-[12px]" style={{ color: 'hsl(var(--muted-foreground))' }}>
                          {model.context_window ? `${(model.context_window / 1000).toFixed(0)}K` : '-'}
                        </td>
                        <td className="px-4 py-2.5 text-center">
                          <div className="flex items-center justify-center gap-1">
                            {model.supports_vision && (
                              <span className="px-1.5 py-0.5 text-[10px] font-semibold rounded-full" style={{ backgroundColor: 'rgba(139,92,246,0.06)', color: '#8b5cf6', border: '1px solid rgba(139,92,246,0.12)' }}>Vision</span>
                            )}
                            {model.supports_function_calling && (
                              <span className="px-1.5 py-0.5 text-[10px] font-semibold rounded-full" style={{ backgroundColor: 'rgba(0,128,255,0.06)', color: '#0080FF', border: '1px solid rgba(0,128,255,0.12)' }}>Functions</span>
                            )}
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                {filteredModels.length > 50 && (
                  <div className="px-4 py-2.5 text-center text-[12px] text-muted-foreground border-t border-border">
                    Showing 50 of {filteredModels.length} models. Use search to filter.
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Add Custom Pricing Modal */}
        {showAddModal && (
          <div className="fixed inset-0 flex items-center justify-center z-50" style={{ backgroundColor: 'rgba(0,0,0,0.4)', backdropFilter: 'blur(4px)' }}>
            <div className="w-full max-w-md p-6 rounded-2xl" style={{ backgroundColor: 'hsl(var(--card))', border: '1px solid hsl(var(--border))', boxShadow: '0 20px 60px rgba(0,0,0,0.15)' }}>
              <div className="flex items-center justify-between mb-5">
                <div className="flex items-center gap-2.5">
                  <div className="w-8 h-8 rounded-lg flex items-center justify-center" style={{ background: 'linear-gradient(135deg, rgba(0,128,255,0.1), rgba(0,200,255,0.06))' }}>
                    <Plus className="w-4 h-4" style={{ color: '#0080FF' }} />
                  </div>
                  <h3 className="text-[16px] font-bold text-foreground">Add Custom Pricing</h3>
                </div>
                <button
                  onClick={() => setShowAddModal(false)}
                  className="p-1.5 rounded-lg transition-all"
                  style={{ color: 'hsl(var(--muted-foreground))' }}
                >
                  <X className="w-4 h-4" />
                </button>
              </div>

              <div className="space-y-3.5">
                <div>
                  <label className="block text-[12px] font-semibold mb-1.5" style={{ color: 'hsl(var(--muted-foreground))', textTransform: 'uppercase', letterSpacing: '0.03em' }}>
                    Model ID *
                  </label>
                  <input
                    type="text"
                    value={newModel.model_id}
                    onChange={(e) => setNewModel({ ...newModel, model_id: e.target.value })}
                    placeholder="e.g., gpt-4o, claude-3-opus"
                    className="w-full px-3 py-2 rounded-xl text-[13px] focus:outline-none transition-all"
                    style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--foreground))' }}
                  />
                </div>

                <div>
                  <label className="block text-[12px] font-semibold mb-1.5" style={{ color: 'hsl(var(--muted-foreground))', textTransform: 'uppercase', letterSpacing: '0.03em' }}>
                    Provider
                  </label>
                  <select
                    value={newModel.provider}
                    onChange={(e) => setNewModel({ ...newModel, provider: e.target.value })}
                    className="w-full px-3 py-2 rounded-xl text-[13px] focus:outline-none transition-all"
                    style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--foreground))' }}
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

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-[12px] font-semibold mb-1.5" style={{ color: 'hsl(var(--muted-foreground))', textTransform: 'uppercase', letterSpacing: '0.03em' }}>
                      Input (per 1M)
                    </label>
                    <div className="relative">
                      <span className="absolute left-3 top-1/2 -translate-y-1/2 text-[13px]" style={{ color: 'hsl(var(--muted-foreground))' }}>$</span>
                      <input
                        type="number"
                        step="0.0001"
                        min="0"
                        value={newModel.input_cost_per_token * 1_000_000}
                        onChange={(e) => setNewModel({
                          ...newModel,
                          input_cost_per_token: parseFloat(e.target.value) / 1_000_000 || 0
                        })}
                        className="w-full pl-7 pr-3 py-2 rounded-xl text-[13px] focus:outline-none transition-all"
                        style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--foreground))' }}
                      />
                    </div>
                  </div>

                  <div>
                    <label className="block text-[12px] font-semibold mb-1.5" style={{ color: 'hsl(var(--muted-foreground))', textTransform: 'uppercase', letterSpacing: '0.03em' }}>
                      Output (per 1M)
                    </label>
                    <div className="relative">
                      <span className="absolute left-3 top-1/2 -translate-y-1/2 text-[13px]" style={{ color: 'hsl(var(--muted-foreground))' }}>$</span>
                      <input
                        type="number"
                        step="0.0001"
                        min="0"
                        value={newModel.output_cost_per_token * 1_000_000}
                        onChange={(e) => setNewModel({
                          ...newModel,
                          output_cost_per_token: parseFloat(e.target.value) / 1_000_000 || 0
                        })}
                        className="w-full pl-7 pr-3 py-2 rounded-xl text-[13px] focus:outline-none transition-all"
                        style={{ backgroundColor: 'hsl(var(--secondary))', border: '1px solid hsl(var(--border))', color: 'hsl(var(--foreground))' }}
                      />
                    </div>
                  </div>
                </div>

                <div className="pt-3 flex justify-end gap-2.5">
                  <button
                    onClick={() => setShowAddModal(false)}
                    className="px-4 py-2 rounded-xl text-[13px] font-semibold transition-all text-muted-foreground"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleAddCustom}
                    disabled={saving || !newModel.model_id.trim()}
                    className="flex items-center gap-2 px-4 py-2 rounded-xl text-[13px] font-semibold transition-all disabled:opacity-50"
                    style={{ backgroundColor: '#0080FF', color: '#ffffff' }}
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
