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

import React, { useEffect, useState } from 'react';
import { Outlet, useParams, useLocation } from 'react-router-dom';
import Sidebar from './Sidebar';
import { cn } from '../lib/utils';
import { useProjects } from '../src/context/project-context';
import { ProjectSwitcher } from './ProjectSwitcher';
import { ProjectSetupInfo } from './ProjectSetupInfo';
import { CommandPalette } from './CommandPalette';
import { Breadcrumbs } from './Breadcrumbs';
import { useAppMode } from '../src/context/app-mode-context';

export function Layout() {
  const { projectId, traceId, sessionId } = useParams<{ projectId?: string; traceId?: string; sessionId?: string }>();
  const { pathname } = useLocation();
  const { currentProject, loading, selectProject } = useProjects();
  const [isTauri, setIsTauri] = useState(false);
  const { appMode, setAppMode } = useAppMode();

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
                  {/* Basic / Pro Toggle */}
                  <div className="hidden sm:flex items-center gap-3">
                    <div className="w-px h-5 bg-border/40" />
                    <div
                      className="flex items-center rounded-full p-0.5 bg-secondary/60 border border-border/40"
                    >
                      <button
                        onClick={() => setAppMode('basic')}
                        className={cn(
                          'px-2.5 py-0.5 rounded-full text-[11px] font-semibold transition-all duration-200',
                          appMode === 'basic'
                            ? 'bg-primary text-primary-foreground shadow-sm'
                            : 'text-muted-foreground hover:text-foreground'
                        )}
                      >
                        Basic
                      </button>
                      <button
                        onClick={() => setAppMode('pro')}
                        className={cn(
                          'px-2.5 py-0.5 rounded-full text-[11px] font-semibold transition-all duration-200',
                          appMode === 'pro'
                            ? 'bg-primary text-primary-foreground shadow-sm'
                            : 'text-muted-foreground hover:text-foreground'
                        )}
                      >
                        Pro
                      </button>
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
