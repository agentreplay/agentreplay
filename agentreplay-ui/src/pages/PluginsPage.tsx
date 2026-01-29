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

import { useState, useCallback } from 'react';
import { usePlugins, PluginInfo, getPluginsDir } from '../lib/plugins';
import { VideoHelpButton } from '../components/VideoHelpButton';

// ============================================================================
// Plugin Card Component
// ============================================================================

interface PluginCardProps {
  plugin: PluginInfo;
  onEnable: (id: string) => Promise<void>;
  onDisable: (id: string) => Promise<void>;
  onUninstall: (id: string) => Promise<unknown>;
  onReload?: (id: string) => Promise<void>;
  onSettings?: (id: string) => void;
}

function PluginCard({ plugin, onEnable, onDisable, onUninstall, onReload, onSettings }: PluginCardProps) {
  const [loading, setLoading] = useState(false);
  const [showConfirmUninstall, setShowConfirmUninstall] = useState(false);

  const handleToggle = async () => {
    setLoading(true);
    try {
      if (plugin.enabled) {
        await onDisable(plugin.id);
      } else {
        await onEnable(plugin.id);
      }
    } finally {
      setLoading(false);
    }
  };

  const handleUninstall = async () => {
    setLoading(true);
    try {
      await onUninstall(plugin.id);
    } finally {
      setLoading(false);
      setShowConfirmUninstall(false);
    }
  };

  const handleReload = async () => {
    if (onReload) {
      setLoading(true);
      try {
        await onReload(plugin.id);
      } finally {
        setLoading(false);
      }
    }
  };

  const getTypeColor = (type: string) => {
    switch (type.toLowerCase()) {
      case 'evaluator': return 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200';
      case 'embedding_provider': return 'bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-200';
      case 'llm_provider': return 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200';
      case 'integration': return 'bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-200';
      default: return 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200';
    }
  };

  const getStateIcon = () => {
    switch (plugin.state.toLowerCase()) {
      case 'active': return '🟢';
      case 'enabled': return '🟡';
      case 'disabled': return '⚪';
      case 'failed': return '🔴';
      default: return '⚪';
    }
  };

  return (
    <div className="border border-border rounded-lg p-4 bg-surface hover:shadow-md transition-shadow">
      <div className="flex items-start justify-between mb-2">
        <div className="flex items-center gap-2">
          <span className="text-lg" title={plugin.state}>{getStateIcon()}</span>
          <h3 className="font-semibold text-lg text-textPrimary">{plugin.name}</h3>
        </div>
        <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${getTypeColor(plugin.plugin_type)}`}>
          {plugin.plugin_type.replace('_', ' ')}
        </span>
      </div>

      <p className="text-sm text-textSecondary mb-2">v{plugin.version}</p>
      
      {plugin.description && (
        <p className="text-sm text-textSecondary mb-3 line-clamp-2">{plugin.description}</p>
      )}

      {plugin.tags.length > 0 && (
        <div className="flex flex-wrap gap-1 mb-3">
          {plugin.tags.slice(0, 3).map(tag => (
            <span key={tag} className="px-2 py-0.5 bg-gray-100 dark:bg-gray-700 rounded text-xs text-textSecondary">
              {tag}
            </span>
          ))}
          {plugin.tags.length > 3 && (
            <span className="text-xs text-textSecondary">+{plugin.tags.length - 3} more</span>
          )}
        </div>
      )}

      {plugin.error && (
        <div className="mb-3 p-2 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded text-sm text-red-600 dark:text-red-400">
          {plugin.error}
        </div>
      )}

      <div className="flex items-center justify-between pt-2 border-t border-border">
        <div className="flex items-center gap-2">
          <label className="relative inline-flex items-center cursor-pointer">
            <input
              type="checkbox"
              checked={plugin.enabled}
              onChange={handleToggle}
              disabled={loading}
              className="sr-only peer"
            />
            <div className="w-9 h-5 bg-gray-200 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
            <span className="ml-2 text-sm text-textSecondary">
              {plugin.enabled ? 'Enabled' : 'Disabled'}
            </span>
          </label>
        </div>

        <div className="flex items-center gap-1">
          {plugin.source === 'development' && onReload && (
            <button
              onClick={handleReload}
              disabled={loading}
              className="p-1.5 text-textSecondary hover:text-textPrimary hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
              title="Reload plugin"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
            </button>
          )}
          
          {onSettings && (
            <button
              onClick={() => onSettings(plugin.id)}
              disabled={loading}
              className="p-1.5 text-textSecondary hover:text-textPrimary hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
              title="Settings"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
              </svg>
            </button>
          )}

          <button
            onClick={() => setShowConfirmUninstall(true)}
            disabled={loading}
            className="p-1.5 text-red-500 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 rounded"
            title="Uninstall"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
            </svg>
          </button>
        </div>
      </div>

      {/* Uninstall Confirmation Modal */}
      {showConfirmUninstall && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-surface rounded-lg p-6 max-w-sm w-full mx-4 shadow-xl">
            <h4 className="text-lg font-semibold text-textPrimary mb-2">Uninstall Plugin</h4>
            <p className="text-textSecondary mb-4">
              Are you sure you want to uninstall <strong>{plugin.name}</strong>? This action cannot be undone.
            </p>
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setShowConfirmUninstall(false)}
                className="px-4 py-2 text-textSecondary hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
              >
                Cancel
              </button>
              <button
                onClick={handleUninstall}
                disabled={loading}
                className="px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600 disabled:opacity-50"
              >
                {loading ? 'Uninstalling...' : 'Uninstall'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Main Plugins Page Component
// ============================================================================

export default function PluginsPage() {
  const { plugins, loading, error, refresh, install, uninstall, enable, disable, reload } = usePlugins();
  const [searchQuery, setSearchQuery] = useState('');
  const [filterType, setFilterType] = useState<string>('all');
  const [showInstallDialog, setShowInstallDialog] = useState(false);
  const [githubUrl, setGithubUrl] = useState('');
  const [installing, setInstalling] = useState(false);
  const [installError, setInstallError] = useState<string | null>(null);
  const [pluginsDir, setPluginsDir] = useState<string | null>(null);

  // Get plugins directory on mount
  useState(() => {
    getPluginsDir().then(setPluginsDir).catch(console.error);
  });

  const filteredPlugins = plugins.filter(plugin => {
    const matchesSearch = searchQuery === '' || 
      plugin.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      plugin.description.toLowerCase().includes(searchQuery.toLowerCase()) ||
      plugin.id.toLowerCase().includes(searchQuery.toLowerCase());
    
    const matchesType = filterType === 'all' || plugin.plugin_type === filterType;
    
    return matchesSearch && matchesType;
  });

  const handleInstall = async () => {
    const url = githubUrl.trim();
    if (!url) return;
    
    // Validate GitHub URL
    const githubRegex = /^https:\/\/github\.com\/[\w-]+\/[\w.-]+(\/.*)?$/;
    if (!githubRegex.test(url)) {
      setInstallError('Please enter a valid GitHub repository URL (e.g., https://github.com/owner/repo)');
      return;
    }
    
    setInstalling(true);
    setInstallError(null);
    try {
      await install(url, false);
      setShowInstallDialog(false);
      setGithubUrl('');
    } catch (err) {
      console.error('Install failed:', err);
      setInstallError(err instanceof Error ? err.message : String(err));
    } finally {
      setInstalling(false);
    }
  };

  const handleOpenPluginsDir = useCallback(async () => {
    if (pluginsDir) {
      // Use Tauri's shell API to open the directory
      try {
        // Try to use the Tauri shell plugin if available
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('open_path', { path: pluginsDir });
      } catch (err) {
        // Fallback: Just log it
        console.log('Plugins directory:', pluginsDir);
        alert(`Plugins directory: ${pluginsDir}`);
      }
    }
  }, [pluginsDir]);

  const uniqueTypes = [...new Set(plugins.map(p => p.plugin_type))];

  if (loading && plugins.length === 0) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-textSecondary">Loading plugins...</div>
      </div>
    );
  }

  return (
    <div className="p-6 max-w-6xl mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-textPrimary">Plugins</h1>
          <p className="text-textSecondary">
            Extend AgentReplay with custom evaluators, providers, and integrations
          </p>
        </div>
        <div className="flex items-center gap-2">
          <VideoHelpButton pageId="plugins" />
          <button
            onClick={() => refresh()}
            className="px-3 py-2 text-textSecondary hover:text-textPrimary hover:bg-gray-100 dark:hover:bg-gray-700 rounded flex items-center gap-1"
            title="Refresh"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
          </button>
          <button
            onClick={handleOpenPluginsDir}
            className="px-3 py-2 text-textSecondary hover:text-textPrimary hover:bg-gray-100 dark:hover:bg-gray-700 rounded flex items-center gap-1"
            title="Open plugins folder"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
          </button>
          <button
            onClick={() => setShowInstallDialog(true)}
            className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 flex items-center gap-2"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
            </svg>
            Install Plugin
          </button>
        </div>
      </div>

      {/* Error Alert */}
      {error && (
        <div className="mb-4 p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-red-600 dark:text-red-400">
          {error}
        </div>
      )}

      {/* Filters */}
      <div className="flex gap-4 mb-6">
        <div className="flex-1">
          <input
            type="text"
            placeholder="Search plugins..."
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            className="w-full px-4 py-2 border border-border rounded-lg bg-surface text-textPrimary placeholder:text-textSecondary focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>
        <select
          value={filterType}
          onChange={e => setFilterType(e.target.value)}
          className="px-4 py-2 border border-border rounded-lg bg-surface text-textPrimary focus:outline-none focus:ring-2 focus:ring-blue-500"
        >
          <option value="all">All Types</option>
          {uniqueTypes.map(type => (
            <option key={type} value={type}>{type.replace('_', ' ')}</option>
          ))}
        </select>
      </div>

      {/* Plugin Grid */}
      {filteredPlugins.length === 0 ? (
        <div className="text-center py-12">
          <div className="text-4xl mb-4">🧩</div>
          <h3 className="text-lg font-medium text-textPrimary mb-2">
            {plugins.length === 0 ? 'No plugins installed' : 'No plugins match your search'}
          </h3>
          <p className="text-textSecondary mb-4">
            {plugins.length === 0 
              ? 'Install plugins to extend AgentReplay functionality'
              : 'Try adjusting your search or filter'
            }
          </p>
          {plugins.length === 0 && (
            <button
              onClick={() => setShowInstallDialog(true)}
              className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600"
            >
              Install Your First Plugin
            </button>
          )}
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filteredPlugins.map(plugin => (
            <PluginCard
              key={plugin.id}
              plugin={plugin}
              onEnable={enable}
              onDisable={disable}
              onUninstall={uninstall}
              onReload={plugin.source === 'development' ? reload : undefined}
            />
          ))}
        </div>
      )}

      {/* Plugins Directory Info */}
      {pluginsDir && (
        <div className="mt-8 p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
          <p className="text-sm text-textSecondary">
            <strong>Plugins directory:</strong> {pluginsDir}
          </p>
          <p className="text-sm text-textSecondary mt-1">
            Install plugins directly from GitHub using the Install button above.
          </p>
        </div>
      )}

      {/* Install Dialog */}
      {showInstallDialog && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-surface rounded-lg p-6 max-w-md w-full mx-4 shadow-xl">
            <h3 className="text-lg font-semibold text-textPrimary mb-4">Install Plugin from GitHub</h3>
            
            <div className="mb-4">
              <label className="block text-sm font-medium text-textPrimary mb-1">
                GitHub Repository URL
              </label>
              <div className="relative">
                <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                  <svg className="w-5 h-5 text-textSecondary" fill="currentColor" viewBox="0 0 24 24">
                    <path fillRule="evenodd" clipRule="evenodd" d="M12 2C6.477 2 2 6.477 2 12c0 4.42 2.865 8.17 6.839 9.49.5.092.682-.217.682-.482 0-.237-.008-.866-.013-1.7-2.782.604-3.369-1.34-3.369-1.34-.454-1.156-1.11-1.464-1.11-1.464-.908-.62.069-.608.069-.608 1.003.07 1.531 1.03 1.531 1.03.892 1.529 2.341 1.087 2.91.831.092-.646.35-1.086.636-1.336-2.22-.253-4.555-1.11-4.555-4.943 0-1.091.39-1.984 1.029-2.683-.103-.253-.446-1.27.098-2.647 0 0 .84-.269 2.75 1.025A9.578 9.578 0 0112 6.836c.85.004 1.705.114 2.504.336 1.909-1.294 2.747-1.025 2.747-1.025.546 1.377.203 2.394.1 2.647.64.699 1.028 1.592 1.028 2.683 0 3.842-2.339 4.687-4.566 4.935.359.309.678.919.678 1.852 0 1.336-.012 2.415-.012 2.743 0 .267.18.578.688.48C19.138 20.167 22 16.418 22 12c0-5.523-4.477-10-10-10z" />
                  </svg>
                </div>
                <input
                  type="url"
                  value={githubUrl}
                  onChange={e => {
                    setGithubUrl(e.target.value);
                    setInstallError(null);
                  }}
                  placeholder="https://github.com/owner/plugin-repo"
                  className="w-full pl-10 pr-4 py-2 border border-border rounded-lg bg-surface text-textPrimary placeholder:text-textSecondary focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
              </div>
              <p className="mt-1 text-sm text-textSecondary">
                Enter a GitHub repository URL containing a AgentReplay plugin
              </p>
            </div>

            {installError && (
              <div className="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-sm text-red-600 dark:text-red-400">
                {installError}
              </div>
            )}

            <div className="flex justify-end gap-2">
              <button
                onClick={() => {
                  setShowInstallDialog(false);
                  setGithubUrl('');
                  setInstallError(null);
                }}
                className="px-4 py-2 text-textSecondary hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
              >
                Cancel
              </button>
              <button
                onClick={handleInstall}
                disabled={installing || !githubUrl.trim()}
                className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50 flex items-center gap-2"
              >
                {installing ? (
                  <>
                    <svg className="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
                      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                    </svg>
                    Installing...
                  </>
                ) : 'Install from GitHub'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
