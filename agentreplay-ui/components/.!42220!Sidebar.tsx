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

import { useMemo, useState, useEffect } from 'react';
import { NavLink, useLocation, useNavigate } from 'react-router-dom';
import {
  Activity,
  BarChart2,
  Beaker,
  BookOpen,
  ChevronLeft,
  MessageSquare,
  NotebookPen,
  Settings,
  HardDrive,
  Search,
  Scale,
  Lightbulb,
  Puzzle,
  Database,
  Wrench,
  GitBranch,
  Keyboard,
  DollarSign,
  Bug,
} from 'lucide-react';
import { cn } from '../lib/utils';
import { useProjects } from '../src/context/project-context';
import { useAppMode } from '../src/context/app-mode-context';

const navItems = [
  { id: 'traces', label: 'Traces', icon: Activity, segment: 'traces', shortcut: '1', tier: 'basic' as const },
  { id: 'search', label: 'Search', icon: Search, segment: 'search', shortcut: '2', tier: 'basic' as const },
  { id: 'insights', label: 'Insights', icon: Lightbulb, segment: 'insights', shortcut: '3', tier: 'pro' as const },
  { id: 'evaluations', label: 'Evaluations', icon: Beaker, segment: 'evaluations', shortcut: '4', tier: 'basic' as const },
  { id: 'eval-pipeline', label: 'Eval Pipeline', icon: GitBranch, segment: 'eval-pipeline', tier: 'pro' as const },
  { id: 'prompts', label: 'Prompts', icon: NotebookPen, segment: 'prompts', shortcut: '6', tier: 'basic' as const },
  { id: 'model-comparison', label: 'Compare', icon: Scale, segment: 'model-comparison', tier: 'pro' as const },
  { id: 'tools', label: 'Tools', icon: Wrench, segment: 'tools', tier: 'pro' as const },
  { id: 'memory', label: 'Memory', icon: Database, segment: 'memory', tier: 'pro' as const },
  { id: 'plugins', label: 'Plugins', icon: Puzzle, segment: 'plugins', tier: 'pro' as const },
  { id: 'analytics', label: 'Analytics', icon: BarChart2, segment: 'analytics', shortcut: '7', tier: 'basic' as const },
  { id: 'costs', label: 'Costs', icon: DollarSign, segment: 'costs', tier: 'pro' as const },
  { id: 'storage', label: 'Storage', icon: HardDrive, segment: 'storage', tier: 'pro' as const },
  { id: 'docs', label: 'Docs', icon: BookOpen, segment: 'docs', tier: 'basic' as const },
  { id: 'settings', label: 'Settings', icon: Settings, segment: 'settings', shortcut: ',', tier: 'basic' as const },
];

