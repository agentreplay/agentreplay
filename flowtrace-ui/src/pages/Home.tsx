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
import { flowtraceClient } from '../lib/flowtrace-api';

export default function Home() {
  const navigate = useNavigate();
  const [showSetup, setShowSetup] = useState(false);
  const [isChecking, setIsChecking] = useState(true);
  const [serviceStatus, setServiceStatus] = useState<'checking' | 'online' | 'offline'>('checking');
  const [serviceError, setServiceError] = useState<string | null>(null);

  useEffect(() => {
    // Check if setup is complete first
    const setupComplete = localStorage.getItem('flowtrace_setup_complete');
    const defaultProject = localStorage.getItem('flowtrace_default_project');

    if (setupComplete === 'true' && defaultProject) {
      // Setup complete, redirect to traces immediately
      navigate('/traces');
      return;
    }

    // If not complete, check service health then show setup
    const checkServiceHealth = async () => {
      try {
        await flowtraceClient.healthCheck();
        setServiceStatus('online');
      } catch (error) {
        setServiceStatus('offline');
        setServiceError(error instanceof Error ? error.message : 'Connection failed');
        console.error('Health check failed:', error);
      } finally {
        setShowSetup(true);
        setIsChecking(false);
      }
    };

    checkServiceHealth();
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
          <h1 className="text-4xl font-bold text-textPrimary mb-4">FlowTrace</h1>
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
            Unable to connect to FlowTrace backend
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
