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
import { createPortal } from 'react-dom';
import { Info, X } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { Button } from './ui/button';
import { EnvironmentConfig } from './EnvironmentConfig';

interface ProjectSetupInfoProps {
  projectId?: string;
  projectName?: string;
}

export function ProjectSetupInfo({ projectId, projectName }: ProjectSetupInfoProps) {
  const [isOpen, setIsOpen] = useState(false);

  if (!projectId) return null;

  const envVars = {
    FLOWTRACE_ENABLED: 'true',
    FLOWTRACE_SERVICE_NAME: (projectName || 'my-app').toLowerCase().replace(/\\s+/g, '-'),
    FLOWTRACE_URL: 'http://localhost:9600',
    FLOWTRACE_OTLP_ENDPOINT: 'http://localhost:4317',
    FLOWTRACE_TENANT_ID: '1',
    FLOWTRACE_PROJECT_ID: projectId,
  };

  return (
    <>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setIsOpen(true)}
        className="gap-2 text-textSecondary hover:text-primary transition-colors"
      >
        <Info className="h-4 w-4" />
        <span className="text-xs font-medium">Setup Info</span>
      </Button>

      {/* Render modal in portal to avoid clipping by header's backdrop-filter */}
      {typeof document !== 'undefined' && createPortal(
        <AnimatePresence>
          {isOpen && (
            <div className="fixed inset-0 z-[100] flex items-center justify-center p-4">
              {/* Backdrop */}
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                onClick={() => setIsOpen(false)}
                className="absolute inset-0 bg-black/60 backdrop-blur-sm"
              />

              {/* Modal */}
              <motion.div
                initial={{ opacity: 0, scale: 0.95, y: 10 }}
                animate={{ opacity: 1, scale: 1, y: 0 }}
                exit={{ opacity: 0, scale: 0.95, y: 10 }}
                transition={{ duration: 0.2, ease: "easeOut" }}
                className="relative bg-surface rounded-xl border border-border shadow-2xl w-full max-w-2xl max-h-[85vh] flex flex-col overflow-hidden"
              >
                {/* Header */}
                <div className="flex items-center justify-between p-6 border-b border-border flex-shrink-0 bg-surface/50 backdrop-blur-sm">
                  <div>
                    <h2 className="text-lg font-bold text-textPrimary flex items-center gap-2">
                      Setup Instructions
                    </h2>
                    <p className="text-xs text-textTertiary mt-1">
                      Configure your environment to start tracing with <span className="font-medium text-textSecondary">{projectName}</span>
                    </p>
                  </div>
                  <button
                    onClick={() => setIsOpen(false)}
                    className="p-2 hover:bg-surface-hover rounded-lg transition-colors text-textTertiary hover:text-textPrimary"
                  >
                    <X className="w-5 h-5" />
                  </button>
                </div>

                {/* Content - Scrollable */}
                <div className="p-6 overflow-y-auto custom-scrollbar">
                  <div className="mb-6">
                    <p className="text-sm text-textSecondary mb-4">
                      Choose your integration method below to see the relevant configuration.
                    </p>
                    <EnvironmentConfig
                      projectId={projectId}
                      projectName={projectName}
                      envVars={envVars}
                    />
                  </div>
                </div>

                {/* Footer */}
                <div className="p-4 border-t border-border bg-surface/50 backdrop-blur-sm flex justify-end">
                  <Button onClick={() => setIsOpen(false)} className="px-6 h-9 text-xs">
                    Done
                  </Button>
                </div>
              </motion.div>
            </div>
          )}
        </AnimatePresence>,
        document.body
      )}
    </>
  );
}
