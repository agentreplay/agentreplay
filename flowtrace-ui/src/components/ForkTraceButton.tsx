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
 * ForkTraceButton - Time-Travel Debugging component
 * 
 * Allows users to fork a trace at any span, reconstructing the full
 * conversation history and opening it in the Playground for experimentation.
 */

import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { GitBranch, Loader2, AlertCircle, Play, ChevronRight } from 'lucide-react';
import { flowtraceClient } from '../lib/flowtrace-api';

interface ForkTraceButtonProps {
  spanId: string;
  spanName?: string;
  variant?: 'button' | 'icon' | 'menu-item';
  className?: string;
}

export function ForkTraceButton({ 
  spanId, 
  spanName,
  variant = 'button',
  className = ''
}: ForkTraceButtonProps) {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<{
    path_depth: number;
    can_fork: boolean;
    message: string;
  } | null>(null);
  const [showPreview, setShowPreview] = useState(false);

  const handleGetPreview = async () => {
    setLoading(true);
    setError(null);
    
    try {
      const result = await flowtraceClient.getForkPreview(spanId);
      setPreview(result);
      setShowPreview(true);
    } catch (err: any) {
      setError(err.message || 'Failed to get fork preview');
    } finally {
      setLoading(false);
    }
  };

  const handleFork = async () => {
    setLoading(true);
    setError(null);
    
    try {
      const result = await flowtraceClient.forkTraceState(spanId);
      
      // Check if we actually have messages to work with
      if (!result.messages || result.messages.length === 0) {
        setError('No conversation messages found in this trace. Fork & Debug works best with LLM/chat traces that contain input/output messages.');
        return;
      }
      
      // Store the forked state in session storage for the Playground
      sessionStorage.setItem('forked_conversation', JSON.stringify({
        messages: result.messages,
        system_prompt: result.system_prompt,
        context_variables: result.context_variables,
        fork_point: result.fork_point,
        total_tokens: result.total_tokens,
        forked_at: new Date().toISOString(),
      }));
      
      // Navigate to playground with fork indicator
      navigate('/playground?forked=true');
    } catch (err: any) {
      setError(err.message || 'Failed to fork trace');
    } finally {
      setLoading(false);
    }
  };

  // Icon-only variant for span rows
  if (variant === 'icon') {
    return (
      <div className="relative group/tooltip">
        <button
          onClick={handleFork}
          disabled={loading}
          className={`p-1.5 rounded hover:bg-surface-hover transition-colors group ${className}`}
          title={`Fork at "${spanName || spanId}" - Replay conversation in Playground`}
        >
          {loading ? (
            <Loader2 className="w-4 h-4 animate-spin text-primary" />
          ) : error ? (
            <AlertCircle className="w-4 h-4 text-error" />
          ) : (
            <GitBranch className="w-4 h-4 text-textSecondary group-hover:text-primary" />
          )}
        </button>
        {/* Error tooltip */}
        {error && (
          <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-2 bg-error text-white text-xs rounded-lg shadow-lg w-64 z-50 opacity-0 group-hover/tooltip:opacity-100 transition-opacity">
            {error}
            <div className="absolute top-full left-1/2 -translate-x-1/2 border-4 border-transparent border-t-error" />
          </div>
        )}
      </div>
    );
  }

  // Menu item variant for dropdown menus
  if (variant === 'menu-item') {
    return (
      <div>
        <button
          onClick={handleFork}
          disabled={loading}
          className={`w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-surface-hover transition-colors ${className}`}
        >
          {loading ? (
            <Loader2 className="w-4 h-4 animate-spin text-primary" />
          ) : (
            <GitBranch className="w-4 h-4 text-textSecondary" />
          )}
          <div className="flex-1">
            <span className="text-sm">Fork & Debug in Playground</span>
            <p className="text-xs text-textTertiary">Replay conversation from this point</p>
          </div>
        </button>
        {error && (
          <div className="px-3 py-2 text-xs text-error bg-error/10">
            {error}
          </div>
        )}
      </div>
    );
  }

  // Full button variant with preview modal
  return (
    <div className="relative inline-block">
      <button
        onClick={showPreview ? handleFork : handleGetPreview}
        disabled={loading}
        className={`flex items-center gap-2 px-3 py-2 bg-surface-elevated border border-border rounded-lg hover:bg-surface-hover transition-colors text-sm font-medium ${className}`}
      >
        {loading ? (
          <>
            <Loader2 className="w-4 h-4 animate-spin" />
            <span>Loading...</span>
          </>
        ) : (
          <>
            <GitBranch className="w-4 h-4" />
            <span>Fork & Debug</span>
          </>
        )}
      </button>

      {/* Preview popup */}
      {showPreview && preview && (
        <div className="absolute top-full left-0 mt-2 w-80 bg-surface-elevated border border-border rounded-lg shadow-lg z-50 p-4">
          <div className="flex items-start gap-3 mb-4">
            <div className="p-2 rounded-lg bg-primary/10">
              <GitBranch className="w-5 h-5 text-primary" />
            </div>
            <div>
              <h4 className="font-medium text-textPrimary">Time-Travel Fork</h4>
              <p className="text-xs text-textSecondary mt-0.5">
                Reconstruct conversation at this point
              </p>
            </div>
          </div>

          <div className="space-y-2 mb-4">
            <div className="flex justify-between text-sm">
              <span className="text-textSecondary">Spans to reconstruct:</span>
              <span className="font-medium text-textPrimary">{preview.path_depth}</span>
            </div>
            <p className="text-xs text-textSecondary">{preview.message}</p>
          </div>

          {error && (
            <div className="flex items-center gap-2 text-error text-xs mb-3">
              <AlertCircle className="w-3 h-3" />
              {error}
            </div>
          )}

          <div className="flex gap-2">
            <button
              onClick={() => setShowPreview(false)}
              className="flex-1 px-3 py-1.5 text-sm border border-border rounded hover:bg-surface-hover transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleFork}
              disabled={loading || !preview.can_fork}
              className="flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 text-sm bg-primary text-white rounded hover:bg-primary-hover disabled:opacity-50 transition-colors"
            >
              {loading ? (
                <Loader2 className="w-3 h-3 animate-spin" />
              ) : (
                <Play className="w-3 h-3" />
              )}
              Open in Playground
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default ForkTraceButton;
