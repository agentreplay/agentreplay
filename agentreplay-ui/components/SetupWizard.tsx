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

import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { motion, AnimatePresence } from 'framer-motion';
import { EnvironmentConfig } from './EnvironmentConfig';
import { API_BASE_URL } from '../src/lib/agentreplay-api';
import {
  Check,
  Copy,
  Download,
  FolderPlus,
  Settings,
  ChevronRight,
  Terminal,
  CheckCircle2,
  Loader2
} from 'lucide-react';

interface SetupWizardProps {
  onComplete: (projectId: number) => void;
}

type SdkType = 'python' | 'typescript' | 'go' | 'rust';
type UsageContext = 'observability' | 'memory' | 'claude'; // New Context Type

const SDKS: Record<SdkType, {
  label: string;
  installCmd: string;
  installLabel: string;
  verifyCode: (projectId: number) => string;
}> = {
  python: {
    label: 'Python',
    installCmd: 'pip install agentreplay-client && agentreplay-install',
    installLabel: 'Install via pip',
    verifyCode: (projectId) => `import os
from agentreplay_client import AgentreplayClient

# Initialize client
client = AgentreplayClient(
    url=os.getenv("AGENTREPLAY_URL", "http://127.0.0.1:9600"),
    tenant_id=int(os.getenv("AGENTREPLAY_TENANT_ID", "1")),
    project_id=int(os.getenv("AGENTREPLAY_PROJECT_ID", "${projectId}")),
)

# Create a test trace
trace = client.create_trace(
    agent_id=1,
    payload={"test": "Hello Agentreplay!"}
)

print(f"✅ Setup successful! Trace ID: {trace['edge_id']}")
print(f"🚀 Visit http://localhost:5173/traces to see your data")`
  },
  typescript: {
    label: 'TypeScript',
    installCmd: 'npm install agentreplay-client',
    installLabel: 'Install via npm',
    verifyCode: (projectId) => `import { AgentreplayClient } from 'agentreplay-client';

// Initialize client
const client = new AgentreplayClient({
  url: process.env.AGENTREPLAY_URL || 'http://127.0.0.1:9600',
  tenantId: parseInt(process.env.AGENTREPLAY_TENANT_ID || '1'),
  projectId: parseInt(process.env.AGENTREPLAY_PROJECT_ID || '${projectId}'),
});

// Create a test trace
const trace = await client.createTrace({
  agentId: 1,
  payload: { test: 'Hello Agentreplay!' }
});

console.log(\`✅ Setup successful! Trace ID: \${trace.edge_id}\`);
console.log(\`🚀 Visit http://localhost:5173/traces to see your data\`);`
  },
  go: {
    label: 'Go',
    installCmd: 'go get github.com/sushanthpy/agentreplay-go',
    installLabel: 'Install via go get',
    verifyCode: (projectId) => `package main

import (
	"fmt"
	"os"
	"strconv"

	"github.com/sushanthpy/agentreplay-go"
)

func main() {
	// Initialize client
	client := agentreplay.NewClient(
		os.Getenv("AGENTREPLAY_URL"),
		getEnvInt("AGENTREPLAY_TENANT_ID", 1),
		getEnvInt("AGENTREPLAY_PROJECT_ID", ${projectId}),
	)

	// Create a test trace
	trace, _ := client.CreateTrace(1, map[string]interface{}{
		"test": "Hello Agentreplay!",
	})

	fmt.Printf("✅ Setup successful! Trace ID: %s\\n", trace.EdgeID)
	fmt.Println("🚀 Visit http://localhost:5173/traces to see your data")
}

func getEnvInt(key string, defaultVal int) int {
	if val, ok := os.LookupEnv(key); ok {
		if i, err := strconv.Atoi(val); err == nil {
			return i
		}
	}
	return defaultVal
}`
  },
  rust: {
    label: 'Rust',
    installCmd: 'cargo add agentreplay-client',
    installLabel: 'Install via cargo',
    verifyCode: (projectId) => `use agentreplay_client::AgentreplayClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize client
    let client = AgentreplayClient::new(
        env::var("AGENTREPLAY_URL").unwrap_or("http://127.0.0.1:9600".to_string()),
        env::var("AGENTREPLAY_TENANT_ID").unwrap_or("1".to_string()).parse()?,
        env::var("AGENTREPLAY_PROJECT_ID").unwrap_or("${projectId}".to_string()).parse()?,
    );

    // Create a test trace
    let trace = client.create_trace(
        1,
        serde_json::json!({"test": "Hello Agentreplay!"})
    ).await?;

    println!("✅ Setup successful! Trace ID: {}", trace.edge_id);
    println!("🚀 Visit http://localhost:5173/traces to see your data");
    Ok(())
}
`
  }
};

