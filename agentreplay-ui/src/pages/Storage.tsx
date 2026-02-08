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
import { useParams, useNavigate } from 'react-router-dom';
import { Loader2, Database, FileJson, HardDrive, RefreshCw, AlertCircle, FolderOpen } from 'lucide-react';
import { formatDistanceToNow } from 'date-fns';
import axios from 'axios';
import { API_BASE_URL } from '../lib/agentreplay-api';
import { VideoHelpButton } from '../components/VideoHelpButton';

interface StorageRecord {
  key: string;
  timestamp_us: number;
  record_type: string;
  size_bytes: number;
  content: any;
}

interface Project {
  id: number;
  name: string;
}

export default function Storage() {
  const { projectId } = useParams<{ projectId: string }>();
  const navigate = useNavigate();
  const [records, setRecords] = useState<StorageRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string | undefined>(projectId);

  useEffect(() => {
    fetchProjects();
  }, []);

  useEffect(() => {
    if (projectId) {
      setSelectedProjectId(projectId);
    }
  }, [projectId]);

  useEffect(() => {
    if (selectedProjectId) {
      fetchStorageDump();
    }
  }, [selectedProjectId]);

  const fetchProjects = async () => {
    try {
      const response = await axios.get(`${API_BASE_URL}/api/v1/projects`);
      // API returns { projects: [...] } not an array directly
      const projectsArray = response.data?.projects || response.data || [];
      setProjects(Array.isArray(projectsArray) ? projectsArray : []);
      // If we don't have a project ID in URL, select the first project
      if (!projectId && projectsArray?.length > 0) {
        setSelectedProjectId(projectsArray[0].id.toString());
      }
    } catch (err) {
      console.error('Failed to fetch projects:', err);
      setProjects([]);
    }
  };

  const fetchStorageDump = async () => {
    if (!selectedProjectId) {
      setError('Please select a project');
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await axios.get(`/api/v1/storage/dump`, {
        params: { project_id: selectedProjectId, limit: 100 }
      });
      setRecords(response.data?.records || []);
    } catch (err: any) {
      console.error('Storage dump error:', err);
      if (err.response?.status === 404) {
        setError('Storage dump endpoint not available. The server may need to be restarted or the endpoint is not enabled.');
      } else if (err.response?.status === 400) {
        setError(err.response?.data || 'Bad request - please select a valid project');
      } else {
        setError(err.message || 'Failed to fetch storage dump');
      }
    } finally {
      setLoading(false);
    }
  };

  const handleProjectChange = (newProjectId: string) => {
    setSelectedProjectId(newProjectId);
    navigate(`/projects/${newProjectId}/storage`);
  };

  return (
    <div className="min-h-screen bg-background">
      <div className="max-w-[1600px] mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {/* Header */}
        <div className="mb-8">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h1 className="text-3xl font-bold text-textPrimary mb-2 flex items-center gap-3">
                <FolderOpen className="w-8 h-8 text-primary" />
                Storage Inspector
              </h1>
              <p className="text-textSecondary">
                Low-level view of data written to LSM Tree and Payload Store
              </p>
            </div>
            <div className="flex items-center gap-4">
              <VideoHelpButton pageId="storage" />
              {/* Project Selector */}
              <select
                value={selectedProjectId || ''}
                onChange={(e) => handleProjectChange(e.target.value)}
                className="px-4 py-2 bg-surface border border-border rounded-lg text-textPrimary focus:outline-none focus:ring-2 focus:ring-primary"
              >
                <option value="" disabled>Select Project</option>
                {projects.map((project) => (
                  <option key={project.id} value={project.id}>
                    {project.name} (ID: {project.id})
                  </option>
                ))}
              </select>
              <button
                onClick={fetchStorageDump}
                disabled={loading || !selectedProjectId}
                className="flex items-center gap-2 px-4 py-2 bg-primary text-background rounded-lg hover:bg-primary-hover transition-colors disabled:opacity-50"
              >
                <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
                Refresh
              </button>
            </div>
          </div>
        </div>

        {/* Error State */}
        {error && (
          <div className="mb-6 p-4 bg-red-500/10 border border-red-500/20 rounded-lg flex items-start gap-3">
            <AlertCircle className="w-5 h-5 text-red-500 mt-0.5 flex-shrink-0" />
            <div>
              <p className="text-red-500 font-medium">Error</p>
              <p className="text-red-400 text-sm">{error}</p>
            </div>
          </div>
        )}

        {/* No Project Selected */}
        {!selectedProjectId && !loading && (
          <div className="bg-surface rounded-xl border border-border p-12 text-center">
            <Database className="w-16 h-16 mx-auto mb-4 text-textTertiary opacity-50" />
            <h3 className="text-lg font-semibold text-textPrimary mb-2">No Project Selected</h3>
            <p className="text-textSecondary mb-4">Select a project to view its storage contents</p>
          </div>
        )}

        {/* Data Table */}
        {selectedProjectId && (
          <div className="bg-surface rounded-xl border border-border overflow-hidden shadow-sm">
            <div className="grid grid-cols-12 gap-4 px-6 py-4 bg-surface-elevated border-b border-border font-semibold text-sm text-textSecondary">
              <div className="col-span-3">Key</div>
              <div className="col-span-2">Type</div>
              <div className="col-span-2">Timestamp</div>
              <div className="col-span-1">Size</div>
              <div className="col-span-4">Content Preview</div>
            </div>

            <div className="divide-y divide-border max-h-[calc(100vh-300px)] overflow-y-auto">
              {loading ? (
                <div className="flex flex-col items-center justify-center py-20 text-textTertiary">
                  <Loader2 className="w-8 h-8 animate-spin mb-4" />
                  <p>Reading directly from disk...</p>
                </div>
              ) : records.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-20 text-textTertiary">
                  <Database className="w-12 h-12 mb-4 opacity-20" />
                  <p>No records found in storage for this project</p>
                  <p className="text-sm mt-2">Try creating some traces first</p>
                </div>
              ) : (
                records.map((record, idx) => (
                  <div 
                    key={`${record.key}-${idx}`}
                    className="grid grid-cols-12 gap-4 px-6 py-4 hover:bg-surface-hover transition-colors text-sm group items-start"
                  >
                    <div className="col-span-3 font-mono text-xs text-primary truncate" title={record.key}>
                      {record.key}
                    </div>
                    <div className="col-span-2 flex items-center gap-2">
                      {record.record_type.includes('Edge') ? (
                        <HardDrive className="w-4 h-4 text-blue-500" />
                      ) : (
                        <FileJson className="w-4 h-4 text-yellow-500" />
                      )}
                      <span className={record.record_type.includes('Edge') ? 'text-blue-500' : 'text-yellow-500'}>
                        {record.record_type}
                      </span>
                    </div>
                    <div className="col-span-2 text-textSecondary">
                      {formatDistanceToNow(new Date(record.timestamp_us / 1000), { addSuffix: true })}
                    </div>
                    <div className="col-span-1 font-mono text-textSecondary">
                      {record.size_bytes} B
                    </div>
                    <div className="col-span-4">
                      <pre className="font-mono text-xs bg-background p-2 rounded border border-border-subtle overflow-x-auto max-h-[100px] text-textPrimary">
                        {JSON.stringify(record.content, null, 2)}
                      </pre>
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        )}
        
        {selectedProjectId && (
          <div className="mt-4 text-center text-xs text-textTertiary">
            Showing last {records.length} records. Raw storage dump bypasses caching and indexing.
          </div>
        )}
      </div>
    </div>
  );
}
