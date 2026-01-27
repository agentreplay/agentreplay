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
import { Link } from 'react-router-dom';
import { API_BASE_URL } from '../lib/flowtrace-api';
import { VideoHelpButton } from '../components/VideoHelpButton';
import { loadPrompts, PromptRecord } from '../lib/prompt-store';
import { 
  Scale, 
  Play, 
  Plus, 
  Minus, 
  Loader2, 
  Clock, 
  DollarSign, 
  Zap,
  ThumbsUp,
  ThumbsDown,
  RefreshCw,
  Settings2,
  Copy,
  Check,
  AlertTriangle,
  Key,
  ExternalLink,
  Library,
  FileText
} from 'lucide-react';

// Types
// OpenAI-compatible provider configuration (matches SettingsPage)
interface ProviderConfig {
  id: string;
  name: string;
  provider: 'openai' | 'anthropic' | 'ollama' | 'custom';
  baseUrl: string;
  modelName: string;
  apiKey: string;
  isDefault?: boolean;
  isValid?: boolean;
}

interface FlowTraceSettings {
  models: {
    providers: ProviderConfig[];
    defaultProviderId: string | null;
    defaultTemperature: number;
    defaultMaxTokens: number;
  };
}

interface ModelOption {
  provider: string;
  model_id: string;
  display_name: string;
  input_cost_per_1m: number | null;
  output_cost_per_1m: number | null;
  context_window: number | null;
  available: boolean;
}

interface ModelSelection {
  provider: string;
  model_id: string;
  display_name?: string;
}

interface ComparisonResult {
  model_key: string;
  provider: string;
  model_id: string;
  content: string;
  input_tokens: number;
  output_tokens: number;
  latency_ms: number;
  cost_usd: number;
  status: string;
  error?: string;
}

interface ComparisonSummary {
  total_models: number;
  successful: number;
  failed: number;
  fastest_model: string | null;
  cheapest_model: string | null;
  total_cost_usd: number;
  total_latency_ms: number;
}

interface ComparisonResponse {
  success: boolean;
  comparison_id: string;
  results: ComparisonResult[];
  summary: ComparisonSummary;
  error?: string;
}

interface OllamaModel {
  name: string;
  size?: number;
  modified_at?: string;
}

// All known provider models (used to show available models when provider is configured)
const ALL_PROVIDER_MODELS: Record<string, { id: string; name: string }[]> = {
  openai: [
    { id: 'gpt-4o', name: 'GPT-4o' },
    { id: 'gpt-4o-mini', name: 'GPT-4o Mini' },
    { id: 'gpt-4-turbo', name: 'GPT-4 Turbo' },
    { id: 'gpt-3.5-turbo', name: 'GPT-3.5 Turbo' },
    { id: 'o1-preview', name: 'o1 Preview' },
    { id: 'o1-mini', name: 'o1 Mini' },
  ],
  anthropic: [
    { id: 'claude-3-5-sonnet-20241022', name: 'Claude 3.5 Sonnet' },
    { id: 'claude-3-5-haiku-20241022', name: 'Claude 3.5 Haiku' },
    { id: 'claude-3-opus-20240229', name: 'Claude 3 Opus' },
  ],
  deepseek: [
    { id: 'deepseek-chat', name: 'DeepSeek Chat' },
    { id: 'deepseek-coder', name: 'DeepSeek Coder' },
  ],
  custom: [
    { id: 'custom-model', name: 'Custom Model' },
  ],
  ollama: [], // Will be populated dynamically
};

// Map settings provider names to lowercase for consistency
const normalizeProviderName = (provider: string): string => {
  return provider.toLowerCase();
};