export default function Sidebar() {
  const { currentProject } = useProjects();
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const { appMode } = useAppMode();

  // Initialize from localStorage if available
  const [collapsed, setCollapsed] = useState(() => {
    if (typeof window !== 'undefined') {
      const stored = localStorage.getItem('sidebar_collapsed');
      return stored ? stored === 'true' : true;
    }
    return true;
  });

  const [showShortcuts, setShowShortcuts] = useState(false);
  const [experimentalFeatures, setExperimentalFeatures] = useState(false);

  useEffect(() => {
    // Read experimental features setting
    try {
      const savedSettings = localStorage.getItem('agentreplay_settings');
      if (savedSettings) {
        const parsed = JSON.parse(savedSettings);
        setExperimentalFeatures(parsed.ui?.experimental_features || false);
      }
    } catch (e) {
      console.error('Failed to read settings in Sidebar', e);
    }

    const handleStorageChange = () => {
      try {
        const savedSettings = localStorage.getItem('agentreplay_settings');
        if (savedSettings) {
          const parsed = JSON.parse(savedSettings);
          setExperimentalFeatures(parsed.ui?.experimental_features || false);
        }
      } catch (e) {
        // ignore
      }
    };

    // Listen for storage changes to update sidebar immediately when settings change
    window.addEventListener('storage', handleStorageChange);
    // Custom event dispatch is often needed for same-window updates
    window.addEventListener('settings-changed', handleStorageChange);

    return () => {
      window.removeEventListener('storage', handleStorageChange);
      window.removeEventListener('settings-changed', handleStorageChange);
    };
  }, []);

  const toggleCollapsed = () => {
    const newState = !collapsed;
    setCollapsed(newState);
    localStorage.setItem('sidebar_collapsed', String(newState));
  };

  // Filter items based on experimental flag and app mode
  const visibleNavItems = useMemo(() => {
    const experimentalIds = ['insights', 'storage', 'tools', 'plugins'];
    return navItems.filter(item => {
      // Experimental features gate
      if (experimentalIds.includes(item.id)) {
        if (!experimentalFeatures) return false;
      }
      // App mode gate: in basic mode, only show basic-tier items
      if (appMode === 'basic' && item.tier === 'pro') {
        return false;
      }
      return true;
    });
  }, [experimentalFeatures, appMode]);

  // Keyboard navigation shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Only handle when Cmd/Ctrl is pressed
      if (!e.metaKey && !e.ctrlKey) return;

      // Don't intercept if in input/textarea
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

      const basePath = currentProject ? `/projects/${currentProject.project_id}` : null;
      if (!basePath) return;

      const item = visibleNavItems.find(i => i.shortcut === e.key);
      if (item) {
        e.preventDefault();
        navigate(`${basePath}/${item.segment}`);
      }

      // Toggle sidebar with Cmd+B
      if (e.key === 'b') {
        e.preventDefault();
        toggleCollapsed();
      }

      // Show shortcuts overlay with Cmd+/
      if (e.key === '/') {
        e.preventDefault();
        setShowShortcuts(s => !s);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [currentProject, navigate, visibleNavItems]);

  const basePath = useMemo(() => {
    if (!currentProject) return null;
    return `/projects/${currentProject.project_id}`;
  }, [currentProject]);

  return (
    <aside
      className={cn(
        'group relative flex flex-col border-r border-border bg-card pt-14 pb-4 text-sm transition-all duration-300 ease-in-out',
        collapsed ? 'w-[80px] px-2' : 'w-64 px-3'
      )}
    >
      <button
        type="button"
        className={cn(
          "absolute -right-3 top-6 flex h-6 w-6 items-center justify-center rounded-full border border-border bg-background text-textSecondary shadow-sm transition-all hover:bg-surface hover:text-textPrimary z-20",
          "opacity-0 group-hover:opacity-100 focus:opacity-100"
        )}
        onClick={toggleCollapsed}
        style={{ WebkitAppRegion: 'no-drag' } as any}
        aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
      >
        <ChevronLeft className={cn('h-3 w-3 transition-transform duration-300', collapsed && 'rotate-180')} />
      </button>

      {/* Drag region for macOS traffic lights area */}
      <div
        className="absolute top-0 left-0 right-0 h-14 z-10"
        data-tauri-drag-region
        style={{ WebkitAppRegion: 'drag' } as any}
      />

      {/* Agentreplay Icon/Logo */}
      <div
        className={cn(
          'mb-6 flex items-center justify-center transition-all duration-200',
          collapsed ? 'px-1' : 'px-3'
        )}
      >
        <div className={cn('flex items-center', collapsed ? 'flex-col gap-1' : 'gap-2')}>
          <img
            src="/logo.svg"
            alt="Agentreplay"
            className={cn(
              'rounded-lg shadow-lg transition-all duration-200',
              collapsed ? 'w-10 h-10' : 'w-10 h-10'
            )}
          />
          {!collapsed && (
            <span className="flex h-5 items-center rounded-full bg-orange-500 px-2 text-[10px] font-semibold uppercase leading-none tracking-wide text-white shadow-sm">
              Alpha
            </span>
          )}
          {collapsed && (
            <span className="rounded-full bg-orange-500 px-2 py-0.5 text-[9px] font-semibold uppercase leading-none tracking-wide text-white shadow-sm">
              Alpha
            </span>
          )}
        </div>
      </div>

      <nav className="flex flex-1 flex-col gap-1 px-2">
        {visibleNavItems.map((item) => {
          const target = basePath ? `${basePath}/${item.segment}` : '#';
          const isActive = basePath ? pathname.startsWith(`${basePath}/${item.segment}`) : false;
          const Icon = item.icon;

          return (
            <NavLink
              key={item.id}
              to={target}
              className={({ isPending }) =>
                cn(
                  'flex items-center gap-3 rounded-lg py-2 text-[15px] font-medium transition-all relative group/item',
                  isActive
                    ? 'bg-primary/10 text-primary'
                    : 'text-muted-foreground hover:bg-secondary hover:text-foreground',
                  collapsed ? 'justify-center px-0' : 'px-3',
                  (!basePath || target === '#') && 'pointer-events-none opacity-50',
                  isPending && 'opacity-70'
                )
              }
              title={collapsed ? `${item.label}${item.shortcut ? ` (⌘${item.shortcut})` : ''}` : undefined}
            >
              <Icon className={cn("flex-shrink-0 transition-all", collapsed ? "h-6 w-6" : "h-5 w-5")} />

              {!collapsed && (
                <>
                  <span className="truncate flex-1 tracking-tight">{item.label}</span>
                  {item.shortcut && (
                    <kbd className="hidden group-hover/item:inline-flex h-5 items-center gap-1 rounded border border-border/40 bg-background/50 px-1.5 font-sans text-[11px] font-medium text-textTertiary">
                      ⌘{item.shortcut}
                    </kbd>
                  )}
                </>
              )}
            </NavLink>
          );
        })}
      </nav>

      {/* Keyboard shortcuts hint */}
      {!collapsed && (
        <button
          onClick={() => setShowShortcuts(true)}
          className="flex items-center gap-2 mx-2 mb-2 px-3 py-2 rounded-md text-xs text-textTertiary hover:text-textSecondary hover:bg-surface transition-colors"
        >
          <Keyboard className="w-3.5 h-3.5" />
          <span>Shortcuts</span>
          <kbd className="ml-auto px-1.5 py-0.5 rounded border border-border/60 bg-surface font-mono text-[10px]">⌘/</kbd>
        </button>
      )}

      {/* Report Issue button */}
      <a
        href="https://github.com/agentreplay/agentreplay/issues"
        target="_blank"
        rel="noopener noreferrer"
        className={cn(
          'flex items-center gap-2 mx-2 mb-2 rounded-md text-xs transition-colors',
          collapsed
            ? 'justify-center px-0 py-2 text-textTertiary hover:text-orange-600 dark:text-orange-400 hover:bg-orange-500/10'
            : 'px-3 py-2 text-textTertiary hover:text-orange-600 dark:text-orange-400 hover:bg-orange-500/10'
        )}
        title="Report an issue on GitHub"
      >
        <Bug className={cn('flex-shrink-0', collapsed ? 'w-5 h-5' : 'w-3.5 h-3.5')} />
        {!collapsed && <span>Report Issue</span>}
      </a>

      {/* Keyboard shortcuts overlay */}
      {showShortcuts && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
          onClick={() => setShowShortcuts(false)}
        >
          <div
            className="bg-surface border border-border rounded-xl shadow-2xl p-6 max-w-md w-full mx-4"
            onClick={e => e.stopPropagation()}
          >
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-textPrimary flex items-center gap-2">
                <Keyboard className="w-5 h-5 text-primary" />
                Keyboard Shortcuts
              </h3>
              <button
                onClick={() => setShowShortcuts(false)}
                className="text-textTertiary hover:text-textPrimary"
              >
