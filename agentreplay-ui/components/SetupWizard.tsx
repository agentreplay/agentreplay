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

import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import { EnvironmentConfig } from "./EnvironmentConfig";
import { API_BASE_URL } from "../src/lib/agentreplay-api";
import {
  Check,
  Copy,
  Download,
  FolderPlus,
  Settings,
  ChevronRight,
  Terminal,
  CheckCircle2,
  Loader2,
  Star,
  Sparkles,
  ExternalLink,
  Brain,
  Activity,
  Shield,
  Laptop,
  Lock,
  Zap,
  Eye,
} from "lucide-react";

interface SetupWizardProps {
  onComplete: (projectId: number) => void;
}

type SdkType = "python" | "typescript";
type UsageContext = "observability" | "memory" | "claude"; // New Context Type

const SDKS: Record<
  SdkType,
  {
    label: string;
    installCmd: string;
    installLabel: string;
    verifyCode: (projectId: number) => string;
  }
> = {
  python: {
    label: "Python",
    installCmd: "pip install agentreplay && agentreplay-install",
    installLabel: "Install via pip",
    verifyCode: (projectId) => `import os
from agentreplay_client import AgentreplayClient

# Initialize client
client = AgentreplayClient(
    url=os.getenv("AGENTREPLAY_URL", "http://localhost:47100"),
    tenant_id=int(os.getenv("AGENTREPLAY_TENANT_ID", "1")),
    project_id=int(os.getenv("AGENTREPLAY_PROJECT_ID", "${projectId}")),
)

# Create a test trace
trace = client.create_trace(
    agent_id=1,
    payload={"test": "Hello Agentreplay!"}
)

print(f"✅ Setup successful! Trace ID: {trace['edge_id']}")
print(f"🚀 Visit http://localhost:5173/traces to see your data")`,
  },
  typescript: {
    label: "Node.js",
    installCmd: "npm install @agentreplay/agentreplay",
    installLabel: "Install via npm",
    verifyCode: (
      projectId,
    ) => `import { AgentreplayClient } from '@agentreplay/agentreplay';

// Initialize client
const client = new AgentreplayClient({
  url: process.env.AGENTREPLAY_URL || 'http://localhost:47100',
  tenantId: parseInt(process.env.AGENTREPLAY_TENANT_ID || '1'),
  projectId: parseInt(process.env.AGENTREPLAY_PROJECT_ID || '${projectId}'),
});

// Create a test trace
const trace = await client.createTrace({
  agentId: 1,
  payload: { test: 'Hello Agentreplay!' }
});

console.log(\`✅ Setup successful! Trace ID: \${trace.edge_id}\`);
console.log(\`🚀 Visit http://localhost:5173/traces to see your data\`);`,
  },
};

