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

import { createContext, ReactNode, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { agentreplayClient } from '../lib/agentreplay-api';
import { getStoredProjectId, persistProjectId } from '../lib/project-store';

export interface ProjectSummary {
  project_id: string;
  name: string;
  created_at: number;
  trace_count: number;
}

interface ProjectContextValue {
  projects: ProjectSummary[];
  currentProject: ProjectSummary | null;
  loading: boolean;
  initialized: boolean;
  error: string | null;
  connectionError: boolean;
  selectProject: (projectId: string) => Promise<void>;
  refreshProjects: () => Promise<void>;
}

const ProjectContext = createContext<ProjectContextValue | undefined>(undefined);

export function ProjectProvider({ children }: { children: ReactNode }) {
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [currentProjectId, setCurrentProjectId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [initialized, setInitialized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [connectionError, setConnectionError] = useState(false);
  const currentProjectIdRef = useRef<string | null>(null);

  useEffect(() => {
    currentProjectIdRef.current = currentProjectId;
  }, [currentProjectId]);

  useEffect(() => {
    let active = true;

    (async () => {
      const stored = await getStoredProjectId();
      if (!active) return;
      if (stored) {
        setCurrentProjectId(stored);
      }
      currentProjectIdRef.current = stored ?? null;
      setInitialized(true);
    })();

    return () => {
      active = false;
    };
  }, []);

  const refreshProjects = useCallback(async () => {
    setLoading(true);
    setError(null);
    setConnectionError(false);

    try {
      const response = await agentreplayClient.listProjects();
      const normalized: ProjectSummary[] = (response?.projects || []).map((raw) => {
        return {
          project_id: String(raw.project_id), // Convert number to string for consistency
          name: raw.name || `Project ${raw.project_id}`,
          created_at: raw.created_at || Date.now(),
          trace_count: raw.trace_count || 0,
        };
      });

      setProjects(normalized);

      if (normalized.length === 0) {
        setCurrentProjectId(null);
        currentProjectIdRef.current = null;
        return;
      }

      const targetId = normalized.find((project) => project.project_id === currentProjectIdRef.current)?.project_id
        ?? normalized[0].project_id;

      if (targetId !== currentProjectIdRef.current) {
        setCurrentProjectId(targetId);
        currentProjectIdRef.current = targetId;
        await persistProjectId(targetId);
      }
    } catch (err) {
      console.error('Failed to load projects', err);
      // Check if it's a connection error (fetch failure)
      // The exact error message depends on the browser/fetch implementation, 
      // but usually involves "Failed to fetch" or similar for network errors.
      const errorMessage = err instanceof Error ? err.message : 'Unable to load projects';
      setError(errorMessage);

      // If it's a network error, set connectionError to true
      // This allows the UI to show a "Connecting..." state instead of "No projects"
      if (errorMessage.includes('Failed to fetch') ||
        errorMessage.includes('Network request failed') ||
        errorMessage.includes('ECONNREFUSED')) {
        setConnectionError(true);
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!initialized) {
      return;
    }
    refreshProjects();
  }, [initialized, refreshProjects]);

  const selectProject = useCallback(async (projectId: string) => {
    // Prevent selecting the same project
    if (projectId === currentProjectIdRef.current) {
      return;
    }
    setCurrentProjectId(projectId);
    currentProjectIdRef.current = projectId;
    await persistProjectId(projectId);
  }, []);

  const currentProject = useMemo(() => {
    if (!currentProjectId) {
      return null;
    }
    return projects.find((project) => project.project_id === currentProjectId) || null;
  }, [currentProjectId, projects]);

  const value: ProjectContextValue = {
    projects,
    currentProject,
    loading,
    initialized,
    error,
    connectionError,
    selectProject,
    refreshProjects,
  };

  return <ProjectContext.Provider value={value}>{children}</ProjectContext.Provider>;
}

export function useProjects() {
  const context = useContext(ProjectContext);
  if (!context) {
    throw new Error('useProjects must be used within a ProjectProvider');
  }
  return context;
}
