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
import { agentreplayClient, PromptResponse, PromptListResponse, CreatePromptRequest, PromptDiffResponse } from '../lib/agentreplay-api';

// ============================================================================
// Prompt Card Component
// ============================================================================

interface PromptCardProps {
  prompt: PromptResponse;
  onView: (id: string) => void;
  onDelete: (id: string) => void;
}

const PromptCard: React.FC<PromptCardProps> = ({ prompt, onView, onDelete }) => {
  const [showConfirmDelete, setShowConfirmDelete] = useState(false);

  return (
    <div className="border border-gray-200 dark:border-gray-700 rounded-lg p-4 bg-white dark:bg-gray-800 hover:shadow-md transition-shadow">
      <div className="flex items-start justify-between mb-2">
        <div>
          <h3 className="font-semibold text-lg">{prompt.name}</h3>
          <p className="text-sm text-gray-600 dark:text-gray-400">{prompt.description || 'No description'}</p>
        </div>
        <span className="px-2 py-1 bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200 rounded-full text-xs">
          v{prompt.version}
        </span>
      </div>

      <div className="mb-3">
        <div className="text-sm text-gray-500">Variables:</div>
        <div className="flex flex-wrap gap-1 mt-1">
          {prompt.variables.length > 0 ? (
            prompt.variables.map((v) => (
              <span key={v} className="px-2 py-0.5 bg-gray-100 dark:bg-gray-700 rounded text-xs font-mono">
                {`{{${v}}}`}
              </span>
            ))
          ) : (
            <span className="text-xs text-gray-400">No variables</span>
          )}
        </div>
      </div>

      {prompt.tags.length > 0 && (
        <div className="flex flex-wrap gap-1 mb-3">
          {prompt.tags.map((tag) => (
            <span key={tag} className="px-2 py-0.5 bg-purple-100 dark:bg-purple-900 text-purple-800 dark:text-purple-200 rounded-full text-xs">
              {tag}
            </span>
          ))}
        </div>
      )}

      <div className="text-xs text-gray-500 mb-3">
        Created by {prompt.created_by} on {new Date(prompt.created_at / 1000).toLocaleDateString()}
      </div>

      <div className="flex gap-2">
        <button
          onClick={() => onView(prompt.id)}
          className="flex-1 px-3 py-2 bg-blue-600 text-white rounded text-sm hover:bg-blue-700"
        >
          View & Edit
        </button>
        {showConfirmDelete ? (
          <div className="flex gap-1">
            <button
              onClick={() => onDelete(prompt.id)}
              className="px-3 py-2 bg-red-600 text-white rounded text-sm hover:bg-red-700"
            >
              Confirm
            </button>
            <button
              onClick={() => setShowConfirmDelete(false)}
              className="px-3 py-2 border rounded text-sm hover:bg-gray-100 dark:hover:bg-gray-700"
            >
              Cancel
            </button>
          </div>
        ) : (
          <button
            onClick={() => setShowConfirmDelete(true)}
            className="px-3 py-2 border border-red-300 text-red-600 rounded text-sm hover:bg-red-50"
          >
            Delete
          </button>
        )}
      </div>
    </div>
  );
};

// ============================================================================
// Create/Edit Prompt Modal
// ============================================================================

interface PromptEditorModalProps {
  prompt?: PromptResponse;
  isOpen: boolean;
  onClose: () => void;
  onSaved: (prompt: PromptResponse) => void;
}

