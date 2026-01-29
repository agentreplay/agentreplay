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

/**
 * CommitTraceButton - Commit a trace response to Prompt Registry
 * 
 * Allows users to commit an LLM response to the prompt registry
 * for tracking changes and experiments over time.
 */

import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { GitCommit, Loader2, AlertCircle, X, Check } from 'lucide-react';
import { agentreplayClient } from '../lib/agentreplay-api';
import { loadPrompts, savePrompts, PromptRecord } from '../lib/prompt-store';

interface CommitTraceButtonProps {
  traceId: string;
  spanId?: string;
  spanName?: string;
  model?: string;
  input?: string;
  output?: string;
  messages?: Array<{ role: string; content: string }>;
  tools?: Array<{ name: string; arguments?: string }>;
  latencyMs?: number;
  cost?: number;
  className?: string;
}

export function CommitTraceButton({
  traceId,
  spanId,
  spanName,
  model,
  input = '',
  output = '',
  messages = [],
  tools = [],
  latencyMs,
  cost,
  className = ''
}: CommitTraceButtonProps) {
  const navigate = useNavigate();
  const { projectId } = useParams<{ projectId: string }>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showModal, setShowModal] = useState(false);
  const [success, setSuccess] = useState(false);
  const [commitMessage, setCommitMessage] = useState('');
  const [branchName, setBranchName] = useState(model || spanName || 'main');

  const handleCommit = async () => {
    if (!commitMessage.trim()) {
      setError('Please enter a commit message');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      // Create a descriptive name
      let promptName = spanName || 'LLM Response';
      if (promptName.includes('.') && promptName.split('.').length <= 3) {
        const parts = promptName.split('.');
        const provider = parts[0].charAt(0).toUpperCase() + parts[0].slice(1);
        const type = parts[parts.length - 1];
        promptName = `${provider} ${type.charAt(0).toUpperCase() + type.slice(1)} - ${model || 'Unknown Model'}`;
      }
      // Overwrite name if branch/experiment name provided
      if (branchName && branchName !== 'main') {
        promptName = branchName;
      }

      // Backend expects u128 ID. We use Date.now() which is safe integer (u64).
      // For robustness against collisions, we could add random bits but Date.now() likely sufficient for single user.
      const promptId = Date.now();

      // Construct PromptTemplate matching backend expectation
      const templateData = {
        id: promptId,
        name: promptName,
        description: commitMessage,
        template: input || 'No input captured', // Use input as the template content
        variables: [], // Variables extraction logic can be added later
        tags: model ? [model] : [],
        version: 1, // Backend will increment this automatically based on name
        created_at: Date.now(),
        updated_at: Date.now(),
        created_by: 'AgentReplay User',
        metadata: {
          trace_id: traceId,
          span_id: spanId,
          model: model,
          output: output,
          latency_ms: latencyMs,
          cost: cost,
          messages: messages,
          tools: tools
        }
      };

      // Save directly to backend 
      // This maps to POST /api/v1/prompts -> store_prompt_handler
      await agentreplayClient.createPrompt(templateData);

      // Also update local store for immediate UI update if still using it
      // mapping backend format to frontend record for compatibility
      const localRecord: PromptRecord = {
        id: promptId.toString(),
        name: promptName,
        tags: templateData.tags,
        description: commitMessage,
        lastEdited: templateData.updated_at,
        deployedVersion: null,
        activeVersion: 1,
        owner: templateData.created_by,
        content: templateData.template,
        variables: [],
        history: [{
          id: `${promptId}-v1`,
          version: 1,
          author: templateData.created_by,
          createdAt: templateData.created_at,
          notes: commitMessage,
          content: templateData.template,
        }]
      };
      const prompts = loadPrompts();
      prompts.unshift(localRecord);
      savePrompts(prompts);

      setSuccess(true);

      // Close modal after short delay and navigate to prompts
      setTimeout(() => {
        setShowModal(false);
        setSuccess(false);
        setCommitMessage('');
        if (projectId) {
          navigate(`/projects/${projectId}/prompts`);
        }
      }, 1500);
    } catch (err: any) {
      console.error('Commit failed', err);
      setError(err.message || 'Failed to commit response');
    } finally {
      setLoading(false);
    }
  };

  return (
    <>
      <button
        onClick={() => setShowModal(true)}
        disabled={loading}
        className={`flex items-center gap-2 px-3 py-2 bg-surface-elevated border border-border rounded-lg hover:bg-surface-hover transition-colors text-sm font-medium ${className}`}
      >
        <GitCommit className="w-4 h-4" />
        <span>Commit</span>
      </button>

      {/* Commit Modal */}
      {showModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
          <div className="w-full max-w-md bg-surface-elevated border border-border rounded-xl shadow-2xl p-6 mx-4">
            {/* Header */}
            <div className="flex items-center justify-between mb-6">
              <div className="flex items-center gap-3">
                <div className="p-2 rounded-lg bg-primary/10">
                  <GitCommit className="w-5 h-5 text-primary" />
                </div>
                <div>
                  <h3 className="font-semibold text-textPrimary">Commit Response</h3>
                  <p className="text-xs text-textSecondary">Save this response to version history</p>
                </div>
              </div>
              <button
                onClick={() => {
                  setShowModal(false);
                  setError(null);
                  setSuccess(false);
                }}
                className="p-1.5 rounded-lg hover:bg-surface-hover transition-colors"
              >
                <X className="w-4 h-4 text-textSecondary" />
              </button>
            </div>

            {success ? (
              <div className="flex flex-col items-center py-8">
                <div className="w-16 h-16 rounded-full bg-green-500/20 flex items-center justify-center mb-4">
                  <Check className="w-8 h-8 text-green-500" />
                </div>
                <h4 className="font-medium text-textPrimary mb-1">Committed Successfully!</h4>
                <p className="text-sm text-textSecondary">Redirecting to prompts...</p>
              </div>
            ) : (
              <>
                {/* Trace Info */}
                <div className="bg-background rounded-lg p-4 mb-4 border border-border">
                  <div className="grid grid-cols-2 gap-3 text-sm">
                    <div>
                      <span className="text-textTertiary">Model</span>
                      <p className="font-medium text-textPrimary">{model || 'Unknown'}</p>
                    </div>
                    <div>
                      <span className="text-textTertiary">Trace</span>
                      <p className="font-mono text-xs text-textSecondary truncate">{traceId}</p>
                    </div>
                    {latencyMs !== undefined && (
                      <div>
                        <span className="text-textTertiary">Latency</span>
                        <p className="font-medium text-textPrimary">{latencyMs.toFixed(0)}ms</p>
                      </div>
                    )}
                    {cost !== undefined && (
                      <div>
                        <span className="text-textTertiary">Cost</span>
                        <p className="font-medium text-textPrimary">${cost.toFixed(4)}</p>
                      </div>
                    )}
                  </div>
                </div>

                {/* Branch Name */}
                <div className="mb-4">
                  <label className="block text-sm font-medium text-textSecondary mb-1.5">
                    Branch / Experiment Name
                  </label>
                  <input
                    type="text"
                    value={branchName}
                    onChange={(e) => setBranchName(e.target.value)}
                    placeholder="e.g., gpt-4-experiment, prompt-v2"
                    className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:ring-2 focus:ring-primary/50"
                  />
                </div>

                {/* Commit Message */}
                <div className="mb-4">
                  <label className="block text-sm font-medium text-textSecondary mb-1.5">
                    Commit Message <span className="text-error">*</span>
                  </label>
                  <textarea
                    value={commitMessage}
                    onChange={(e) => setCommitMessage(e.target.value)}
                    placeholder="Describe what changed or why you're saving this version..."
                    rows={3}
                    className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary placeholder:text-textTertiary focus:outline-none focus:ring-2 focus:ring-primary/50 resize-none"
                  />
                </div>

                {/* Error */}
                {error && (
                  <div className="flex items-center gap-2 text-error text-sm mb-4 p-3 bg-error/10 rounded-lg">
                    <AlertCircle className="w-4 h-4 flex-shrink-0" />
                    {error}
                  </div>
                )}

                {/* Actions */}
                <div className="flex gap-3">
                  <button
                    onClick={() => {
                      setShowModal(false);
                      setError(null);
                    }}
                    className="flex-1 px-4 py-2 text-sm border border-border rounded-lg hover:bg-surface-hover transition-colors"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleCommit}
                    disabled={loading || !commitMessage.trim()}
                    className="flex-1 flex items-center justify-center gap-2 px-4 py-2 text-sm bg-primary text-white rounded-lg hover:bg-primary-hover disabled:opacity-50 transition-colors"
                  >
                    {loading ? (
                      <>
                        <Loader2 className="w-4 h-4 animate-spin" />
                        Committing...
                      </>
                    ) : (
                      <>
                        <GitCommit className="w-4 h-4" />
                        Commit
                      </>
                    )}
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </>
  );
}

export default CommitTraceButton;
