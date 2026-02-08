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

import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import SetupWizard from '../../components/SetupWizard';
import { AlertCircle, CheckCircle2, Loader2 } from 'lucide-react';
import { agentreplayClient, API_BASE_URL } from '../lib/agentreplay-api';

export default function Home() {
  const navigate = useNavigate();
  const [showSetup, setShowSetup] = useState(false);
  const [isChecking, setIsChecking] = useState(true);
  const [serviceStatus, setServiceStatus] = useState<'checking' | 'online' | 'offline'>('checking');
  const [serviceError, setServiceError] = useState<string | null>(null);

  useEffect(() => {
    // Check if setup is complete first
    const setupComplete = localStorage.getItem('agentreplay_setup_complete');
    const defaultProject = localStorage.getItem('agentreplay_default_project');

    if (setupComplete === 'true' && defaultProject) {
      // Setup complete, redirect to traces immediately
      navigate('/traces');
      return;
    }

    // If not complete, check if server has projects (e.g. Claude Code auto-created)
    const checkAndRedirect = async () => {
      // Wait for server to be ready (it may still be starting)
      let serverReady = false;
      for (let i = 0; i < 40; i++) {
        try {
          const health = await fetch(`${API_BASE_URL}/api/v1/health`, {
            signal: AbortSignal.timeout(2000),
          });
          if (health.ok) {
            serverReady = true;
            setServiceStatus('online');
            break;
          }
        } catch {
          // Server not ready yet
        }
        await new Promise(resolve => setTimeout(resolve, 500));
      }

      if (!serverReady) {
        setServiceStatus('offline');
        setServiceError('Server did not start in time');
        setShowSetup(true);
        setIsChecking(false);
        return;
      }

      try {
        // Check if projects already exist (Claude Code is auto-created)
        const response = await fetch(`${API_BASE_URL}/api/v1/projects`);
        if (response.ok) {
          const data = await response.json();
          if (data.projects && data.projects.length > 0) {
            // Projects exist - mark setup complete and skip wizard
            localStorage.setItem('agentreplay_setup_complete', 'true');
            localStorage.setItem('agentreplay_default_project', data.projects[0].project_id.toString());
            navigate('/traces');
            return;
          }
        }
      } catch (error) {
        console.error('Project check failed:', error);
      }

      // No projects found, show setup wizard
      setShowSetup(true);
      setIsChecking(false);
    };

    checkAndRedirect();
  }, [navigate]);

  const handleSetupComplete = (projectId: number) => {
    // This will be handled by the SetupWizard component
    // which redirects to /traces
  };

  if (isChecking || serviceStatus === 'checking') {
    return (
      <div className="flex items-center justify-center min-h-screen bg-background">
        <div className="text-center">
          <Loader2 className="w-12 h-12 text-primary animate-spin mx-auto mb-4" />
          <h1 className="text-4xl font-bold text-textPrimary mb-4">AgentReplay</h1>
          <p className="text-textSecondary">Checking service status...</p>
        </div>
      </div>
    );
  }

  if (serviceStatus === 'offline') {
    return (
      <div className="flex items-center justify-center min-h-screen bg-background">
        <div className="text-center max-w-md">
          <AlertCircle className="w-16 h-16 text-red-500 mx-auto mb-4" />
          <h1 className="text-4xl font-bold text-textPrimary mb-4">Service Offline</h1>
          <p className="text-textSecondary mb-4">
            Unable to connect to AgentReplay backend
          </p>
          {serviceError && (
            <p className="text-sm text-red-400 mb-6">
              Error: {serviceError}
            </p>
          )}
          <button
            onClick={() => window.location.reload()}
            className="mt-4 w-full px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg transition-colors"
          >
            Retry Connection
          </button>
        </div>
      </div>
    );
  }

  if (showSetup) {
    return (
      <div>
        <div className="flex items-center gap-2 px-4 py-2 bg-green-500/10 border-b border-green-500/20">
          <CheckCircle2 className="w-4 h-4 text-green-500" />
          <span className="text-sm text-green-500">Service Online</span>
        </div>
        <SetupWizard onComplete={handleSetupComplete} />
      </div>
    );
  }

  return null;
}
