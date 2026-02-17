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

import React, { useEffect, useState, useCallback } from 'react';
import { Outlet, useParams, useLocation, useNavigate } from 'react-router-dom';
import Sidebar from './Sidebar';
import { cn } from '../lib/utils';
import { useProjects } from '../src/context/project-context';
import { ProjectSwitcher } from './ProjectSwitcher';
import { ProjectSetupInfo } from './ProjectSetupInfo';
import { CommandPalette } from './CommandPalette';
import { Breadcrumbs } from './Breadcrumbs';

// Service definitions for the header toggle pills
const SERVICE_DEFS: { key: string; label: string; activeColor: string; projectHint?: string; defaultPage?: string }[] = [
  { key: 'agents', label: 'AI Agents', activeColor: 'bg-purple-500/15 text-purple-600 dark:text-purple-400 border-purple-500/30', defaultPage: 'traces' },
  { key: 'claude', label: 'Claude Code', activeColor: 'bg-sky-500/15 text-sky-600 dark:text-sky-400 border-sky-500/30', projectHint: 'claude', defaultPage: 'memory' },
  { key: 'testing', label: 'Testing', activeColor: 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 border-emerald-500/30', defaultPage: 'mcp-tester' },
  // OpenClaw pill hidden — not yet tested for release
  // { key: 'openclaw', label: 'OpenClaw', activeColor: 'bg-orange-500/15 text-orange-600 dark:text-orange-400 border-orange-500/30', defaultPage: 'openclaw' },
];

export function Layout() {
  const { projectId, traceId, sessionId } = useParams<{ projectId?: string; traceId?: string; sessionId?: string }>();
  const { pathname } = useLocation();
  const { currentProject, projects, loading, selectProject } = useProjects();
  const navigate = useNavigate();
  const [isTauri, setIsTauri] = useState(false);

  // Enabled services — persisted to localStorage
  const validKeys = new Set(SERVICE_DEFS.map(s => s.key));
  const [enabledServices, setEnabledServices] = useState<Set<string>>(() => {
    try {
      const stored = localStorage.getItem('agentreplay_enabled_services');
      if (stored) {
        const parsed: string[] = JSON.parse(stored);
        const filtered = parsed.filter(k => validKeys.has(k));
        if (filtered.length > 0) return new Set(filtered);
      }
    } catch {}
    return new Set(['agents', 'claude', 'testing']);
  });

  const toggleService = useCallback((key: string) => {
    setEnabledServices(prev => {
      const next = new Set(prev);
      if (next.has(key)) {
        if (next.size <= 1) return prev; // keep at least one
        next.delete(key);
      } else {
        next.add(key);
      }
      localStorage.setItem('agentreplay_enabled_services', JSON.stringify([...next]));
      window.dispatchEvent(new Event('services-changed'));
      return next;
    });

    // If toggling ON, try to switch to a matching project and navigate to its default page
    const svc = SERVICE_DEFS.find(s => s.key === key);
    if (svc?.projectHint) {
      const match = projects.find(p =>
        p.name.toLowerCase().includes(svc.projectHint!)
      );
      if (match && match.project_id !== currentProject?.project_id) {
        selectProject(match.project_id).then(() => {
          navigate(`/projects/${match.project_id}/${svc.defaultPage || 'traces'}`);
        });
        return;
      }
    }
    // Navigate to the service's default page within current project
    if (svc?.defaultPage && currentProject) {
      navigate(`/projects/${currentProject.project_id}/${svc.defaultPage}`);
    }
  }, [projects, currentProject, selectProject, navigate]);

  // Determine if we're on a detail page (needs breadcrumbs)
  const isDetailPage = traceId || sessionId || pathname.includes('/prompts/') || pathname.includes('/runs/');

  useEffect(() => {
    // Check if running in Tauri
    const checkTauri = () => {
      // Check for Tauri v1 or v2 global objects
      return typeof window !== 'undefined' && (
        '__TAURI__' in window ||
        '__TAURI_INTERNALS__' in window
      );
    };
    setIsTauri(checkTauri());
  }, []);

  useEffect(() => {
    if (!projectId || currentProject?.project_id === projectId) {
      return;
    }
    selectProject(projectId).catch((error) => {
      console.warn('Failed to sync project from route', error);
    });
  }, [projectId, currentProject?.project_id, selectProject]);

  return (
    <div className="flex h-screen w-full bg-background overflow-hidden flex-col">

      {/* Main content area */}
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />

        {/* Main content with safe area padding */}
        <main className="flex-1 flex flex-col min-w-0 overflow-hidden relative">
          {/* Header — acts as drag region in Tauri; interactive children opt out */}
          <header
            className={cn(
              'flex items-center justify-between border-b border-border/40 bg-card/80 backdrop-blur-sm px-4 flex-shrink-0 relative',
              isTauri ? 'h-[54px]' : 'h-11'
            )}
            data-tauri-drag-region
            style={isTauri ? { WebkitAppRegion: 'drag' } as any : undefined}
          >
            <div className="flex items-center gap-3 h-full" style={isTauri ? { WebkitAppRegion: 'no-drag' } as any : undefined}>
              <ProjectSwitcher />
              {!isDetailPage && (
                <>
                  <div className="w-px h-5 bg-border/40" />
                  <ProjectSetupInfo
                    projectId={currentProject?.project_id}
                    projectName={currentProject?.name}
                  />
                  {currentProject && (
                    <div className="hidden lg:flex items-center gap-3">
                      <div className="w-px h-5 bg-border/40" />
                      <div className="flex items-center gap-1.5">
                        <span className="uppercase tracking-widest text-[9px] text-muted-foreground/70 font-medium">Scope</span>
                        <span className="text-[12px] font-semibold text-foreground">
                          {currentProject.name}
                          <span className="text-muted-foreground/60 font-normal ml-1 text-[11px]">#{currentProject.project_id}</span>
                        </span>
                      </div>
                    </div>
                  )}
                  {/* Service Toggle Pills */}
                  <div className="hidden sm:flex items-center gap-3">
                    <div className="w-px h-5 bg-border/40" />
                    <div className="flex items-center gap-1">
                      {SERVICE_DEFS.map(s => {
                        const isOn = enabledServices.has(s.key);
                        return (
                          <button
                            key={s.key}
                            onClick={() => toggleService(s.key)}
                            className={cn(
                              'px-2 py-[3px] rounded-md text-[10px] font-semibold border transition-all duration-150',
                              isOn
                                ? s.activeColor
                                : 'bg-transparent text-muted-foreground/40 border-transparent hover:border-border/40 hover:text-muted-foreground/60'
                            )}
                            title={`${isOn ? 'Hide' : 'Show'} ${s.label}`}
                          >
                            {s.label}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                </>
              )}
              {isDetailPage && (
                <Breadcrumbs className="hidden sm:flex" />
              )}
            </div>
            <div className="flex items-center gap-2" style={isTauri ? { WebkitAppRegion: 'no-drag' } as any : undefined}>
              <CommandPalette />
            </div>
          </header>

          {/* Main Content with safe area padding */}
          <div className="flex-1 overflow-auto px-4 pb-4">
            <Outlet />
          </div>
        </main>
      </div>
    </div>
  );
}
