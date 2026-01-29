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

import { ChangeEvent, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { Badge } from '../../components/ui/badge';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
import { PromptRecord, PromptVersion, loadPrompts, upsertPrompt } from '../lib/prompt-store';
import { AlertCircle, ArrowLeft, Copy, Loader2, Rocket, Save, Sparkles, Settings } from 'lucide-react';
import { cn } from '../../lib/utils';
import { API_BASE_URL } from '../lib/agentreplay-api';

interface ModelInfo {
  id: string;
  name: string;
  provider: string;
}

interface VariableDraft {
  key: string;
  description?: string;
  required?: boolean;
}

interface ToolDraft {
  name: string;
  description?: string;
}

// Helper to parse content - handles JSON array format like [{"type": "text", "text": "..."}]
const parseContent = (content: any): string => {
  if (!content) return '';
  if (typeof content !== 'string') return String(content);
  
  // Try to parse as JSON array
  try {
    if (content.startsWith('[')) {
      const parsed = JSON.parse(content);
      if (Array.isArray(parsed)) {
        return parsed
          .filter((item: any) => item.type === 'text' && item.text)
          .map((item: any) => item.text)
          .join('\n');
      }
    }
  } catch (e) {
    // Not valid JSON, return as-is
  }
  return content;
};

// Helper to format content for display - converts [System]/[User] blocks to clean Jinja template
const formatTemplateContent = (content: string): string => {
  // If already parsed, just clean up any remaining JSON artifacts
  let cleaned = parseContent(content);
  
  // Parse [System] and [User] blocks and convert to Jinja-friendly format
  const systemMatch = cleaned.match(/\[System\]\n([\s\S]*?)(?=\n\n\[User\]|\n\[User\]|$)/);
  const userMatch = cleaned.match(/\[User\]\n([\s\S]*?)$/);
  
  if (systemMatch || userMatch) {
    const parts: string[] = [];
    if (systemMatch && systemMatch[1]) {
      parts.push(`{# SYSTEM PROMPT #}\n${parseContent(systemMatch[1].trim())}`);
    }
    if (userMatch && userMatch[1]) {
      parts.push(`{# USER MESSAGE #}\n${parseContent(userMatch[1].trim())}`);
    }
    return parts.join('\n\n');
  }
  
  return cleaned;
};

export default function PromptDetail() {
  const { projectId, promptId } = useParams<{ projectId: string; promptId: string }>();
  const navigate = useNavigate();
  const [prompt, setPrompt] = useState<PromptRecord | null>(null);
  const [testModel, setTestModel] = useState('gpt-4o-mini');
  const [temperature, setTemperature] = useState(0.2);
  const [variableDrafts, setVariableDrafts] = useState<VariableDraft[]>([]);
  const [toolDrafts, setToolDrafts] = useState<ToolDraft[]>([]);
  const [testBindings, setTestBindings] = useState<Record<string, string>>({});
  const [testOutput, setTestOutput] = useState<string>('');
  const [testing, setTesting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    const records = loadPrompts();
    const found = records.find((record) => record.id === promptId) || null;
    if (found) {
      // Format the content for display
      const formattedContent = formatTemplateContent(found.content);
      setPrompt({ ...found, content: formattedContent });
      
      // Load tools from metadata if available
      const meta = (found as any).metadata;
      if (meta?.tools && Array.isArray(meta.tools)) {
        setToolDrafts(meta.tools.map((t: any) => ({ name: t.name, description: t.arguments || '' })));
      }
    } else {
      setPrompt(null);
    }
    setVariableDrafts(found?.variables ?? []);
    setTestBindings(
      Object.fromEntries((found?.variables ?? []).map((variable) => [variable.key, '']))
    );
  }, [promptId]);

  const versions = useMemo(() => prompt?.history ?? [], [prompt]);

  const handlePromptChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
    if (!prompt) return;
    const value = event.target.value;
    setPrompt({ ...prompt, content: value, lastEdited: Date.now() });
  };

  const handleVariableChange = (index: number, updates: Partial<VariableDraft>) => {
    setVariableDrafts((prev) => prev.map((variable, idx) => (idx === index ? { ...variable, ...updates } : variable)));
  };

  const addVariable = () => {
    setVariableDrafts((prev) => [...prev, { key: `var_${prev.length + 1}`, description: '' }]);
  };

  const removeVariable = (index: number) => {
    setVariableDrafts((prev) => prev.filter((_, idx) => idx !== index));
  };

  const handleSave = () => {
    if (!prompt) return;
    const updated: PromptRecord = {
      ...prompt,
      content: prompt.content,
      variables: variableDrafts,
      lastEdited: Date.now(),
      history: [
        {
          id: `${prompt.id}-v${prompt.activeVersion}`,
          author: prompt.owner || 'unknown',
          createdAt: Date.now(),
          version: prompt.activeVersion,
          content: prompt.content,
        },
        ...prompt.history,
      ].slice(0, 25),
    };
    setPrompt(updated);
    upsertPrompt(updated);
    setMessage('Draft saved');
    setTimeout(() => setMessage(null), 2000);
  };

  const handleDeploy = () => {
    if (!prompt) return;
    const nextVersion = prompt.activeVersion + 1;
    const updated: PromptRecord = {
      ...prompt,
      deployedVersion: nextVersion,
      activeVersion: nextVersion,
      lastEdited: Date.now(),
    };
    setPrompt(updated);
    upsertPrompt(updated);
    setMessage('Prompt deployed');
    setTimeout(() => setMessage(null), 2000);
  };

  const runTest = async () => {
    if (!prompt) return;
    setTesting(true);
    setMessage(null);
    setTestOutput('Running...');
    
    try {
      // Get the provider config for the selected model
      const savedSettings = localStorage.getItem('agentreplay_settings');
      let provider: { baseUrl?: string; apiKey?: string; modelName?: string } | null = null;
      
      if (savedSettings) {
        const settings = JSON.parse(savedSettings);
        const providers = settings?.models?.providers || [];
        provider = providers.find((p: any) => p.modelName === testModel);
      }
      
      if (!provider) {
        setTestOutput(`Error: No provider configured for model "${testModel}". Configure it in Settings.`);
        setTesting(false);
        return;
      }
      
      // Apply variable bindings to content
      let processedContent = prompt.content;
      Object.entries(testBindings).forEach(([key, value]) => {
        const regex = new RegExp(`\\{\\{\\s*${key}\\s*\\}\\}`, 'g');
        processedContent = processedContent.replace(regex, value);
      });
      
      // Parse content into messages - look for Jinja-style comments
      const messages: Array<{ role: string; content: string }> = [];
      const systemMatch = processedContent.match(/\{#\s*SYSTEM PROMPT\s*#\}\n([\s\S]*?)(?=\{#|$)/);
      const userMatch = processedContent.match(/\{#\s*USER MESSAGE\s*#\}\n([\s\S]*?)$/);
      
      if (systemMatch) {
        messages.push({ role: 'system', content: systemMatch[1].trim() });
      }
      if (userMatch) {
        messages.push({ role: 'user', content: userMatch[1].trim() });
      }
      
      // Fallback: treat whole content as system prompt
      if (messages.length === 0) {
        messages.push({ role: 'system', content: processedContent });
        messages.push({ role: 'user', content: 'Test this prompt.' });
      }
      
      // Make OpenAI-compatible API call
      const baseUrl = (provider.baseUrl || '').replace(/\/$/, '');
      const response = await fetch(`${baseUrl}/chat/completions`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...(provider.apiKey ? { 'Authorization': `Bearer ${provider.apiKey}` } : {}),
        },
        body: JSON.stringify({
          model: testModel,
          messages: messages,
          temperature: temperature,
          max_tokens: 1024,
        }),
      });
      
      if (!response.ok) {
        const errorText = await response.text();
        setTestOutput(`Error (${response.status}): ${errorText}`);
      } else {
        const data = await response.json();
        const content = data.choices?.[0]?.message?.content || 'No response content';
        setTestOutput(content);
      }
    } catch (e: any) {
      setTestOutput(`Error: ${e.message || 'Failed to run test'}`);
    } finally {
      setTesting(false);
    }
  };

  if (!prompt) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-textSecondary">
        <AlertCircle className="h-10 w-10 text-red-400" />
        Prompt not found.
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-6">
      <header className="flex items-center justify-between">
        <div>
          <button className="mb-1 flex items-center gap-2 text-xs uppercase tracking-widest text-textTertiary" onClick={() => projectId && navigate(`/projects/${projectId}/prompts`)}>
            <ArrowLeft className="h-3 w-3" /> Prompts
          </button>
          <h1 className="text-2xl font-semibold text-textPrimary">{prompt.name}</h1>
          <p className="text-sm text-textSecondary">v{prompt.activeVersion} · {prompt.tags.join(', ') || 'No tags'}</p>
        </div>
        <div className="flex gap-2">
          <Button variant="ghost" size="sm" onClick={handleSave} className="gap-2">
            <Save className="h-4 w-4" /> Save draft
          </Button>
          <Button variant="default" size="sm" onClick={handleDeploy} className="gap-2">
            <Rocket className="h-4 w-4" /> Deploy
          </Button>
        </div>
      </header>

      {message && <p className="rounded-2xl border border-emerald-500/40 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-200">{message}</p>}

      <section className="grid gap-4 lg:grid-cols-2">
        <div className="rounded-3xl border border-border/60 bg-background/90 p-4">
          <div className="flex items-center justify-between border-b border-border/60 pb-3">
            <div>
              <p className="text-xs uppercase tracking-widest text-textTertiary">Template</p>
              <p className="text-sm text-textSecondary">Jinja-style with variables.</p>
            </div>
            <Button variant="ghost" size="sm" className="gap-2" onClick={() => navigator.clipboard.writeText(prompt.content)}>
              <Copy className="h-4 w-4" /> Copy
            </Button>
          </div>
          <textarea
            value={prompt.content}
            onChange={handlePromptChange}
            className="mt-4 h-[420px] w-full rounded-2xl border border-border/60 bg-surface/70 p-4 font-mono text-sm text-textPrimary"
          />
        </div>
        <div className="rounded-3xl border border-border/60 bg-background/90 p-4">
          <div className="space-y-6">
            <VariableEditor variables={variableDrafts} onAdd={addVariable} onRemove={removeVariable} onChange={handleVariableChange} setBindings={setTestBindings} bindings={testBindings} />
            <ToolEditor tools={toolDrafts} onAdd={() => setToolDrafts(prev => [...prev, { name: `tool_${prev.length + 1}` }])} onRemove={(idx) => setToolDrafts(prev => prev.filter((_, i) => i !== idx))} onChange={(idx, updates) => setToolDrafts(prev => prev.map((t, i) => i === idx ? { ...t, ...updates } : t))} />
            <LiveTester
              testModel={testModel}
              setTestModel={setTestModel}
              temperature={temperature}
              setTemperature={setTemperature}
              runTest={runTest}
              testing={testing}
              testOutput={testOutput}
            />
          </div>
        </div>
      </section>

      <section className="rounded-3xl border border-border/60 bg-background/80 p-4">
        <div className="flex items-center justify-between border-b border-border/50 pb-3">
          <div>
            <p className="text-xs uppercase tracking-widest text-textTertiary">History</p>
            <p className="text-sm text-textSecondary">Latest 10 deployments.</p>
          </div>
        </div>
        <div className="mt-4 divide-y divide-border/40">
          {versions.length === 0 && <p className="py-6 text-sm text-textSecondary">No history captured yet.</p>}
          {versions.map((version: PromptVersion) => (
            <div key={version.id} className="flex items-center justify-between py-3 text-sm text-textSecondary">
              <div>
                <p className="font-semibold text-textPrimary">v{version.version}</p>
                <p className="text-xs text-textTertiary">{new Date(version.createdAt).toLocaleString()}</p>
              </div>
              <Badge className="bg-surface/70 text-textSecondary">{version.author}</Badge>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

function VariableEditor({ variables, onAdd, onRemove, onChange, bindings, setBindings }: {
  variables: VariableDraft[];
  onAdd: () => void;
  onRemove: (index: number) => void;
  onChange: (index: number, updates: Partial<VariableDraft>) => void;
  bindings: Record<string, string>;
  setBindings: (bindings: Record<string, string>) => void;
}) {
  return (
    <div>
      <div className="flex items-center justify-between">
        <div>
          <p className="text-xs uppercase tracking-widest text-textTertiary">Variables</p>
          <p className="text-sm text-textSecondary">Expose inputs to your team.</p>
        </div>
        <Button variant="ghost" size="sm" onClick={onAdd}>
          + Variable
        </Button>
      </div>
      <div className="mt-3 space-y-3">
        {variables.map((variable, index) => (
          <div key={`${variable.key}-${index}`} className="rounded-2xl border border-border/50 bg-surface/70 p-3">
            <div className="flex items-center gap-2">
              <Input value={variable.key} onChange={(event) => onChange(index, { key: event.target.value })} className="flex-1" />
              <label className="inline-flex items-center gap-2 text-xs text-textSecondary">
                <input type="checkbox" checked={Boolean(variable.required)} onChange={(event) => onChange(index, { required: event.target.checked })} />
                required
              </label>
              <Button variant="ghost" size="sm" onClick={() => onRemove(index)}>
                Remove
              </Button>
            </div>
            <Input
              className="mt-2"
              placeholder="Description"
              value={variable.description}
              onChange={(event) => onChange(index, { description: event.target.value })}
            />
            <Input
              className="mt-2 font-mono"
              placeholder="Test value"
              value={bindings[variable.key] || ''}
              onChange={(event) =>
                setBindings({
                  ...bindings,
                  [variable.key]: event.target.value,
                })
              }
            />
          </div>
        ))}
        {variables.length === 0 && <p className="text-sm text-textSecondary">No variables defined.</p>}
      </div>
    </div>
  );
}

function ToolEditor({ tools, onAdd, onRemove, onChange }: {
  tools: ToolDraft[];
  onAdd: () => void;
  onRemove: (index: number) => void;
  onChange: (index: number, updates: Partial<ToolDraft>) => void;
}) {
  return (
    <div>
      <div className="flex items-center justify-between">
        <div>
          <p className="text-xs uppercase tracking-widest text-textTertiary">Tools</p>
          <p className="text-sm text-textSecondary">Available function calls.</p>
        </div>
        <Button variant="ghost" size="sm" onClick={onAdd}>
          + Tool
        </Button>
      </div>
      <div className="mt-3 space-y-3">
        {tools.map((tool, index) => (
          <div key={`${tool.name}-${index}`} className="rounded-2xl border border-purple-500/30 bg-purple-500/5 p-3">
            <div className="flex items-center gap-2">
              <span className="text-purple-400">🔧</span>
              <Input 
                value={tool.name} 
                onChange={(event) => onChange(index, { name: event.target.value })} 
                className="flex-1 font-mono text-purple-300" 
                placeholder="function_name"
              />
              <Button variant="ghost" size="sm" onClick={() => onRemove(index)}>
                Remove
              </Button>
            </div>
            <Input
              className="mt-2"
              placeholder="Description / Arguments schema"
              value={tool.description || ''}
              onChange={(event) => onChange(index, { description: event.target.value })}
            />
          </div>
        ))}
        {tools.length === 0 && <p className="text-sm text-textSecondary">No tools defined. Tools are extracted from traces with function calls.</p>}
      </div>
    </div>
  );
}

function LiveTester({
  testModel,
  setTestModel,
  temperature,
  setTemperature,
  runTest,
  testing,
  testOutput,
}: {
  testModel: string;
  setTestModel: (value: string) => void;
  temperature: number;
  setTemperature: (value: number) => void;
  runTest: () => Promise<void> | void;
  testing: boolean;
  testOutput: string;
}) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loadingModels, setLoadingModels] = useState(true);
  const navigate = useNavigate();

  useEffect(() => {
    const fetchModels = async () => {
      setLoadingModels(true);
      try {
        // First load user-configured models from localStorage (note: underscore, not hyphen)
        const savedSettings = localStorage.getItem('agentreplay_settings');
        const userModels: ModelInfo[] = [];
        
        if (savedSettings) {
          const settings = JSON.parse(savedSettings);
          const providers = settings?.models?.providers || [];
          
          console.log('[PromptDetail] Loaded providers from settings:', providers);
          
          for (const provider of providers) {
            // Match Playground logic: check modelName && baseUrl
            if (provider.modelName && provider.baseUrl) {
              userModels.push({
                id: provider.modelName,
                name: `${provider.name || provider.provider} (${provider.modelName})`,
                provider: provider.provider || 'unknown',
              });
            }
          }
          
          console.log('[PromptDetail] Configured models:', userModels);
        }

        // Then fetch dynamic models from server
        try {
          const response = await fetch(`${API_BASE_URL}/api/v1/llm/models`);
          if (response.ok) {
            const data = await response.json();
            const serverModels = (data.models || []) as ModelInfo[];
            
            // Merge: user-configured models first, then server models (avoid duplicates)
            const userModelIds = new Set(userModels.map(m => m.id));
            const uniqueServerModels = serverModels.filter(m => !userModelIds.has(m.id));
            
            const allModels = [...userModels, ...uniqueServerModels];
            setModels(allModels);
            
            // Set default model - prefer user's default provider
            if (allModels.length > 0) {
              const defaultProviderId = JSON.parse(savedSettings || '{}')?.models?.defaultProviderId;
              const defaultProvider = JSON.parse(savedSettings || '{}')?.models?.providers?.find(
                (p: any) => p.id === defaultProviderId
              );
              
              if (defaultProvider?.modelName) {
                setTestModel(defaultProvider.modelName);
              } else if (userModels.length > 0) {
                setTestModel(userModels[0].id);
              } else if (allModels.length > 0) {
                setTestModel(allModels[0].id);
              }
            }
          } else {
            setModels(userModels);
            if (userModels.length > 0) {
              setTestModel(userModels[0].id);
            }
          }
        } catch (e) {
          console.error('Failed to fetch server models:', e);
          setModels(userModels);
          if (userModels.length > 0) {
            setTestModel(userModels[0].id);
          }
        }
      } catch (e) {
        console.error('Failed to load models:', e);
      } finally {
        setLoadingModels(false);
      }
    };

    fetchModels();
  }, []);

  // Group models by provider
  const groupedModels = useMemo(() => {
    return models.reduce((acc, model) => {
      if (!acc[model.provider]) {
        acc[model.provider] = [];
      }
      acc[model.provider].push(model);
      return acc;
    }, {} as Record<string, ModelInfo[]>);
  }, [models]);

  return (
    <div className="rounded-2xl border border-border/50 bg-surface/70 p-4">
      <div className="flex items-center justify-between">
        <div>
          <p className="text-xs uppercase tracking-widest text-textTertiary">Live test</p>
          <p className="text-sm text-textSecondary">Fire a single completion.</p>
        </div>
        <Button variant="ghost" size="sm" className="gap-2" onClick={runTest} disabled={testing}>
          {testing ? <Loader2 className="h-4 w-4 animate-spin" /> : <Sparkles className="h-4 w-4" />} Test prompt
        </Button>
      </div>
      <div className="mt-3 space-y-3 text-sm">
        <label className="flex flex-col text-textSecondary">
          Model
          <select 
            className="mt-1 rounded-xl border border-border/50 bg-background px-3 py-2" 
            value={testModel} 
            onChange={(event) => setTestModel(event.target.value)}
            disabled={loadingModels}
          >
            {loadingModels ? (
              <option>Loading models...</option>
            ) : models.length === 0 ? (
              <option value="">No models configured</option>
            ) : (
              <>
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
          {models.length === 0 && !loadingModels && (
            <button
              onClick={() => navigate('/settings')}
              className="mt-2 flex items-center gap-1 text-xs text-primary hover:text-primary-hover"
            >
              <Settings className="h-3 w-3" /> Configure models in Settings
            </button>
          )}
        </label>
        <label className="flex flex-col text-textSecondary">
          Temperature {temperature.toFixed(1)}
          <input type="range" min={0} max={1} step={0.1} value={temperature} onChange={(event) => setTemperature(Number(event.target.value))} />
        </label>
        <textarea readOnly value={testOutput || 'Run a test to preview the response.'} className="h-40 w-full rounded-2xl border border-border/60 bg-background p-3 font-mono text-xs text-textPrimary" />
      </div>
    </div>
  );
}