export default function ModelComparisonPage() {
  // State
  const [availableModels, setAvailableModels] = useState<ModelOption[]>([]);
  const [configuredProviders, setConfiguredProviders] = useState<string[]>([]);
  const [providerModels, setProviderModels] = useState<Record<string, { id: string; name: string }[]>>(ALL_PROVIDER_MODELS);
  const [selectedModels, setSelectedModels] = useState<ModelSelection[]>([]);
  const [prompt, setPrompt] = useState('');
  const [systemPrompt, setSystemPrompt] = useState('');
  const [temperature, setTemperature] = useState(0.7);
  const [maxTokens, setMaxTokens] = useState(1024);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [isLoadingConfig, setIsLoadingConfig] = useState(true);
  const [ollamaAvailable, setOllamaAvailable] = useState(false);
  
  // Prompt source state
  const [promptSource, setPromptSource] = useState<'custom' | 'registry'>('custom');
  const [registryPrompts, setRegistryPrompts] = useState<PromptRecord[]>([]);
  const [selectedPromptId, setSelectedPromptId] = useState<string | null>(null);
  
  // Results state
  const [isRunning, setIsRunning] = useState(false);
  const [results, setResults] = useState<ComparisonResult[]>([]);
  const [summary, setSummary] = useState<ComparisonSummary | null>(null);
  const [comparisonId, setComparisonId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  
  // Ratings state
  const [ratings, setRatings] = useState<Record<string, 'up' | 'down' | null>>({});
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);

  // Load user configuration and available models on mount
  useEffect(() => {
    loadUserConfiguration();
    // Load prompts from registry
    const prompts = loadPrompts();
    setRegistryPrompts(prompts);
    
    // Reload prompts when window gets focus (in case user added prompts in another tab)
    const handleFocus = () => {
      const updatedPrompts = loadPrompts();
      setRegistryPrompts(updatedPrompts);
    };
    window.addEventListener('focus', handleFocus);
    return () => window.removeEventListener('focus', handleFocus);
  }, []);

  // Parse prompt template to extract system and user messages
  const parsePromptTemplate = useCallback((content: string): { system: string; user: string } => {
    // Check for Jinja-style comments like {# SYSTEM PROMPT #} or {# SYSTEM #} or {# SYSTEM MESSAGE #}
    // And {# USER PROMPT #} or {# USER #} or {# USER MESSAGE #}
    const systemMarkerRegex = /\{#\s*SYSTEM\s*(?:PROMPT|MESSAGE)?\s*#\}/i;
    const userMarkerRegex = /\{#\s*USER\s*(?:PROMPT|MESSAGE)?\s*#\}/i;
    
    const systemMarkerMatch = content.match(systemMarkerRegex);
    const userMarkerMatch = content.match(userMarkerRegex);
    
    if (systemMarkerMatch || userMarkerMatch) {
      let systemContent = '';
      let userContent = '';
      
      if (systemMarkerMatch && userMarkerMatch) {
        // Both markers present - split content between them
        const systemStart = systemMarkerMatch.index! + systemMarkerMatch[0].length;
        const userStart = userMarkerMatch.index!;
        const userContentStart = userStart + userMarkerMatch[0].length;
        
        // System content is between system marker and user marker
        systemContent = content.substring(systemStart, userStart).trim();
        // User content is after user marker
        userContent = content.substring(userContentStart).trim();
      } else if (systemMarkerMatch) {
        // Only system marker - everything after it is system prompt
        const systemStart = systemMarkerMatch.index! + systemMarkerMatch[0].length;
        systemContent = content.substring(systemStart).trim();
      } else if (userMarkerMatch) {
        // Only user marker - everything before is system, after is user
        const userStart = userMarkerMatch.index!;
        const userContentStart = userStart + userMarkerMatch[0].length;
        systemContent = content.substring(0, userStart).trim();
        userContent = content.substring(userContentStart).trim();
      }
      
      return { system: systemContent, user: userContent };
    }
    
    // Otherwise treat entire content as system prompt
    return {
      system: content.trim(),
      user: ''
    };
  }, []);

  // Handle prompt selection from registry
  const handlePromptSelection = useCallback((promptId: string) => {
    const selected = registryPrompts.find(p => p.id === promptId);
    if (selected) {
      setSelectedPromptId(promptId);
      const parsed = parsePromptTemplate(selected.content);
      setSystemPrompt(parsed.system);
      setPrompt(parsed.user);
    }
  }, [registryPrompts, parsePromptTemplate]);

  // Check Ollama availability
  const checkOllamaStatus = useCallback(async (): Promise<OllamaModel[]> => {
    try {
      const response = await fetch('http://localhost:11434/api/tags', {
        method: 'GET',
        signal: AbortSignal.timeout(3000),
      });
      if (response.ok) {
        const data = await response.json();
        setOllamaAvailable(true);
        return data.models || [];
      }
    } catch {
      setOllamaAvailable(false);
    }
    return [];
  }, []);

  const loadUserConfiguration = async () => {
    setIsLoadingConfig(true);
    try {
      // Load settings from localStorage (same as SettingsPage)
      const savedSettings = localStorage.getItem('flowtrace_settings');
      const settings: FlowTraceSettings | null = savedSettings ? JSON.parse(savedSettings) : null;
      
      const providers: string[] = [];
      const updatedProviderModels = { ...ALL_PROVIDER_MODELS };
      const configuredModelsMap: Record<string, { id: string; name: string }[]> = {};
      
      // Check which providers are configured with their models
      if (settings?.models?.providers) {
        for (const providerConfig of settings.models.providers) {
          // For cloud providers, require API key; Ollama is optional
          const hasValidConfig = providerConfig.provider === 'ollama' 
            ? providerConfig.baseUrl && providerConfig.modelName
            : providerConfig.apiKey && providerConfig.baseUrl && providerConfig.modelName;
          
          if (hasValidConfig) {
            const normalizedProvider = providerConfig.provider;
            if (!providers.includes(normalizedProvider)) {
              providers.push(normalizedProvider);
              configuredModelsMap[normalizedProvider] = [];
            }
            // Add the configured model to the list for this provider
            configuredModelsMap[normalizedProvider].push({
              id: providerConfig.modelName,
              name: `${providerConfig.name} (${providerConfig.modelName})`,
            });
          }
        }
        
        // Merge configured models with known models for each provider
        for (const provider of providers) {
          const configuredModels = configuredModelsMap[provider] || [];
          const knownModels = ALL_PROVIDER_MODELS[provider] || [];
          // Put configured models first, then append known models that aren't already configured
          const configuredIds = new Set(configuredModels.map(m => m.id));
          const additionalKnownModels = knownModels.filter(m => !configuredIds.has(m.id));
          updatedProviderModels[provider] = [...configuredModels, ...additionalKnownModels];
        }
      }
      
      // Check if Ollama is available (local, no API key needed)
      const ollamaModels = await checkOllamaStatus();
      if (ollamaModels.length > 0) {
        if (!providers.includes('ollama')) {
          providers.push('ollama');
        }
        // Populate Ollama models dynamically
        updatedProviderModels.ollama = ollamaModels.map((m: OllamaModel) => ({
          id: m.name,
          name: `${m.name} (Local)`,
        }));
      }
      
      setConfiguredProviders(providers);
      setProviderModels(updatedProviderModels);
      
      // Set default model selections based on configured providers
      if (providers.length >= 2) {
        const defaultSelections: ModelSelection[] = [];
        for (let i = 0; i < Math.min(2, providers.length); i++) {
          const provider = providers[i];
          const firstModel = updatedProviderModels[provider]?.[0];
          if (firstModel) {
            defaultSelections.push({ provider, model_id: firstModel.id });
          }
        }
        setSelectedModels(defaultSelections);
      } else if (providers.length === 1) {
        const provider = providers[0];
        const models = updatedProviderModels[provider] || [];
        if (models.length >= 2) {
          setSelectedModels([
            { provider, model_id: models[0].id },
            { provider, model_id: models[1].id },
          ]);
        } else if (models.length === 1) {
          setSelectedModels([{ provider, model_id: models[0].id }]);
        }
      }
      
      // Also try to load pricing info from backend
      loadModels();
    } catch (e) {
      console.error('Failed to load configuration:', e);
    } finally {
      setIsLoadingConfig(false);
    }
  };

  const loadModels = async () => {
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/comparison/models`);
      if (response.ok) {
        const data = await response.json();
        setAvailableModels(data.models || []);
      }
    } catch (e) {
      console.error('Failed to load models:', e);
    }
  };

  const addModel = () => {
    if (selectedModels.length >= 3) return;
    
    // Find a model that's not already selected from configured providers only
    const usedIds = new Set(selectedModels.map(m => m.model_id));
    for (const provider of configuredProviders) {
      const models = providerModels[provider] || [];
      for (const model of models) {
        if (!usedIds.has(model.id)) {
          setSelectedModels([...selectedModels, { provider, model_id: model.id }]);
          return;
        }
      }
    }
  };

  const removeModel = (index: number) => {
    if (selectedModels.length <= 2) return;
    setSelectedModels(selectedModels.filter((_, i) => i !== index));
  };

  const updateModel = (index: number, provider: string, model_id: string) => {
    const newModels = [...selectedModels];
    newModels[index] = { provider, model_id };
    setSelectedModels(newModels);
  };

  const runComparison = async () => {
    if (!prompt.trim()) {
      setError('Please enter a prompt');
      return;
    }
    
    if (selectedModels.length < 2) {
      setError('Please select at least 2 models');
      return;
    }

    setIsRunning(true);
    setError(null);
    setResults([]);
    setSummary(null);
    setRatings({});

    try {
      // Load provider configurations from settings
      const savedSettings = localStorage.getItem('flowtrace_settings');
      const settings: FlowTraceSettings | null = savedSettings ? JSON.parse(savedSettings) : null;
      const providers = settings?.models?.providers || [];

      // Build models with provider configuration
      const modelsWithConfig = selectedModels.map(m => {
        // Find matching provider config for this model
        // First try exact match on model name
        let providerConfig = providers.find(p => p.modelName === m.model_id);
        
        // If no exact match, try matching by provider type
        if (!providerConfig) {
          providerConfig = providers.find(p => p.provider === m.provider);
        }
        
        // If still no match, try finding any provider that might work
        // (e.g., user selected "openai" but model is from Llama API configured as "custom")
        if (!providerConfig) {
          providerConfig = providers.find(p => 
            p.modelName && m.model_id.toLowerCase().includes(p.modelName.toLowerCase().split('-')[0])
          );
        }

        console.log(`Model ${m.model_id}: Found provider config:`, providerConfig ? {
          provider: providerConfig.provider,
          baseUrl: providerConfig.baseUrl,
          hasApiKey: !!providerConfig.apiKey
        } : 'NONE');

        return {
          provider: providerConfig?.provider || m.provider,
          model_id: m.model_id,
          display_name: providerModels[m.provider]?.find(pm => pm.id === m.model_id)?.name || m.model_id,
          base_url: providerConfig?.baseUrl || null,
          api_key: providerConfig?.apiKey || null,
        };
      });

      console.log('Sending models with config:', modelsWithConfig.map(m => ({
        provider: m.provider,
        model_id: m.model_id,
        base_url: m.base_url,
        hasApiKey: !!m.api_key
      })));

      const response = await fetch(`${API_BASE_URL}/api/v1/comparison/run`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          prompt,
          models: modelsWithConfig,
          temperature,
          max_tokens: maxTokens,
          system_prompt: systemPrompt || undefined,
          variables: {},
        }),
      });

      if (!response.ok) {
        throw new Error(`HTTP error: ${response.status}`);
      }

      const data: ComparisonResponse = await response.json();
      
      if (data.success) {
        setResults(data.results);
        setSummary(data.summary);
        setComparisonId(data.comparison_id);
      } else {
        setError(data.error || 'Comparison failed');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to run comparison');
    } finally {
      setIsRunning(false);
    }
  };

  const rateResponse = (modelKey: string, rating: 'up' | 'down') => {
    setRatings(prev => ({
      ...prev,
      [modelKey]: prev[modelKey] === rating ? null : rating,
    }));
  };

  const copyToClipboard = async (text: string, index: number) => {
    await navigator.clipboard.writeText(text);
    setCopiedIndex(index);
    setTimeout(() => setCopiedIndex(null), 2000);
  };

  const formatCost = (cost: number): string => {
    if (cost === 0) return 'Free';
    if (cost < 0.0001) return `$${cost.toFixed(6)}`;
    if (cost < 0.01) return `$${cost.toFixed(4)}`;
    return `$${cost.toFixed(4)}`;
  };

  const formatLatency = (ms: number): string => {
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(2)}s`;
  };

  const getStatusColor = (status: string): string => {
    switch (status) {
      case 'completed': return 'text-green-500';
      case 'error': return 'text-red-500';
      case 'timeout': return 'text-yellow-500';
      default: return 'text-gray-500';
    }
  };

  // Show loading state while checking configuration
  if (isLoadingConfig) {
    return (
      <div className="min-h-screen bg-background p-6 flex items-center justify-center">
        <div className="text-center">
          <Loader2 className="w-10 h-10 animate-spin text-primary mx-auto mb-4" />
          <p className="text-textSecondary">Loading configuration...</p>
        </div>
      </div>
    );
  }

  // No providers configured - show helpful message
  if (configuredProviders.length === 0) {
    return (
      <div className="min-h-screen bg-background p-6">
        <div className="max-w-2xl mx-auto">
          <div className="flex items-center gap-3 mb-8">
            <Scale className="w-8 h-8 text-primary" />
            <div>
              <h1 className="text-3xl font-bold text-textPrimary">Model Comparison</h1>
              <p className="text-textSecondary">
                Compare responses from multiple models side-by-side
              </p>
            </div>
          </div>

          <div className="bg-surface border border-border rounded-xl p-8 text-center">
            <div className="w-16 h-16 bg-amber-500/10 rounded-full flex items-center justify-center mx-auto mb-4">
              <AlertTriangle className="w-8 h-8 text-amber-500" />
            </div>
            <h2 className="text-xl font-semibold text-textPrimary mb-2">No Models Configured</h2>
            <p className="text-textSecondary mb-6 max-w-md mx-auto">
              To compare models, you need to configure at least one API provider in Settings. 
              Add your OpenAI, Anthropic, or other API keys to get started.
            </p>
            
            <div className="flex flex-col items-center gap-4">
              <Link
                to="/settings"
                className="px-6 py-3 bg-primary text-white rounded-lg flex items-center gap-2 hover:bg-primary/90 transition-colors font-medium"
              >
                <Key className="w-5 h-5" />
                Configure API Keys in Settings
                <ExternalLink className="w-4 h-4 ml-1" />
              </Link>
              
              {ollamaAvailable ? (
                <p className="text-sm text-green-500 flex items-center gap-2">
                  <Check className="w-4 h-4" />
                  Ollama detected locally - refresh to use local models
                </p>
              ) : (
                <p className="text-sm text-textSecondary">
                  Or install <a href="https://ollama.ai" target="_blank" rel="noopener noreferrer" className="text-primary hover:underline">Ollama</a> to run models locally without API keys
                </p>
              )}
            </div>
          </div>
        </div>
      </div>
    );
  }

  // Only one provider with insufficient models
  const totalModelsAvailable = configuredProviders.reduce(
    (sum, provider) => sum + (providerModels[provider]?.length || 0), 
    0
  );
  
  if (totalModelsAvailable < 2) {
    return (
      <div className="min-h-screen bg-background p-6">
        <div className="max-w-2xl mx-auto">
          <div className="flex items-center gap-3 mb-8">
            <Scale className="w-8 h-8 text-primary" />
            <div>
              <h1 className="text-3xl font-bold text-textPrimary">Model Comparison</h1>
              <p className="text-textSecondary">
                Compare responses from multiple models side-by-side
              </p>
            </div>
          </div>

          <div className="bg-surface border border-border rounded-xl p-8 text-center">
            <div className="w-16 h-16 bg-blue-500/10 rounded-full flex items-center justify-center mx-auto mb-4">
              <Scale className="w-8 h-8 text-blue-500" />
            </div>
            <h2 className="text-xl font-semibold text-textPrimary mb-2">Need More Models</h2>
            <p className="text-textSecondary mb-6 max-w-md mx-auto">
              Model comparison requires at least 2 models. You currently have {totalModelsAvailable} model(s) available.
              Add another API provider or run more Ollama models locally.
            </p>
            
            <div className="flex flex-col items-center gap-4">
              <Link
                to="/settings"
                className="px-6 py-3 bg-primary text-white rounded-lg flex items-center gap-2 hover:bg-primary/90 transition-colors font-medium"
              >
                <Key className="w-5 h-5" />
                Add More API Keys
                <ExternalLink className="w-4 h-4 ml-1" />
              </Link>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-background p-6">
      <div className="max-w-7xl mx-auto space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Scale className="w-8 h-8 text-primary" />
            <div>
              <h1 className="text-3xl font-bold text-textPrimary">Model Comparison</h1>
              <p className="text-textSecondary">
                Compare responses from up to 3 models side-by-side
              </p>
            </div>
          </div>
          
          <div className="flex items-center gap-3">
            <VideoHelpButton pageId="compare" />
            <button
              onClick={loadUserConfiguration}
              className="px-4 py-2 bg-surface border border-border rounded-lg flex items-center gap-2 hover:bg-surface-hover transition-colors text-textPrimary"
              title="Refresh available models"
            >
              <RefreshCw className="w-4 h-4" />
            </button>
            <button
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="px-4 py-2 bg-surface border border-border rounded-lg flex items-center gap-2 hover:bg-surface-hover transition-colors text-textPrimary"
            >
              <Settings2 className="w-4 h-4" />
              {showAdvanced ? 'Hide' : 'Show'} Settings
            </button>
          </div>
        </div>

        {/* Configured Providers Info */}
        <div className="bg-surface/50 border border-border rounded-lg px-4 py-3 flex items-center justify-between">
          <div className="flex items-center gap-2 text-sm text-textSecondary">
            <Key className="w-4 h-4" />
            <span>Configured providers:</span>
            <span className="flex gap-2">
              {configuredProviders.map(provider => (
                <span 
                  key={provider}
                  className="px-2 py-0.5 bg-primary/20 text-primary rounded text-xs font-medium capitalize"
                >
                  {provider}
                </span>
              ))}
            </span>
          </div>
          <Link 
            to="/settings?tab=models" 
            className="text-sm text-primary hover:underline flex items-center gap-1"
          >
            Manage <ExternalLink className="w-3 h-3" />
          </Link>
        </div>

        {/* Model Selection */}
        <div className="bg-surface border border-border rounded-xl p-6">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold text-textPrimary">Models to Compare</h2>
            <button
              onClick={addModel}
              disabled={selectedModels.length >= 3}
              className="px-3 py-1.5 bg-primary text-white rounded-lg flex items-center gap-1.5 hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm"
            >
              <Plus className="w-4 h-4" />
              Add Model
            </button>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            {selectedModels.map((model, index) => (
              <div 
                key={index} 
                className="bg-surface-elevated border border-border rounded-lg p-4 space-y-3"
              >
                <div className="flex items-center justify-between">
                  <span className="text-xs font-medium text-textSecondary uppercase tracking-wide">
                    Model {index + 1}
                  </span>
                  {selectedModels.length > 2 && (
                    <button
                      onClick={() => removeModel(index)}
                      className="p-1 hover:bg-red-500/10 rounded transition-colors"
                    >
                      <Minus className="w-4 h-4 text-red-500" />
                    </button>
                  )}
                </div>

                <select
                  value={model.provider}
                  onChange={(e) => {
                    const newProvider = e.target.value;
                    const firstModel = providerModels[newProvider]?.[0];
                    if (firstModel) {
                      updateModel(index, newProvider, firstModel.id);
                    }
                  }}
                  className="w-full bg-background border border-border rounded-lg px-3 py-2 text-textPrimary text-sm"
                >
                  {configuredProviders.map(provider => (
                    <option key={provider} value={provider}>
                      {provider === 'openai' ? 'OpenAI' : 
                       provider === 'anthropic' ? 'Anthropic' : 
                       provider === 'deepseek' ? 'DeepSeek' :
                       provider === 'ollama' ? 'Ollama (Local)' :
                       provider.charAt(0).toUpperCase() + provider.slice(1)}
                    </option>
                  ))}
                </select>

                <select
                  value={model.model_id}
                  onChange={(e) => updateModel(index, model.provider, e.target.value)}
                  className="w-full bg-background border border-border rounded-lg px-3 py-2 text-textPrimary text-sm"
                >
                  {providerModels[model.provider]?.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.name}
                    </option>
                  ))}
                </select>

                {/* Pricing info */}
                {availableModels.find(m => m.model_id === model.model_id) && (
                  <div className="flex items-center gap-2 text-xs text-textSecondary">
                    <DollarSign className="w-3 h-3" />
                    <span>
                      ${availableModels.find(m => m.model_id === model.model_id)?.input_cost_per_1m?.toFixed(2) || '?'}/1M in
                    </span>
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>

        {/* Advanced Settings */}
        {showAdvanced && (
          <div className="bg-surface border border-border rounded-xl p-6 space-y-4">
            <h2 className="text-lg font-semibold text-textPrimary">Advanced Settings</h2>
            
            <div className="grid grid-cols-2 gap-6">
              <div>
                <label className="block text-sm font-medium text-textSecondary mb-2">
                  Temperature: {temperature.toFixed(2)}
                </label>
                <input
                  type="range"
                  min="0"
                  max="2"
                  step="0.1"
                  value={temperature}
                  onChange={(e) => setTemperature(parseFloat(e.target.value))}
                  className="w-full accent-primary"
                />
                <div className="flex justify-between text-xs text-textSecondary">
                  <span>Precise</span>
                  <span>Balanced</span>
                  <span>Creative</span>
                </div>
              </div>
              
              <div>
                <label className="block text-sm font-medium text-textSecondary mb-2">
                  Max Tokens: {maxTokens}
                </label>
                <input
                  type="range"
                  min="128"
                  max="4096"
                  step="128"
                  value={maxTokens}
                  onChange={(e) => setMaxTokens(parseInt(e.target.value))}
                  className="w-full accent-primary"
                />
              </div>
            </div>
          </div>
        )}

        {/* Prompt Source Selection */}
        <div className="bg-surface border border-border rounded-xl p-6 space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-semibold text-textPrimary">Prompt</h2>
            <div className="flex items-center gap-2">
              <button
                onClick={() => {
                  setPromptSource('custom');
                  setSelectedPromptId(null);
                }}
                className={`px-3 py-1.5 rounded-lg flex items-center gap-2 text-sm font-medium transition-colors ${
                  promptSource === 'custom'
                    ? 'bg-primary text-white'
                    : 'bg-surface-elevated text-textSecondary hover:text-textPrimary border border-border'
                }`}
              >
                <FileText className="w-4 h-4" />
                Custom
              </button>
              <button
                onClick={() => setPromptSource('registry')}
                className={`px-3 py-1.5 rounded-lg flex items-center gap-2 text-sm font-medium transition-colors ${
                  promptSource === 'registry'
                    ? 'bg-primary text-white'
                    : 'bg-surface-elevated text-textSecondary hover:text-textPrimary border border-border'
                }`}
              >
                <Library className="w-4 h-4" />
                From Registry
              </button>
            </div>
          </div>

          {/* Registry Prompt Selection */}
          {promptSource === 'registry' && (
            <div className="space-y-3">
              <label className="block text-sm font-medium text-textSecondary">
                Select a prompt from your registry
              </label>
              <select
                value={selectedPromptId || ''}
                onChange={(e) => handlePromptSelection(e.target.value)}
                className="w-full bg-background border border-border rounded-lg px-4 py-3 text-textPrimary"
              >
                <option value="">-- Select a prompt --</option>
                {registryPrompts.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} {p.tags.length > 0 && `(${p.tags.join(', ')})`}
                  </option>
                ))}
              </select>
              {selectedPromptId && (
                <p className="text-xs text-textSecondary">
                  {registryPrompts.find(p => p.id === selectedPromptId)?.description || 'No description'}
                </p>
              )}
            </div>
          )}

          {/* System Prompt (shown when registry prompt selected or in custom mode) */}
          {(promptSource === 'custom' || selectedPromptId) && (
            <div className="space-y-2">
              <label className="block text-sm font-medium text-textSecondary">
                System Prompt {promptSource === 'registry' && '(from registry)'}
              </label>
              <textarea
                value={systemPrompt}
                onChange={(e) => setSystemPrompt(e.target.value)}
                placeholder="Enter a system prompt to set the model's behavior..."
                className="w-full bg-background border border-border rounded-lg px-4 py-3 text-textPrimary min-h-[100px] resize-y font-mono text-sm"
                readOnly={promptSource === 'registry'}
              />
            </div>
          )}

          {/* User Prompt */}
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <label className="block text-sm font-medium text-textSecondary">
                User Prompt
              </label>
              <span className="text-xs text-textSecondary">
                {prompt.length} characters
              </span>
            </div>
            <textarea
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              placeholder="Enter your prompt here. All selected models will receive the same prompt..."
              className="w-full bg-background border border-border rounded-lg px-4 py-3 text-textPrimary min-h-[120px] resize-y font-mono text-sm"
            />
          </div>

          {/* Variables hint for registry prompts */}
          {promptSource === 'registry' && selectedPromptId && (
            <div className="text-xs text-textSecondary bg-surface-elevated rounded-lg p-3">
              <span className="font-medium">Tip:</span> This prompt may contain variables like <code className="bg-background px-1 py-0.5 rounded">{'{{variable}}'}</code>. 
              You can replace them manually in the prompts above before comparing.
            </div>
          )}
          
          <div className="flex items-center justify-between pt-2">
            <div className="text-sm text-textSecondary">
              {selectedModels.length} model{selectedModels.length !== 1 ? 's' : ''} selected
            </div>
            
            <button
              onClick={runComparison}
              disabled={isRunning || !prompt.trim() || selectedModels.length < 2}
              className="px-6 py-2.5 bg-primary text-white rounded-lg flex items-center gap-2 hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed font-medium"
            >
              {isRunning ? (
                <>
                  <Loader2 className="w-5 h-5 animate-spin" />
                  Running Comparison...
                </>
              ) : (
                <>
                  <Play className="w-5 h-5" />
                  Compare Models
                </>
              )}
            </button>
          </div>
        </div>

        {/* Error Display */}
        {error && (
          <div className="bg-red-500/10 border border-red-500/20 rounded-xl p-4 text-red-500">
            <p className="font-medium">Error</p>
            <p className="text-sm mt-1">{error}</p>
          </div>
        )}

        {/* Results */}
        {results.length > 0 && (
          <div className="space-y-6">
            {/* Summary */}
            {summary && (
              <div className="bg-surface border border-border rounded-xl p-6">
                <h2 className="text-lg font-semibold text-textPrimary mb-4">Summary</h2>
                <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                  <div className="bg-surface-elevated rounded-lg p-4">
                    <div className="flex items-center gap-2 text-textSecondary mb-1">
                      <Clock className="w-4 h-4" />
                      <span className="text-sm">Total Time</span>
                    </div>
                    <p className="text-xl font-semibold text-textPrimary">
                      {formatLatency(summary.total_latency_ms)}
                    </p>
                  </div>
                  
                  <div className="bg-surface-elevated rounded-lg p-4">
                    <div className="flex items-center gap-2 text-textSecondary mb-1">
                      <DollarSign className="w-4 h-4" />
                      <span className="text-sm">Total Cost</span>
                    </div>
                    <p className="text-xl font-semibold text-textPrimary">
                      {formatCost(summary.total_cost_usd)}
                    </p>
                  </div>
                  
                  {summary.fastest_model && (
                    <div className="bg-green-500/10 rounded-lg p-4">
                      <div className="flex items-center gap-2 text-green-500 mb-1">
                        <Zap className="w-4 h-4" />
                        <span className="text-sm">Fastest</span>
                      </div>
                      <p className="text-lg font-semibold text-textPrimary truncate">
                        {summary.fastest_model.split('/').pop()}
                      </p>
                    </div>
                  )}
                  
                  {summary.cheapest_model && (
                    <div className="bg-blue-500/10 rounded-lg p-4">
                      <div className="flex items-center gap-2 text-blue-500 mb-1">
                        <DollarSign className="w-4 h-4" />
                        <span className="text-sm">Cheapest</span>
                      </div>
                      <p className="text-lg font-semibold text-textPrimary truncate">
                        {summary.cheapest_model.split('/').pop()}
                      </p>
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* Results Grid */}
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {results.map((result, index) => (
                <div 
                  key={result.model_key}
                  className={`bg-surface border rounded-xl overflow-hidden ${
                    summary?.fastest_model === result.model_key 
                      ? 'border-green-500/50' 
                      : summary?.cheapest_model === result.model_key 
                        ? 'border-blue-500/50'
                        : 'border-border'
                  }`}
                >
                  {/* Model Header */}
                  <div className="bg-surface-elevated px-4 py-3 border-b border-border">
                    <div className="flex items-center justify-between">
                      <div>
                        <p className="font-medium text-textPrimary">
                          {providerModels[result.provider]?.find(m => m.id === result.model_id)?.name || result.model_id}
                        </p>
                        <p className="text-xs text-textSecondary capitalize">{result.provider}</p>
                      </div>
                      <span className={`text-xs font-medium ${getStatusColor(result.status)}`}>
                        {result.status}
                      </span>
                    </div>
                    
                    {/* Badges */}
                    <div className="flex gap-2 mt-2">
                      {summary?.fastest_model === result.model_key && (
                        <span className="px-2 py-0.5 bg-green-500/20 text-green-500 text-xs rounded-full flex items-center gap-1">
                          <Zap className="w-3 h-3" /> Fastest
                        </span>
                      )}
                      {summary?.cheapest_model === result.model_key && (
                        <span className="px-2 py-0.5 bg-blue-500/20 text-blue-500 text-xs rounded-full flex items-center gap-1">
                          <DollarSign className="w-3 h-3" /> Cheapest
                        </span>
                      )}
                    </div>
                  </div>
                  
                  {/* Metrics */}
                  <div className="grid grid-cols-3 divide-x divide-border text-center py-2 bg-surface-elevated/50">
                    <div className="px-2">
                      <p className="text-xs text-textSecondary">Latency</p>
                      <p className="text-sm font-medium text-textPrimary">
                        {formatLatency(result.latency_ms)}
                      </p>
                    </div>
                    <div className="px-2">
                      <p className="text-xs text-textSecondary">Tokens</p>
                      <p className="text-sm font-medium text-textPrimary">
                        {result.input_tokens + result.output_tokens}
                      </p>
                    </div>
                    <div className="px-2">
                      <p className="text-xs text-textSecondary">Cost</p>
                      <p className="text-sm font-medium text-textPrimary">
                        {formatCost(result.cost_usd)}
                      </p>
                    </div>
                  </div>
                  
                  {/* Response Content */}
                  <div className="p-4">
                    {result.error ? (
                      <div className="text-red-500 text-sm">
                        <p className="font-medium">Error</p>
                        <p>{result.error}</p>
                      </div>
                    ) : (
                      <div className="relative">
                        <pre className="text-sm text-textPrimary whitespace-pre-wrap font-sans max-h-64 overflow-y-auto">
                          {result.content}
                        </pre>
                        <button
                          onClick={() => copyToClipboard(result.content, index)}
                          className="absolute top-0 right-0 p-1.5 bg-surface rounded hover:bg-surface-hover transition-colors"
                          title="Copy to clipboard"
                        >
                          {copiedIndex === index ? (
                            <Check className="w-4 h-4 text-green-500" />
                          ) : (
                            <Copy className="w-4 h-4 text-textSecondary" />
                          )}
                        </button>
                      </div>
                    )}
                  </div>
                  
                  {/* Rating Buttons */}
                  {result.status === 'completed' && (
                    <div className="px-4 pb-4 flex gap-2">
                      <button
                        onClick={() => rateResponse(result.model_key, 'up')}
                        className={`flex-1 py-2 rounded-lg flex items-center justify-center gap-1.5 transition-colors ${
                          ratings[result.model_key] === 'up'
                            ? 'bg-green-500/20 text-green-500'
                            : 'bg-surface-elevated text-textSecondary hover:bg-surface-hover'
                        }`}
                      >
                        <ThumbsUp className="w-4 h-4" />
                        <span className="text-sm">Good</span>
                      </button>
                      <button
                        onClick={() => rateResponse(result.model_key, 'down')}
                        className={`flex-1 py-2 rounded-lg flex items-center justify-center gap-1.5 transition-colors ${
                          ratings[result.model_key] === 'down'
                            ? 'bg-red-500/20 text-red-500'
                            : 'bg-surface-elevated text-textSecondary hover:bg-surface-hover'
                        }`}
                      >
                        <ThumbsDown className="w-4 h-4" />
                        <span className="text-sm">Bad</span>
                      </button>
                    </div>
                  )}
                </div>
              ))}
            </div>
            
            {/* Run Again */}
            <div className="flex justify-center">
              <button
                onClick={runComparison}
                disabled={isRunning}
                className="px-6 py-2.5 bg-surface border border-border rounded-lg flex items-center gap-2 hover:bg-surface-hover transition-colors text-textPrimary"
              >
                <RefreshCw className="w-4 h-4" />
                Run Again
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