export default function SetupWizard({ onComplete }: SetupWizardProps) {
  const navigate = useNavigate();
  const [currentStep, setCurrentStep] = useState(0);
  const [selectedSdk, setSelectedSdk] = useState<SdkType>('python');
  // const [usageContext, setUsageContext] = useState<UsageContext>('observability'); // Moved to EnvironmentConfig
  const [projectName, setProjectName] = useState('');
  const [projectDescription, setProjectDescription] = useState('');
  const [createdProject, setCreatedProject] = useState<any>(null);
  const [copiedEnvVar, setCopiedEnvVar] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const steps = [
    { id: 'install', title: 'Install SDK', icon: Download },
    { id: 'create', title: 'Create Project', icon: FolderPlus },
    { id: 'configure', title: 'Configure Environment', icon: Settings },
    { id: 'verify', title: 'Verify Setup', icon: CheckCircle2 },
  ];

  // Helper function to wait for server to be ready
  const waitForServer = async (maxRetries = 10, delayMs = 500): Promise<boolean> => {
    for (let i = 0; i < maxRetries; i++) {
      try {
        const response = await fetch(`${API_BASE_URL}/api/v1/health`, {
          method: 'GET',
          signal: AbortSignal.timeout(2000),
        });
        if (response.ok) {
          return true;
        }
      } catch {
        // Server not ready yet
      }
      if (i < maxRetries - 1) {
        await new Promise(resolve => setTimeout(resolve, delayMs));
      }
    }
    return false;
  };

  const handleCreateProject = async () => {
    if (!projectName.trim()) return;

    setIsCreating(true);
    try {
      // First, wait for server to be ready
      const serverReady = await waitForServer();
      if (!serverReady) {
        throw new Error('Server is not responding. Please wait a moment and try again.');
      }

      // Call the backend API to create the project with retry
      let lastError: Error | null = null;
      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          const response = await fetch(`${API_BASE_URL}/api/v1/projects`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              name: projectName,
              description: projectDescription || undefined,
            }),
          });

          if (!response.ok) {
            const errorText = await response.text().catch(() => 'Unknown error');
            throw new Error(`Failed to create project: ${errorText}`);
          }

          const data = await response.json();

          // Format the project data for the wizard
          const createdProjectData = {
            project_id: data.project_id,
            name: data.name,
            description: data.description,
            created_at: Date.now(),
            env_vars: {
              AGENTREPLAY_URL: data.env_vars.agentreplay_url,
              AGENTREPLAY_TENANT_ID: data.env_vars.tenant_id,
              AGENTREPLAY_PROJECT_ID: data.env_vars.project_id,
            }
          };

          setCreatedProject(createdProjectData);
          setCurrentStep(2);
          return; // Success, exit the function
        } catch (error) {
          lastError = error instanceof Error ? error : new Error('Unknown error');
          if (attempt < 2) {
            await new Promise(resolve => setTimeout(resolve, 1000));
          }
        }
      }

      // All retries failed
      throw lastError || new Error('Failed to create project after retries');
    } catch (error) {
      console.error('Failed to create project:', error);
      alert('Failed to create project: ' + (error instanceof Error ? error.message : 'Unknown error'));
    } finally {
      setIsCreating(false);
    }
  };

  const copyToClipboard = (text: string, key: string) => {
    navigator.clipboard.writeText(text);
    setCopiedEnvVar(key);
    setTimeout(() => setCopiedEnvVar(null), 2000);
  };

  const completeSetup = () => {
    // Mark setup as complete in localStorage
    localStorage.setItem('agentreplay_setup_complete', 'true');
    localStorage.setItem('agentreplay_default_project', createdProject.project_id.toString());

    // Call parent callback
    onComplete(createdProject.project_id);
  };

  return (
    <div className="min-h-screen bg-background flex items-center justify-center px-4">
      <div className="max-w-4xl w-full">
        {/* Progress Steps */}
        <div className="mb-12">
          <div className="flex items-center justify-between">
            {steps.map((step, index) => {
              const Icon = step.icon;
              const isActive = index === currentStep;
              const isCompleted = index < currentStep;

              return (
                <div key={step.id} className="flex items-center flex-1">
                  <div className="flex flex-col items-center flex-1">
                    <div
                      className={`w-12 h-12 rounded-full flex items-center justify-center mb-2 transition-all ${isCompleted
                        ? 'bg-green-500 text-white'
                        : isActive
                          ? 'bg-primary text-white'
                          : 'bg-surface border-2 border-border text-textTertiary'
                        }`}
                    >
                      {isCompleted ? (
                        <Check className="w-6 h-6" />
                      ) : (
                        <Icon className="w-6 h-6" />
                      )}
                    </div>
                    <span
                      className={`text-sm font-medium ${isActive ? 'text-textPrimary' : 'text-textTertiary'
                        }`}
                    >
                      {step.title}
                    </span>
                  </div>
                  {index < steps.length - 1 && (
                    <div
                      className={`h-0.5 flex-1 mx-4 ${isCompleted ? 'bg-green-500' : 'bg-border'
                        }`}
                    />
                  )}
                </div>
              );
            })}
          </div>
        </div>

        {/* Step Content */}
        <AnimatePresence mode="wait">
          <motion.div
            key={currentStep}
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            className="bg-surface rounded-xl border border-border p-8"
          >
            {/* Step 0: Install SDK */}
            {currentStep === 0 && (
              <div>
                <h2 className="text-2xl font-bold text-textPrimary mb-4">
                  Install Agentreplay SDK
                </h2>
                <p className="text-textSecondary mb-6">
                  Select your preferred language and install the Agentreplay SDK to start tracking your LLM applications.
                </p>

                {/* SDK Selection Tabs */}
                <div className="flex gap-2 mb-6 border-b border-border">
                  {(Object.keys(SDKS) as SdkType[]).map((sdk) => (
                    <button
                      key={sdk}
                      onClick={() => setSelectedSdk(sdk)}
                      className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${selectedSdk === sdk
                        ? 'border-primary text-primary'
                        : 'border-transparent text-textTertiary hover:text-textPrimary'
                        }`}
                    >
                      {SDKS[sdk].label}
                    </button>
                  ))}
                </div>

                <div className="space-y-4">
                  <div className="relative">
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-sm font-semibold text-textPrimary">
                        {SDKS[selectedSdk].installLabel}
                      </span>
                      <button
                        onClick={() => copyToClipboard(SDKS[selectedSdk].installCmd, 'install')}
                        className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface-hover hover:bg-surface-elevated transition-colors text-sm text-textSecondary"
                      >
                        {copiedEnvVar === 'install' ? (
                          <>
                            <Check className="w-4 h-4 text-green-500" />
                            Copied!
                          </>
                        ) : (
                          <>
                            <Copy className="w-4 h-4" />
                            Copy
                          </>
                        )}
                      </button>
                    </div>
                    <pre className="bg-surface-elevated rounded-lg p-4 overflow-x-auto border border-border-subtle">
                      <code className="text-sm text-textSecondary font-mono">
                        {SDKS[selectedSdk].installCmd}
                      </code>
                    </pre>
                  </div>

                  <div className="p-4 bg-blue-500/10 border border-blue-500/20 rounded-lg">
                    <div className="flex items-start gap-3">
                      <Terminal className="w-5 h-5 text-blue-500 mt-0.5 flex-shrink-0" />
                      <div>
                        <h4 className="font-semibold text-blue-500 mb-1">
                          Recommended Setup
                        </h4>
                        <p className="text-sm text-textSecondary">
                          {selectedSdk === 'python'
                            ? 'We recommend installing in a virtual environment to avoid conflicts with other packages.'
                            : selectedSdk === 'typescript'
                              ? 'Ensure you have Node.js 18+ installed.'
                              : selectedSdk === 'go'
                                ? 'Requires Go 1.21 or later.'
                                : 'Requires Rust 1.75 or later.'}
                        </p>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="mt-8 flex justify-end">
                  <button
                    onClick={() => setCurrentStep(1)}
                    className="flex items-center gap-2 px-6 py-3 bg-primary hover:bg-primary/90 text-white rounded-lg font-medium transition-colors"
                  >
                    Next: Create Project
                    <ChevronRight className="w-5 h-5" />
                  </button>
                </div>
              </div>
            )}

            {/* Step 1: Create Project */}
            {currentStep === 1 && (
              <div>
                <h2 className="text-2xl font-bold text-textPrimary mb-4">
                  Create Your First Project
                </h2>
                <p className="text-textSecondary mb-6">
                  Projects help you organize traces by application or environment. Create your first project to get started.
                </p>

                <div className="space-y-4">
                  <div>
                    <label className="block text-sm font-medium text-textPrimary mb-2">
                      Project Name <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="text"
                      value={projectName}
                      onChange={(e) => setProjectName(e.target.value)}
                      placeholder="e.g., My AI Agent"
                      className="w-full px-4 py-3 bg-surface-elevated border border-border rounded-lg text-textPrimary placeholder-textTertiary focus:outline-none focus:ring-2 focus:ring-primary"
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-textPrimary mb-2">
                      Description (Optional)
                    </label>
                    <textarea
                      value={projectDescription}
                      onChange={(e) => setProjectDescription(e.target.value)}
                      placeholder="What does this project track?"
                      rows={3}
                      className="w-full px-4 py-3 bg-surface-elevated border border-border rounded-lg text-textPrimary placeholder-textTertiary focus:outline-none focus:ring-2 focus:ring-primary resize-none"
                    />
                  </div>
                </div>

                <div className="mt-8 flex justify-between">
                  <button
                    onClick={() => setCurrentStep(0)}
                    className="px-6 py-3 bg-surface-hover hover:bg-surface-elevated text-textPrimary rounded-lg font-medium transition-colors"
                  >
                    Back
                  </button>
                  <button
                    onClick={handleCreateProject}
                    disabled={!projectName.trim() || isCreating}
                    className="flex items-center gap-2 px-6 py-3 bg-primary hover:bg-primary/90 text-white rounded-lg font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    {isCreating ? (
                      <>
                        <Loader2 className="w-5 h-5 animate-spin" />
                        Creating...
                      </>
                    ) : (
                      <>
                        Create Project
                        <ChevronRight className="w-5 h-5" />
                      </>
                    )}
                  </button>
                </div>
              </div>
            )}

            {/* Step 2: Configure Environment */}
            {currentStep === 2 && createdProject && (
              <div>
                <h2 className="text-2xl font-bold text-textPrimary mb-4">
                  Configure Environment Variables
                </h2>
                <p className="text-textSecondary mb-6">
                  Add these environment variables to your project to connect to AgentReplay.
                </p>

                <div className="space-y-6 mb-8">
                  <EnvironmentConfig
                    projectId={createdProject.project_id}
                    projectName={createdProject.name}
                    envVars={createdProject.env_vars}
                    onCopy={copyToClipboard}
                  />
                </div>

                <div className="p-4 bg-green-500/10 border border-green-500/20 rounded-lg mb-6">
                  <div className="flex items-start gap-3">
                    <CheckCircle2 className="w-5 h-5 text-green-500 mt-0.5 flex-shrink-0" />
                    <div>
                      <h4 className="font-semibold text-green-500 mb-1">
                        Project Created Successfully!
                      </h4>
                      <p className="text-sm text-textSecondary">
                        Your project <strong>{createdProject.name}</strong> (ID: {createdProject.project_id}) is ready.
                        Add the environment variables above to start tracking.
                      </p>
                    </div>
                  </div>
                </div>

                <div className="mt-8 flex justify-between">
                  <button
                    onClick={() => setCurrentStep(1)}
                    className="px-6 py-3 bg-surface-hover hover:bg-surface-elevated text-textPrimary rounded-lg font-medium transition-colors"
                  >
                    Back
                  </button>
                  <button
                    onClick={() => setCurrentStep(3)}
                    className="flex items-center gap-2 px-6 py-3 bg-primary hover:bg-primary/90 text-white rounded-lg font-medium transition-colors"
                  >
                    Next: Verify Setup
                    <ChevronRight className="w-5 h-5" />
                  </button>
                </div>
              </div>
            )}

            {/* Step 3: Verify Setup */}
            {currentStep === 3 && createdProject && (
              <div>
                <h2 className="text-2xl font-bold text-textPrimary mb-4">
                  Verify Your Setup
                </h2>
                <p className="text-textSecondary mb-6">
                  Run this example code to verify your AgentReplay setup is working correctly.
                </p>

                <div className="space-y-4">
                  <div className="relative">
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-sm font-semibold text-textPrimary">
                        {SDKS[selectedSdk].label} Test Script
                      </span>
                      <button
                        onClick={() => copyToClipboard(SDKS[selectedSdk].verifyCode(createdProject.project_id), 'test')}
                        className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface-hover hover:bg-surface-elevated transition-colors text-sm text-textSecondary"
                      >
                        {copiedEnvVar === 'test' ? (
                          <>
                            <Check className="w-4 h-4 text-green-500" />
                            Copied!
                          </>
                        ) : (
                          <>
                            <Copy className="w-4 h-4" />
                            Copy
                          </>
                        )}
                      </button>
                    </div>
                    <pre className="bg-surface-elevated rounded-lg p-4 overflow-x-auto border border-border-subtle max-h-96">
                      <code className="text-sm text-textSecondary font-mono whitespace-pre">
                        {SDKS[selectedSdk].verifyCode(createdProject.project_id)}
                      </code>
                    </pre>
                  </div>
                </div>

                <div className="mt-8 flex justify-between">
                  <button
                    onClick={() => setCurrentStep(2)}
                    className="px-6 py-3 bg-surface-hover hover:bg-surface-elevated text-textPrimary rounded-lg font-medium transition-colors"
                  >
                    Back
                  </button>
                  <button
                    onClick={completeSetup}
                    className="flex items-center gap-2 px-6 py-3 bg-green-500 hover:bg-green-600 text-white rounded-lg font-medium transition-colors"
                  >
                    <CheckCircle2 className="w-5 h-5" />
                    Complete Setup
                  </button>
                </div>
              </div>
            )}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
