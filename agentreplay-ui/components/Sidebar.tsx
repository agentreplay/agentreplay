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
        'group relative flex flex-col border-r border-border/60 bg-card pt-14 pb-2 transition-all duration-300 ease-in-out',
        collapsed ? 'w-[56px]' : 'w-[208px]'
      )}
    >
      {/* Collapse toggle — appears on hover */}
      <button
        type="button"
        className={cn(
          "absolute -right-2.5 top-[22px] flex h-5 w-5 items-center justify-center rounded-full border border-border/60 bg-card text-muted-foreground shadow-sm transition-all hover:bg-secondary hover:text-foreground z-20",
          "opacity-0 group-hover:opacity-100 focus:opacity-100"
        )}
        onClick={toggleCollapsed}
        style={{ WebkitAppRegion: 'no-drag' } as any}
        aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
      >
        <ChevronLeft className={cn('h-2.5 w-2.5 transition-transform duration-300', collapsed && 'rotate-180')} />
      </button>

      {/* Drag region for macOS traffic lights area */}
      <div
        className="absolute top-0 left-0 right-0 h-14 z-10"
        data-tauri-drag-region
        style={{ WebkitAppRegion: 'drag' } as any}
      />

      {/* Logo */}
      <div className={cn(
        'mb-4 flex items-center transition-all duration-200',
        collapsed ? 'justify-center px-0' : 'px-4 gap-2'
      )}>
        <img
          src="/logo.svg"
          alt="Agentreplay"
          className="w-8 h-8 rounded-lg shadow-md flex-shrink-0"
        />
        {!collapsed && (
          <span className="inline-flex h-[18px] items-center rounded-full bg-orange-500/90 px-1.5 text-[9px] font-bold uppercase tracking-wider text-white">
            Alpha
          </span>
        )}
      </div>

      {/* Navigation */}
      <nav className={cn('flex flex-1 flex-col gap-0.5', collapsed ? 'px-1.5' : 'px-2')}>
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
                  'flex items-center rounded-md transition-all duration-150 relative group/item',
                  collapsed
                    ? 'justify-center h-9 w-full'
                    : 'gap-2.5 px-2.5 py-[7px]',
                  isActive
                    ? 'bg-primary/10 text-primary font-semibold'
                    : 'text-muted-foreground hover:bg-secondary/80 hover:text-foreground',
                  (!basePath || target === '#') && 'pointer-events-none opacity-40',
                  isPending && 'opacity-70'
                )
              }
              title={collapsed ? `${item.label}${item.shortcut ? ` (⌘${item.shortcut})` : ''}` : undefined}
            >
              <Icon className={cn("flex-shrink-0", collapsed ? "h-[18px] w-[18px]" : "h-4 w-4")} />

              {!collapsed && (
                <>
                  <span className="truncate flex-1 text-[13px] tracking-tight">{item.label}</span>
                  {item.shortcut && (
                    <kbd className="hidden group-hover/item:inline-flex h-4 items-center rounded border border-border/30 bg-background/40 px-1 font-sans text-[10px] text-muted-foreground/60">
                      ⌘{item.shortcut}
                    </kbd>
                  )}
                </>
              )}

              {/* Collapsed tooltip */}
              {collapsed && (
                <div className="absolute left-full ml-2 px-2 py-1 rounded-md bg-foreground text-background text-xs font-medium whitespace-nowrap opacity-0 pointer-events-none group-hover/item:opacity-100 transition-opacity shadow-lg z-50">
                  {item.label}
                  {item.shortcut && <span className="ml-1.5 opacity-60">⌘{item.shortcut}</span>}
                </div>
              )}
            </NavLink>
          );
        })}
      </nav>

      {/* Bottom actions */}
      <div className={cn('flex flex-col gap-0.5 mt-auto', collapsed ? 'px-1.5' : 'px-2')}>
        {/* Shortcuts */}
        {!collapsed && (
          <button
            onClick={() => setShowShortcuts(true)}
            className="flex items-center gap-2 px-2.5 py-[6px] rounded-md text-[12px] text-muted-foreground/70 hover:text-foreground hover:bg-secondary/80 transition-colors"
          >
            <Keyboard className="w-3.5 h-3.5" />
            <span>Shortcuts</span>
            <kbd className="ml-auto px-1 py-0.5 rounded border border-border/40 font-mono text-[9px] text-muted-foreground/50">⌘/</kbd>
          </button>
        )}

        {/* Report Issue */}
        <a
          href="https://github.com/agentreplay/agentreplay/issues"
          target="_blank"
          rel="noopener noreferrer"
          className={cn(
            'flex items-center rounded-md text-[12px] transition-colors text-muted-foreground/70 hover:text-orange-600 dark:hover:text-orange-400 hover:bg-orange-500/8',
            collapsed ? 'justify-center h-9 w-full' : 'gap-2 px-2.5 py-[6px]'
          )}
          title="Report an issue on GitHub"
        >
          <Bug className={cn('flex-shrink-0', collapsed ? 'w-4 h-4' : 'w-3.5 h-3.5')} />
          {!collapsed && <span>Report Issue</span>}
        </a>
      </div>

      {/* Keyboard shortcuts overlay */}
      {showShortcuts && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
          onClick={() => setShowShortcuts(false)}
        >
          <div
            className="bg-card border border-border rounded-xl shadow-2xl p-5 max-w-sm w-full mx-4"
            onClick={e => e.stopPropagation()}
          >
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-sm font-semibold text-foreground flex items-center gap-2">
                <Keyboard className="w-4 h-4 text-primary" />
                Keyboard Shortcuts
              </h3>
              <button
                onClick={() => setShowShortcuts(false)}
                className="text-muted-foreground hover:text-foreground text-xs"
              >
                ✕
              </button>
            </div>
            <div className="space-y-0.5">
              <div className="text-[10px] text-muted-foreground/60 uppercase tracking-wider mb-1.5">Navigation</div>
              {navItems.filter(i => i.shortcut).map(item => (
                <div key={item.id} className="flex items-center justify-between py-1 text-[13px]">
                  <span className="text-muted-foreground">{item.label}</span>
                  <kbd className="px-1.5 py-0.5 rounded border border-border bg-background font-mono text-[11px] text-muted-foreground/70">
                    ⌘{item.shortcut}
                  </kbd>
                </div>
              ))}
              <div className="border-t border-border/40 my-2" />
              <div className="text-[10px] text-muted-foreground/60 uppercase tracking-wider mb-1.5">General</div>
              {[
                { label: 'Command Palette', key: '⌘K' },
                { label: 'Toggle Sidebar', key: '⌘B' },
                { label: 'Show Shortcuts', key: '⌘/' },
              ].map(s => (
                <div key={s.key} className="flex items-center justify-between py-1 text-[13px]">
                  <span className="text-muted-foreground">{s.label}</span>
                  <kbd className="px-1.5 py-0.5 rounded border border-border bg-background font-mono text-[11px] text-muted-foreground/70">{s.key}</kbd>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Project info */}
      <div className={cn(
        'border-t border-border/30 mt-1 pt-2 pb-1 transition-all duration-300',
        collapsed ? 'px-1 flex justify-center' : 'px-3'
      )}>
        {currentProject ? (
          <div className={cn("flex flex-col", collapsed && "items-center")}>
            <div className={cn(
              "font-semibold text-foreground transition-all duration-200",
              collapsed ? "text-[9px] text-center leading-tight" : "text-[12px]"
            )}>
              {collapsed ? (
                <span className="block truncate">{currentProject.name.substring(0, 2).toUpperCase()}</span>
              ) : (
                currentProject.name
              )}
            </div>
            {!collapsed && (
              <p className="text-[11px] text-muted-foreground/70">{currentProject.trace_count.toLocaleString()} traces</p>
            )}
          </div>
        ) : (
          !collapsed && <p className="text-[11px] text-muted-foreground/50">Select a project</p>
        )}
      </div>

      {/* Version & Branding */}
      <div className={cn(
        'border-t border-border/30 pt-2 pb-0.5 transition-all duration-300',
        collapsed ? 'flex flex-col items-center' : 'px-3'
      )}>
        {!collapsed ? (
          <div className="flex items-center gap-1.5">
            <img
              src="/icons/32x32.png"
              alt="Agentreplay"
              className="w-5 h-5 rounded flex-shrink-0"
            />
            <div className="flex flex-col min-w-0">
              <span className="text-[11px] font-medium text-foreground/80">Agentreplay</span>
              <span className="text-[9px] text-muted-foreground/50">v0.1.0</span>
            </div>
          </div>
        ) : (
          <div className="flex flex-col items-center">
            <img
              src="/icons/32x32.png"
              alt="Agentreplay"
              className="w-5 h-5 rounded"
            />
          </div>
        )}
      </div>
    </aside>
  );
}
