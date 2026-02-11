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
