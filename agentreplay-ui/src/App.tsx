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

import { useEffect, useRef } from 'react';
import { BrowserRouter as Router, Navigate, Route, Routes, useNavigate, useLocation, useParams } from 'react-router-dom';
import { Layout } from '../components/Layout';
import SetupWizard from '../components/SetupWizard';
import '../app/globals.css';
import { initTheme } from './lib/theme';
import { useProjects } from './context/project-context';
import { AppModeProvider } from './context/app-mode-context';
import { BotProvider } from './context/bot-context';
import Traces from './pages/Traces';
import TraceDetail from './pages/TraceDetail';
import Evaluations from './pages/Evaluations';
import EvaluationRunDetail from './pages/EvaluationRunDetail';
import Prompts from './pages/Prompts';
import PromptDetail from './pages/PromptDetail';
import PromptRegistry from './pages/PromptRegistry';
import Experiments from './pages/Experiments';
import Datasets from './pages/Datasets';
import Playground from './pages/Playground';
import ModelComparisonPage from './pages/ModelComparisonPage';
import AnnotationQueue from './pages/AnnotationQueue';
import ExperimentComparison from './pages/ExperimentComparison';
import Analytics from './pages/Analytics';
import Settings from './pages/Settings';
import Storage from './pages/Storage';
import Docs from './pages/Docs';
import Agents from './pages/Agents';
import Search from './pages/Search';
import InsightsPage from './pages/InsightsPage';
import PluginsPage from './pages/PluginsPage';
import MemoryPage from './pages/MemoryPage';
import MetricsDemoPage from './pages/MetricsDemoPage';
import ToolsPage from './pages/ToolsPage';
import EvalPipelinePage from './pages/EvalPipelinePage';
import CostManagementPage from './pages/CostManagementPage';
import CodingSessions from './pages/CodingSessions';
import McpTesterPage from './pages/McpTesterPage';
import SkillTesterPage from './pages/SkillTesterPage';
import SkillMemoryPage from './pages/SkillMemoryPage';
// OpenClaw page hidden — not yet tested for release
// import OpenClawPage from './pages/OpenClawPage';

import ServerStatus from '../components/ServerStatus';
import SyncingScreen from '../components/SyncingScreen';

const STORAGE_KEY_LAST_PATH = 'agentreplay_last_path';

function usePathPersistence() {
  const location = useLocation();
  const { currentProject } = useProjects();

  useEffect(() => {
    // Only save project-specific paths
    if (currentProject && location.pathname.startsWith(`/projects/${currentProject.project_id}`)) {
      localStorage.setItem(STORAGE_KEY_LAST_PATH, location.pathname);
    }
  }, [location, currentProject]);
}

/**
 * WebKit-safe redirect that builds an absolute path using useParams.
 * Fires the navigate call exactly once per mount to avoid exceeding
 * WebKit's history.replaceState rate limit (100 calls / 10 s).
 */
function SafeRedirect({ to }: { to: string }) {
  const { projectId } = useParams<{ projectId: string }>();
  const navigate = useNavigate();
  const fired = useRef(false);

  useEffect(() => {
    if (fired.current) return;
    fired.current = true;
    navigate(`/projects/${projectId}/${to}`, { replace: true });
  }, [projectId, to, navigate]);

  return null;
}

function ProjectLanding() {
  const { currentProject, loading, connectionError } = useProjects();
  const navigate = useNavigate();
  const didNavigate = useRef(false);

  useEffect(() => {
    // Reset the guard when we re-mount (e.g. user navigated back to /)
    didNavigate.current = false;
  }, []);

  useEffect(() => {
    if (didNavigate.current || connectionError || loading) return;
    if (!currentProject) {
      didNavigate.current = true;
      navigate('/get-started', { replace: true });
      return;
    }
    const lastPath = localStorage.getItem(STORAGE_KEY_LAST_PATH);
    const target = (lastPath && lastPath.startsWith(`/projects/${currentProject.project_id}`))
      ? lastPath
      : `/projects/${currentProject.project_id}/traces`;
    didNavigate.current = true;
    navigate(target, { replace: true });
  }, [currentProject, loading, connectionError, navigate]);

  if (connectionError) {
    return (
      <div className="flex h-screen flex-col items-center justify-center bg-background gap-4">
        <ServerStatus compact={false} />
        <p className="text-textSecondary text-sm">Waiting for AgentReplay server...</p>
      </div>
    );
  }

  return <SyncingScreen />;
}

