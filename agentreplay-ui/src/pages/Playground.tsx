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
import { useSearchParams, useNavigate, useParams } from 'react-router-dom';
import { Loader2, Play, AlertCircle, CheckCircle, Zap, Plus, Trash2, User, Bot, Settings, ArrowLeft } from 'lucide-react';
import { API_BASE_URL } from '../lib/agentreplay-api';

// Types
interface PlaygroundRunResponse {
  output: string;
  metadata: RunMetadata;
}

interface RunMetadata {
  latency_ms: number;
  tokens_used: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
  cost_usd: number;
  model_used: string;
}

interface ModelInfo {
  id: string;
  name: string;
  provider: string;
  available: boolean;
}

interface LLMHealth {
  ollama: { available: boolean; configured: boolean; message: string };
  openai: { available: boolean; configured: boolean; message: string };
  anthropic: { available: boolean; configured: boolean; message: string };
}

interface Message {
  id: string;
  role: 'system' | 'user' | 'assistant';
  content: string;
}

export default function Playground() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const { projectId } = useParams();
  const [messages, setMessages] = useState<Message[]>([
    { id: '1', role: 'system', content: 'You are a helpful assistant.' },
    { id: '2', role: 'user', content: '' }
  ]);
  const [model, setModel] = useState<string>(""); // Start empty, will be set from user config
  const [temperature, setTemperature] = useState(0.7);
  const [maxTokens, setMaxTokens] = useState(2048);
  const [output, setOutput] = useState("");
  const [metadata, setMetadata] = useState<RunMetadata | null>(null);
  const [sourceTraceId, setSourceTraceId] = useState<string | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [health, setHealth] = useState<LLMHealth | null>(null);
  const [loadingModels, setLoadingModels] = useState(true);

  // Load available models and check health on mount
  useEffect(() => {
    loadModels();
    checkHealth();
  }, []);

  // Debug: log model state changes
  useEffect(() => {
    console.log('[Playground] Model state updated to:', model);
  }, [model]);

  // Load data from sessionStorage if coming from a trace or prompt
  useEffect(() => {
    const from = searchParams.get('from');
    if (from === 'trace' || from === 'prompt' || from === 'session') {
      const playgroundData = sessionStorage.getItem('playground_data');
      if (playgroundData) {
        try {
          const data = JSON.parse(playgroundData);
          console.log('[Playground] Loaded data from', from, ':', data);

          // Load full messages array if available (includes all roles: system, user, assistant)
          if (data.messages && data.messages.length > 0) {
            // Filter to only include system, user, and assistant messages
            // Skip tool responses and other message types
            const validMessages = data.messages.filter((m: any) =>
              ['system', 'user', 'assistant'].includes(m.role)
            );

            const loadedMessages: Message[] = validMessages.map((m: any, index: number) => ({
              id: String(index + 1),
              role: m.role as 'system' | 'user' | 'assistant',
              content: m.content || '',
            }));
            setMessages(loadedMessages);
          } else if (data.prompt) {
            // Fallback to single prompt format
            setMessages([
              { id: '1', role: 'system', content: data.prompt },
              { id: '2', role: 'user', content: '' }
            ]);
          }

          // DON'T set model from trace - the trace's model may not be configured
          // User should select from their configured models instead
          // if (data.model) setModel(data.model);

          if (data.temperature !== undefined) setTemperature(data.temperature);
          if (data.sourceTraceId) setSourceTraceId(data.sourceTraceId);
          // Don't clear the data - keep it so user can navigate back and forth
          // sessionStorage.removeItem('playground_data');
        } catch (e) {
          console.error('Failed to parse playground data:', e);
        }
      }
    }
  }, [searchParams]);

  const loadModels = async () => {
    try {
      setLoadingModels(true);

      // First, load models from user settings (localStorage)
      const savedSettings = localStorage.getItem('agentreplay_settings');
      const userModels: ModelInfo[] = [];

      if (savedSettings) {
        try {
          const settings = JSON.parse(savedSettings);
          const providers = settings?.models?.providers || [];

          for (const provider of providers) {
            if (provider.modelName && provider.baseUrl) {
              userModels.push({
                id: provider.modelName,
                name: `${provider.name} (${provider.modelName})`,
                provider: provider.provider,
                available: true,
              });
            }
          }
        } catch (e) {
          console.error('Failed to parse settings:', e);
        }
      }

      // Then fetch dynamic models from server (primarily Ollama)
      try {
        const response = await fetch(`${API_BASE_URL}/api/v1/llm/models`);
        if (response.ok) {
          const data = await response.json();
          const serverModels = (data.models || []) as ModelInfo[];

          // Merge: user-configured models first, then server models (avoid duplicates)
          const userModelIds = new Set(userModels.map(m => m.id));
          const uniqueServerModels = serverModels.filter(m => !userModelIds.has(m.id));

          setModels([...userModels, ...uniqueServerModels]);

          // Set default model: prefer user's default, then first configured, then first Ollama
          // ALWAYS set a default model (removed the from check - user should always have a working model)
          const defaultProviderId = JSON.parse(savedSettings || '{}')?.models?.defaultProviderId;
          const defaultProvider = JSON.parse(savedSettings || '{}')?.models?.providers?.find(
            (p: any) => p.id === defaultProviderId
          );

          if (defaultProvider?.modelName) {
            setModel(defaultProvider.modelName);
          } else if (userModels.length > 0) {
            setModel(userModels[0].id);
          } else {
            const ollamaModel = serverModels.find((m: ModelInfo) => m.provider === 'ollama');
            if (ollamaModel) {
              setModel(ollamaModel.id);
            }
          }
        }
      } catch (e) {
        console.error('Failed to fetch server models:', e);
        // Still use user-configured models even if server is down
        setModels(userModels);
        if (userModels.length > 0) {
          setModel(userModels[0].id);
        }
      }
    } catch (e) {
      console.error('Failed to load models:', e);
    } finally {
      setLoadingModels(false);
    }
  };

  const checkHealth = async () => {
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/llm/check`);
      if (response.ok) {
        const data = await response.json();
        setHealth(data);
      }
    } catch (e) {
      console.error('Failed to check LLM health:', e);
    }
  };

  // Message management functions
  const addMessage = (role: 'user' | 'assistant') => {
    setMessages([...messages, { id: Date.now().toString(), role, content: '' }]);
  };

  const updateMessage = (id: string, content: string) => {
    setMessages(messages.map(m => m.id === id ? { ...m, content } : m));
  };

  const deleteMessage = (id: string) => {
    // Don't allow deleting if only system + 1 user message
    if (messages.length <= 2) return;
    setMessages(messages.filter(m => m.id !== id));
  };

  const changeRole = (id: string, role: 'system' | 'user' | 'assistant') => {
    setMessages(messages.map(m => m.id === id ? { ...m, role } : m));
  };

  // Build prompt from messages for API
  const buildPromptFromMessages = (): string => {
    return messages
      .filter(m => m.content.trim())
      .map(m => {
        const roleLabel = m.role.charAt(0).toUpperCase() + m.role.slice(1);
        return `${roleLabel}: ${m.content}`;
      })
      .join('\n\n');
  };

  const runPrompt = async () => {
    const userMessages = messages.filter(m => m.role === 'user' && m.content.trim());
    if (userMessages.length === 0) {
      setError('Please enter at least one user message');
      return;
    }

    setIsRunning(true);
    setError(null);
    setOutput('');
    setMetadata(null);

    // Use the current model value from state
    const selectedModel = model;
    console.log('[Playground] Running with model:', selectedModel);

    if (!selectedModel) {
      setError('Please select a model first');
      setIsRunning(false);
      return;
    }

    try {
      // Find provider config for the selected model
      const savedSettings = localStorage.getItem('agentreplay_settings');
      let providerConfig: { baseUrl?: string; apiKey?: string; provider?: string } | null = null;

      if (savedSettings) {
        try {
          const settings = JSON.parse(savedSettings);
          const providers = settings?.models?.providers || [];
          // Try exact match first, then partial match (model might be embedded in name)
          providerConfig = providers.find((p: { modelName?: string }) => p.modelName === selectedModel)
            || providers.find((p: { modelName?: string }) => selectedModel.includes(p.modelName || '') || (p.modelName || '').includes(selectedModel));

          console.log('[Playground] Looking for model:', selectedModel);
          console.log('[Playground] Available providers:', providers.map((p: { name?: string; modelName?: string; provider?: string }) => ({ name: p.name, modelName: p.modelName, provider: p.provider })));
          console.log('[Playground] Found provider config:', providerConfig);
        } catch (e) {
          console.error('Failed to parse settings:', e);
        }
      }

      const prompt = buildPromptFromMessages();
      const requestBody: any = {
        prompt,
        model: selectedModel, // Use the captured model value
        temperature,
        max_tokens: maxTokens,
      };

      // Add provider config if available (for cloud APIs)
      if (providerConfig) {
        requestBody.base_url = providerConfig.baseUrl;
        requestBody.api_key = providerConfig.apiKey;
        requestBody.provider = providerConfig.provider;
      }

      const response = await fetch(`${API_BASE_URL}/api/v1/playground/run`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(requestBody),
      });

      if (!response.ok) {
        throw new Error(`HTTP error: ${response.status}`);
      }

      const data: PlaygroundRunResponse = await response.json();

      // Check if the output is an error message
      if (data.output.startsWith('LLM Error:')) {
        setError(data.output);
      } else {
        setOutput(data.output);
        setMetadata(data.metadata);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to run prompt');
    } finally {
      setIsRunning(false);
    }
  };

  // Group models by provider
  const groupedModels = models.reduce((acc, m) => {
    if (!acc[m.provider]) acc[m.provider] = [];
    acc[m.provider].push(m);
    return acc;
  }, {} as Record<string, ModelInfo[]>);

  return (
    <div className="h-full flex flex-col bg-background">
      {/* Header */}
      <div className="border-b border-border bg-surface px-6 py-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            {/* Back button */}
            {/* Back button - Smart navigation */}
            <button
              onClick={() => {
                const fromSession = searchParams.get('from') === 'session';
                if (fromSession) {
                  const sessionId = JSON.parse(sessionStorage.getItem('playground_data') || '{}').sourceSessionId;
                  navigate(`/projects/${projectId}/sessions?session_id=${sessionId}`);
                } else {
                  navigate(-1);
                }
              }}
              className="p-2 rounded-lg text-textSecondary hover:text-textPrimary hover:bg-surface-hover transition-colors"
              title={searchParams.get('from') === 'session' ? "Back to Session" : "Go back"}
            >
              <ArrowLeft className="w-5 h-5" />
            </button>
            <div>
              <h1 className="text-2xl font-bold text-textPrimary">Playground</h1>
              <p className="text-sm text-textSecondary mt-1">
                Test prompts with local and cloud LLM models
              </p>
            </div>
          </div>

          {/* Health Status */}
          {health && (
            <div className="flex gap-4 text-sm">
              <div className="flex items-center gap-1.5">
                {health.ollama.available ? (
                  <CheckCircle className="w-4 h-4 text-green-500" />
                ) : (
                  <AlertCircle className="w-4 h-4 text-yellow-500" />
                )}
                <span className={health.ollama.available ? 'text-green-500' : 'text-yellow-500'}>
                  Ollama
                </span>
              </div>
              <div className="flex items-center gap-1.5">
                {health.openai.configured ? (
                  <CheckCircle className="w-4 h-4 text-blue-500" />
                ) : (
                  <AlertCircle className="w-4 h-4 text-gray-400" />
                )}
                <span className={health.openai.configured ? 'text-blue-500' : 'text-gray-400'}>
                  OpenAI
                </span>
              </div>
              <div className="flex items-center gap-1.5">
                {health.anthropic.configured ? (
                  <CheckCircle className="w-4 h-4 text-purple-500" />
                ) : (
                  <AlertCircle className="w-4 h-4 text-gray-400" />
                )}
                <span className={health.anthropic.configured ? 'text-purple-500' : 'text-gray-400'}>
                  Anthropic
                </span>
              </div>
            </div>
          )}
        </div>

        {/* Source trace/session indicator with back navigation */}
        {(sourceTraceId || searchParams.get('from') === 'session') && (
          <div className="mt-3 px-4 py-2 bg-primary/10 border border-primary/20 rounded-lg text-sm flex items-center gap-2">
            <span className="text-primary">📋</span>
            <span className="text-textSecondary">
              {sourceTraceId ? 'Loaded from trace:' : 'Loaded from session:'}
            </span>
            {sourceTraceId && <code className="font-mono text-xs bg-surface px-2 py-0.5 rounded">{sourceTraceId.slice(0, 16)}...</code>}

            <div className="ml-auto flex items-center gap-2">
              {sourceTraceId && (
                <button
                  onClick={() => navigate(`/projects/${projectId}/traces/${sourceTraceId}`)}
                  className="flex items-center gap-1 px-2 py-1 text-primary hover:bg-primary/10 rounded text-xs font-medium transition-colors"
                >
                  <ArrowLeft className="w-3 h-3" />
                  Back to Trace
                </button>
              )}

              <button
                onClick={() => {
                  setSourceTraceId(null);
                  sessionStorage.removeItem('playground_data');
                }}
                className="text-textTertiary hover:text-textPrimary text-xs"
              >
                Dismiss
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Main Content */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left Panel - Messages */}
        <div className="w-1/2 flex flex-col border-r border-border">
          {/* Messages List */}
          <div className="flex-1 p-4 overflow-y-auto space-y-3">
            {messages.map((message, index) => (
              <div key={message.id} className="bg-surface border border-border rounded-lg overflow-hidden">
                {/* Message Header */}
                <div className="flex items-center gap-2 px-3 py-2 bg-surface-elevated border-b border-border">
                  <select
                    value={message.role}
                    onChange={(e) => changeRole(message.id, e.target.value as any)}
                    className="bg-transparent text-sm font-medium text-textPrimary focus:outline-none cursor-pointer"
                  >
                    <option value="system">System</option>
                    <option value="user">User</option>
                    <option value="assistant">Assistant</option>
                  </select>

                  <div className="flex-1 flex items-center gap-1">
                    {message.role === 'system' && <Settings className="w-4 h-4 text-purple-500" />}
                    {message.role === 'user' && <User className="w-4 h-4 text-blue-500" />}
                    {message.role === 'assistant' && <Bot className="w-4 h-4 text-green-500" />}
                  </div>

                  {index > 0 && (
                    <button
                      onClick={() => deleteMessage(message.id)}
                      className="p-1 text-textTertiary hover:text-error transition-colors"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  )}
                </div>

                {/* Message Content */}
                <textarea
                  className="w-full p-3 bg-transparent font-mono text-sm resize-none focus:outline-none text-textPrimary placeholder-textTertiary min-h-[80px]"
                  placeholder={
                    message.role === 'system'
                      ? 'Enter system instructions...'
                      : message.role === 'user'
                        ? 'Enter user message...'
                        : 'Enter assistant response...'
                  }
                  value={message.content}
                  onChange={(e) => updateMessage(message.id, e.target.value)}
                  rows={Math.max(3, message.content.split('\n').length)}
                />
              </div>
            ))}

            {/* Add Message Buttons */}
            <div className="flex gap-2">
              <button
                onClick={() => addMessage('user')}
                className="flex-1 flex items-center justify-center gap-2 px-3 py-2 bg-blue-500/10 border border-blue-500/20 rounded-lg text-blue-500 hover:bg-blue-500/20 transition-colors text-sm"
              >
                <Plus className="w-4 h-4" />
                Add User Message
              </button>
              <button
                onClick={() => addMessage('assistant')}
                className="flex-1 flex items-center justify-center gap-2 px-3 py-2 bg-green-500/10 border border-green-500/20 rounded-lg text-green-500 hover:bg-green-500/20 transition-colors text-sm"
              >
                <Plus className="w-4 h-4" />
                Add Assistant Message
              </button>
            </div>
          </div>

          {/* Controls */}
          <div className="p-4 border-t border-border bg-surface-elevated space-y-4">
            {/* Model Selection */}
            <div className="flex gap-4">
              <div className="flex-1">
                <label className="block text-xs font-medium text-textSecondary mb-1.5">Model</label>
                <select
                  className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                  value={model}
                  onChange={(e) => {
                    console.log('[Playground] Model changed to:', e.target.value);
                    setModel(e.target.value);
                  }}
                  disabled={loadingModels}
                >
                  {loadingModels ? (
                    <option>Loading models...</option>
                  ) : models.length === 0 ? (
                    <option value="">No models configured - go to Settings</option>
                  ) : (
                    <>
                      {!model && <option value="">Select a model...</option>}
                      {Object.entries(groupedModels).map(([provider, providerModels]) => (
                        <optgroup key={provider} label={provider.charAt(0).toUpperCase() + provider.slice(1)}>
                          {providerModels.map((m) => (
                            <option key={m.id} value={m.id}>
                              {m.name}
                            </option>
                          ))}
                        </optgroup>
                      ))}
                    </>
                  )}
                </select>
              </div>

              <div className="w-32">
                <label className="block text-xs font-medium text-textSecondary mb-1.5">
                  Temperature: {temperature.toFixed(1)}
                </label>
                <input
                  type="range"
                  min="0"
                  max="2"
                  step="0.1"
                  className="w-full accent-primary"
                  value={temperature}
                  onChange={(e) => setTemperature(parseFloat(e.target.value))}
                />
              </div>

              <div className="w-32">
                <label className="block text-xs font-medium text-textSecondary mb-1.5">
                  Max Tokens
                </label>
                <input
                  type="number"
                  min="1"
                  max="8192"
                  className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                  value={maxTokens}
                  onChange={(e) => setMaxTokens(parseInt(e.target.value) || 2048)}
                />
              </div>
            </div>

            {/* Run Button */}
            <button
              onClick={runPrompt}
              disabled={isRunning || !messages.some(m => m.role === 'user' && m.content.trim())}
              className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-primary text-background rounded-lg font-semibold hover:bg-primary-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {isRunning ? (
                <>
                  <Loader2 className="w-5 h-5 animate-spin" />
                  Running...
                </>
              ) : (
                <>
                  <Play className="w-5 h-5" />
                  Run (⌘+Enter)
                </>
              )}
            </button>
          </div>
        </div>

        {/* Right Panel - Output */}
        <div className="w-1/2 flex flex-col">
          {/* Output Content */}
          <div className="flex-1 p-4 overflow-auto">
            {error ? (
              <div className="p-4 bg-error/10 border border-error/20 rounded-lg">
                <div className="flex items-start gap-3">
                  <AlertCircle className="w-5 h-5 text-error flex-shrink-0 mt-0.5" />
                  <div>
                    <h3 className="font-semibold text-error">Error</h3>
                    <p className="text-sm text-textSecondary mt-1">{error}</p>
                    {error.includes('Ollama') && (
                      <p className="text-xs text-textTertiary mt-2">
                        Tip: Start Ollama with <code className="bg-surface px-1 rounded">ollama serve</code>
                      </p>
                    )}
                  </div>
                </div>
              </div>
            ) : (output || (metadata?.tokens_used?.completion_tokens ?? 0) > 0) ? (
              <div className="h-full">
                <div className="text-xs font-medium text-textTertiary uppercase tracking-wide mb-2">Output</div>
                <div className="p-4 bg-surface border border-border rounded-lg font-mono text-sm whitespace-pre-wrap text-textPrimary">
                  {output || <span className="text-textTertiary italic">&lt;No text output generated (Length: {metadata?.tokens_used?.completion_tokens} tokens)&gt;</span>}
                </div>
              </div>
            ) : (
              <div className="h-full flex items-center justify-center text-textTertiary">
                <div className="text-center">
                  <Zap className="w-12 h-12 mx-auto mb-3 opacity-50" />
                  <p>Run a prompt to see output</p>
                </div>
              </div>
            )}
          </div>

          {/* Metadata Footer */}
          {metadata && (
            <div className="p-4 border-t border-border bg-surface-elevated">
              <div className="flex gap-6 text-sm">
                <div>
                  <span className="text-textTertiary">Model:</span>
                  <span className="ml-2 text-textPrimary font-medium">{metadata.model_used}</span>
                </div>
                <div>
                  <span className="text-textTertiary">Latency:</span>
                  <span className="ml-2 text-textPrimary font-medium">{metadata.latency_ms}ms</span>
                </div>
                <div>
                  <span className="text-textTertiary">Tokens:</span>
                  <span className="ml-2 text-textPrimary font-medium">
                    {metadata.tokens_used.prompt_tokens} → {metadata.tokens_used.completion_tokens}
                    <span className="text-textSecondary"> ({metadata.tokens_used.total_tokens} total)</span>
                  </span>
                </div>
                {metadata.cost_usd > 0 && (
                  <div>
                    <span className="text-textTertiary">Cost:</span>
                    <span className="ml-2 text-textPrimary font-medium">${metadata.cost_usd.toFixed(6)}</span>
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
