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

'use client';

import { useEffect, useState } from 'react';
import { Activity, AlertCircle, Wifi, WifiOff, Terminal, X, ChevronDown, ChevronUp } from 'lucide-react';

// Determine the API base URL based on environment
function getApiBaseUrl(): string {
  const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
  if (isTauri) return 'http://127.0.0.1:9600';
  if (typeof window !== 'undefined' && window.location.port === '5173') return '';
  return 'http://127.0.0.1:9600';
}
const API_BASE_URL = getApiBaseUrl();

interface ServerStatusProps {
  compact?: boolean;
  asTopBanner?: boolean;
}

export default function ServerStatus({ compact = false, asTopBanner = false }: ServerStatusProps) {
  const [status, setStatus] = useState<'checking' | 'online' | 'offline'>('checking');
  const [lastCheck, setLastCheck] = useState<Date | null>(null);
  const [showTroubleshooting, setShowTroubleshooting] = useState(false);

  const checkHealth = async () => {
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/health`, {
        signal: AbortSignal.timeout(3000),
      });
      
      if (response.ok) {
        setStatus('online');
        setLastCheck(new Date());
      } else {
        setStatus('offline');
      }
    } catch (error) {
      setStatus('offline');
    }
  };

  useEffect(() => {
    // Initial check
    checkHealth();

    // Check every 10 seconds
    const interval = setInterval(checkHealth, 10000);

    return () => clearInterval(interval);
  }, []);

  // Debug logging
  useEffect(() => {
    console.log('ServerStatus render:', { status, asTopBanner, compact });
  }, [status, asTopBanner, compact]);

  // Top banner mode - only shows when offline or checking
  if (asTopBanner) {
    console.log('Top banner mode:', status);
    if (status === 'online') {
      return null; // Don't show banner when everything is working
    }

    return (
      <div className={`w-full border-b ${
        status === 'offline'
          ? 'bg-red-500/10 border-red-500/30'
          : 'bg-yellow-500/10 border-yellow-500/30'
      }`}>
        <div className="container mx-auto px-4 py-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              {status === 'offline' ? (
                <WifiOff className="w-5 h-5 text-red-500" />
              ) : (
                <Activity className="w-5 h-5 text-yellow-500 animate-pulse" />
              )}
              
              <div>
                <div className={`font-medium ${
                  status === 'offline' ? 'text-red-500' : 'text-yellow-500'
                }`}>
                  {status === 'offline' ? '⚠️ Server Offline' : '🔄 Checking Server...'}
                </div>
                <div className="text-xs text-textTertiary">
                  {status === 'offline' 
                    ? 'Unable to connect to FlowTrace backend'
                    : 'Verifying server health...'
                  }
                </div>
              </div>
            </div>

            <div className="flex items-center gap-2">
              {status === 'offline' && (
                <>
                  <button
                    onClick={() => setShowTroubleshooting(!showTroubleshooting)}
                    className="flex items-center gap-2 px-3 py-1.5 text-sm bg-red-500/20 hover:bg-red-500/30 text-red-500 rounded-lg transition-colors"
                  >
                    <Terminal className="w-4 h-4" />
                    Troubleshoot
                    {showTroubleshooting ? (
                      <ChevronUp className="w-4 h-4" />
                    ) : (
                      <ChevronDown className="w-4 h-4" />
                    )}
                  </button>
                  <button
                    onClick={checkHealth}
                    className="px-3 py-1.5 text-sm bg-red-500/20 hover:bg-red-500/30 text-red-500 rounded-lg transition-colors"
                  >
                    Retry
                  </button>
                </>
              )}
            </div>
          </div>

          {/* Troubleshooting Panel */}
          {showTroubleshooting && status === 'offline' && (
            <div className="mt-4 p-4 bg-surface rounded-lg border border-border">
              <div className="flex items-start justify-between mb-3">
                <div className="flex items-center gap-2">
                  <Terminal className="w-5 h-5 text-primary" />
                  <h3 className="font-semibold text-textPrimary">How to Start the Server</h3>
                </div>
                <button
                  onClick={() => setShowTroubleshooting(false)}
                  className="text-textTertiary hover:text-textPrimary"
                >
                  <X className="w-5 h-5" />
                </button>
              </div>

              <div className="space-y-4">
                <div>
                  <p className="text-sm text-textSecondary mb-2">
                    <strong className="text-textPrimary">Step 1:</strong> Navigate to the FlowTrace directory
                  </p>
                  <pre className="bg-background border border-border rounded p-3 text-xs overflow-x-auto">
                    <code className="text-green-400">cd flowtrace</code>
                  </pre>
                </div>

                <div>
                  <p className="text-sm text-textSecondary mb-2">
                    <strong className="text-textPrimary">Step 2:</strong> Start the server
                  </p>
                  <pre className="bg-background border border-border rounded p-3 text-xs overflow-x-auto">
                    <code className="text-green-400">cargo run --bin flowtrace-server</code>
                  </pre>
                </div>

                <div>
                  <p className="text-sm text-textSecondary mb-2">
                    <strong className="text-textPrimary">Alternative:</strong> Use the pre-built binary (faster)
                  </p>
                  <pre className="bg-background border border-border rounded p-3 text-xs overflow-x-auto">
                    <code className="text-green-400">./target/debug/flowtrace-server --config flowtrace-server-config.toml</code>
                  </pre>
                </div>

                <div className="pt-3 border-t border-border-subtle">
                  <p className="text-sm text-textSecondary mb-2">
                    <strong className="text-textPrimary">Verify server is running:</strong>
                  </p>
                  <pre className="bg-background border border-border rounded p-3 text-xs overflow-x-auto">
                    <code className="text-green-400">curl http://127.0.0.1:9600/api/v1/health</code>
                  </pre>
                </div>

                <div className="flex items-start gap-2 p-3 bg-blue-500/10 border border-blue-500/20 rounded-lg">
                  <AlertCircle className="w-4 h-4 text-blue-500 mt-0.5 flex-shrink-0" />
                  <div className="text-xs text-blue-400">
                    <strong>Tip:</strong> The server runs on port 9600 by default. 
                    Make sure nothing else is using this port. To check: 
                    <code className="ml-1 px-1 py-0.5 bg-background rounded">lsof -i :9600</code>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    );
  }

  if (compact) {
    return (
      <div className={`flex items-center gap-2 px-3 py-1.5 rounded-lg border ${
        status === 'online' 
          ? 'bg-green-500/10 border-green-500/20'
          : status === 'offline'
          ? 'bg-red-500/10 border-red-500/20'
          : 'bg-yellow-500/10 border-yellow-500/20'
      }`}>
        <div className={`w-2 h-2 rounded-full ${
          status === 'online'
            ? 'bg-green-500 animate-pulse'
            : status === 'offline'
            ? 'bg-red-500'
            : 'bg-yellow-500 animate-pulse'
        }`} />
        <span className={`text-sm font-medium ${
          status === 'online'
            ? 'text-green-500'
            : status === 'offline'
            ? 'text-red-500'
            : 'text-yellow-500'
        }`}>
          {status === 'online' ? 'Server Online' : status === 'offline' ? 'Server Offline' : 'Checking...'}
        </span>
      </div>
    );
  }

  return (
    <div className={`flex items-center gap-3 px-4 py-3 rounded-lg border ${
      status === 'online' 
        ? 'bg-green-500/5 border-green-500/20'
        : status === 'offline'
        ? 'bg-red-500/5 border-red-500/20'
        : 'bg-yellow-500/5 border-yellow-500/20'
    }`}>
      {status === 'online' ? (
        <Wifi className="w-5 h-5 text-green-500" />
      ) : status === 'offline' ? (
        <WifiOff className="w-5 h-5 text-red-500" />
      ) : (
        <Activity className="w-5 h-5 text-yellow-500 animate-pulse" />
      )}
      
      <div className="flex-1">
        <div className={`font-medium ${
          status === 'online'
            ? 'text-green-500'
            : status === 'offline'
            ? 'text-red-500'
            : 'text-yellow-500'
        }`}>
          {status === 'online' ? 'Server Online' : status === 'offline' ? 'Server Offline' : 'Checking Server...'}
        </div>
        {lastCheck && status === 'online' && (
          <div className="text-xs text-textTertiary">
            Last checked: {lastCheck.toLocaleTimeString()}
          </div>
        )}
        {status === 'offline' && (
          <div className="text-xs text-red-400">
            Unable to reach FlowTrace backend
          </div>
        )}
      </div>

      {status === 'offline' && (
        <button
          onClick={checkHealth}
          className="px-3 py-1.5 text-sm bg-red-500/20 hover:bg-red-500/30 text-red-500 rounded-lg transition-colors"
        >
          Retry
        </button>
      )}
    </div>
  );
}
