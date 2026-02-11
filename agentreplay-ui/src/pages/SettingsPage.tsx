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

import React, { useState, useEffect, useCallback } from 'react';
import { useSearchParams } from 'react-router-dom';
import { agentreplayClient, API_BASE_URL } from '../lib/agentreplay-api';
import { applyTheme as applyThemeFromLib, resolveTheme } from '../lib/theme';
import axios from 'axios';
import { motion, AnimatePresence } from 'framer-motion';
import { VideoHelpButton } from '../components/VideoHelpButton';
import {
  Save,
  RotateCcw,
  Loader2,
  CheckCircle,
  AlertCircle,
  Database,
  Server,
  Palette,
  User,
  Folder,
  FolderOpen,
  HardDrive,
  Download,
  Upload,
  RefreshCw,
  Key,
  Eye,
  EyeOff,
  Cpu,
  Activity,
  Wifi,
  WifiOff,
  Trash2,
  AlertTriangle,
  Layers,
  Zap,
  Gauge,
  Play,
  Info
} from 'lucide-react';

// Service status interface
interface ServiceStatus {
  name: string;
  status: 'online' | 'offline' | 'checking';
  port?: number;
  version?: string;
  uptime?: string;
  error?: string;
}

interface HealthResponse {
  status: string;
  version: string;
  uptime_seconds: number;
  storage: {
    reachable: boolean;
    total_edges: number;
  };
  api: {
    requests_total: number;
    avg_latency_ms: number;
  };
}

// OpenAI-compatible provider configuration
interface ProviderConfig {
  id: string;                    // Unique identifier
  name: string;                  // Display name (e.g., "My OpenAI", "Claude Prod")
  provider: 'openai' | 'anthropic' | 'ollama' | 'custom';  // Provider type
  baseUrl: string;               // API endpoint URL
  modelName: string;             // Model identifier (freeform, e.g., "gpt-4o", "claude-3-opus")
  apiKey: string;                // API key (empty for local providers like Ollama)
  isDefault?: boolean;           // Whether this is the default provider
  isValid?: boolean;             // Validation status
  lastValidated?: string;        // Last validation timestamp
  tags?: string[];               // Tags for routing: "default", "eval", "chat", "analysis"
}

// Available tags for provider routing (embedding is handled separately in Embeddings tab)
const PROVIDER_TAGS = [
  { id: 'default', label: 'Default', description: 'Used for all purposes when no specific tag matches' },
  { id: 'eval', label: 'Evaluation', description: 'Used for G-EVAL and LLM-as-judge evaluations' },
  { id: 'chat', label: 'Chat', description: 'Used for playground/conversations' },
  { id: 'analysis', label: 'Analysis', description: 'Used for trace analysis and insights' },
];

// Default base URLs for known providers
const PROVIDER_DEFAULTS: Record<string, { baseUrl: string; placeholder: string }> = {
  openai: {
    baseUrl: 'https://api.openai.com/v1',
    placeholder: 'gpt-4o, gpt-4-turbo, gpt-3.5-turbo...'
  },
  anthropic: {
    baseUrl: 'https://api.anthropic.com/v1',
    placeholder: 'claude-3-opus-20240229, claude-3-sonnet...'
  },
  ollama: {
    baseUrl: 'http://localhost:11434/v1',
    placeholder: 'llama3, mistral, codellama...'
  },
  custom: {
    baseUrl: '',
    placeholder: 'Enter model name...'
  },
};

interface AgentReplaySettings {
  database: {
    max_traces: number | null;
    retention_days: number | null;
    auto_compact: boolean;
  };
  server: {
    port: number;
    enable_cors: boolean;
    max_payload_size_mb: number;
  };
  ui: {
    theme: 'light' | 'dark' | 'midnight';
    animations_enabled: boolean;
    auto_refresh_interval_secs: number;
    experimental_features: boolean;
  };
  analytics: {
    enabled: boolean;
  };
  models: {
    providers: ProviderConfig[];      // List of configured providers
    defaultProviderId: string | null; // ID of the default provider
    defaultTemperature: number;
    defaultMaxTokens: number;
  };
  embedding: {
    provider: 'openai' | 'ollama' | 'fastembed' | 'custom';
    model: string;
    dimensions: number;
    apiKey: string | null;
    baseUrl: string | null;
    enabled: boolean;
    autoIndexNewTraces: boolean;
    batchSize: number;
  };
}

type SettingsScope = 'user' | 'project' | 'local';

