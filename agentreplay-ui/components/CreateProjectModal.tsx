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

import { useState, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { X, Loader2, FolderPlus } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

interface CreateProjectModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

export function CreateProjectModal({ isOpen, onClose, onSuccess }: CreateProjectModalProps) {
  const [projectName, setProjectName] = useState('');
  const [description, setDescription] = useState('');
  const [creating, setCreating] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!projectName.trim()) return;

    setCreating(true);
    try {
      const response = await fetch('http://localhost:9600/api/v1/projects', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: projectName,
          description: description || undefined,
        }),
      });

      if (!response.ok) {
        const errorText = await response.text().catch(() => 'Unknown error');
        throw new Error(`Failed to create project: ${errorText}`);
      }

      const data = await response.json();
      console.log('Project created:', data);
      
      // Reset form and close
      setProjectName('');
      setDescription('');
      onSuccess();
      onClose();
    } catch (error) {
      console.error('Failed to create project:', error);
      alert('Error creating project: ' + (error instanceof Error ? error.message : 'Unknown error'));
    } finally {
      setCreating(false);
    }
  };

  const handleClose = () => {
    if (!creating) {
      setProjectName('');
      setDescription('');
      onClose();
    }
  };

  return createPortal(
    <AnimatePresence>
      {isOpen && (
        <div className="fixed inset-0 z-[9999] flex items-center justify-center p-4 overflow-y-auto">
          {/* Backdrop */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={handleClose}
            className="fixed inset-0 z-[9998] bg-black/60 backdrop-blur-sm"
          />

          {/* Modal */}
          <motion.div
            initial={{ opacity: 0, scale: 0.95 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.95 }}
            transition={{ type: 'spring', duration: 0.3 }}
            className="relative z-[9999] bg-surface rounded-2xl border border-border shadow-2xl max-w-lg w-full overflow-hidden my-auto"
          >
            {/* Header with gradient */}
            <div className="relative bg-gradient-to-br from-primary/10 to-primary/5 border-b border-border p-6">
              <div className="flex items-start gap-4">
                <div className="flex-shrink-0 w-12 h-12 rounded-xl bg-primary/20 flex items-center justify-center">
                  <FolderPlus className="w-6 h-6 text-primary" />
                </div>
                <div className="flex-1">
                  <h2 className="text-2xl font-bold text-textPrimary">Create New Project</h2>
                  <p className="text-sm text-textSecondary mt-1">
                    Organize your traces by application or environment
                  </p>
                </div>
                <button
                  onClick={handleClose}
                  disabled={creating}
                  className="flex-shrink-0 p-2 hover:bg-surface-hover rounded-lg transition-colors disabled:opacity-50"
                >
                  <X className="w-5 h-5 text-textSecondary" />
                </button>
              </div>
            </div>

            {/* Form */}
            <form onSubmit={handleSubmit} className="p-6 space-y-5">
              <div>
                <label className="block text-sm font-semibold text-textPrimary mb-2">
                  Project Name <span className="text-red-500">*</span>
                </label>
                <input
                  type="text"
                  value={projectName}
                  onChange={(e) => setProjectName(e.target.value)}
                  placeholder="e.g., Production Chatbot, Staging API"
                  className="w-full px-4 py-3 bg-background border border-border rounded-xl text-textPrimary placeholder-textTertiary focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent transition-all"
                  disabled={creating}
                  autoFocus
                />
              </div>

              <div>
                <label className="block text-sm font-semibold text-textPrimary mb-2">
                  Description <span className="text-textTertiary text-xs font-normal">(Optional)</span>
                </label>
                <textarea
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="What does this project track? E.g., Customer-facing chatbot in production environment"
                  rows={3}
                  className="w-full px-4 py-3 bg-background border border-border rounded-xl text-textPrimary placeholder-textTertiary focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent resize-none transition-all"
                  disabled={creating}
                />
              </div>

              {/* Actions */}
              <div className="flex gap-3 pt-2">
                <button
                  type="button"
                  onClick={handleClose}
                  disabled={creating}
                  className="flex-1 px-4 py-3 bg-gray-200 border-2 border-gray-300 hover:bg-gray-300 text-gray-800 rounded-xl font-medium transition-all disabled:opacity-50 disabled:cursor-not-allowed dark:bg-gray-700 dark:border-gray-600 dark:hover:bg-gray-600 dark:text-white"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={!projectName.trim() || creating}
                  style={{ backgroundColor: '#2563eb', color: 'white' }}
                  className="flex-1 px-4 py-3 hover:bg-blue-700 rounded-xl font-semibold transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 shadow-lg"
                >
                  {creating ? (
                    <>
                      <Loader2 className="w-5 h-5 animate-spin" />
                      Creating...
                    </>
                  ) : (
                    <>
                      <FolderPlus className="w-5 h-5" />
                      Create Project
                    </>
                  )}
                </button>
              </div>
            </form>
          </motion.div>
        </div>
      )}
    </AnimatePresence>,
    document.body
  );
}