const PromptEditorModal: React.FC<PromptEditorModalProps> = ({ prompt, isOpen, onClose, onSaved }) => {
  const [name, setName] = useState(prompt?.name || '');
  const [description, setDescription] = useState(prompt?.description || '');
  const [template, setTemplate] = useState(prompt?.template || '');
  const [tags, setTags] = useState(prompt?.tags.join(', ') || '');
  const [testVariables, setTestVariables] = useState<Record<string, string>>({});
  const [renderedTemplate, setRenderedTemplate] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Extract variables from template
  const extractedVariables = React.useMemo(() => {
    const matches = template.match(/\{\{(\w+)\}\}/g) || [];
    return [...new Set(matches.map(m => m.replace(/\{\{|\}\}/g, '')))];
  }, [template]);

  useEffect(() => {
    if (prompt) {
      setName(prompt.name);
      setDescription(prompt.description);
      setTemplate(prompt.template);
      setTags(prompt.tags.join(', '));
    }
  }, [prompt]);

  const handleTestRender = async () => {
    if (!prompt) return;
    try {
      const response = await agentreplayClient.renderPrompt(prompt.id, testVariables);
      setRenderedTemplate(response.rendered);
    } catch (err: any) {
      setError(err.message || 'Failed to render template');
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);

    try {
      const tagList = tags.split(',').map(t => t.trim()).filter(Boolean);
      
      if (prompt) {
        // Update existing prompt
        const updated = await agentreplayClient.updatePrompt(prompt.id, {
          name: name !== prompt.name ? name : undefined,
          description: description !== prompt.description ? description : undefined,
          template: template !== prompt.template ? template : undefined,
          tags: JSON.stringify(tagList) !== JSON.stringify(prompt.tags) ? tagList : undefined,
        });
        onSaved(updated);
      } else {
        // Create new prompt
        const created = await agentreplayClient.createPrompt({
          name,
          description,
          template,
          tags: tagList,
        });
        onSaved(created);
      }
      onClose();
    } catch (err: any) {
      setError(err.message || 'Failed to save prompt');
    } finally {
      setLoading(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-white dark:bg-gray-800 rounded-lg p-6 w-full max-w-4xl max-h-[90vh] overflow-y-auto">
        <h2 className="text-xl font-bold mb-4">{prompt ? 'Edit Prompt' : 'Create New Prompt'}</h2>
        
        <form onSubmit={handleSubmit}>
          <div className="grid grid-cols-2 gap-4 mb-4">
            <div>
              <label className="block text-sm font-medium mb-1">Name</label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="w-full p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
                placeholder="customer_support_v1"
                required
              />
            </div>
            <div>
              <label className="block text-sm font-medium mb-1">Tags (comma separated)</label>
              <input
                type="text"
                value={tags}
                onChange={(e) => setTags(e.target.value)}
                className="w-full p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
                placeholder="support, production"
              />
            </div>
          </div>

          <div className="mb-4">
            <label className="block text-sm font-medium mb-1">Description</label>
            <input
              type="text"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className="w-full p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
              placeholder="What is this prompt for?"
            />
          </div>

          <div className="mb-4">
            <label className="block text-sm font-medium mb-1">Template</label>
            <textarea
              value={template}
              onChange={(e) => setTemplate(e.target.value)}
              className="w-full p-2 border rounded font-mono text-sm dark:bg-gray-700 dark:border-gray-600"
              rows={8}
              placeholder="You are a helpful assistant for {{company}}. Your role is to {{role}}."
              required
            />
            {extractedVariables.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-1">
                <span className="text-sm text-gray-500">Variables:</span>
                {extractedVariables.map((v) => (
                  <span key={v} className="px-2 py-0.5 bg-blue-100 dark:bg-blue-900 rounded text-xs font-mono">
                    {`{{${v}}}`}
                  </span>
                ))}
              </div>
            )}
          </div>

          {/* Test Panel (only for existing prompts) */}
          {prompt && extractedVariables.length > 0 && (
            <div className="mb-4 p-4 bg-gray-50 dark:bg-gray-700 rounded">
              <h3 className="font-medium mb-2">Test Rendering</h3>
              <div className="grid grid-cols-2 gap-2 mb-2">
                {extractedVariables.map((v) => (
                  <div key={v}>
                    <label className="block text-sm text-gray-600 dark:text-gray-400">{v}</label>
                    <input
                      type="text"
                      value={testVariables[v] || ''}
                      onChange={(e) => setTestVariables({ ...testVariables, [v]: e.target.value })}
                      className="w-full p-1 border rounded text-sm dark:bg-gray-600 dark:border-gray-500"
                      placeholder={`Enter ${v}...`}
                    />
                  </div>
                ))}
              </div>
              <button
                type="button"
                onClick={handleTestRender}
                className="px-3 py-1 bg-gray-200 dark:bg-gray-600 rounded text-sm hover:bg-gray-300"
              >
                Render Preview
              </button>
              {renderedTemplate && (
                <pre className="mt-2 p-2 bg-white dark:bg-gray-800 rounded text-sm whitespace-pre-wrap border">
                  {renderedTemplate}
                </pre>
              )}
            </div>
          )}

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
              disabled={loading || !name || !template}
              className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
            >
              {loading ? 'Saving...' : prompt ? 'Update Prompt' : 'Create Prompt'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

// ============================================================================
// Version History Component
// ============================================================================

interface VersionHistoryProps {
  promptId: string;
  currentVersion: number;
}

const VersionHistory: React.FC<VersionHistoryProps> = ({ promptId, currentVersion }) => {
  const [versions, setVersions] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [diff, setDiff] = useState<PromptDiffResponse | null>(null);
  const [comparing, setComparing] = useState<{ v1: number; v2: number } | null>(null);

  useEffect(() => {
    const fetchVersions = async () => {
      setLoading(true);
      try {
        const response = await agentreplayClient.getPromptVersionHistory(promptId);
        setVersions(response.versions);
      } catch (err) {
        console.error('Failed to fetch versions:', err);
      } finally {
        setLoading(false);
      }
    };
    fetchVersions();
  }, [promptId]);

  const handleCompare = async (v1: number, v2: number) => {
    setComparing({ v1, v2 });
    try {
      const response = await agentreplayClient.getPromptDiff(promptId, v1, v2);
      setDiff(response);
    } catch (err) {
      console.error('Failed to fetch diff:', err);
    } finally {
      setComparing(null);
    }
  };

  if (loading) {
    return <div className="text-center py-4">Loading versions...</div>;
  }

  return (
    <div className="mt-6">
      <h3 className="font-semibold mb-3">Version History</h3>
      {versions.length === 0 ? (
        <p className="text-sm text-gray-500">No version history available</p>
      ) : (
        <div className="space-y-2">
          {versions.map((v) => (
            <div key={v.version} className="flex items-center justify-between p-3 border rounded">
              <div>
                <span className="font-medium">v{v.version}</span>
                <span className="text-sm text-gray-500 ml-2">
                  {new Date(v.created_at / 1000).toLocaleString()}
                </span>
                {v.change_summary && (
                  <span className="text-sm text-gray-600 ml-2">- {v.change_summary}</span>
                )}
              </div>
              {v.version !== currentVersion && (
                <button
                  onClick={() => handleCompare(v.version, currentVersion)}
                  className="text-sm text-blue-600 hover:text-blue-800"
                  disabled={!!comparing}
                >
                  {comparing?.v1 === v.version ? 'Comparing...' : 'Compare with current'}
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      {diff && (
        <div className="mt-4 p-4 bg-gray-50 dark:bg-gray-700 rounded">
          <h4 className="font-medium mb-2">Diff: v{diff.version1} → v{diff.version2}</h4>
          <div className="space-y-1 font-mono text-sm">
            {diff.diff.map((line, i) => (
              <div
                key={i}
                className={
                  line.line_type === 'added'
                    ? 'bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200'
                    : line.line_type === 'removed'
                    ? 'bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200'
                    : ''
                }
              >
                {line.line_type === 'added' && '+ '}
                {line.line_type === 'removed' && '- '}
                {line.content}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

// ============================================================================
// Main Prompt Registry Page
// ============================================================================

export default function PromptRegistry() {
  const [prompts, setPrompts] = useState<PromptResponse[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showEditor, setShowEditor] = useState(false);
  const [selectedPrompt, setSelectedPrompt] = useState<PromptResponse | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [tagFilter, setTagFilter] = useState('');

  const fetchPrompts = async () => {
    setLoading(true);
    try {
      const response = await agentreplayClient.listPrompts({
        search: searchQuery || undefined,
        tag: tagFilter || undefined,
      });
      setPrompts(response.prompts);
      setError(null);
    } catch (err: any) {
      console.error('Failed to fetch prompts:', err);
      setError(err.message || 'Failed to load prompts');
      setPrompts([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchPrompts();
  }, [searchQuery, tagFilter]);

  const handleView = async (id: string) => {
    try {
      const prompt = await agentreplayClient.getPrompt(id);
      setSelectedPrompt(prompt);
      setShowEditor(true);
    } catch (err) {
      console.error('Failed to fetch prompt:', err);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await agentreplayClient.deletePrompt(id);
      fetchPrompts();
    } catch (err) {
      console.error('Failed to delete prompt:', err);
    }
  };

  const handleSaved = (prompt: PromptResponse) => {
    if (selectedPrompt) {
      setPrompts(prompts.map(p => p.id === prompt.id ? prompt : p));
    } else {
      setPrompts([prompt, ...prompts]);
    }
    setSelectedPrompt(null);
  };

  const allTags = [...new Set(prompts.flatMap(p => p.tags))];

  return (
    <div className="container mx-auto p-6">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold">Prompt Registry</h1>
          <p className="text-gray-600 dark:text-gray-400">
            Manage versioned prompt templates with variable interpolation
          </p>
        </div>
        <button
          onClick={() => {
            setSelectedPrompt(null);
            setShowEditor(true);
          }}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 flex items-center gap-2"
        >
          <span>+</span> New Prompt
        </button>
      </div>

      <div className="flex gap-4 mb-6">
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="flex-1 p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
          placeholder="Search prompts..."
        />
        {allTags.length > 0 && (
          <select
            value={tagFilter}
            onChange={(e) => setTagFilter(e.target.value)}
            className="p-2 border rounded dark:bg-gray-700 dark:border-gray-600"
          >
            <option value="">All tags</option>
            {allTags.map(tag => (
              <option key={tag} value={tag}>{tag}</option>
            ))}
          </select>
        )}
      </div>

      {loading ? (
        <div className="flex items-center justify-center h-64">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600" />
        </div>
      ) : error ? (
        <div className="text-center py-12">
          <div className="text-red-500 mb-2">⚠️ {error}</div>
          <button onClick={fetchPrompts} className="text-blue-600 hover:text-blue-800">
            Try again
          </button>
        </div>
      ) : prompts.length === 0 ? (
        <div className="text-center py-12 bg-gray-50 dark:bg-gray-800 rounded-lg">
          <div className="text-4xl mb-4">📝</div>
          <h3 className="text-lg font-medium mb-2">No prompts yet</h3>
          <p className="text-gray-600 dark:text-gray-400 mb-4">
            Create your first prompt template to get started
          </p>
          <button
            onClick={() => setShowEditor(true)}
            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700"
          >
            Create Prompt
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {prompts.map((prompt) => (
            <PromptCard
              key={prompt.id}
              prompt={prompt}
              onView={handleView}
              onDelete={handleDelete}
            />
          ))}
        </div>
      )}

      <PromptEditorModal
        prompt={selectedPrompt || undefined}
        isOpen={showEditor}
        onClose={() => {
          setShowEditor(false);
          setSelectedPrompt(null);
        }}
        onSaved={handleSaved}
      />
    </div>
  );
}