export default function SetupWizard({ onComplete }: SetupWizardProps) {
  const navigate = useNavigate();
  const [currentStep, setCurrentStep] = useState(0);
  const [selectedSdk, setSelectedSdk] = useState<SdkType>("python");
  // const [usageContext, setUsageContext] = useState<UsageContext>('observability'); // Moved to EnvironmentConfig
  const [projectName, setProjectName] = useState("");
  const [projectDescription, setProjectDescription] = useState("");
  const [createdProject, setCreatedProject] = useState<any>(null);
  const [copiedEnvVar, setCopiedEnvVar] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const steps = [
    { id: "welcome", title: "Welcome", icon: Sparkles },
    { id: "create", title: "Create Project", icon: FolderPlus },
    { id: "install", title: "Install SDK", icon: Download },
    { id: "verify", title: "Verify Setup", icon: CheckCircle2 },
  ];

  // Helper function to wait for server to be ready
  const waitForServer = async (
    maxRetries = 30,
    delayMs = 500,
  ): Promise<boolean> => {
    for (let i = 0; i < maxRetries; i++) {
      try {
        const response = await fetch(`${API_BASE_URL}/api/v1/health`, {
          method: "GET",
          signal: AbortSignal.timeout(2000),
        });
        if (response.ok) {
          return true;
        }
      } catch {
        // Server not ready yet
      }
      if (i < maxRetries - 1) {
        await new Promise((resolve) => setTimeout(resolve, delayMs));
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
        throw new Error(
          "Server is not responding. Please wait a moment and try again.",
        );
      }

      // Call the backend API to create the project with retry
      let lastError: Error | null = null;
      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          // Create the Main Project
          const response = await fetch(`${API_BASE_URL}/api/v1/projects`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              name: projectName,
              description: projectDescription || undefined,
            }),
          });

          if (!response.ok) {
            const errorText = await response
              .text()
              .catch(() => "Unknown error");
            throw new Error(`Failed to create project: ${errorText}`);
          }

          const data = await response.json();

          // Create "Claude Code" Project (ID: 49455)
          // We do this in parallel but don't fail the main flow if it fails (it might already exist)
          fetch(`${API_BASE_URL}/api/v1/projects`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              name: "Claude Code",
              description: "Claude Code coding sessions",
              id: 49455,
            }),
          }).catch((err) =>
            console.error(
              "Failed to create Claude Code project (it might already exist):",
              err,
            ),
          );

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
            },
          };

          setCreatedProject(createdProjectData);
          setCurrentStep(2); // Go to Install SDK step
          return; // Success, exit the function
        } catch (error) {
          lastError =
            error instanceof Error ? error : new Error("Unknown error");
          if (attempt < 2) {
            await new Promise((resolve) => setTimeout(resolve, 1000));
          }
        }
      }

      // All retries failed
      throw lastError || new Error("Failed to create project after retries");
    } catch (error) {
      console.error("Failed to create project:", error);
      alert(
        "Failed to create project: " +
          (error instanceof Error ? error.message : "Unknown error"),
      );
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
    localStorage.setItem("agentreplay_setup_complete", "true");
    localStorage.setItem(
      "agentreplay_default_project",
      createdProject.project_id.toString(),
    );

    // Call parent callback
    onComplete(createdProject.project_id);
  };

  return (
    <div className="min-h-screen bg-background flex flex-col items-center justify-center px-4 pt-16 pb-14 sm:pt-20 sm:pb-16">
      <div className="max-w-4xl w-full mx-auto">
        {/* Progress Steps */}
        <div className="mb-14 sm:mb-16">
          <div className="flex items-start justify-between gap-2">
            {steps.map((step, index) => {
              const Icon = step.icon;
              const isActive = index === currentStep;
              const isCompleted = index < currentStep;

              return (
                <div key={step.id} className="flex items-center flex-1 min-w-0">
                  <div className="flex flex-col items-center flex-1 gap-2">
                    <div
                      className={`w-11 h-11 sm:w-12 sm:h-12 rounded-full flex items-center justify-center flex-shrink-0 transition-all duration-200 ${
                        isCompleted
                          ? "bg-emerald-500 text-white shadow-sm"
                          : isActive
                            ? "bg-primary text-primary-foreground shadow-md ring-2 ring-primary/20"
                            : "bg-surface border border-border text-textTertiary"
                      }`}
                    >
                      {isCompleted ? (
                        <Check className="w-5 h-5 sm:w-6 sm:h-6" />
                      ) : (
                        <Icon className="w-5 h-5 sm:w-6 sm:h-6" />
                      )}
                    </div>
                    <span
                      className={`text-xs sm:text-sm font-medium text-center truncate w-full px-0.5 ${
                        isActive ? "text-textPrimary" : "text-textTertiary"
                      }`}
                    >
                      {step.title}
                    </span>
                  </div>
                  {index < steps.length - 1 && (
                    <div
                      className={`h-0.5 flex-1 mx-1 sm:mx-2 mt-5 sm:mt-6 self-start shrink-0 ${
                        isCompleted ? "bg-emerald-500/80" : "bg-border"
                      }`}
                      aria-hidden
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
            transition={{ duration: 0.2 }}
            className="bg-surface rounded-2xl border border-border shadow-lg shadow-black/5 dark:shadow-black/25 p-8 sm:p-10"
          >
            {/* Step 0: Welcome */}
            {currentStep === 0 && (
              <div className="text-center">
                <div className="flex justify-center mb-8">
                  <div className="relative">
                    <img
                      src="/logo.svg"
                      alt="AgentReplay"
                      className="w-24 h-24 sm:w-28 sm:h-28 rounded-2xl shadow-lg ring-1 ring-black/5 dark:ring-white/5"
                    />
                    <div className="absolute -bottom-0.5 -right-0.5 w-8 h-8 bg-emerald-500 rounded-full flex items-center justify-center border-2 border-surface shadow-md">
                      <Check className="w-4 h-4 text-white" />
                    </div>
                  </div>
                </div>

                <h2 className="text-2xl sm:text-3xl font-bold text-textPrimary mb-3 tracking-tight">
                  Welcome to AgentReplay
                </h2>
                <p className="text-base font-semibold text-primary mb-2 max-w-xl mx-auto">
                  Local-First Desktop Observability & AI Memory for Your Agents
                  and Coding Tools
                </p>
                <p className="text-sm text-textSecondary mb-10 max-w-2xl mx-auto leading-relaxed">
                  The open-source desktop app purpose-built for AI agents &
                  Claude Code. Trace every interaction, build persistent memory,
                  and debug with confidence — all without sending a single byte
                  off your machine.
                </p>

                {/* Privacy banner */}
                <div className="bg-emerald-500/10 dark:bg-emerald-500/15 border border-emerald-500/25 rounded-xl p-4 mb-6 text-left">
                  <div className="flex items-center gap-3">
                    <Lock className="w-5 h-5 text-emerald-600 dark:text-emerald-400 flex-shrink-0" />
                    <p className="text-sm font-medium text-emerald-800 dark:text-emerald-300">
                      Your data never leaves your laptop. Zero cloud
                      dependencies. Fully optimized for desktops & laptops.
                    </p>
                  </div>
                </div>

                {/* GitHub Star CTA */}
                <div className="bg-amber-500/10 dark:bg-amber-500/15 border border-amber-500/25 rounded-xl p-5 mb-10 text-left">
                  <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
                    <div className="flex items-center gap-3">
                      <Star className="w-5 h-5 text-amber-500 flex-shrink-0" />
                      <p className="text-sm text-textSecondary">
                        Love AgentReplay? Help us grow — give us a star on
                        GitHub!
                      </p>
                    </div>
                    <a
                      href="https://github.com/agentreplay/agentreplay"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="inline-flex items-center justify-center gap-2 px-4 py-2.5 bg-amber-500 hover:bg-amber-600 text-white rounded-xl text-sm font-medium transition-colors shadow-sm shrink-0"
                    >
                      <Star className="w-4 h-4" />
                      Star on GitHub
                      <ExternalLink className="w-3.5 h-3.5" />
                    </a>
                  </div>
                </div>

                <div className="flex justify-center">
                  <button
                    onClick={() => setCurrentStep(1)}
                    className="flex items-center gap-2 px-8 py-3.5 bg-primary hover:bg-primary/90 text-primary-foreground rounded-xl font-semibold transition-colors text-base shadow-md hover:shadow-lg focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 focus:ring-offset-background"
                  >
                    Get Started
                    <ChevronRight className="w-5 h-5" />
                  </button>
                </div>
              </div>
            )}

            {/* Step 1: Create Project */}
            {currentStep === 1 && (
              <div>
                <h2 className="text-2xl font-bold text-textPrimary mb-2 tracking-tight">
                  Create Your First Project
                </h2>
                <p className="text-textSecondary mb-8 text-sm leading-relaxed">
                  Projects help you organize traces by application or
                  environment. Create your first project to get started.
                </p>

                <div className="space-y-6">
                  <div>
                    <label className="block text-sm font-medium text-textPrimary mb-2">
                      Project Name <span className="text-destructive">*</span>
                    </label>
                    <input
                      type="text"
                      value={projectName}
                      onChange={(e) => setProjectName(e.target.value)}
                      placeholder="e.g., My AI Agent"
                      className="w-full px-4 py-3 bg-surface-elevated border border-border rounded-xl text-textPrimary placeholder-textTertiary focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 focus:ring-offset-background"
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
                      className="w-full px-4 py-3 bg-surface-elevated border border-border rounded-xl text-textPrimary placeholder-textTertiary focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 focus:ring-offset-background resize-none"
                    />
                  </div>
                </div>

                <div className="mt-10 flex justify-between gap-4">
                  <button
                    onClick={() => setCurrentStep(0)}
                    className="px-6 py-3 bg-surface-hover hover:bg-surface-elevated text-textPrimary rounded-xl font-medium transition-colors border border-border"
                  >
                    Back
                  </button>
                  <button
                    onClick={handleCreateProject}
                    disabled={!projectName.trim() || isCreating}
                    className="flex items-center gap-2 px-6 py-3 bg-primary hover:bg-primary/90 text-primary-foreground rounded-xl font-medium transition-colors shadow-sm disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 focus:ring-offset-background"
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

            {/* Step 2: Install SDK & Configure */}
            {currentStep === 2 && createdProject && (
              <div>
                <div className="p-4 bg-emerald-500/10 dark:bg-emerald-500/15 border border-emerald-500/25 rounded-xl mb-8">
                  <div className="flex items-start gap-3">
                    <CheckCircle2 className="w-5 h-5 text-emerald-600 dark:text-emerald-400 mt-0.5 flex-shrink-0" />
                    <div>
                      <h4 className="font-semibold text-emerald-700 dark:text-emerald-400 mb-1">
                        Project Created Successfully!
                      </h4>
                      <p className="text-sm text-textSecondary">
                        Your project{" "}
                        <strong className="text-textPrimary">
                          {createdProject.name}
                        </strong>{" "}
                        (ID: {createdProject.project_id}) is ready.
                      </p>
                    </div>
                  </div>
                </div>

                <h2 className="text-2xl font-bold text-textPrimary mb-2 tracking-tight">
                  Install SDK & Configure
                </h2>
                <p className="text-textSecondary mb-8 text-sm leading-relaxed">
                  Select your preferred language, install the SDK, and configure
                  your environment.
                </p>

                {/* SDK Selection Tabs */}
                <div className="flex gap-2 mb-6 border-b border-border">
                  {(Object.keys(SDKS) as SdkType[]).map((sdk) => (
                    <button
                      key={sdk}
                      onClick={() => setSelectedSdk(sdk)}
                      className={`px-4 py-2.5 text-sm font-medium border-b-2 -mb-px transition-colors rounded-t-lg ${
                        selectedSdk === sdk
                          ? "border-primary text-primary bg-surface-hover/50"
                          : "border-transparent text-textTertiary hover:text-textPrimary hover:bg-surface-hover/30"
                      }`}
                    >
                      {SDKS[sdk].label}
                    </button>
                  ))}
                </div>

                <div className="space-y-6">
                  <div className="relative">
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-sm font-semibold text-textPrimary">
                        {SDKS[selectedSdk].installLabel}
                      </span>
                      <button
                        onClick={() =>
                          copyToClipboard(
                            SDKS[selectedSdk].installCmd,
                            "install",
                          )
                        }
                        className="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-surface-hover hover:bg-surface-elevated transition-colors text-sm text-textSecondary border border-border"
                      >
                        {copiedEnvVar === "install" ? (
                          <>
                            <Check className="w-4 h-4 text-emerald-500" />
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
                    <pre className="bg-surface-elevated rounded-xl p-4 overflow-x-auto border border-border">
                      <code className="text-sm text-textSecondary font-mono">
                        {SDKS[selectedSdk].installCmd}
                      </code>
                    </pre>
                  </div>

                  <div className="p-4 bg-primary/10 dark:bg-primary/15 border border-primary/25 rounded-xl">
                    <div className="flex items-start gap-3">
                      <Terminal className="w-5 h-5 text-primary mt-0.5 flex-shrink-0" />
                      <div>
                        <h4 className="font-semibold text-primary mb-1">
                          Recommended Setup
                        </h4>
                        <p className="text-sm text-textSecondary leading-relaxed">
                          {selectedSdk === "python"
                            ? "We recommend installing in a virtual environment to avoid conflicts with other packages."
                            : "Ensure you have Node.js 18+ installed."}
                        </p>
                      </div>
                    </div>
                  </div>
                </div>

                {/* Environment Variables */}
                <div className="mt-6">
                  <h3 className="text-lg font-semibold text-textPrimary mb-3">
                    Environment Variables
                  </h3>
                  <div className="space-y-6">
                    <EnvironmentConfig
                      projectId={createdProject.project_id}
                      projectName={createdProject.name}
                      envVars={createdProject.env_vars}
                      onCopy={copyToClipboard}
                    />
                  </div>
                </div>

                <div className="mt-10 flex justify-between gap-4">
                  <button
                    onClick={() => setCurrentStep(1)}
                    className="px-6 py-3 bg-surface-hover hover:bg-surface-elevated text-textPrimary rounded-xl font-medium transition-colors border border-border"
                  >
                    Back
                  </button>
                  <button
                    onClick={() => setCurrentStep(3)}
                    className="flex items-center gap-2 px-6 py-3 bg-primary hover:bg-primary/90 text-primary-foreground rounded-xl font-medium transition-colors shadow-sm focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 focus:ring-offset-background"
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
                <h2 className="text-2xl font-bold text-textPrimary mb-2 tracking-tight">
                  Verify Your Setup
                </h2>
                <p className="text-textSecondary mb-8 text-sm leading-relaxed">
                  Run this example code to verify your AgentReplay setup is
                  working correctly.
                </p>

                <div className="space-y-4">
                  <div className="relative">
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-sm font-semibold text-textPrimary">
                        {SDKS[selectedSdk].label} Test Script
                      </span>
                      <button
                        onClick={() =>
                          copyToClipboard(
                            SDKS[selectedSdk].verifyCode(
                              createdProject.project_id,
                            ),
                            "test",
                          )
                        }
                        className="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-surface-hover hover:bg-surface-elevated transition-colors text-sm text-textSecondary border border-border"
                      >
                        {copiedEnvVar === "test" ? (
                          <>
                            <Check className="w-4 h-4 text-emerald-500" />
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
                    <pre className="bg-surface-elevated rounded-xl p-4 overflow-x-auto border border-border max-h-96">
                      <code className="text-sm text-textSecondary font-mono whitespace-pre">
                        {SDKS[selectedSdk].verifyCode(
                          createdProject.project_id,
                        )}
                      </code>
                    </pre>
                  </div>
                </div>

                <div className="mt-10 flex justify-between gap-4">
                  <button
                    onClick={() => setCurrentStep(2)}
                    className="px-6 py-3 bg-surface-hover hover:bg-surface-elevated text-textPrimary rounded-xl font-medium transition-colors border border-border"
                  >
                    Back
                  </button>
                  <button
                    onClick={completeSetup}
                    className="flex items-center gap-2 px-6 py-3 bg-emerald-500 hover:bg-emerald-600 text-white rounded-xl font-medium transition-colors shadow-sm focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:ring-offset-2 focus:ring-offset-background"
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
