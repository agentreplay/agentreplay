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

'use client';

import { useState, useEffect } from 'react';
import { ChevronDown, FolderOpen, Plus } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { agentreplayClient } from '@/lib/api/chronolake';
import { API_BASE_URL } from '../src/lib/agentreplay-api';
import { CreateProjectModal } from './CreateProjectModal';

interface Project {
  id: string;
  name: string;
  description?: string;
  created_at: string;
}

interface ProjectSelectorProps {
  onProjectChange?: (projectId: string) => void;
  currentProject?: string;
}

export function ProjectSelector({ onProjectChange, currentProject }: ProjectSelectorProps) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const [selectedProject, setSelectedProject] = useState<string>(currentProject || '');
  const [showCreateModal, setShowCreateModal] = useState(false);

  useEffect(() => {
    fetch(`${API_BASE_URL}/api/v1/projects`)
      .then((res) => res.json())
      .then((data) => {
        if (data.projects && Array.isArray(data.projects)) {
          setProjects(data.projects);
          if (!selectedProject && data.projects.length > 0) {
            setSelectedProject(data.projects[0].id);
          }
        }
      })
      .catch((err) => console.error('Failed to fetch projects:', err));
  }, []);

  useEffect(() => {
    if (currentProject) {
      setSelectedProject(currentProject);
    }
  }, [currentProject]);

  const handleProjectSelect = (projectId: string) => {
    setSelectedProject(projectId);
    setIsOpen(false);
    
    // Update API client context
    if (projectId) {
      agentreplayClient.setProject(projectId);
    }
    
    if (onProjectChange) {
      onProjectChange(projectId);
    }
  };

  const currentProjectData = projects.find((p) => p.id === selectedProject);

  const handleCreateProject = () => {
    setIsOpen(false);
    // If user has no projects, take them to the setup wizard
    if (projects.length === 0) {
      window.location.href = '/get-started';
    } else {
      // If user has projects, show quick create modal
      setShowCreateModal(true);
    }
  };

  const handleProjectCreated = async () => {
    // Refresh the projects list
    const response = await fetch(`${API_BASE_URL}/api/v1/projects`);
    const data = await response.json();
    if (data.projects && Array.isArray(data.projects)) {
      setProjects(data.projects);
    }
  };

  return (
    <>
      <CreateProjectModal
        isOpen={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        onSuccess={handleProjectCreated}
      />

      <div className="relative">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-2 px-4 py-2 rounded-lg bg-surface border border-border hover:bg-surface-hover transition-colors min-w-[200px]"
      >
        <FolderOpen className="w-4 h-4 text-textTertiary" />
        <span className="flex-1 text-left text-sm text-textPrimary truncate">
          {currentProjectData?.name || 'All Projects'}
        </span>
        <ChevronDown className={`w-4 h-4 text-textTertiary transition-transform ${isOpen ? 'rotate-180' : ''}`} />
      </button>

      <AnimatePresence>
        {isOpen && (
          <>
            <div
              className="fixed inset-0 z-10"
              onClick={() => setIsOpen(false)}
            />
            <motion.div
              initial={{ opacity: 0, y: -10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -10 }}
              className="absolute top-full left-0 right-0 mt-2 bg-surface border border-border rounded-lg shadow-lg overflow-hidden z-20"
            >
              <div className="max-h-[300px] overflow-y-auto">
                <button
                  onClick={() => handleProjectSelect('')}
                  className={`w-full text-left px-4 py-3 hover:bg-surface-hover transition-colors border-b border-border ${
                    selectedProject === '' ? 'bg-surface-elevated' : ''
                  }`}
                >
                  <div className="flex items-center gap-2">
                    <FolderOpen className="w-4 h-4 text-textTertiary" />
                    <span className="text-sm font-medium text-textPrimary">All Projects</span>
                  </div>
                </button>

                {projects.map((project) => (
                  <button
                    key={project.id}
                    onClick={() => handleProjectSelect(project.id)}
                    className={`w-full text-left px-4 py-3 hover:bg-surface-hover transition-colors ${
                      selectedProject === project.id ? 'bg-surface-elevated border-l-2 border-primary' : ''
                    }`}
                  >
                    <div className="flex items-center gap-2 mb-1">
                      <FolderOpen className="w-4 h-4 text-primary" />
                      <span className="text-sm font-medium text-textPrimary">{project.name}</span>
                    </div>
                    {project.description && (
                      <p className="text-xs text-textTertiary ml-6">{project.description}</p>
                    )}
                  </button>
                ))}
              </div>

              <button
                onClick={handleCreateProject}
                className="w-full text-left px-4 py-3 border-t border-border hover:bg-surface-hover transition-colors"
              >
                <div className="flex items-center gap-2 text-primary">
                  <Plus className="w-4 h-4" />
                  <span className="text-sm font-medium">Create New Project</span>
                </div>
              </button>
            </motion.div>
          </>
        )}
      </AnimatePresence>
    </div>
    </>
  );
}
