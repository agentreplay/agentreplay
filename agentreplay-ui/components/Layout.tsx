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
import Titlebar from './Titlebar';
import { useProjects } from '../src/context/project-context';
import { ProjectSwitcher } from './ProjectSwitcher';
import { ProjectSetupInfo } from './ProjectSetupInfo';
import { CommandPalette } from './CommandPalette';
import { Breadcrumbs } from './Breadcrumbs';

export function Layout() {
  const { projectId, traceId, sessionId } = useParams<{ projectId?: string; traceId?: string; sessionId?: string }>();
  const { pathname } = useLocation();
  const { currentProject, loading, selectProject } = useProjects();
  const [isTauri, setIsTauri] = useState(false);

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
        <main className={`flex-1 flex flex-col min-w-0 overflow-hidden bg-background relative`}>
          {/* Header with better spacing */}
          <header className={`flex h-14 items-center justify-between border-b border-border/60 bg-background/95 px-6 backdrop-blur flex-shrink-0 relative z-10 ${isTauri ? 'pt-0' : ''}`}>
            {/* Drag region for main content area */}
            {isTauri && (
              <div
                className="absolute inset-0 z-[-1]"
                data-tauri-drag-region
                style={{ WebkitAppRegion: 'drag' } as any}
              />
            )}
            <div className="flex items-center gap-3">
              <ProjectSwitcher />
              {!isDetailPage && (
                <>
                  <ProjectSetupInfo
                    projectId={currentProject?.project_id}
                    projectName={currentProject?.name}
                  />
                  {currentProject && (
                    <div className="hidden lg:flex flex-col text-xs text-textSecondary">
                      <span className="uppercase tracking-widest text-[11px] text-textTertiary">Current scope</span>
                      <span className="font-semibold text-textPrimary">
                        {currentProject.name}
                        <span className="text-textTertiary font-normal ml-1.5">#{currentProject.project_id}</span>
                      </span>
                    </div>
                  )}
                </>
              )}
              {isDetailPage && (
                <Breadcrumbs className="hidden sm:flex" />
              )}
            </div>
            <div className="flex items-center gap-3">
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