// BackupsList Component - shows list of available backups with export/import
function BackupsList({ onMessage, onRefreshNeeded }: {
  onMessage: (msg: { type: 'success' | 'error'; text: string }) => void;
  onRefreshNeeded?: () => void;
}) {
  const [backups, setBackups] = useState<Array<{
    backup_id: string;
    created_at: number;
    size_bytes: number;
    path: string;
  }>>([]);
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [restoring, setRestoring] = useState<string | null>(null);
  const fileInputRef = React.useRef<HTMLInputElement>(null);

  useEffect(() => {
    loadBackups();
  }, []);

  const loadBackups = async () => {
    try {
      setLoading(true);
      const result = await agentreplayClient.listBackups();
      setBackups(result.backups || []);
    } catch (error) {
      console.error('Failed to load backups:', error);
    } finally {
      setLoading(false);
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const formatDate = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  // Export backup as downloadable ZIP file
  const handleExportBackup = async (backupId: string) => {
    try {
      setExporting(backupId);
      onMessage({ type: 'success', text: 'Preparing backup for download...' });

      // Request the server to create a zip and return it
      const response = await fetch(`http://localhost:47100/api/v1/admin/backups/${backupId}/export`, {
        method: 'GET',
      });

      if (!response.ok) {
        throw new Error('Failed to export backup');
      }

      // Get the blob and create download link
      const blob = await response.blob();
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      // Get filename from content-disposition or generate one
      const contentDisposition = response.headers.get('content-disposition');
      const filenameMatch = contentDisposition?.match(/filename="?([^"]+)"?/);
      const filename = filenameMatch?.[1] || `agentreplay_backup_${backupId.replace('backup_', '')}.zip`;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      window.URL.revokeObjectURL(url);
      document.body.removeChild(a);

      onMessage({ type: 'success', text: `Backup exported: ${filename}` });
    } catch (error) {
      onMessage({ type: 'error', text: `Export failed: ${error}` });
    } finally {
      setExporting(null);
    }
  };

  // Import backup from uploaded ZIP file
  const handleImportBackup = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    try {
      setImporting(true);
      onMessage({ type: 'success', text: `Importing backup: ${file.name}...` });

      const formData = new FormData();
      formData.append('backup', file);

      const response = await fetch('http://localhost:47100/api/v1/admin/backups/import', {
        method: 'POST',
        body: formData,
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.error || 'Import failed');
      }

      const result = await response.json();
      onMessage({ type: 'success', text: `Backup imported: ${result.backup_id}. Restart the app to apply.` });
      loadBackups(); // Refresh the list
    } catch (error) {
      onMessage({ type: 'error', text: `Import failed: ${error}` });
    } finally {
      setImporting(false);
      // Reset file input
      if (fileInputRef.current) {
        fileInputRef.current.value = '';
      }
    }
  };

  const handleDeleteBackup = async (backupId: string) => {
    if (!confirm(`Are you sure you want to delete backup "${backupId}"?`)) {
      return;
    }

    try {
      const response = await fetch(`http://localhost:47100/api/v1/admin/backups/${backupId}`, {
        method: 'DELETE',
      });

      if (response.ok) {
        onMessage({ type: 'success', text: `Deleted backup: ${backupId}` });
        loadBackups();
      } else {
        onMessage({ type: 'error', text: 'Failed to delete backup' });
      }
    } catch (error) {
      onMessage({ type: 'error', text: `Failed to delete backup: ${error}` });
    }
  };

  const [restoreMode, setRestoreMode] = useState<'replace' | 'merge'>('replace');
  const [showRestoreDialog, setShowRestoreDialog] = useState<string | null>(null);

  const handleRestoreBackup = async (backupId: string, mode: 'replace' | 'merge' = 'replace') => {
    setShowRestoreDialog(null);

    try {
      onMessage({ type: 'success', text: `${mode === 'merge' ? 'Merging' : 'Restoring'} backup... Please wait.` });

      const response = await fetch(`http://localhost:47100/api/v1/admin/backups/${backupId}/restore?mode=${mode}`, {
        method: 'POST',
      });

      const result = await response.json();

      if (result.success) {
        onMessage({ type: 'success', text: result.message });
        // Refresh the backups list to show the new pre-restore backup
        loadBackups();
      } else {
        onMessage({ type: 'error', text: result.error || 'Restore failed' });
      }
    } catch (error) {
      onMessage({ type: 'error', text: `Restore failed: ${error}` });
    }
  };

  if (loading) {
    return (
      <div className="bg-card rounded-lg border border-border p-6 mt-6">
        <div className="flex items-center gap-3 mb-4">
          <Folder className="w-6 h-6 text-blue-600 dark:text-blue-400" />
          <h2 className="text-xl font-semibold text-foreground">Available Backups</h2>
        </div>
        <div className="flex items-center justify-center py-8 text-muted-foreground">
          <Loader2 className="w-5 h-5 animate-spin mr-2" />
          Loading backups...
        </div>
      </div>
    );
  }

  return (
    <div className="bg-card rounded-lg border border-border p-6 mt-6">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <Folder className="w-6 h-6 text-blue-600 dark:text-blue-400" />
          <h2 className="text-xl font-semibold text-foreground">Backups</h2>
          <span className="px-2 py-0.5 bg-blue-500/20 text-blue-600 dark:text-blue-400 text-xs rounded-full">
            {backups.length}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {/* Import backup button */}
          <input
            type="file"
            ref={fileInputRef}
            accept=".zip"
            onChange={handleImportBackup}
            className="hidden"
          />
          <button
            onClick={() => fileInputRef.current?.click()}
            disabled={importing}
            className="px-3 py-1.5 text-sm bg-purple-600 text-foreground rounded-lg hover:bg-purple-700 flex items-center gap-1.5 disabled:opacity-50"
            title="Import backup from ZIP file"
          >
            {importing ? <Loader2 className="w-4 h-4 animate-spin" /> : <Upload className="w-4 h-4" />}
            Import
          </button>
          <button
            onClick={loadBackups}
            className="p-2 text-muted-foreground hover:text-foreground hover:bg-secondary/80 rounded-lg transition-colors"
            title="Refresh"
          >
            <RefreshCw className="w-4 h-4" />
          </button>
        </div>
      </div>

      {backups.length === 0 ? (
        <div className="text-center py-8 text-muted-foreground">
          <HardDrive className="w-12 h-12 mx-auto mb-3 opacity-50" />
          <p>No backups found</p>
          <p className="text-sm mt-1">Create your first backup using the button above</p>
        </div>
      ) : (
        <div className="space-y-3">
          {backups.map((backup) => (
            <div
              key={backup.backup_id}
              className="flex items-center justify-between p-4 bg-card rounded-lg border border-border/50"
            >
              <div className="flex items-center gap-4">
                <div className="p-2 bg-orange-500/20 rounded-lg">
                  <HardDrive className="w-5 h-5 text-orange-600 dark:text-orange-400" />
                </div>
                <div>
                  <p className="font-medium text-foreground">{backup.backup_id}</p>
                  <div className="flex items-center gap-3 text-sm text-muted-foreground mt-1">
                    <span>{formatDate(backup.created_at)}</span>
                    <span>•</span>
                    <span>{formatBytes(backup.size_bytes)}</span>
                  </div>
                </div>
              </div>

              <div className="flex items-center gap-2">
                {/* Export/Download as ZIP */}
                <button
                  onClick={() => handleExportBackup(backup.backup_id)}
                  disabled={exporting === backup.backup_id}
                  className="px-3 py-1.5 text-sm bg-green-600 text-foreground rounded-lg hover:bg-green-700 flex items-center gap-1.5 disabled:opacity-50"
                  title="Download backup as ZIP"
                >
                  {exporting === backup.backup_id ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <Download className="w-4 h-4" />
                  )}
                  Export
                </button>
                {/* Restore */}
                <button
                  onClick={() => setShowRestoreDialog(backup.backup_id)}
                  disabled={restoring === backup.backup_id}
                  className="px-3 py-1.5 text-sm bg-blue-600 text-foreground rounded-lg hover:bg-blue-700 flex items-center gap-1.5 disabled:opacity-50"
                  title="Restore from this backup"
                >
                  {restoring === backup.backup_id ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <RefreshCw className="w-4 h-4" />
                  )}
                  Restore
                </button>
                {/* Delete */}
                <button
                  onClick={() => handleDeleteBackup(backup.backup_id)}
                  className="px-3 py-1.5 text-sm bg-red-600/20 text-red-600 dark:text-red-400 rounded-lg hover:bg-red-600/30 flex items-center gap-1.5"
                  title="Delete backup"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      <p className="text-xs text-muted-foreground mt-4">
        💡 Export downloads a ZIP file you can save anywhere. Import uploads a previously exported ZIP to restore.
      </p>

      {/* Restore Dialog */}
      {showRestoreDialog && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-secondary rounded-lg border border-border p-6 max-w-md w-full mx-4 shadow-xl">
            <h3 className="text-lg font-semibold text-foreground mb-4">Restore Backup</h3>
            <p className="text-muted-foreground mb-4">
              Restore from backup <span className="text-blue-600 dark:text-blue-400 font-mono">{showRestoreDialog}</span>
            </p>

            <div className="mb-4">
              <label className="text-sm text-muted-foreground mb-2 block">Restore Mode:</label>
              <div className="space-y-2">
                <label className="flex items-start gap-3 p-3 bg-secondary/50 rounded-lg cursor-pointer hover:bg-secondary/80">
                  <input
                    type="radio"
                    name="restoreMode"
                    value="replace"
                    checked={restoreMode === 'replace'}
                    onChange={() => setRestoreMode('replace')}
                    className="mt-1"
                  />
                  <div>
                    <p className="text-foreground font-medium">Replace (Full Restore)</p>
                    <p className="text-sm text-muted-foreground">Replace all current data with backup data. A pre-restore backup will be created.</p>
                  </div>
                </label>
                <label className="flex items-start gap-3 p-3 bg-secondary/50 rounded-lg cursor-pointer hover:bg-secondary/80">
                  <input
                    type="radio"
                    name="restoreMode"
                    value="merge"
                    checked={restoreMode === 'merge'}
                    onChange={() => setRestoreMode('merge')}
                    className="mt-1"
                  />
                  <div>
                    <p className="text-foreground font-medium">Merge (Append) <span className="text-yellow-600 dark:text-yellow-400 text-xs">(Experimental)</span></p>
                    <p className="text-sm text-muted-foreground">Add backup traces to existing data. ⚠️ Project associations may not be preserved - traces may appear in wrong projects.</p>
                  </div>
                </label>
              </div>
            </div>

            <div className="flex gap-3 justify-end">
              <button
                onClick={() => setShowRestoreDialog(null)}
                className="px-4 py-2 text-sm text-muted-foreground hover:text-foreground"
              >
                Cancel
              </button>
              <button
                onClick={() => handleRestoreBackup(showRestoreDialog, restoreMode)}
                className="px-4 py-2 text-sm bg-blue-600 text-foreground rounded-lg hover:bg-blue-700"
              >
                {restoreMode === 'merge' ? 'Merge Data' : 'Restore'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export function SettingsPage() {
  // Read tab from URL query params
  const [searchParams] = useSearchParams();
  const initialTab = (searchParams.get('tab') as 'database' | 'server' | 'ui' | 'models' | 'embedding' | 'backup' | 'danger') || 'database';

  const [scope, setScope] = useState<SettingsScope>('user');
  const [activeTab, setActiveTab] = useState<'database' | 'server' | 'ui' | 'models' | 'embedding' | 'backup' | 'danger'>(initialTab);

  // Calibration state
  const [isCalibrating, setIsCalibrating] = useState(false);
  const [calibrationResults, setCalibrationResults] = useState<{
    systemInfo: {
      totalMemoryGB: number;
      usedMemoryGB: number;
      availableMemoryGB: number;
      memoryUsagePercent: number;
      cpuCores: number;
      cpuBrand: string;
      cpuUsagePercent: number;
      platform: string;
      osType?: string;
      osVersion?: string;
      arch?: string;
      hostname?: string;
      currentTraceCount?: number;
      currentStorageMB?: number;
      avgTraceSizeKB?: number;
      jsHeapUsedMB?: number;
      serverOnline?: boolean;
      isTauriApp?: boolean;
    };
    recommendations: {
      maxTraces: number;
      estimatedStorageGB: number;
      memoryUsagePercent: number;
      performanceLevel: 'low' | 'medium' | 'high';
      tracesPerSecond?: number;
    };
    benchmarks: {
      writeSpeedMBps: number;
      readSpeedMBps: number;
      indexingTimeMs: number;
      opsPerSecond?: number;
    };
  } | null>(null);
  const [calibrationError, setCalibrationError] = useState<string | null>(null);
  const [settings, setSettings] = useState<AgentReplaySettings | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const [hasChanges, setHasChanges] = useState(false);
  const [resetConfirmation, setResetConfirmation] = useState('');
  const [isResetting, setIsResetting] = useState(false);
  const [services, setServices] = useState<ServiceStatus[]>([
    { name: 'Agentreplay API', status: 'checking', port: 47100 },
    { name: 'Ollama (Local LLM)', status: 'checking', port: 11434 },
  ]);

  // Check service status
  const checkServiceStatus = useCallback(async () => {
    const updatedServices: ServiceStatus[] = [];
    const apiPort = settings?.server?.port || 47100;

    // Check AgentReplay API
    try {
      const baseUrl = API_BASE_URL || `http://127.0.0.1:${apiPort}`;
      const response = await axios.get<HealthResponse>(`${baseUrl}/api/v1/health`, { timeout: 3000 });

      const data = response.data;
      const uptimeHours = Math.floor(data.uptime_seconds / 3600);
      const uptimeMins = Math.floor((data.uptime_seconds % 3600) / 60);
      updatedServices.push({
        name: 'Agentreplay API',
        status: data.status === 'healthy' ? 'online' : 'offline',
        port: apiPort,
        version: data.version,
        uptime: `${uptimeHours}h ${uptimeMins}m`,
      });
    } catch (error: any) {
      updatedServices.push({
        name: 'Agentreplay API',
        status: 'offline',
        port: apiPort,
        error: error.message || 'Connection failed',
      });
    }

    // Check Ollama
    try {
      const ollamaResponse = await fetch('http://localhost:11434/api/tags', {
        method: 'GET',
        signal: AbortSignal.timeout(3000),
      });
      if (ollamaResponse.ok) {
        const ollamaData = await ollamaResponse.json();
        const modelCount = ollamaData.models?.length || 0;
        updatedServices.push({
          name: 'Ollama (Local LLM)',
          status: 'online',
          port: 11434,
          version: `${modelCount} models`,
        });
      } else {
        throw new Error('Not reachable');
      }
    } catch (error: any) {
      updatedServices.push({
        name: 'Ollama (Local LLM)',
        status: 'offline',
        port: 11434,
        error: 'Not running',
      });
    }

    setServices(updatedServices);
  }, [settings?.server?.port]);

  useEffect(() => {
    loadCurrentProjectPath();
  }, []);

  useEffect(() => {
    loadSettings();
  }, [scope]);

  // Check services on mount and when tab is server
  useEffect(() => {
    if (activeTab === 'server') {
      checkServiceStatus();
      // Refresh every 10 seconds while on server tab
      const interval = setInterval(checkServiceStatus, 10000);
      return () => clearInterval(interval);
    }
  }, [activeTab, checkServiceStatus]);

  const loadCurrentProjectPath = async () => {
    try {
      // TODO: Implement project path API endpoint
      setProjectPath(null);
    } catch (error) {
      console.error('Failed to get project path:', error);
    }
  };

  const loadSettings = async () => {
    setLoading(true);
    setMessage(null);

    try {
      // Load from localStorage for now (can be replaced with API endpoint)
      const savedSettings = localStorage.getItem('agentreplay_settings');

      const loaded: AgentReplaySettings = savedSettings ? JSON.parse(savedSettings) : {
        database: {
          max_traces: null,
          retention_days: null,
          auto_compact: true,
        },
        server: {
          port: 47100,
          enable_cors: true,
          max_payload_size_mb: 10,
        },
        ui: {
          theme: 'dark',
          animations_enabled: true,
          auto_refresh_interval_secs: 30,
          experimental_features: true,
        },
        analytics: {
          enabled: true,  // Enabled by default
        },
        models: {
          providers: [],
          defaultProviderId: null,
          defaultTemperature: 0.7,
          defaultMaxTokens: 2048,
        },
        embedding: {
          provider: 'fastembed',
          model: 'BAAI/bge-small-en-v1.5',
          dimensions: 384,
          apiKey: null,
          baseUrl: null,
          enabled: false,
          autoIndexNewTraces: true,
          batchSize: 32,
        },
      };

      // Ensure analytics settings exist (migration for existing users)
      if (!loaded.analytics) {
        loaded.analytics = { enabled: true };
      }

      // Migrate old settings format (apiKeys -> providers)
      if (!loaded.models) {
        loaded.models = {
          providers: [],
          defaultProviderId: null,
          defaultTemperature: 0.7,
          defaultMaxTokens: 2048,
        };
      } else if ('apiKeys' in loaded.models && !('providers' in loaded.models)) {
        // Migrate old apiKeys format to new providers format
        const oldApiKeys = (loaded.models as any).apiKeys || [];
        loaded.models = {
          providers: oldApiKeys.map((key: any, idx: number) => ({
            id: `migrated_${idx}_${Date.now()}`,
            name: key.provider,
            provider: key.provider.toLowerCase() as any,
            baseUrl: PROVIDER_DEFAULTS[key.provider.toLowerCase()]?.baseUrl || '',
            modelName: '',
            apiKey: key.apiKey,
            isDefault: idx === 0,
          })),
          defaultProviderId: oldApiKeys.length > 0 ? `migrated_0_${Date.now()}` : null,
          defaultTemperature: (loaded.models as any).defaultTemperature ?? 0.7,
          defaultMaxTokens: (loaded.models as any).defaultMaxTokens ?? 2048,
        };
      } else {
        // Ensure providers array exists even if settings were partially saved
        if (!loaded.models.providers) {
          loaded.models.providers = [];
        }
        if (loaded.models.defaultProviderId === undefined) {
          loaded.models.defaultProviderId = null;
        }
      }

      // Migrate/ensure embedding settings exist
      if (!loaded.embedding) {
        loaded.embedding = {
          provider: 'fastembed',
          model: 'BAAI/bge-small-en-v1.5',
          dimensions: 384,
          apiKey: null,
          baseUrl: null,
          enabled: false,
          autoIndexNewTraces: true,
          batchSize: 32,
        };
      }

      // Sync the loaded settings theme with the actual applied theme from localStorage
      // (theme is stored separately in 'agentreplay-theme' by applyTheme)
      const actualTheme = localStorage.getItem('agentreplay-theme') as 'light' | 'dark' | 'midnight' | null;
      if (actualTheme && loaded.ui) {
        loaded.ui.theme = actualTheme;
      }

      setSettings(loaded);
      setHasChanges(false);

      // Theme is already applied by initTheme() in App.tsx
      // Don't re-apply here to avoid flicker
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to load settings: ${error}`
      });
    } finally {
      setLoading(false);
    }
  };

  const saveSettings = async () => {
    if (!settings) return;

    setSaving(true);
    setMessage(null);

    try {
      // Save to localStorage
      localStorage.setItem('agentreplay_settings', JSON.stringify(settings));

      // Apply theme immediately
      applyTheme(settings.ui.theme);

      // Sync LLM settings to backend if models are configured
      if (settings.models?.providers && settings.models.providers.length > 0) {
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          await invoke('sync_llm_settings', { models: settings.models });
          console.log('LLM settings synced to backend');
        } catch (e) {
          console.warn('Failed to sync LLM settings (non-Tauri environment):', e);
        }
      }

      setMessage({ type: 'success', text: 'Settings saved successfully' });
      setHasChanges(false);

      // Auto-hide success message after 3 seconds
      setTimeout(() => setMessage(null), 3000);
    } catch (error) {
      setMessage({
        type: 'error',
        text: `Failed to save settings: ${error}`
      });
    } finally {
      setSaving(false);
    }
  };

  // Apply theme function - uses imported function from theme.ts
  const applyTheme = (theme: 'light' | 'dark' | 'midnight') => {
    localStorage.setItem('agentreplay-theme', theme);
    applyThemeFromLib(resolveTheme(theme));
  };

  const handleReset = () => {
    if (window.confirm('Are you sure you want to reset to default settings? This will clear all provider configurations.')) {
      // Clear localStorage and reinitialize with defaults
      localStorage.removeItem('agentreplay_settings');

      const defaultSettings: AgentReplaySettings = {
        database: {
          max_traces: null,
          retention_days: null,
          auto_compact: true,
        },
        server: {
          port: 47100,
          enable_cors: true,
          max_payload_size_mb: 10,
        },
        ui: {
          theme: 'dark',
          animations_enabled: true,
          auto_refresh_interval_secs: 30,
          experimental_features: true,
        },
        analytics: {
          enabled: true,  // Enabled by default
        },
        models: {
          providers: [],
          defaultProviderId: null,
          defaultTemperature: 0.7,
          defaultMaxTokens: 2048,
        },
        embedding: {
          provider: 'fastembed',
          model: 'BAAI/bge-small-en-v1.5',
          dimensions: 384,
          apiKey: null,
          baseUrl: null,
          enabled: false,
          autoIndexNewTraces: true,
          batchSize: 32,
        },
      };

      setSettings(defaultSettings);
      setHasChanges(true);
      setMessage({ type: 'success', text: 'Settings reset to defaults. Click "Save Settings" to apply.' });
    }
  };

  const updateSettings = (updates: Partial<AgentReplaySettings>) => {
    if (!settings) return;

    setSettings({ ...settings, ...updates });
    setHasChanges(true);
  };

  const updateDatabaseSettings = (updates: Partial<AgentReplaySettings['database']>) => {
    if (!settings) return;

    setSettings({
      ...settings,
      database: { ...settings.database, ...updates }
    });
    setHasChanges(true);
  };

  const updateServerSettings = (updates: Partial<AgentReplaySettings['server']>) => {
    if (!settings) return;

    setSettings({
      ...settings,
      server: { ...settings.server, ...updates }
    });
    setHasChanges(true);
  };

  const updateUiSettings = (updates: Partial<AgentReplaySettings['ui']>) => {
    if (!settings) return;

    const newUiSettings = { ...settings.ui, ...updates };

    // Apply theme immediately when changed
    if (updates.theme) {
      applyTheme(updates.theme);
    }

    setSettings({
      ...settings,
      ui: newUiSettings
    });
    setHasChanges(true);
  };

  const updateModelSettings = (updates: Partial<AgentReplaySettings['models']>) => {
    if (!settings) return;

    setSettings({
      ...settings,
      models: { ...settings.models, ...updates }
    });
    setHasChanges(true);
  };

  const generateProviderId = () => `provider_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;

  const addProvider = (providerType: 'openai' | 'anthropic' | 'ollama' | 'custom') => {
    if (!settings) return;

    const defaults = PROVIDER_DEFAULTS[providerType];
    const existingProviders = settings.models?.providers || [];

    // First provider gets 'default' tag automatically
    const isFirstProvider = existingProviders.length === 0;

    const newProvider: ProviderConfig = {
      id: generateProviderId(),
      name: providerType === 'custom' ? 'Custom Provider' : providerType.charAt(0).toUpperCase() + providerType.slice(1),
      provider: providerType,
      baseUrl: defaults.baseUrl,
      modelName: '',
      apiKey: '',
      isDefault: isFirstProvider,
      isValid: undefined,
      tags: isFirstProvider ? ['default'] : [],
    };

    updateModelSettings({
      providers: [...existingProviders, newProvider],
      defaultProviderId: isFirstProvider ? newProvider.id : settings.models?.defaultProviderId || null,
    });
  };

  const updateProvider = (id: string, updates: Partial<ProviderConfig>) => {
    if (!settings) return;

    const existingProviders = settings.models?.providers || [];
    const newProviders = existingProviders.map(p =>
      p.id === id ? { ...p, ...updates } : p
    );

    updateModelSettings({ providers: newProviders });
  };

  const removeProvider = (id: string) => {
    if (!settings) return;

    const existingProviders = settings.models?.providers || [];
    const newProviders = existingProviders.filter(p => p.id !== id);
    const wasDefault = settings.models?.defaultProviderId === id;

    updateModelSettings({
      providers: newProviders,
      defaultProviderId: wasDefault && newProviders.length > 0 ? newProviders[0].id : null,
    });
  };

  const setDefaultProvider = (id: string) => {
    if (!settings) return;
    updateModelSettings({ defaultProviderId: id });
  };

  const testProviderConnection = async (provider: ProviderConfig) => {
    // Mark as testing
    updateProvider(provider.id, { isValid: undefined });

    try {
      // Simple validation - just check if we have required fields
      if (!provider.baseUrl) {
        throw new Error('Base URL is required');
      }
      if (provider.provider !== 'ollama' && !provider.apiKey) {
        throw new Error('API key is required');
      }
      if (!provider.modelName) {
        throw new Error('Model name is required');
      }

      // In a real implementation, we'd call the backend to test the connection
      // For now, mark as valid if all fields are present
      updateProvider(provider.id, {
        isValid: true,
        lastValidated: new Date().toISOString()
      });
      setMessage({ type: 'success', text: `Provider "${provider.name}" configured successfully` });
    } catch (error) {
      updateProvider(provider.id, { isValid: false });
      setMessage({ type: 'error', text: `Configuration error: ${error}` });
    }
  };

  // System Calibration function - uses sysinfo crate for REAL system info
  const runCalibration = async () => {
    setIsCalibrating(true);
    setCalibrationError(null);
    setCalibrationResults(null);

    try {
      // === REAL SYSTEM METRICS via Tauri sysinfo state ===
      let cpuCores = navigator.hardwareConcurrency || 0;
      let totalMemoryGB = 0;
      let usedMemoryGB = 0;
      let platform = navigator.platform || 'Unknown';
      let osType = 'Unknown';
      let osVersion = 'Unknown';
      let arch = 'Unknown';
      let hostname = 'Unknown';
      let cpuBrand = 'Unknown';
      let cpuUsagePercent = 0;
      let memoryUsagePercent = 0;

      // Check if running in Tauri - check for both v1 and v2 globals
      const isTauri = typeof window !== 'undefined' &&
        ('__TAURI__' in window || '__TAURI_INTERNALS__' in window);
      console.log('[Calibration] isTauri:', isTauri,
        '__TAURI__:', '__TAURI__' in (window || {}),
        '__TAURI_INTERNALS__:', '__TAURI_INTERNALS__' in (window || {}));

      if (isTauri) {
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          console.log('[Calibration] Calling get_all_system_info...');

          // Call our new get_all_system_info command
          const sysInfo = await invoke<{
            hostname: string | null;
            os_name: string | null;
            os_version: string | null;
            kernel_version: string | null;
            arch: string | null;
            cpu: {
              core_count: number;
              brand: string;
              vendor_id: string;
              frequency_mhz: number;
              usage_percent: number;
              per_core_usage: number[];
            };
            memory: {
              total_bytes: number;
              used_bytes: number;
              available_bytes: number;
              usage_percent: number;
              total_gb: number;
              used_gb: number;
              available_gb: number;
            };
            swap: {
              total_bytes: number;
              used_bytes: number;
              free_bytes: number;
            };
          }>('get_all_system_info');

          console.log('[Calibration] sysInfo response:', JSON.stringify(sysInfo, null, 2));

          if (sysInfo) {
            // Real OS info - handle both null and empty strings
            hostname = sysInfo.hostname && sysInfo.hostname.trim() ? sysInfo.hostname : 'Unknown';
            osType = sysInfo.os_name && sysInfo.os_name.trim() ? sysInfo.os_name : 'Unknown';
            osVersion = sysInfo.os_version && sysInfo.os_version.trim() ? sysInfo.os_version : 'Unknown';
            arch = sysInfo.arch && sysInfo.arch.trim() ? sysInfo.arch : 'Unknown';
            platform = `${osType} ${osVersion} (${arch})`;

            // Real CPU info - handle empty brand string
            cpuCores = sysInfo.cpu.core_count || cpuCores;
            cpuBrand = sysInfo.cpu.brand && sysInfo.cpu.brand.trim() ? sysInfo.cpu.brand : 'Unknown';
            cpuUsagePercent = Math.round(sysInfo.cpu.usage_percent * 10) / 10;

            // Real Memory info (already in GB from Rust)
            totalMemoryGB = Math.round(sysInfo.memory.total_gb * 100) / 100;
            usedMemoryGB = Math.round(sysInfo.memory.used_gb * 100) / 100;
            memoryUsagePercent = Math.round(sysInfo.memory.usage_percent * 10) / 10;

            console.log('[Calibration] Parsed values:', {
              hostname, osType, osVersion, arch,
              totalMemoryGB, usedMemoryGB, cpuCores, cpuBrand
            });
          }
        } catch (invokeErr) {
          console.error('[Calibration] System info invoke FAILED:', invokeErr);
          // Log more details about the error
          if (invokeErr instanceof Error) {
            console.error('[Calibration] Error name:', invokeErr.name);
            console.error('[Calibration] Error message:', invokeErr.message);
            console.error('[Calibration] Error stack:', invokeErr.stack);
          }
        }
      }

      // Fallback: use browser APIs if Tauri didn't provide memory
      console.log('[Calibration] After Tauri call, totalMemoryGB:', totalMemoryGB);
      if (totalMemoryGB === 0) {
        console.log('[Calibration] Using browser fallback for memory');
        const deviceMemoryGB = (navigator as any).deviceMemory;
        if (deviceMemoryGB) {
          totalMemoryGB = deviceMemoryGB;
        } else if ((performance as any).memory) {
          const heapLimitGB = (performance as any).memory.jsHeapSizeLimit / (1024 * 1024 * 1024);
          totalMemoryGB = Math.round(heapLimitGB * 4);
        } else {
          totalMemoryGB = 8;
        }
      }

      // Get JS heap memory (Chrome/Chromium only)
      let jsHeapUsedMB = 0;
      let jsHeapTotalMB = 0;
      if ((performance as any).memory) {
        const memInfo = (performance as any).memory;
        jsHeapUsedMB = Math.round(memInfo.usedJSHeapSize / (1024 * 1024));
        jsHeapTotalMB = Math.round(memInfo.jsHeapSizeLimit / (1024 * 1024));
      }

      // === REAL BACKEND STORAGE STATS ===
      let currentTraceCount = 0;
      let currentStorageBytes = 0;
      let avgTraceSizeBytes = 0;
      let serverOnline = false;

      try {
        const storageRes = await axios.get(`${API_BASE_URL}/api/v1/storage/stats`, { timeout: 10000 });
        if (storageRes.data) {
          serverOnline = true;
          currentTraceCount = storageRes.data.total_traces || 0;
          currentStorageBytes = storageRes.data.storage_bytes || 0;
          avgTraceSizeBytes = storageRes.data.avg_trace_size_bytes || 0;
        }
      } catch (e) {
        console.log('Storage stats not available:', e);
      }

      // If no server stats, try health endpoint
      if (!serverOnline) {
        try {
          const healthRes = await axios.get(`${API_BASE_URL}/api/v1/health`, { timeout: 5000 });
          if (healthRes.data) {
            serverOnline = true;
            currentTraceCount = healthRes.data.storage?.total_edges || 0;
          }
        } catch (e) {
          console.log('Health endpoint not available');
        }
      }

      // === REAL I/O BENCHMARK ===
      const benchmarkResults = await performRealBenchmarks();

      // === CALCULATE REAL METRICS ===
      const currentStorageMB = currentStorageBytes / (1024 * 1024);
      const avgTraceSizeKB = avgTraceSizeBytes > 0
        ? avgTraceSizeBytes / 1024
        : (currentTraceCount > 0 && currentStorageMB > 0
          ? (currentStorageMB * 1024) / currentTraceCount
          : 2.5); // Default 2.5KB if no data

      // Memory available for traces (30% of total)
      const availableForTracesGB = totalMemoryGB * 0.3;

      // Calculate max traces based on REAL average size
      const maxTracesMemory = Math.floor((availableForTracesGB * 1024 * 1024) / avgTraceSizeKB);

      // Throughput based on REAL benchmark
      const tracesPerSecond = Math.floor((benchmarkResults.writeSpeedMBps * 1024) / avgTraceSizeKB);

      // Storage estimate
      const maxTraces = Math.min(maxTracesMemory, 5000000);
      const estimatedStorageGB = (maxTraces * avgTraceSizeKB) / (1024 * 1024);

      // Performance level based on REAL benchmarks
      let performanceLevel: 'low' | 'medium' | 'high' = 'medium';
      if (cpuCores >= 8 && benchmarkResults.writeSpeedMBps >= 50 && totalMemoryGB >= 16) {
        performanceLevel = 'high';
      } else if (cpuCores <= 2 || benchmarkResults.writeSpeedMBps < 10 || totalMemoryGB <= 4) {
        performanceLevel = 'low';
      }

      setCalibrationResults({
        systemInfo: {
          totalMemoryGB,
          usedMemoryGB,
          availableMemoryGB: Math.round(availableForTracesGB * 100) / 100,
          memoryUsagePercent,
          cpuCores,
          cpuBrand,
          cpuUsagePercent,
          platform,
          osType,
          osVersion,
          arch,
          hostname,
          currentTraceCount,
          currentStorageMB: Math.round(currentStorageMB * 100) / 100,
          avgTraceSizeKB: Math.round(avgTraceSizeKB * 100) / 100,
          jsHeapUsedMB,
          serverOnline,
          isTauriApp: isTauri,
        },
        recommendations: {
          maxTraces,
          estimatedStorageGB: Math.round(estimatedStorageGB * 100) / 100,
          memoryUsagePercent: Math.round(memoryUsagePercent),
          performanceLevel,
          tracesPerSecond,
        },
        benchmarks: {
          writeSpeedMBps: benchmarkResults.writeSpeedMBps,
          readSpeedMBps: benchmarkResults.readSpeedMBps,
          indexingTimeMs: benchmarkResults.totalTimeMs,
          opsPerSecond: benchmarkResults.opsPerSecond,
        },
      });

    } catch (error) {
      console.error('Calibration failed:', error);
      setCalibrationError(
        error instanceof Error
          ? error.message
          : 'Failed to run system calibration. Please check browser permissions.'
      );
    } finally {
      setIsCalibrating(false);
    }
  };

  // Real I/O benchmark using localStorage (synchronous storage)
  const performRealBenchmarks = async (): Promise<{
    writeSpeedMBps: number;
    readSpeedMBps: number;
    totalTimeMs: number;
    opsPerSecond: number;
  }> => {
    // Use realistic trace data size
    const iterations = 200;
    const traceData = {
      trace_id: 'bench_' + crypto.randomUUID(),
      span_id: crypto.randomUUID(),
      timestamp_us: Date.now() * 1000,
      duration_us: 1500000,
      model: 'gpt-4-turbo',
      provider: 'openai',
      tokens: { input: 150, output: 200 },
      cost: 0.0035,
      input: 'What is the capital of France? Please provide a detailed answer.',
      output: 'The capital of France is Paris. Paris is located in northern France.',
      metadata: { session_id: 'sess_123', user_id: 'user_456', environment: 'production' }
    };

    const dataString = JSON.stringify(traceData);
    const dataSizeBytes = new Blob([dataString]).size;
    const totalDataMB = (iterations * dataSizeBytes) / (1024 * 1024);

    // Clear any old benchmark data first
    for (let i = 0; i < iterations; i++) {
      localStorage.removeItem(`_ftcal_${i}`);
    }

    // WRITE benchmark - measure actual time
    const writeStart = performance.now();
    for (let i = 0; i < iterations; i++) {
      const uniqueData = { ...traceData, trace_id: `bench_${i}_${Date.now()}` };
      localStorage.setItem(`_ftcal_${i}`, JSON.stringify(uniqueData));
    }
    const writeEnd = performance.now();
    const writeTimeMs = writeEnd - writeStart;

    // READ benchmark - measure actual time
    const readStart = performance.now();
    const readResults: string[] = [];
    for (let i = 0; i < iterations; i++) {
      const data = localStorage.getItem(`_ftcal_${i}`);
      if (data) readResults.push(data);
    }
    const readEnd = performance.now();
    const readTimeMs = readEnd - readStart;

    // CLEANUP
    for (let i = 0; i < iterations; i++) {
      localStorage.removeItem(`_ftcal_${i}`);
    }

    const totalTimeMs = Math.round(writeTimeMs + readTimeMs);
    const writeSpeedMBps = writeTimeMs > 0 ? Math.round((totalDataMB / (writeTimeMs / 1000)) * 100) / 100 : 0;
    const readSpeedMBps = readTimeMs > 0 ? Math.round((totalDataMB / (readTimeMs / 1000)) * 100) / 100 : 0;
    const opsPerSecond = totalTimeMs > 0 ? Math.round((iterations * 2) / (totalTimeMs / 1000)) : 0;

    return { writeSpeedMBps, readSpeedMBps, totalTimeMs, opsPerSecond };
  };

  // Visibility state for API keys
  const [visibleKeys, setVisibleKeys] = useState<Set<number>>(new Set());

  const toggleKeyVisibility = (index: number) => {
    setVisibleKeys(prev => {
      const newSet = new Set(prev);
      if (newSet.has(index)) {
        newSet.delete(index);
      } else {
        newSet.add(index);
      }
      return newSet;
    });
  };

  if (loading && !settings) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 className="w-8 h-8 animate-spin text-blue-500" />
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col bg-background">
      {/* Header */}
      <div className="flex-none border-b border-border bg-card backdrop-blur-sm">
        <div className="px-5 pt-5 pb-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div>
                <h1 className="text-2xl font-bold text-foreground mb-1">Settings</h1>
                <p className="text-muted-foreground">Configure Agentreplay for your workflow</p>
              </div>
            </div>

            <div className="flex items-center gap-3">
              <VideoHelpButton pageId="settings" />
              <motion.button
                onClick={handleReset}
                disabled={saving || !hasChanges}
                whileHover={{ scale: 1.02 }}
                whileTap={{ scale: 0.98 }}
                className="px-4 py-2 bg-secondary text-secondary-foreground rounded-lg hover:bg-secondary/80 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
              >
                <RotateCcw className="w-4 h-4" />
                Reset
              </motion.button>

              <motion.button
                onClick={saveSettings}
                disabled={saving || !hasChanges}
                whileHover={{ scale: 1.02 }}
                whileTap={{ scale: 0.98 }}
                className="px-4 py-2 bg-blue-600 text-foreground rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
              >
                {saving ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Saving...
                  </>
                ) : (
                  <>
                    <Save className="w-4 h-4" />
                    Save Settings
                  </>
                )}
              </motion.button>
            </div>
          </div>

          {/* Message Banner */}
          <AnimatePresence>
            {message && (
              <motion.div
                initial={{ opacity: 0, y: -10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -10 }}
                className={`mt-4 p-3 rounded-lg flex items-center gap-2 ${message.type === 'success'
                  ? 'bg-green-100 dark:bg-green-900/50 border border-green-300 dark:border-green-700 text-green-800 dark:text-green-300'
                  : 'bg-red-100 dark:bg-red-900/50 border border-red-300 dark:border-red-700 text-red-800 dark:text-red-300'
                  }`}
              >
                {message.type === 'success' ? (
                  <CheckCircle className="w-5 h-5 flex-shrink-0" />
                ) : (
                  <AlertCircle className="w-5 h-5 flex-shrink-0" />
                )}
                <span className="text-sm">{message.text}</span>
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        {/* Scope Tabs */}
        <div className="flex gap-1 px-5 pb-4">
          {(['user', 'project', 'local'] as SettingsScope[]).map((s) => (
            <button
              key={s}
              onClick={() => setScope(s)}
              disabled={s !== 'user' && !projectPath}
              className={`px-4 py-2 rounded-lg flex items-center gap-2 transition-colors ${scope === s
                ? 'bg-blue-600 text-foreground'
                : 'bg-secondary text-muted-foreground hover:bg-secondary/80 hover:text-foreground'
                } ${s !== 'user' && !projectPath ? 'opacity-50 cursor-not-allowed' : ''}`}
            >
              {s === 'user' && <User className="w-4 h-4" />}
              {s === 'project' && <Folder className="w-4 h-4" />}
              {s === 'local' && <FolderOpen className="w-4 h-4" />}
              <span className="capitalize">{s}</span>
            </button>
          ))}
        </div>

        {/* Settings Category Tabs */}
        <div className="flex gap-1 px-5 pb-4 border-t border-border pt-4 flex-wrap">
          {[
            { id: 'database' as const, icon: Database, label: 'Database', color: 'purple' },
            { id: 'server' as const, icon: Server, label: 'Server', color: 'green' },
            { id: 'ui' as const, icon: Palette, label: 'Theme', color: 'pink' },
            { id: 'models' as const, icon: Cpu, label: 'Models', color: 'indigo' },
            { id: 'embedding' as const, icon: Layers, label: 'Embeddings', color: 'cyan' },
            { id: 'backup' as const, icon: HardDrive, label: 'Backup', color: 'orange' },
            { id: 'danger' as const, icon: AlertTriangle, label: 'Danger Zone', color: 'red' },
          ].map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`px-4 py-2 rounded-lg flex items-center gap-2 transition-colors ${activeTab === tab.id
                ? `bg-${tab.color}-600/20 text-${tab.color}-400 border border-${tab.color}-600/40`
                : 'bg-card text-muted-foreground hover:bg-secondary/80 hover:text-foreground border border-transparent'
                }`}
            >
              <tab.icon className="w-4 h-4" />
              <span>{tab.label}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Settings Content */}
      <div className="flex-1 overflow-y-auto px-5 py-5">
        {settings ? (
          <div>
            {/* Database Settings */}
            {activeTab === 'database' && (
              <div className="space-y-6">
                {/* Database Configuration */}
                <div className="bg-card rounded-lg border border-border p-6">
                  <div className="flex items-center gap-3 mb-4">
                    <Database className="w-6 h-6 text-purple-600 dark:text-purple-400" />
                    <h2 className="text-xl font-semibold text-foreground">Database Configuration</h2>
                  </div>

                  <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <div>
                      <label className="block text-sm font-medium text-muted-foreground mb-2">
                        Max Traces (0 for unlimited)
                      </label>
                      <input
                        type="number"
                        value={settings.database.max_traces || 0}
                        onChange={(e) => updateDatabaseSettings({
                          max_traces: e.target.value === '0' ? null : parseInt(e.target.value)
                        })}
                        className="w-full px-3 py-2 bg-input border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-blue-500"
                      />
                    </div>

                    <div>
                      <label className="block text-sm font-medium text-muted-foreground mb-2">
                        Retention Days (0 for keep forever)
                      </label>
                      <input
                        type="number"
                        value={settings.database.retention_days || 0}
                        onChange={(e) => updateDatabaseSettings({
                          retention_days: e.target.value === '0' ? null : parseInt(e.target.value)
                        })}
                        className="w-full px-3 py-2 bg-input border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-blue-500"
                      />
                    </div>
                  </div>

                  <label className="flex items-center gap-3 cursor-pointer mt-4">
                    <input
                      type="checkbox"
                      checked={settings.database.auto_compact}
                      onChange={(e) => updateDatabaseSettings({ auto_compact: e.target.checked })}
                      className="w-5 h-5 rounded border-border bg-background text-blue-600 focus:ring-blue-500"
                    />
                    <span className="text-sm text-muted-foreground">Enable Auto-Compaction</span>
                  </label>
                </div>

                {/* System Calibration - Auto-Configure */}
                <div className="bg-card rounded-lg border border-border p-6">
                  <div className="flex items-center gap-3 mb-4">
                    <Gauge className="w-6 h-6 text-teal-600 dark:text-teal-400" />
                    <h2 className="text-xl font-semibold text-foreground">Auto-Configure from System</h2>
                  </div>

                  <p className="text-sm text-muted-foreground mb-4">
                    Analyze your system resources and automatically configure optimal database limits.
                  </p>

                  {/* Run Calibration Button */}
                  <motion.button
                    whileHover={{ scale: 1.02 }}
                    whileTap={{ scale: 0.98 }}
                    onClick={runCalibration}
                    disabled={isCalibrating}
                    className={`w-full px-4 py-3 rounded-lg flex items-center justify-center gap-2 font-medium transition-colors ${isCalibrating
                      ? 'bg-teal-600/50 text-teal-200 cursor-not-allowed'
                      : 'bg-teal-600 text-foreground hover:bg-teal-700'
                      }`}
                  >
                    {isCalibrating ? (
                      <>
                        <Loader2 className="w-4 h-4 animate-spin" />
                        Analyzing System...
                      </>
                    ) : (
                      <>
                        <Gauge className="w-4 h-4" />
                        Run System Calibration
                      </>
                    )}
                  </motion.button>

                  {/* Error Display */}
                  {calibrationError && (
                    <div className="mt-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-700/50 rounded-lg p-3">
                      <div className="flex items-center gap-2 text-red-600 dark:text-red-400 text-sm">
                        <AlertCircle className="w-4 h-4" />
                        {calibrationError}
                      </div>
                    </div>
                  )}

                  {/* Calibration Results */}
                  {calibrationResults && (
                    <div className="mt-4 space-y-4">
                      {/* Real System Info from sysinfo crate */}
                      <div className="bg-card rounded-lg border border-border p-4">
                        <div className="flex items-center gap-2 mb-3">
                          <Cpu className="w-4 h-4 text-purple-600 dark:text-purple-400" />
                          <span className="text-sm font-medium text-muted-foreground">System Information</span>
                          {calibrationResults.systemInfo.isTauriApp && (
                            <span className="px-2 py-0.5 bg-teal-600/30 text-teal-600 dark:text-teal-400 text-xs rounded-full">
                              Native
                            </span>
                          )}
                        </div>

                        {/* OS & Host Info */}
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-2 mb-3 text-xs">
                          <div className="bg-card rounded px-2 py-1.5">
                            <span className="text-muted-foreground">OS: </span>
                            <span className="text-foreground">{calibrationResults.systemInfo.osType}</span>
                          </div>
                          <div className="bg-card rounded px-2 py-1.5">
                            <span className="text-muted-foreground">Version: </span>
                            <span className="text-foreground">{calibrationResults.systemInfo.osVersion}</span>
                          </div>
                          <div className="bg-card rounded px-2 py-1.5">
                            <span className="text-muted-foreground">Arch: </span>
                            <span className="text-foreground">{calibrationResults.systemInfo.arch}</span>
                          </div>
                          <div className="bg-card rounded px-2 py-1.5">
                            <span className="text-muted-foreground">Host: </span>
                            <span className="text-foreground">{calibrationResults.systemInfo.hostname}</span>
                          </div>
                        </div>

                        {/* CPU Info */}
                        <div className="bg-card/80 rounded-lg p-3 mb-3">
                          <p className="text-xs text-muted-foreground mb-1">CPU</p>
                          <p className="text-sm font-medium text-foreground">{calibrationResults.systemInfo.cpuBrand}</p>
                          <div className="flex items-center gap-4 mt-1">
                            <span className="text-xs text-muted-foreground">{calibrationResults.systemInfo.cpuCores} cores</span>
                            <span className="text-xs text-muted-foreground">Usage: {calibrationResults.systemInfo.cpuUsagePercent}%</span>
                          </div>
                        </div>

                        {/* Memory Info */}
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                          <div className="bg-card rounded-lg p-3 text-center">
                            <p className="text-xs text-muted-foreground mb-1">Total RAM</p>
                            <p className="text-lg font-bold text-foreground">{calibrationResults.systemInfo.totalMemoryGB} GB</p>
                          </div>
                          <div className="bg-card rounded-lg p-3 text-center">
                            <p className="text-xs text-muted-foreground mb-1">Used</p>
                            <p className="text-lg font-bold text-yellow-600 dark:text-yellow-400">{calibrationResults.systemInfo.usedMemoryGB} GB</p>
                          </div>
                          <div className="bg-card rounded-lg p-3 text-center">
                            <p className="text-xs text-muted-foreground mb-1">Available</p>
                            <p className="text-lg font-bold text-green-600 dark:text-green-400">{calibrationResults.systemInfo.availableMemoryGB} GB</p>
                          </div>
                          <div className="bg-card rounded-lg p-3 text-center">
                            <p className="text-xs text-muted-foreground mb-1">Memory Usage</p>
                            <p className={`text-lg font-bold ${calibrationResults.systemInfo.memoryUsagePercent > 80
                              ? 'text-red-600 dark:text-red-400'
                              : calibrationResults.systemInfo.memoryUsagePercent > 60
                                ? 'text-yellow-600 dark:text-yellow-400'
                                : 'text-green-600 dark:text-green-400'
                              }`}>{calibrationResults.systemInfo.memoryUsagePercent}%</p>
                          </div>
                        </div>
                      </div>

                      {/* I/O Benchmarks */}
                      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                        <div className="bg-card rounded-lg p-3 text-center">
                          <p className="text-xs text-muted-foreground mb-1">Write Speed</p>
                          <p className="text-lg font-bold text-foreground">{calibrationResults.benchmarks.writeSpeedMBps.toFixed(0)} MB/s</p>
                        </div>
                        <div className="bg-card rounded-lg p-3 text-center">
                          <p className="text-xs text-muted-foreground mb-1">Read Speed</p>
                          <p className="text-lg font-bold text-foreground">{calibrationResults.benchmarks.readSpeedMBps.toFixed(0)} MB/s</p>
                        </div>
                        <div className="bg-card rounded-lg p-3 text-center">
                          <p className="text-xs text-muted-foreground mb-1">IOPS</p>
                          <p className="text-lg font-bold text-foreground">{calibrationResults.benchmarks.opsPerSecond?.toLocaleString()}</p>
                        </div>
                        <div className="bg-card rounded-lg p-3 text-center">
                          <p className="text-xs text-muted-foreground mb-1">Performance</p>
                          <p className={`text-lg font-bold ${calibrationResults.recommendations.performanceLevel === 'high'
                            ? 'text-green-600 dark:text-green-400'
                            : calibrationResults.recommendations.performanceLevel === 'medium'
                              ? 'text-yellow-600 dark:text-yellow-400'
                              : 'text-orange-600 dark:text-orange-400'
                            }`}>
                            {calibrationResults.recommendations.performanceLevel.toUpperCase()}
                          </p>
                        </div>
                      </div>

                      {/* Current Storage Usage (if available) */}
                      {calibrationResults.systemInfo.serverOnline && calibrationResults.systemInfo.currentTraceCount !== undefined && (
                        <div className="bg-purple-50 dark:bg-purple-900/20 border border-purple-200 dark:border-purple-700/30 rounded-lg p-3">
                          <div className="flex items-center gap-2 mb-2">
                            <Database className="w-4 h-4 text-purple-600 dark:text-purple-400" />
                            <span className="text-sm font-medium text-purple-600 dark:text-purple-300">Current Storage</span>
                            <span className="px-2 py-0.5 bg-green-600/30 text-green-600 dark:text-green-400 text-xs rounded-full">
                              Server Online
                            </span>
                          </div>
                          <div className="grid grid-cols-3 gap-3 text-center">
                            <div>
                              <p className="text-xs text-muted-foreground">Traces</p>
                              <p className="text-sm font-bold text-purple-600 dark:text-purple-400">{calibrationResults.systemInfo.currentTraceCount?.toLocaleString()}</p>
                            </div>
                            <div>
                              <p className="text-xs text-muted-foreground">Storage</p>
                              <p className="text-sm font-bold text-purple-600 dark:text-purple-400">{calibrationResults.systemInfo.currentStorageMB?.toFixed(1)} MB</p>
                            </div>
                            <div>
                              <p className="text-xs text-muted-foreground">Avg Size</p>
                              <p className="text-sm font-bold text-purple-600 dark:text-purple-400">{calibrationResults.systemInfo.avgTraceSizeKB?.toFixed(1)} KB</p>
                            </div>
                          </div>
                        </div>
                      )}

                      {/* Recommendations */}
                      <div className="bg-teal-900/20 border border-teal-700/30 rounded-lg p-4">
                        <div className="flex items-center gap-2 mb-3">
                          <Zap className="w-4 h-4 text-teal-600 dark:text-teal-400" />
                          <span className="text-sm font-medium text-teal-300">Recommended Settings</span>
                        </div>
                        <div className="grid grid-cols-2 gap-4 mb-4">
                          <div>
                            <p className="text-xs text-muted-foreground mb-1">Max Traces</p>
                            <p className="text-xl font-bold text-teal-600 dark:text-teal-400">{calibrationResults.recommendations.maxTraces.toLocaleString()}</p>
                          </div>
                          <div>
                            <p className="text-xs text-muted-foreground mb-1">Est. Storage</p>
                            <p className="text-xl font-bold text-teal-600 dark:text-teal-400">{calibrationResults.recommendations.estimatedStorageGB.toFixed(1)} GB</p>
                          </div>
                        </div>
                        <motion.button
                          whileHover={{ scale: 1.02 }}
                          whileTap={{ scale: 0.98 }}
                          onClick={() => {
                            updateDatabaseSettings({
                              max_traces: calibrationResults.recommendations.maxTraces,
                              auto_compact: true,
                            });
                            setMessage({
                              type: 'success',
                              text: `Applied: Max ${calibrationResults.recommendations.maxTraces.toLocaleString()} traces with auto-compaction`
                            });
                          }}
                          className="w-full px-4 py-2 bg-teal-600 text-foreground rounded-lg hover:bg-teal-700 flex items-center justify-center gap-2 text-sm font-medium"
                        >
                          <CheckCircle className="w-4 h-4" />
                          Apply Recommended Settings
                        </motion.button>
                      </div>
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* Server Settings */}
            {activeTab === 'server' && (
              <div className="space-y-6">
                {/* Running Services Status */}
                <div className="bg-card rounded-lg border border-border p-6">
                  <div className="flex items-center justify-between mb-4">
                    <div className="flex items-center gap-3">
                      <Activity className="w-6 h-6 text-green-600 dark:text-green-400" />
                      <h2 className="text-xl font-semibold text-foreground">Running Services</h2>
                    </div>
                    <button
                      onClick={checkServiceStatus}
                      className="px-3 py-1.5 bg-secondary hover:bg-secondary text-muted-foreground rounded-lg flex items-center gap-2 text-sm transition-colors"
                    >
                      <RefreshCw className="w-4 h-4" />
                      Refresh
                    </button>
                  </div>

                  <div className="grid gap-3">
                    {services.map((service, idx) => (
                      <div
                        key={idx}
                        className={`p-4 rounded-lg border ${service.status === 'online'
                          ? 'bg-green-50 dark:bg-green-900/20 border-green-300 dark:border-green-700/50'
                          : service.status === 'checking'
                            ? 'bg-card border-border'
                            : 'bg-red-50 dark:bg-red-900/20 border-red-300 dark:border-red-700/50'
                          }`}
                      >
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-3">
                            {service.status === 'online' ? (
                              <Wifi className="w-5 h-5 text-green-600 dark:text-green-400" />
                            ) : service.status === 'checking' ? (
                              <Loader2 className="w-5 h-5 text-muted-foreground animate-spin" />
                            ) : (
                              <WifiOff className="w-5 h-5 text-red-600 dark:text-red-400" />
                            )}
                            <div>
                              <h3 className="font-medium text-foreground">{service.name}</h3>
                              <p className="text-sm text-muted-foreground">
                                Port {service.port}
                                {service.version && ` · v${service.version}`}
                                {service.uptime && ` · Uptime: ${service.uptime}`}
                              </p>
                            </div>
                          </div>
                          <span
                            className={`px-2 py-1 rounded text-xs font-medium ${service.status === 'online'
                              ? 'bg-green-100 dark:bg-green-600/30 text-green-700 dark:text-green-300'
                              : service.status === 'checking'
                                ? 'bg-gray-100 dark:bg-gray-600/30 text-gray-600 dark:text-muted-foreground'
                                : 'bg-red-100 dark:bg-red-600/30 text-red-700 dark:text-red-300'
                              }`}
                          >
                            {service.status === 'online' ? 'Online' : service.status === 'checking' ? 'Checking...' : 'Offline'}
                          </span>
                        </div>
                        {service.error && (
                          <p className="mt-2 text-sm text-red-600 dark:text-red-400">{service.error}</p>
                        )}
                      </div>
                    ))}
                  </div>
                </div>

                {/* Server Configuration */}
                <div className="bg-card rounded-lg border border-border p-6">
                  <div className="flex items-center gap-3 mb-4">
                    <Server className="w-6 h-6 text-green-600 dark:text-green-400" />
                    <h2 className="text-xl font-semibold text-foreground">Server Configuration</h2>
                  </div>

                  <div className="space-y-4">
                    <div>
                      <label className="block text-sm font-medium text-muted-foreground mb-2">
                        HTTP Port
                      </label>
                      <input
                        type="number"
                        value={settings.server.port}
                        onChange={(e) => updateServerSettings({ port: parseInt(e.target.value) })}
                        className="w-full px-3 py-2 bg-input border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-blue-500"
                      />
                    </div>

                    <div>
                      <label className="block text-sm font-medium text-muted-foreground mb-2">
                        Max Payload Size (MB)
                      </label>
                      <input
                        type="number"
                        value={settings.server.max_payload_size_mb}
                        onChange={(e) => updateServerSettings({ max_payload_size_mb: parseInt(e.target.value) })}
                        className="w-full px-3 py-2 bg-input border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-blue-500"
                      />
                    </div>

                    <label className="flex items-center gap-3 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={settings.server.enable_cors}
                        onChange={(e) => updateServerSettings({ enable_cors: e.target.checked })}
                        className="w-5 h-5 rounded border-border bg-background text-blue-600 focus:ring-blue-500"
                      />
                      <span className="text-sm text-muted-foreground">Enable CORS</span>
                    </label>
                  </div>
                </div>
              </div>
            )}

            {/* UI Settings */}
            {activeTab === 'ui' && (
              <div className="bg-card rounded-lg border border-border p-6">
                <div className="flex items-center gap-3 mb-4">
                  <Palette className="w-6 h-6 text-pink-600 dark:text-pink-400" />
                  <h2 className="text-xl font-semibold text-foreground">Theme & Appearance</h2>
                </div>

                <div className="space-y-4">
                  <div>
                    <h3 className="text-sm font-medium text-foreground mb-3">Appearance</h3>
                    <div className="grid grid-cols-3 gap-3">
                      {[
                        { id: 'dark' as const, label: 'Dark', icon: '🌙', desc: 'Deep gunmetal' },
                        { id: 'midnight' as const, label: 'Midnight', icon: '🖥️', desc: 'True black OLED' },
                        { id: 'light' as const, label: 'Light', icon: '☀️', desc: 'Clean & bright' },
                      ].map((theme) => (
                        <button
                          key={theme.id}
                          onClick={() => updateUiSettings({ theme: theme.id })}
                          className={`relative flex flex-col items-center justify-center gap-2 p-5 rounded-xl border-2 transition-all duration-200 ${settings.ui.theme === theme.id
                            ? 'border-primary bg-primary/5 ring-1 ring-primary/30 shadow-lg shadow-primary/10'
                            : 'border-border bg-card hover:border-muted-foreground/40 hover:bg-secondary/50'
                            }`}
                        >
                          <span className="text-2xl">{theme.icon}</span>
                          <span className={`text-sm font-medium ${settings.ui.theme === theme.id ? 'text-primary' : 'text-foreground'
                            }`}>
                            {theme.label}
                          </span>
                          {settings.ui.theme === theme.id && (
                            <div className="absolute top-2 right-2 w-2 h-2 rounded-full bg-primary" />
                          )}
                        </button>
                      ))}
                    </div>
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-muted-foreground mb-2">
                      Auto-Refresh Interval (seconds)
                    </label>
                    <input
                      type="number"
                      value={settings.ui.auto_refresh_interval_secs}
                      onChange={(e) => updateUiSettings({ auto_refresh_interval_secs: parseInt(e.target.value) })}
                      className="w-full px-3 py-2 bg-input border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-blue-500"
                    />
                  </div>

                  <label className="flex items-center gap-3 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={settings.ui.animations_enabled}
                      onChange={(e) => updateUiSettings({ animations_enabled: e.target.checked })}
                      className="w-5 h-5 rounded border-border bg-background text-blue-600 focus:ring-blue-500"
                    />
                    <span className="text-sm text-muted-foreground">Enable Animations</span>
                  </label>

                  <label className="flex items-center gap-3 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={settings.ui.experimental_features}
                      onChange={(e) => {
                        updateUiSettings({ experimental_features: e.target.checked });
                        if (e.target.checked) {
                          alert('Experimental features enabled. Please restart the application for all changes to take full effect.');
                        } else {
                          alert('Experimental features disabled. Please restart the application.');
                        }
                      }}
                      className="w-5 h-5 rounded border-border bg-background text-blue-600 focus:ring-blue-500"
                    />
                    <div>
                      <span className="text-sm text-muted-foreground">Experimental Features</span>
                      <p className="text-xs text-muted-foreground">Enable Tools, Plugins, Memory, Insights, and Storage features</p>
                    </div>
                  </label>
                </div>
              </div>
            )}

            {/* Privacy & Analytics Settings - shown after UI */}
            {activeTab === 'ui' && settings.analytics && (
              <div className="bg-card rounded-lg border border-border p-6 mt-6">
                <div className="flex items-center gap-3 mb-4">
                  <Activity className="w-6 h-6 text-green-600 dark:text-green-400" />
                  <h2 className="text-xl font-semibold text-foreground">Privacy & Analytics</h2>
                </div>

                <div className="space-y-4">
                  <div className="bg-card rounded-lg p-4 border border-border">
                    <label className="flex items-start gap-3 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={settings.analytics.enabled}
                        onChange={(e) => {
                          const newValue = e.target.checked;
                          setSettings({
                            ...settings,
                            analytics: { enabled: newValue }
                          });
                          setHasChanges(true);
                        }}
                        className="w-5 h-5 rounded border-border bg-background text-green-600 focus:ring-green-500 mt-0.5"
                      />
                      <div>
                        <span className="text-sm font-medium text-foreground">Share Anonymous Usage Data</span>
                        <p className="text-xs text-muted-foreground mt-1">
                          Help improve AgentReplay by sharing anonymous usage analytics.
                          No personal data, trace contents, or API keys are ever collected.
                        </p>
                      </div>
                    </label>
                  </div>

                  <div className="text-xs text-muted-foreground flex items-start gap-2">
                    <Info className="w-4 h-4 mt-0.5 flex-shrink-0" />
                    <span>
                      We collect only aggregated feature usage metrics (e.g., which features are used most).
                      This data helps us prioritize development. You can disable this anytime.
                    </span>
                  </div>
                </div>
              </div>
            )}

            {/* Model Configuration - OpenAI Compatible Provider Setup */}
            {activeTab === 'models' && (
              <div className="bg-card rounded-lg border border-border p-6">
                <div className="flex items-center gap-3 mb-4">
                  <Cpu className="w-6 h-6 text-indigo-600 dark:text-indigo-400" />
                  <h2 className="text-xl font-semibold text-foreground">LLM Provider Configuration</h2>
                </div>

                {/* Intro Banner */}
                <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-700/30 rounded-lg p-4 mb-6">
                  <h3 className="text-sm font-semibold text-blue-700 dark:text-blue-300 mb-2">📋 How to Configure</h3>
                  <ol className="text-sm text-blue-600 dark:text-blue-200/80 space-y-1 list-decimal list-inside">
                    <li>Click a provider button below (OpenAI, Anthropic, Ollama, or Custom)</li>
                    <li>Enter a <strong>Display Name</strong> to identify this config (e.g., "Production GPT-4")</li>
                    <li>Enter the exact <strong>Model Name</strong> from your provider (e.g., "gpt-4o", "claude-3-5-sonnet-20241022")</li>
                    <li>Set the <strong>API Endpoint</strong> (pre-filled for known providers)</li>
                    <li>Add your <strong>API Key</strong> (not needed for local Ollama)</li>
                  </ol>
                  <p className="text-xs text-blue-500 dark:text-blue-200/60 mt-2">
                    💡 You can add multiple configurations for different models or API endpoints.
                  </p>
                </div>

                <div className="space-y-6">
                  {/* Default Generation Settings */}
                  <div className="space-y-4">
                    <h3 className="text-sm font-medium text-muted-foreground uppercase tracking-wide">Default Generation Settings</h3>

                    <div className="grid grid-cols-2 gap-4">
                      <div>
                        <label className="block text-sm font-medium text-muted-foreground mb-2">
                          Temperature
                        </label>
                        <input
                          type="number"
                          step="0.1"
                          min="0"
                          max="2"
                          value={settings.models?.defaultTemperature ?? 0.7}
                          onChange={(e) => updateModelSettings({ defaultTemperature: parseFloat(e.target.value) })}
                          className="w-full px-3 py-2 bg-input border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-blue-500"
                        />
                        <p className="text-xs text-muted-foreground mt-1">0 = deterministic, 2 = creative</p>
                      </div>

                      <div>
                        <label className="block text-sm font-medium text-muted-foreground mb-2">
                          Max Tokens
                        </label>
                        <input
                          type="number"
                          min="1"
                          max="128000"
                          value={settings.models?.defaultMaxTokens ?? 2048}
                          onChange={(e) => updateModelSettings({ defaultMaxTokens: parseInt(e.target.value) })}
                          className="w-full px-3 py-2 bg-input border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-blue-500"
                        />
                        <p className="text-xs text-muted-foreground mt-1">Maximum tokens for response</p>
                      </div>
                    </div>
                  </div>

                  {/* Provider Configurations */}
                  <div className="space-y-4 border-t border-border pt-4">
                    <div className="flex items-center justify-between">
                      <h3 className="text-sm font-medium text-muted-foreground uppercase tracking-wide flex items-center gap-2">
                        <Key className="w-4 h-4" />
                        Configured Providers
                      </h3>
                      <div className="flex gap-2">
                        <button
                          onClick={() => addProvider('openai')}
                          className="px-3 py-1 text-xs bg-green-600/20 text-green-600 dark:text-green-400 border border-green-600/40 rounded hover:bg-green-600/30"
                        >
                          + OpenAI
                        </button>
                        <button
                          onClick={() => addProvider('anthropic')}
                          className="px-3 py-1 text-xs bg-purple-600/20 text-purple-600 dark:text-purple-400 border border-purple-600/40 rounded hover:bg-purple-600/30"
                        >
                          + Anthropic
                        </button>
                        <button
                          onClick={() => addProvider('ollama')}
                          className="px-3 py-1 text-xs bg-orange-600/20 text-orange-600 dark:text-orange-400 border border-orange-600/40 rounded hover:bg-orange-600/30"
                        >
                          + Ollama
                        </button>
                        <button
                          onClick={() => addProvider('custom')}
                          className="px-3 py-1 text-xs bg-secondary text-muted-foreground border border-border/80/40 rounded hover:bg-secondary/30"
                        >
                          + Custom
                        </button>
                      </div>
                    </div>

                    {settings.models?.providers?.length === 0 ? (
                      <div className="text-center py-8 border border-dashed border-border rounded-lg">
                        <Cpu className="w-8 h-8 text-muted-foreground/60 mx-auto mb-2" />
                        <p className="text-sm text-muted-foreground">No providers configured</p>
                        <p className="text-xs text-muted-foreground/60 mt-1">Add a provider to start using LLM features</p>
                      </div>
                    ) : (
                      <div className="space-y-4">
                        {settings.models?.providers?.map((provider) => (
                          <div
                            key={provider.id}
                            className={`p-4 bg-card rounded-lg border ${settings.models?.defaultProviderId === provider.id
                              ? 'border-blue-500/50 ring-1 ring-blue-500/30'
                              : 'border-border'
                              }`}
                          >
                            {/* Provider Header */}
                            <div className="flex items-center justify-between mb-4">
                              <div className="flex items-center gap-3">
                                <span className={`px-2 py-1 text-xs rounded font-medium ${provider.provider === 'openai' ? 'bg-green-600/20 text-green-600 dark:text-green-400' :
                                  provider.provider === 'anthropic' ? 'bg-purple-600/20 text-purple-600 dark:text-purple-400' :
                                    provider.provider === 'ollama' ? 'bg-orange-600/20 text-orange-600 dark:text-orange-400' :
                                      'bg-secondary text-muted-foreground'
                                  }`}>
                                  {provider.provider.toUpperCase()}
                                </span>
                                {settings.models?.defaultProviderId === provider.id && (
                                  <span className="px-2 py-0.5 text-xs bg-blue-600/20 text-blue-600 dark:text-blue-400 rounded">
                                    DEFAULT
                                  </span>
                                )}
                                {provider.isValid === true && (
                                  <span className="text-green-600 dark:text-green-400 text-xs">✓ Configured</span>
                                )}
                                {provider.isValid === false && (
                                  <span className="text-red-600 dark:text-red-400 text-xs">✗ Invalid</span>
                                )}
                              </div>
                              <div className="flex items-center gap-2">
                                {settings.models?.defaultProviderId !== provider.id && (
                                  <button
                                    onClick={() => setDefaultProvider(provider.id)}
                                    className="px-2 py-1 text-xs text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/20 rounded"
                                  >
                                    Set Default
                                  </button>
                                )}
                                <button
                                  onClick={() => testProviderConnection(provider)}
                                  className="px-2 py-1 text-xs text-muted-foreground hover:bg-secondary/80 rounded"
                                >
                                  Test
                                </button>
                                <button
                                  onClick={() => removeProvider(provider.id)}
                                  className="p-1 text-muted-foreground hover:text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded transition-colors"
                                >
                                  ×
                                </button>
                              </div>
                            </div>

                            {/* Provider Fields */}
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                              {/* Display Name */}
                              <div>
                                <label className="block text-xs font-medium text-muted-foreground mb-1">
                                  Display Name
                                </label>
                                <input
                                  type="text"
                                  value={provider.name}
                                  onChange={(e) => updateProvider(provider.id, { name: e.target.value })}
                                  placeholder="My Provider"
                                  className="w-full px-3 py-2 bg-input border border-border rounded text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                                />
                              </div>

                              {/* Model Name - FREEFORM INPUT */}
                              <div>
                                <label className="block text-xs font-medium text-muted-foreground mb-1">
                                  Model Name
                                </label>
                                <input
                                  type="text"
                                  value={provider.modelName}
                                  onChange={(e) => updateProvider(provider.id, { modelName: e.target.value })}
                                  placeholder={PROVIDER_DEFAULTS[provider.provider]?.placeholder || 'Enter model name...'}
                                  className="w-full px-3 py-2 bg-input border border-border rounded text-foreground text-sm font-mono focus:outline-none focus:ring-2 focus:ring-blue-500"
                                />
                              </div>

                              {/* Base URL / Endpoint */}
                              <div className="md:col-span-2">
                                <label className="block text-xs font-medium text-muted-foreground mb-1">
                                  API Endpoint (Base URL)
                                </label>
                                <input
                                  type="text"
                                  value={provider.baseUrl}
                                  onChange={(e) => updateProvider(provider.id, { baseUrl: e.target.value })}
                                  placeholder="https://api.example.com/v1"
                                  className="w-full px-3 py-2 bg-input border border-border rounded text-foreground text-sm font-mono focus:outline-none focus:ring-2 focus:ring-blue-500"
                                />
                                <p className="text-xs text-muted-foreground mt-1">
                                  OpenAI-compatible endpoint (e.g., /v1/chat/completions will be appended)
                                </p>
                              </div>

                              {/* API Key */}
                              <div className="md:col-span-2">
                                <label className="block text-xs font-medium text-muted-foreground mb-1">
                                  API Key {provider.provider === 'ollama' && <span className="text-muted-foreground">(optional for local)</span>}
                                </label>
                                <div className="relative">
                                  <input
                                    type={visibleKeys.has(settings.models.providers.indexOf(provider)) ? 'text' : 'password'}
                                    value={provider.apiKey}
                                    onChange={(e) => updateProvider(provider.id, { apiKey: e.target.value })}
                                    placeholder={provider.provider === 'ollama' ? 'Optional for local Ollama' : 'sk-...'}
                                    className="w-full px-3 py-2 pr-10 bg-input border border-border rounded text-foreground text-sm font-mono focus:outline-none focus:ring-2 focus:ring-blue-500"
                                  />
                                  <button
                                    onClick={() => toggleKeyVisibility(settings.models.providers.indexOf(provider))}
                                    className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-muted-foreground"
                                  >
                                    {visibleKeys.has(settings.models.providers.indexOf(provider)) ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                  </button>
                                </div>
                              </div>

                              {/* Tags / Purpose Routing */}
                              <div className="md:col-span-2">
                                <label className="block text-xs font-medium text-muted-foreground mb-2">
                                  Purpose Tags
                                  <span className="ml-2 text-muted-foreground font-normal">(each tag can only be assigned to one provider)</span>
                                </label>
                                <div className="flex flex-wrap gap-2">
                                  {PROVIDER_TAGS.map(tagInfo => {
                                    const isSelected = provider.tags?.includes(tagInfo.id) ?? (tagInfo.id === 'default');
                                    // Check if another provider already has this tag
                                    const otherProviderHasTag = settings.models.providers.some(
                                      p => p.id !== provider.id && p.tags?.includes(tagInfo.id)
                                    );
                                    const isDisabled = !isSelected && otherProviderHasTag;

                                    return (
                                      <button
                                        key={tagInfo.id}
                                        disabled={isDisabled}
                                        onClick={() => {
                                          if (isDisabled) return;
                                          const currentTags = provider.tags || ['default'];
                                          const newTags = isSelected
                                            ? currentTags.filter(t => t !== tagInfo.id)
                                            : [...currentTags, tagInfo.id];
                                          // Ensure at least 'default' if all tags removed
                                          updateProvider(provider.id, {
                                            tags: newTags.length > 0 ? newTags : ['default']
                                          });
                                        }}
                                        title={isDisabled
                                          ? `Already assigned to another provider`
                                          : tagInfo.description}
                                        className={`px-3 py-1.5 text-xs rounded-full border transition-all ${isSelected
                                          ? 'bg-blue-100 dark:bg-blue-600/30 border-blue-400 dark:border-blue-500 text-blue-700 dark:text-blue-300'
                                          : isDisabled
                                            ? 'bg-input border-border text-muted-foreground/60 cursor-not-allowed opacity-50'
                                            : 'bg-input border-border/80 text-muted-foreground hover:border-gray-500'
                                          }`}
                                      >
                                        {tagInfo.id === 'default' && '🎯 '}
                                        {tagInfo.id === 'eval' && '📊 '}
                                        {tagInfo.id === 'chat' && '💬 '}
                                        {tagInfo.id === 'analysis' && '🔍 '}
                                        {tagInfo.label}
                                        {isDisabled && ' ✓'}
                                      </button>
                                    );
                                  })}
                                </div>
                                <p className="text-xs text-muted-foreground mt-2">
                                  <strong>Default:</strong> General fallback • <strong>Eval:</strong> G-EVAL scoring • <strong>Chat:</strong> Conversations • <strong>Analysis:</strong> Trace analysis
                                </p>
                              </div>
                            </div>
                          </div>
                        ))}
                      </div>
                    )}

                    <div className="bg-card/80 rounded-lg p-4 border border-border/50">
                      <p className="text-xs text-muted-foreground">
                        🔒 All credentials are stored locally on your device and never sent to AgentReplay servers.
                      </p>
                      <p className="text-xs text-muted-foreground mt-2">
                        💡 <strong>Tip:</strong> Use any OpenAI-compatible API. Popular options include:
                      </p>
                      <ul className="text-xs text-muted-foreground mt-1 ml-4 list-disc">
                        <li>OpenAI: <code className="text-muted-foreground">https://api.openai.com/v1</code></li>
                        <li>Anthropic: <code className="text-muted-foreground">https://api.anthropic.com/v1</code></li>
                        <li>Ollama (local): <code className="text-muted-foreground">http://localhost:11434/v1</code></li>
                        <li>Together AI: <code className="text-muted-foreground">https://api.together.xyz/v1</code></li>
                        <li>Groq: <code className="text-muted-foreground">https://api.groq.com/openai/v1</code></li>
                        <li>OpenRouter: <code className="text-muted-foreground">https://openrouter.ai/api/v1</code></li>
                      </ul>
                    </div>
                  </div>
                </div>
              </div>
            )}

            {/* Embedding Configuration */}
            {activeTab === 'embedding' && (
              <div className="bg-card rounded-lg border border-border p-6">
                <div className="flex items-center gap-3 mb-4">
                  <Layers className="w-6 h-6 text-cyan-600 dark:text-cyan-400" />
                  <h2 className="text-xl font-semibold text-foreground">Embedding Configuration</h2>
                </div>

                {/* Intro Banner */}
                <div className="bg-cyan-50 dark:bg-cyan-900/20 border border-cyan-200 dark:border-cyan-700/30 rounded-lg p-4 mb-6">
                  <h3 className="text-sm font-semibold text-cyan-700 dark:text-cyan-300 mb-2">🔍 What are Embeddings?</h3>
                  <p className="text-sm text-cyan-600 dark:text-cyan-200/80">
                    Embeddings convert text into numerical vectors for semantic search. This enables finding traces
                    by meaning, not just keyword matching. Configure an embedding provider to enable semantic search
                    across your traces, prompts, and outputs.
                  </p>
                </div>

                <div className="space-y-6">
                  {/* Enable/Disable Toggle */}
                  <div className="flex items-center justify-between p-4 bg-card rounded-lg border border-border">
                    <div className="flex items-center gap-3">
                      <Zap className={`w-5 h-5 ${settings.embedding?.enabled ? 'text-cyan-600 dark:text-cyan-400' : 'text-muted-foreground'}`} />
                      <div>
                        <p className="text-sm font-medium text-foreground">Enable Embeddings</p>
                        <p className="text-xs text-muted-foreground">Turn on semantic indexing for traces</p>
                      </div>
                    </div>
                    <label className="relative inline-flex items-center cursor-pointer">
                      <input
                        type="checkbox"
                        checked={settings.embedding?.enabled ?? false}
                        onChange={(e) => setSettings({
                          ...settings,
                          embedding: { ...settings.embedding!, enabled: e.target.checked }
                        })}
                        className="sr-only peer"
                      />
                      <div className="w-11 h-6 bg-secondary peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-cyan-500/50 rounded-full peer peer-checked:after:translate-x-full after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-gray-400 after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-cyan-600 peer-checked:after:bg-white"></div>
                    </label>
                  </div>

                  {/* Provider Selection */}
                  <div className="space-y-4">
                    <h3 className="text-sm font-medium text-muted-foreground uppercase tracking-wide">Embedding Provider</h3>

                    <div className="grid grid-cols-4 gap-3">
                      {[
                        { id: 'fastembed', label: 'FastEmbed', desc: 'Local, free, fast', color: 'cyan' },
                        { id: 'openai', label: 'OpenAI', desc: 'text-embedding-3-*', color: 'green' },
                        { id: 'ollama', label: 'Ollama', desc: 'Local models', color: 'orange' },
                        { id: 'custom', label: 'Custom', desc: 'Any endpoint', color: 'gray' },
                      ].map((provider) => (
                        <button
                          key={provider.id}
                          onClick={() => {
                            const defaults: Record<string, { model: string; dimensions: number; baseUrl: string | null }> = {
                              fastembed: { model: 'BAAI/bge-small-en-v1.5', dimensions: 384, baseUrl: null },
                              openai: { model: 'text-embedding-3-small', dimensions: 1536, baseUrl: 'https://api.openai.com/v1' },
                              ollama: { model: 'nomic-embed-text', dimensions: 768, baseUrl: 'http://localhost:11434' },
                              custom: { model: '', dimensions: 768, baseUrl: '' },
                            };
                            const d = defaults[provider.id];
                            setSettings({
                              ...settings,
                              embedding: {
                                ...settings.embedding!,
                                provider: provider.id as 'openai' | 'ollama' | 'fastembed' | 'custom',
                                model: d.model,
                                dimensions: d.dimensions,
                                baseUrl: d.baseUrl,
                              }
                            });
                            setHasChanges(true);
                          }}
                          className={`p-3 rounded-lg border transition-all text-left ${settings.embedding?.provider === provider.id
                            ? `bg-${provider.color}-600/20 border-${provider.color}-500/50 ring-1 ring-${provider.color}-500/30`
                            : 'bg-card border-border hover:border-border/80'
                            }`}
                        >
                          <p className={`text-sm font-medium ${settings.embedding?.provider === provider.id ? `text-${provider.color}-400` : 'text-foreground'
                            }`}>{provider.label}</p>
                          <p className="text-xs text-muted-foreground">{provider.desc}</p>
                        </button>
                      ))}
                    </div>
                  </div>

                  {/* Model Configuration */}
                  <div className="space-y-4 border-t border-border pt-4">
                    <h3 className="text-sm font-medium text-muted-foreground uppercase tracking-wide">Model Configuration</h3>

                    <div className="grid grid-cols-2 gap-4">
                      <div>
                        <label className="block text-sm font-medium text-muted-foreground mb-2">
                          Model Name
                        </label>
                        <input
                          type="text"
                          value={settings.embedding?.model ?? ''}
                          onChange={(e) => {
                            setSettings({
                              ...settings,
                              embedding: { ...settings.embedding!, model: e.target.value }
                            });
                            setHasChanges(true);
                          }}
                          placeholder={
                            settings.embedding?.provider === 'fastembed' ? 'BAAI/bge-small-en-v1.5' :
                              settings.embedding?.provider === 'openai' ? 'text-embedding-3-small' :
                                settings.embedding?.provider === 'ollama' ? 'nomic-embed-text' :
                                  'Enter model name'
                          }
                          className="w-full px-3 py-2 bg-input border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-cyan-500"
                        />
                        <p className="text-xs text-muted-foreground mt-1">
                          {settings.embedding?.provider === 'fastembed' && 'Supports BAAI/bge-*, sentence-transformers/*'}
                          {settings.embedding?.provider === 'openai' && 'text-embedding-3-small (1536d) or text-embedding-3-large (3072d)'}
                          {settings.embedding?.provider === 'ollama' && 'nomic-embed-text, mxbai-embed-large, etc.'}
                        </p>
                      </div>

                      <div>
                        <label className="block text-sm font-medium text-muted-foreground mb-2">
                          Dimensions
                        </label>
                        <input
                          type="number"
                          value={settings.embedding?.dimensions ?? 768}
                          onChange={(e) => {
                            setSettings({
                              ...settings,
                              embedding: { ...settings.embedding!, dimensions: parseInt(e.target.value) }
                            });
                            setHasChanges(true);
                          }}
                          min="64"
                          max="4096"
                          className="w-full px-3 py-2 bg-input border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-cyan-500"
                        />
                        <p className="text-xs text-muted-foreground mt-1">Vector dimensions (must match model)</p>
                      </div>
                    </div>

                    {/* API Configuration - only show for non-fastembed */}
                    {settings.embedding?.provider !== 'fastembed' && (
                      <div className="grid grid-cols-2 gap-4 mt-4">
                        <div>
                          <label className="block text-sm font-medium text-muted-foreground mb-2">
                            API Endpoint
                          </label>
                          <input
                            type="text"
                            value={settings.embedding?.baseUrl ?? ''}
                            onChange={(e) => {
                              setSettings({
                                ...settings,
                                embedding: { ...settings.embedding!, baseUrl: e.target.value || null }
                              });
                              setHasChanges(true);
                            }}
                            placeholder={
                              settings.embedding?.provider === 'openai' ? 'https://api.openai.com/v1' :
                                settings.embedding?.provider === 'ollama' ? 'http://localhost:11434' :
                                  'https://your-api.com/v1'
                            }
                            className="w-full px-3 py-2 bg-input border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-cyan-500"
                          />
                        </div>

                        {settings.embedding?.provider !== 'ollama' && (
                          <div>
                            <label className="block text-sm font-medium text-muted-foreground mb-2">
                              API Key
                            </label>
                            <input
                              type="password"
                              value={settings.embedding?.apiKey ?? ''}
                              onChange={(e) => {
                                setSettings({
                                  ...settings,
                                  embedding: { ...settings.embedding!, apiKey: e.target.value || null }
                                });
                                setHasChanges(true);
                              }}
                              placeholder="sk-..."
                              className="w-full px-3 py-2 bg-input border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-cyan-500"
                            />
                            <p className="text-xs text-muted-foreground mt-1">Required for OpenAI and custom endpoints</p>
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  {/* Indexing Settings */}
                  <div className="space-y-4 border-t border-border pt-4">
                    <h3 className="text-sm font-medium text-muted-foreground uppercase tracking-wide">Indexing Settings</h3>

                    <div className="flex items-center justify-between p-4 bg-card rounded-lg border border-border">
                      <div>
                        <p className="text-sm font-medium text-foreground">Auto-index New Traces</p>
                        <p className="text-xs text-muted-foreground">Automatically create embeddings for new traces</p>
                      </div>
                      <label className="relative inline-flex items-center cursor-pointer">
                        <input
                          type="checkbox"
                          checked={settings.embedding?.autoIndexNewTraces ?? true}
                          onChange={(e) => {
                            setSettings({
                              ...settings,
                              embedding: { ...settings.embedding!, autoIndexNewTraces: e.target.checked }
                            });
                            setHasChanges(true);
                          }}
                          className="sr-only peer"
                        />
                        <div className="w-11 h-6 bg-secondary peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-cyan-500/50 rounded-full peer peer-checked:after:translate-x-full after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-gray-400 after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-cyan-600 peer-checked:after:bg-white"></div>
                      </label>
                    </div>

                    <div>
                      <label className="block text-sm font-medium text-muted-foreground mb-2">
                        Batch Size
                      </label>
                      <input
                        type="number"
                        value={settings.embedding?.batchSize ?? 32}
                        onChange={(e) => {
                          setSettings({
                            ...settings,
                            embedding: { ...settings.embedding!, batchSize: parseInt(e.target.value) }
                          });
                          setHasChanges(true);
                        }}
                        min="1"
                        max="256"
                        className="w-48 px-3 py-2 bg-input border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-cyan-500"
                      />
                      <p className="text-xs text-muted-foreground mt-1">Number of texts to embed in each batch (higher = faster, but more memory)</p>
                    </div>
                  </div>

                  {/* Recommended Models Info */}
                  <div className="bg-card rounded-lg border border-border p-4">
                    <h4 className="text-sm font-semibold text-muted-foreground mb-2">📚 Recommended Models</h4>
                    <ul className="text-xs text-muted-foreground space-y-1">
                      <li><span className="text-cyan-600 dark:text-cyan-400">FastEmbed (Local):</span> BAAI/bge-small-en-v1.5 (384d, fast), BAAI/bge-base-en-v1.5 (768d)</li>
                      <li><span className="text-green-600 dark:text-green-400">OpenAI:</span> text-embedding-3-small (1536d, $0.02/1M tokens), text-embedding-3-large (3072d)</li>
                      <li><span className="text-orange-500 dark:text-orange-400">Ollama:</span> nomic-embed-text (768d), mxbai-embed-large (1024d)</li>
                    </ul>
                  </div>
                </div>
              </div>
            )}

            {/* Backup & Restore */}
            {activeTab === 'backup' && (
              <>
                <div className="bg-card rounded-lg border border-border p-6">
                  <div className="flex items-center gap-3 mb-4">
                    <HardDrive className="w-6 h-6 text-orange-600 dark:text-orange-400" />
                    <h2 className="text-xl font-semibold text-foreground">Backup & Restore</h2>
                  </div>

                  <div className="space-y-4">
                    <p className="text-sm text-muted-foreground">
                      Create backups of your database to prevent data loss. Backups are stored in your app data directory.
                    </p>

                    <motion.button
                      whileHover={{ scale: 1.02 }}
                      whileTap={{ scale: 0.98 }}
                      onClick={async () => {
                        try {
                          setMessage({ type: 'success', text: 'Creating backup...' });
                          const result = await agentreplayClient.createBackup();
                          setMessage({ type: 'success', text: `Backup created: ${result.backup_id}` });
                        } catch (error) {
                          setMessage({ type: 'error', text: `Backup failed: ${error}` });
                        }
                      }}
                      className="w-full px-4 py-3 bg-green-600 text-foreground rounded-lg hover:bg-green-700 flex items-center justify-center gap-2"
                    >
                      <Download className="w-4 h-4" />
                      Create Backup
                    </motion.button>
                  </div>
                </div>

                {/* Backups List */}
                <BackupsList onMessage={setMessage} />

                {/* Updates */}
                <div className="bg-card rounded-lg border border-border p-6 mt-6">
                  <div className="flex items-center gap-3 mb-4">
                    <RefreshCw className="w-6 h-6 text-cyan-600 dark:text-cyan-400" />
                    <h2 className="text-xl font-semibold text-foreground">Updates</h2>
                  </div>

                  <div className="space-y-4">
                    <div className="flex items-center justify-between">
                      <div>
                        <p className="text-sm font-medium text-muted-foreground">Current Version</p>
                        <p className="text-xs text-muted-foreground mt-1">v0.1.0</p>
                      </div>

                      <motion.button
                        whileHover={{ scale: 1.02 }}
                        whileTap={{ scale: 0.98 }}
                        onClick={async () => {
                          try {
                            const updateInfo = await agentreplayClient.checkForUpdates();
                            if (updateInfo.available) {
                              setMessage({ type: 'success', text: `Update available: ${updateInfo.latest_version}` });
                            } else {
                              setMessage({ type: 'success', text: 'You are on the latest version!' });
                            }
                          } catch (error) {
                            setMessage({ type: 'error', text: `Update check failed: ${error}` });
                          }
                        }}
                        className="px-4 py-2 bg-cyan-600 text-white rounded-lg hover:bg-cyan-700 flex items-center gap-2"
                      >
                        <RefreshCw className="w-4 h-4" />
                        Check for Updates
                      </motion.button>
                    </div>

                    <p className="text-xs text-muted-foreground">
                      AgentReplay automatically checks for updates on startup. Click to manually check now.
                    </p>
                  </div>
                </div>
              </>
            )}

            {/* Danger Zone - Reset Data */}
            {activeTab === 'danger' && (
              <div className="space-y-6">
                <div className="bg-red-50 dark:bg-red-900/20 rounded-lg border border-red-200 dark:border-red-700/50 p-6">
                  <div className="flex items-center gap-3 mb-4">
                    <AlertTriangle className="w-6 h-6 text-red-600 dark:text-red-400" />
                    <h2 className="text-xl font-semibold text-foreground">Danger Zone</h2>
                  </div>

                  <p className="text-sm text-muted-foreground mb-6">
                    These actions are irreversible. Please proceed with caution.
                  </p>

                  {/* Delete All Data */}
                  <div className="bg-card rounded-lg border border-red-700/30 p-6">
                    <div className="flex items-center gap-3 mb-4">
                      <Trash2 className="w-5 h-5 text-red-600 dark:text-red-400" />
                      <h3 className="text-lg font-medium text-foreground">Delete All Data</h3>
                    </div>

                    <p className="text-sm text-muted-foreground mb-4">
                      This will permanently delete all traces, sessions, spans, and analytics data from the database.
                      This action cannot be undone. Make sure to create a backup first if you need to preserve any data.
                    </p>

                    <div className="space-y-4">
                      <div>
                        <label className="block text-sm font-medium text-muted-foreground mb-2">
                          Type <span className="text-red-600 dark:text-red-400 font-mono">DELETE ALL DATA</span> to confirm
                        </label>
                        <input
                          type="text"
                          value={resetConfirmation}
                          onChange={(e) => setResetConfirmation(e.target.value)}
                          placeholder="DELETE ALL DATA"
                          className="w-full px-3 py-2 bg-input border border-red-700/50 rounded-lg text-foreground placeholder-gray-600 focus:outline-none focus:ring-2 focus:ring-red-500"
                        />
                      </div>

                      <motion.button
                        whileHover={resetConfirmation === 'DELETE ALL DATA' ? { scale: 1.02 } : {}}
                        whileTap={resetConfirmation === 'DELETE ALL DATA' ? { scale: 0.98 } : {}}
                        disabled={resetConfirmation !== 'DELETE ALL DATA' || isResetting}
                        onClick={async () => {
                          if (resetConfirmation !== 'DELETE ALL DATA') return;

                          setIsResetting(true);
                          setMessage({ type: 'success', text: 'Deleting all data and restarting app...' });

                          try {
                            // This will delete all data and restart the app
                            // The app will restart, so we may not see the response
                            await agentreplayClient.resetAllData();

                            // If we get here (non-Tauri mode), show success
                            setMessage({ type: 'success', text: 'Data cleared. Please restart the app for a complete reset.' });
                            setResetConfirmation('');
                          } catch (error: any) {
                            setMessage({ type: 'error', text: `Failed to reset data: ${error.message}` });
                            setIsResetting(false);
                          }
                        }}
                        className={`w-full px-4 py-3 rounded-lg flex items-center justify-center gap-2 transition-colors ${resetConfirmation === 'DELETE ALL DATA' && !isResetting
                          ? 'bg-red-600 text-foreground hover:bg-red-700'
                          : 'bg-secondary text-muted-foreground cursor-not-allowed'
                          }`}
                      >
                        {isResetting ? (
                          <>
                            <Loader2 className="w-4 h-4 animate-spin" />
                            Deleting...
                          </>
                        ) : (
                          <>
                            <Trash2 className="w-4 h-4" />
                            Delete All Data
                          </>
                        )}
                      </motion.button>
                    </div>
                  </div>

                  {/* Clear Local Settings */}
                  <div className="bg-card rounded-lg border border-orange-700/30 p-6 mt-4">
                    <div className="flex items-center gap-3 mb-4">
                      <RotateCcw className="w-5 h-5 text-orange-600 dark:text-orange-400" />
                      <h3 className="text-lg font-medium text-foreground">Reset Local Settings</h3>
                    </div>

                    <p className="text-sm text-muted-foreground mb-4">
                      Reset all local settings (theme, preferences, cached data) to their default values.
                      This will not affect your traces or server data.
                    </p>

                    <motion.button
                      whileHover={{ scale: 1.02 }}
                      whileTap={{ scale: 0.98 }}
                      onClick={() => {
                        if (window.confirm('Are you sure you want to reset all local settings to defaults?')) {
                          localStorage.removeItem('agentreplay_settings');
                          localStorage.removeItem('agentreplay_project_id');
                          setMessage({ type: 'success', text: 'Local settings have been reset to defaults.' });
                          // Reload to apply default settings
                          setTimeout(() => window.location.reload(), 1500);
                        }
                      }}
                      className="px-4 py-2 bg-orange-600 text-foreground rounded-lg hover:bg-orange-700 flex items-center gap-2"
                    >
                      <RotateCcw className="w-4 h-4" />
                      Reset Local Settings
                    </motion.button>
                  </div>
                </div>
              </div>
            )}

            {/* Scope Information */}
            <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-700/50 rounded-lg p-4 mt-6">
              <p className="text-sm text-blue-700 dark:text-blue-300">
                <strong>Current Scope: {scope}</strong>
                <br />
                {scope === 'user' && '~/.agentreplay/settings.json - Applies to all AgentReplay instances'}
                {scope === 'project' && `${projectPath}/.agentreplay/settings.json - Project-specific settings`}
                {scope === 'local' && `${projectPath}/.agentreplay/settings.local.json - Local overrides (not committed)`}
              </p>
            </div>
          </div>
        ) : (
          <div className="text-center py-12 text-muted-foreground">
            No settings loaded
          </div>
        )}
      </div>
    </div>
  );
}
