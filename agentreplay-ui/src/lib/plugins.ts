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

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

export interface PluginInfo {
  id: string;
  name: string;
  version: string;
  description: string;
  plugin_type: string;
  authors: string[];
  state: string;
  enabled: boolean;
  install_path: string;
  installed_at: string;
  source: string;
  capabilities: string[];
  tags: string[];
  error?: string;
}

export interface PluginListResponse {
  plugins: PluginInfo[];
  total: number;
}

export interface InstallResult {
  plugin_id: string;
  version: string;
  install_path: string;
  installed_at: string;
  dependencies_installed: string[];
}

export interface UninstallResult {
  plugin_id: string;
  removed_files: number;
  data_preserved: boolean;
  broken_dependents: string[];
}

export interface PluginStats {
  total: number;
  active: number;
  evaluators: number;
}

// ============================================================================
// API Functions
// ============================================================================

export async function listPlugins(): Promise<PluginListResponse> {
  return invoke<PluginListResponse>('plugin_list');
}

export async function getPlugin(pluginId: string): Promise<PluginInfo | null> {
  return invoke<PluginInfo | null>('plugin_get', { pluginId });
}

export async function installPlugin(
  source: { type: 'directory' | 'file' | 'dev'; path: string }
): Promise<InstallResult> {
  return invoke<InstallResult>('plugin_install', {
    request: { source }
  });
}

export async function uninstallPlugin(
  pluginId: string,
  mode: 'safe' | 'cascade' | 'force' = 'safe',
  preserveData: boolean = false
): Promise<UninstallResult> {
  return invoke<UninstallResult>('plugin_uninstall', {
    request: {
      plugin_id: pluginId,
      mode,
      preserve_data: preserveData
    }
  });
}

export async function enablePlugin(pluginId: string): Promise<void> {
  return invoke<void>('plugin_enable', { pluginId });
}

export async function disablePlugin(pluginId: string): Promise<void> {
  return invoke<void>('plugin_disable', { pluginId });
}

export async function getPluginSettings(pluginId: string): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>('plugin_get_settings', { pluginId });
}

export async function updatePluginSettings(
  pluginId: string,
  settings: Record<string, unknown>
): Promise<void> {
  return invoke<void>('plugin_update_settings', {
    request: { plugin_id: pluginId, settings }
  });
}

export async function searchPlugins(query: string): Promise<PluginInfo[]> {
  return invoke<PluginInfo[]>('plugin_search', { query });
}

export async function reloadPlugin(pluginId: string): Promise<void> {
  return invoke<void>('plugin_reload', { pluginId });
}

export async function scanPlugins(): Promise<PluginInfo[]> {
  return invoke<PluginInfo[]>('plugin_scan');
}

export async function getPluginsDir(): Promise<string> {
  return invoke<string>('plugin_get_dir');
}

export async function getPluginStats(): Promise<PluginStats> {
  return invoke<PluginStats>('plugin_stats');
}

// ============================================================================
// React Hook
// ============================================================================

export function usePlugins() {
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setLoading(true);
      setError(null);
      const response = await listPlugins();
      setPlugins(response.plugins);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const install = async (path: string, isDev: boolean = false) => {
    const result = await installPlugin({
      type: isDev ? 'dev' : 'directory',
      path
    });
    await refresh();
    return result;
  };

  const uninstall = async (pluginId: string, force: boolean = false) => {
    const result = await uninstallPlugin(pluginId, force ? 'force' : 'safe');
    await refresh();
    return result;
  };

  const enable = async (pluginId: string) => {
    await enablePlugin(pluginId);
    await refresh();
  };

  const disable = async (pluginId: string) => {
    await disablePlugin(pluginId);
    await refresh();
  };

  const reload = async (pluginId: string) => {
    await reloadPlugin(pluginId);
    await refresh();
  };

  return {
    plugins,
    loading,
    error,
    refresh,
    install,
    uninstall,
    enable,
    disable,
    reload
  };
}
