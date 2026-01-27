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
// Video Configuration - Centralized YouTube links for help videos
// ============================================================================
// Update this file to manage all video content across the app.
// Videos will show as help icons on each page that open in a modal player.

export interface VideoConfig {
  pageId: string;
  title: string;
  description: string;
  youtubeUrl: string;
  // Extract video ID from URL for embedding
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
// VIDEO CONTENT - Edit this section to update videos
// ============================================================================
export const pageVideos: Record<string, VideoConfig> = {
  // Dashboard / Home
  dashboard: {
    pageId: 'dashboard',
    title: 'Getting Started with Flowtrace',
    description: 'Learn how to navigate the dashboard and understand your AI agent traces.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Traces Page
  traces: {
    pageId: 'traces',
    title: 'Understanding Traces',
    description: 'How to view, filter, and analyze your LLM traces.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Sessions Page
  sessions: {
    pageId: 'sessions',
    title: 'Session Management',
    description: 'View and analyze conversation sessions.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Agents / System Map
  agents: {
    pageId: 'agents',
    title: 'System Map & Agents',
    description: 'Visualize your AI system topology.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Timeline
  timeline: {
    pageId: 'timeline',
    title: 'Timeline View',
    description: 'Visualize trace execution over time.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Search
  search: {
    pageId: 'search',
    title: 'Semantic Search',
    description: 'Use AI-powered natural language search.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Insights
  insights: {
    pageId: 'insights',
    title: 'AI Insights',
    description: 'Automatic anomaly detection and pattern recognition.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Prompts
  prompts: {
    pageId: 'prompts',
    title: 'Prompt Registry',
    description: 'Version control and manage your prompts.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Model Comparison
  compare: {
    pageId: 'compare',
    title: 'Model Comparison',
    description: 'Compare responses from multiple models.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Memory Page
  memory: {
    pageId: 'memory',
    title: 'Memory & Vector Storage',
    description: 'Learn how to use semantic memory for your AI agents.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Evals Page
  evals: {
    pageId: 'evals',
    title: 'Running Evaluations',
    description: 'How to evaluate your AI outputs with custom criteria.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Datasets Page
  datasets: {
    pageId: 'datasets',
    title: 'Managing Datasets',
    description: 'Create and manage datasets for testing and evaluation.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Analytics Page
  analytics: {
    pageId: 'analytics',
    title: 'Analytics & Metrics',
    description: 'Understanding your AI performance metrics and costs.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Plugins
  plugins: {
    pageId: 'plugins',
    title: 'Plugins & Extensions',
    description: 'Extend Flowtrace with custom plugins.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Storage
  storage: {
    pageId: 'storage',
    title: 'Storage Inspector',
    description: 'Low-level view of stored data.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Settings Page
  settings: {
    pageId: 'settings',
    title: 'Configuring Flowtrace',
    description: 'How to configure embeddings, models, and other settings.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },

  // Playground Page
  playground: {
    pageId: 'playground',
    title: 'Using the Playground',
    description: 'Test prompts and compare model outputs.',
    youtubeUrl: 'https://youtu.be/3dhz36V0-L4',
  },
};

// Get video config for a page
export function getVideoForPage(pageId: string): VideoConfig | null {
  return pageVideos[pageId] || null;
}

// Check if a page has a video
export function hasVideo(pageId: string): boolean {
  return pageId in pageVideos && !!pageVideos[pageId].youtubeUrl;
}
