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
import { useNavigate, useLocation } from 'react-router-dom';
import { ChevronsUpDown, FolderKanban, Loader2, Plus, RefreshCcw } from 'lucide-react';
import { Button } from './ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from './ui/dropdown-menu';
import { useProjects } from '../src/context/project-context';
import { CreateProjectModal } from './CreateProjectModal';

export function ProjectSwitcher() {
  const { projects, currentProject, loading, selectProject, refreshProjects } = useProjects();
  const navigate = useNavigate();
  const location = useLocation();
  const [isOpen, setIsOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [showCreateModal, setShowCreateModal] = useState(false);

  const handleSelect = async (projectId: string) => {
    if (!projectId || projectId === currentProject?.project_id) {
      return;
    }
    setBusy(true);
    setIsOpen(false);
    try {
      await selectProject(projectId);

      // Navigate to the new project's corresponding page
      const currentPath = location.pathname;
      const pathSegments = currentPath.split('/');

      // If we're on a project-specific page, navigate to the same page for the new project
      if (pathSegments[1] === 'projects' && pathSegments[2]) {
        const pagePath = pathSegments.slice(3).join('/') || 'traces';
        navigate(`/projects/${projectId}/${pagePath}`);
      } else {
        // Otherwise, go to traces page
        navigate(`/projects/${projectId}/traces`);
      }
    } finally {
      setBusy(false);
    }
  };

  const disabled = loading || busy || projects.length === 0;

  const handleCreateProject = () => {
    // If user has no projects, take them to the setup wizard
    if (projects.length === 0) {
      window.location.href = '/get-started';
    } else {
      // If user has projects, show quick create modal
      setShowCreateModal(true);
    }
  };

  const handleProjectCreated = async () => {
    await refreshProjects();
  };

  return (
    <>
      <CreateProjectModal
        isOpen={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        onSuccess={handleProjectCreated}
      />

      <DropdownMenu open={isOpen} onOpenChange={setIsOpen}>
        <DropdownMenuTrigger
          className="flex items-center gap-2 rounded-lg border border-border/40 bg-transparent px-3 py-1.5 text-left font-semibold text-sm hover:bg-surface/50 transition-colors outline-none focus:ring-2 focus:ring-primary/20"
          onClick={() => setIsOpen(!isOpen)}
        >
          <FolderKanban className="h-4 w-4 text-primary" />
          <div className="flex flex-col leading-tight">
            <span className="text-xs text-textTertiary">Project</span>
            <span className="text-sm text-textPrimary">
              {currentProject?.name || (loading ? 'Loading…' : 'No project')}
            </span>
          </div>
          <ChevronsUpDown className="ml-2 h-3.5 w-3.5 text-textTertiary" />
        </DropdownMenuTrigger>
        <DropdownMenuContent className="w-72" align="start" sideOffset={4}>
          <DropdownMenuLabel className="flex items-center justify-between text-xs uppercase tracking-widest text-textTertiary">
            <span>Select project</span>
            <button
              type="button"
              className="inline-flex items-center gap-1 text-[11px] font-medium text-primary"
              onClick={() => refreshProjects()}
            >
              <RefreshCcw className="h-3 w-3" /> Refresh
            </button>
          </DropdownMenuLabel>
          <DropdownMenuSeparator />
          {projects.length === 0 ? (
            <DropdownMenuItem disabled className="text-sm text-textSecondary">
              {loading ? 'Looking for projects…' : 'No projects found'}
            </DropdownMenuItem>
          ) : (
            <div className="max-h-80 overflow-y-auto">
              {projects.map((project) => {
                const isSelected = project.project_id === currentProject?.project_id;
                return (
                  <DropdownMenuItem
                    key={project.project_id}
                    onClick={() => handleSelect(project.project_id)}
                    className={`cursor-pointer flex-col items-start py-2.5 px-4 ${isSelected ? 'bg-primary/10 text-primary' : ''
                      }`}
                  >
                    <div className="flex items-center gap-2 w-full">
                      <div className={`w-2 h-2 rounded-full ${isSelected ? 'bg-primary' : 'bg-transparent border border-border'}`} />
                      <div className="flex-1">
                        <div className="text-sm font-medium text-textPrimary">
                          {project.name}
                        </div>
                        <div className="text-xs text-textTertiary">
                          {project.trace_count.toLocaleString()} traces
                        </div>
                      </div>
                    </div>
                  </DropdownMenuItem>
                );
              })}
            </div>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem className="gap-2" onClick={handleCreateProject}>
            <Plus className="h-4 w-4 text-primary" />
            <span>Create new project</span>
          </DropdownMenuItem>
          {busy && (
            <div className="mt-2 flex items-center gap-2 rounded-md bg-muted/20 px-3 py-2 text-xs text-textSecondary">
              <Loader2 className="h-3 w-3 animate-spin" />
              Switching project…
            </div>
          )}
          {disabled && projects.length > 0 && (
            <p className="mt-2 px-3 text-[11px] text-textTertiary">
              {loading ? 'Syncing projects from desktop runtime…' : 'Ready to switch'}
            </p>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  );
}
