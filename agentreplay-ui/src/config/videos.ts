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

// ============================================================================
// Video Configuration - Fetched from GitHub JSON at runtime
// ============================================================================
// Videos are loaded from video-config.json hosted on GitHub at runtime.
// To update videos, edit video-config.json in the repo and push — no app rebuild.
// A hardcoded fallback is used when offline or before the fetch completes.

export interface VideoConfig {
  pageId: string;
  title: string;
  description: string;
  youtubeUrl: string;
  videoId?: string;
}

// Helper to extract YouTube video ID from various URL formats
export function getYouTubeVideoId(url: string): string | null {
  const patterns = [
    /(?:youtube\.com\/watch\?v=|youtu\.be\/|youtube\.com\/embed\/)([^&\n?#]+)/,
    /^([a-zA-Z0-9_-]{11})$/, // Direct video ID
  ];
  
  for (const pattern of patterns) {
    const match = url.match(pattern);
    if (match) return match[1];
  }
  return null;
}

// ============================================================================
// GITHUB JSON CONFIG URL
// ============================================================================
const VIDEO_CONFIG_URL =
  'https://raw.githubusercontent.com/agentreplay/agentreplay/refs/heads/main/video-config.json';

// Cache: loaded once per app session
let _remoteVideos: Record<string, VideoConfig> | null = null;
let _fetchPromise: Promise<Record<string, VideoConfig> | null> | null = null;

// Parse the JSON config into our internal format
function parseVideoConfig(data: Record<string, unknown>): Record<string, VideoConfig> {
  const result: Record<string, VideoConfig> = {};
  const videos = (data.videos || data) as Record<string, Record<string, string>>;

  for (const [pageId, entry] of Object.entries(videos)) {
    if (pageId.startsWith('_') || typeof entry !== 'object') continue;
    result[pageId] = {
      pageId,
      title: entry.title || '',
      description: entry.description || '',
      youtubeUrl: entry.youtube_url || entry.youtubeUrl || '',
    };
  }
  return result;
}

// Fetch remote config (called once, cached)
async function fetchRemoteVideos(): Promise<Record<string, VideoConfig> | null> {
  try {
    const response = await fetch(VIDEO_CONFIG_URL, {
      signal: AbortSignal.timeout(5000),
      cache: 'no-cache',
    });
    if (!response.ok) return null;
    const data = await response.json();
    const parsed = parseVideoConfig(data);
    if (Object.keys(parsed).length > 0) {
      console.log(`[videos] Loaded ${Object.keys(parsed).length} video configs from GitHub`);
      return parsed;
    }
    return null;
  } catch (err) {
    console.warn('[videos] Failed to fetch remote video config, using fallback', err);
    return null;
  }
}

// Trigger fetch on module load (non-blocking)
function ensureFetched(): void {
  if (!_fetchPromise) {
    _fetchPromise = fetchRemoteVideos().then((videos) => {
      _remoteVideos = videos;
      return videos;
    });
  }
}

// Start fetching immediately on import
ensureFetched();

// ============================================================================
// HARDCODED FALLBACK - Used when offline or before remote fetch completes
// ============================================================================
const fallbackVideos: Record<string, VideoConfig> = {
  dashboard: { pageId: 'dashboard', title: 'Getting Started with Agentreplay', description: 'Learn how to navigate the dashboard.', youtubeUrl: 'https://youtu.be/3dhz36V0-L4' },
  traces: { pageId: 'traces', title: 'Understanding Traces', description: 'How to view and analyze LLM traces.', youtubeUrl: 'https://youtu.be/3dhz36V0-L4' },
  sessions: { pageId: 'sessions', title: 'Session Management', description: 'View and analyze conversation sessions.', youtubeUrl: 'https://youtu.be/3dhz36V0-L4' },
  search: { pageId: 'search', title: 'Semantic Search', description: 'Use AI-powered natural language search.', youtubeUrl: 'https://youtu.be/3dhz36V0-L4' },
  prompts: { pageId: 'prompts', title: 'Prompt Registry', description: 'Version control and manage your prompts.', youtubeUrl: 'https://youtu.be/3dhz36V0-L4' },
  compare: { pageId: 'compare', title: 'Model Comparison', description: 'Compare responses from multiple models.', youtubeUrl: 'https://youtu.be/3dhz36V0-L4' },
  memory: { pageId: 'memory', title: 'Memory & Vector Storage', description: 'Semantic memory for your AI agents.', youtubeUrl: 'https://youtu.be/3dhz36V0-L4' },
  evals: { pageId: 'evals', title: 'Running Evaluations', description: 'Evaluate AI outputs with custom criteria.', youtubeUrl: 'https://youtu.be/3dhz36V0-L4' },
  analytics: { pageId: 'analytics', title: 'Analytics & Metrics', description: 'AI performance metrics and costs.', youtubeUrl: 'https://youtu.be/3dhz36V0-L4' },
  settings: { pageId: 'settings', title: 'Configuring Agentreplay', description: 'Configure embeddings, models, and settings.', youtubeUrl: 'https://youtu.be/3dhz36V0-L4' },
};

// Exported as pageVideos for backward compatibility
export const pageVideos = fallbackVideos;

// Get video config for a page (prefers remote, falls back to hardcoded)
export function getVideoForPage(pageId: string): VideoConfig | null {
  if (_remoteVideos && _remoteVideos[pageId]) {
    return _remoteVideos[pageId];
  }
  return fallbackVideos[pageId] || null;
}

// Check if a page has a video
export function hasVideo(pageId: string): boolean {
  if (_remoteVideos) {
    return pageId in _remoteVideos && !!_remoteVideos[pageId].youtubeUrl;
  }
  return pageId in fallbackVideos && !!fallbackVideos[pageId].youtubeUrl;
}