function SetupWizardWrapper() {
  const { refreshProjects, selectProject } = useProjects();
  const navigate = useNavigate();

  const handleComplete = async (projectId: number) => {
    await refreshProjects();
    await selectProject(String(projectId));
    navigate(`/projects/${projectId}/traces`);
  };

  return <SetupWizard onComplete={handleComplete} />;
}

// Component to handle side-effects inside Router context
function AppContent() {
  usePathPersistence();

  return (
    <Routes>
      <Route path="/" element={<ProjectLanding />} />
      <Route path="/memory" element={<MemoryPage />} />
      <Route path="/get-started" element={<SetupWizardWrapper />} />
      <Route element={<Layout />}>
        <Route path="/projects/:projectId/traces" element={<Traces />} />
        <Route path="/projects/:projectId/traces/:traceId" element={<TraceDetail />} />
        <Route path="/projects/:projectId/coding-sessions" element={<CodingSessions />} />
        <Route path="/projects/:projectId/coding-sessions/:sessionId" element={<CodingSessions />} />
        <Route path="/projects/:projectId/agents" element={<Agents />} />
        <Route path="/projects/:projectId/timeline" element={<SafeRedirect to="analytics" />} />
        <Route path="/projects/:projectId/search" element={<Search />} />
        <Route path="/projects/:projectId/evaluations" element={<Evaluations />} />
        <Route path="/projects/:projectId/evaluations/runs/:runId" element={<EvaluationRunDetail />} />
        <Route path="/projects/:projectId/eval-pipeline" element={<EvalPipelinePage />} />
        <Route path="/projects/:projectId/prompts" element={<Prompts />} />
        <Route path="/projects/:projectId/prompts/:promptId" element={<PromptDetail />} />
        <Route path="/projects/:projectId/prompt-registry" element={<PromptRegistry />} />
        <Route path="/projects/:projectId/experiments" element={<Experiments />} />
        <Route path="/projects/:projectId/datasets" element={<Datasets />} />
        <Route path="/projects/:projectId/playground" element={<Playground />} />
        <Route path="/projects/:projectId/model-comparison" element={<ModelComparisonPage />} />
        <Route path="/projects/:projectId/tools" element={<ToolsPage />} />
        <Route path="/projects/:projectId/versions" element={<SafeRedirect to="prompts" />} />
        <Route path="/projects/:projectId/annotation-queue" element={<AnnotationQueue evalRunId="123" />} />
        <Route path="/projects/:projectId/comparison" element={<ExperimentComparison runIds={['123', '456']} />} />
        <Route path="/projects/:projectId/analytics" element={<Analytics />} />
        <Route path="/projects/:projectId/insights" element={<InsightsPage />} />
        <Route path="/projects/:projectId/plugins" element={<PluginsPage />} />
        <Route path="/projects/:projectId/memory" element={<MemoryPage />} />
        <Route path="/projects/:projectId/metrics-demo" element={<MetricsDemoPage />} />
        <Route path="/projects/:projectId/storage" element={<Storage />} />
        <Route path="/projects/:projectId/settings" element={<Settings />} />
        <Route path="/projects/:projectId/costs" element={<CostManagementPage />} />
        <Route path="/projects/:projectId/mcp-tester" element={<McpTesterPage />} />
        <Route path="/projects/:projectId/skill-tester" element={<SkillTesterPage />} />
        <Route path="/projects/:projectId/skill-memory" element={<SkillMemoryPage />} />
        {/* OpenClaw routes hidden — not yet tested for release */}
        {/* <Route path="/projects/:projectId/openclaw" element={<OpenClawPage />} /> */}
        {/* <Route path="/projects/:projectId/bots" element={<SafeRedirect to="openclaw" />} /> */}
        <Route path="/projects/:projectId/docs" element={<Docs />} />
        <Route path="/projects/:projectId" element={<SafeRedirect to="traces" />} />
      </Route>
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}

function App() {
  useEffect(() => {
    let cleanup: (() => void) | undefined;
    initTheme().then((dispose) => {
      cleanup = dispose;
    });
    return () => cleanup?.();
  }, []);

  return (
    <BotProvider>
    <AppModeProvider>
      <Router future={{ v7_startTransition: true, v7_relativeSplatPath: true }}>
        <AppContent />
      </Router>
    </AppModeProvider>
    </BotProvider>
  );
}

export default App;
