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

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Activity,
  BarChart3,
  Beaker,
  Command,
  History,
  MessageSquare,
  Rows,
  ScrollText,
  Search,
  Settings,
  Sparkles,
  ToggleLeft,
} from 'lucide-react';
import { cn } from '../lib/utils';
import { useProjects } from '../src/context/project-context';
import { flowtraceClient } from '../src/lib/flowtrace-api';
import { COMMAND_PALETTE_EVENT, LIVE_MODE_EVENT } from '../src/lib/events';

interface PaletteCommand {
  id: string;
  label: string;
  section: string;
  action: () => void;
  hint?: string;
  shortcut?: string;
  icon: React.ReactNode;
  keywords?: string[];
  requiresProject?: boolean;
}

interface TracePreview {
  id: string;
  model?: string;
  user?: string;
  timestamp: string;
}

interface SessionPreview {
  id: string;
  user?: string;
  updatedAt: string;
}

export function triggerCommandPalette() {
  if (typeof window === 'undefined') return;
  window.dispatchEvent(new CustomEvent(COMMAND_PALETTE_EVENT));
}

export function CommandPalette() {
  const [isOpen, setIsOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [recentTraces, setRecentTraces] = useState<TracePreview[]>([]);
  const [recentSessions, setRecentSessions] = useState<SessionPreview[]>([]);
  const [loadingRecents, setLoadingRecents] = useState(false);
  const navigate = useNavigate();
  const { currentProject, projects, selectProject } = useProjects();

  const projectBase = currentProject ? `/projects/${currentProject.project_id}` : null;

  useEffect(() => {
    if (!isOpen) return;
    setLoadingRecents(true);
    let active = true;

    (async () => {
      try {
        const response = await flowtraceClient.listTraces({ limit: 20 });
        if (!active) return;
        const traceData = (response.traces || []).slice(0, 5).map((trace) => ({
          id: trace.trace_id,
          model: trace.metadata?.model || trace.metadata?.agent_name,
          user: trace.metadata?.user_id,
          timestamp: new Date((trace.timestamp_us || Date.now() * 1000) / 1000).toLocaleTimeString(),
        }));
        setRecentTraces(traceData);

        const sessionMap = new Map<string, SessionPreview>();
        for (const trace of response.traces || []) {
          if (!trace.session_id) continue;
          sessionMap.set(String(trace.session_id), {
            id: String(trace.session_id),
            user: trace.metadata?.user_id,
            updatedAt: new Date((trace.timestamp_us || Date.now() * 1000) / 1000).toLocaleTimeString(),
          });
          if (sessionMap.size >= 5) break;
        }
        setRecentSessions(Array.from(sessionMap.values()));
      } catch (error) {
        console.warn('Failed to hydrate recent commands', error);
      } finally {
        if (active) setLoadingRecents(false);
      }
    })();

    return () => {
      active = false;
    };
  }, [isOpen]);

  const baseCommands: PaletteCommand[] = useMemo(() => {
    const scoped = (segment: string) => (projectBase ? `${projectBase}/${segment}` : '/');
    return [
      {
        id: 'nav-traces',
        label: 'Go to Traces',
        section: 'Navigation',
        action: () => projectBase && navigate(scoped('traces')),
        hint: 'Most recent spans',
        shortcut: 'G T',
        icon: <Activity className="h-4 w-4" />,
        keywords: ['trace', 'spans', 'home'],
        requiresProject: true,
      },
      {
        id: 'nav-sessions',
        label: 'Go to Sessions',
        section: 'Navigation',
        action: () => projectBase && navigate(scoped('sessions')),
        shortcut: 'G S',
        icon: <MessageSquare className="h-4 w-4" />,
        keywords: ['session', 'conversation'],
        requiresProject: true,
      },
      {
        id: 'nav-evals',
        label: 'Go to Evaluations',
        section: 'Navigation',
        action: () => projectBase && navigate(scoped('evaluations')),
        shortcut: 'G E',
        icon: <Beaker className="h-4 w-4" />,
        keywords: ['eval', 'tests'],
        requiresProject: true,
      },
      {
        id: 'nav-prompts',
        label: 'Go to Prompts',
        section: 'Navigation',
        action: () => projectBase && navigate(scoped('prompts')),
        shortcut: 'G P',
        icon: <ScrollText className="h-4 w-4" />,
        keywords: ['prompts', 'playground'],
        requiresProject: true,
      },
      {
        id: 'nav-analytics',
        label: 'Go to Analytics',
        section: 'Navigation',
        action: () => projectBase && navigate(scoped('analytics')),
        shortcut: 'G A',
        icon: <BarChart3 className="h-4 w-4" />,
        keywords: ['cost', 'metrics'],
        requiresProject: true,
      },
      {
        id: 'nav-settings',
        label: 'Go to Settings',
        section: 'Navigation',
        action: () => projectBase && navigate(scoped('settings')),
        shortcut: 'G ,',
        icon: <Settings className="h-4 w-4" />,
        keywords: ['preferences'],
        requiresProject: true,
      },
      {
        id: 'action-live-mode',
        label: 'Toggle live mode',
        section: 'Quick Actions',
        action: () => {
          if (typeof window !== 'undefined') {
            window.dispatchEvent(new CustomEvent(LIVE_MODE_EVENT));
          }
        },
        icon: <ToggleLeft className="h-4 w-4" />,
        keywords: ['live', 'stream'],
      },
      {
        id: 'action-new-eval',
        label: 'Create new evaluation run',
        section: 'Quick Actions',
        action: () => projectBase && navigate(`${projectBase}/evaluations?view=runs&create=1`),
        icon: <Sparkles className="h-4 w-4" />,
        keywords: ['run', 'tests'],
        requiresProject: true,
      },
    ];
  }, [navigate, projectBase]);

  const projectCommands: PaletteCommand[] = useMemo(() => {
    return projects
      .filter((project) => project.project_id !== currentProject?.project_id)
      .map((project) => ({
        id: `switch-${project.project_id}`,
        label: `Switch to ${project.name}`,
        section: 'Projects',
        action: async () => {
          await selectProject(project.project_id);
          navigate(`/projects/${project.project_id}/traces`);
        },
        hint: `${project.trace_count.toLocaleString()} traces`,
        icon: <Rows className="h-4 w-4" />,
        keywords: ['project', project.name.toLowerCase()],
      }));
  }, [projects, currentProject?.project_id, selectProject, navigate]);

  const recentTraceCommands: PaletteCommand[] = recentTraces.map((trace) => ({
    id: `trace-${trace.id}`,
    label: `Open trace ${trace.id.slice(0, 8)}…`,
    section: 'Recent Traces',
    action: () => projectBase && navigate(`${projectBase}/traces/${trace.id}`),
    hint: `${trace.model || 'model'} · ${trace.timestamp}`,
    icon: <Activity className="h-4 w-4" />,
    requiresProject: true,
  }));

  const recentSessionCommands: PaletteCommand[] = recentSessions.map((session) => ({
    id: `session-${session.id}`,
    label: `Open session ${session.id}`,
    section: 'Recent Sessions',
    action: () => projectBase && navigate(`${projectBase}/sessions/${session.id}`),
    hint: session.user ? `User ${session.user}` : session.updatedAt,
    icon: <MessageSquare className="h-4 w-4" />,
    requiresProject: true,
  }));

  const traceSearchCommand: PaletteCommand[] = query.length >= 6 && projectBase
    ? [
        {
          id: `search-${query}`,
          label: `Open trace ${query}`,
          section: 'Quick Actions',
          action: () => navigate(`${projectBase}/traces/${query}`),
          icon: <Search className="h-4 w-4" />,
          keywords: ['trace', 'search'],
          requiresProject: true,
        },
      ]
    : [];

  const combinedCommands = useMemo(() => {
    return [
      ...traceSearchCommand,
      ...baseCommands,
      ...projectCommands,
      ...recentTraceCommands,
      ...recentSessionCommands,
    ].filter((command) => (command.requiresProject ? Boolean(projectBase) : true));
  }, [traceSearchCommand, baseCommands, projectCommands, recentTraceCommands, recentSessionCommands, projectBase]);

  const filteredCommands = useMemo(() => {
    if (!query) {
      return combinedCommands;
    }
    const q = query.toLowerCase();
    return combinedCommands.filter((command) =>
      command.label.toLowerCase().includes(q) || command.keywords?.some((keyword) => keyword.includes(q))
    );
  }, [combinedCommands, query]);

  const executeCommand = useCallback((command?: PaletteCommand) => {
    if (!command) return;
    command.action();
    setIsOpen(false);
    setQuery('');
    setSelectedIndex(0);
  }, []);

  useEffect(() => {
    if (selectedIndex >= filteredCommands.length) {
      setSelectedIndex(Math.max(filteredCommands.length - 1, 0));
    }
  }, [filteredCommands.length, selectedIndex]);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    const keyHandler = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setIsOpen(true);
        return;
      }
      if (event.key === 'Escape') {
        setIsOpen(false);
        setQuery('');
        return;
      }
      if (!isOpen) return;
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        setSelectedIndex((index) => Math.min(index + 1, Math.max(filteredCommands.length - 1, 0)));
        return;
      }
      if (event.key === 'ArrowUp') {
        event.preventDefault();
        setSelectedIndex((index) => Math.max(index - 1, 0));
        return;
      }
      if (event.key === 'Enter') {
        event.preventDefault();
        executeCommand(filteredCommands[selectedIndex]);
      }
    };

    window.addEventListener('keydown', keyHandler);
    return () => window.removeEventListener('keydown', keyHandler);
  }, [executeCommand, filteredCommands, isOpen, selectedIndex]);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    const listener = () => setIsOpen(true);
    window.addEventListener(COMMAND_PALETTE_EVENT, listener);
    return () => window.removeEventListener(COMMAND_PALETTE_EVENT, listener);
  }, []);

  if (!isOpen) return null;

  const grouped = new Map<string, PaletteCommand[]>();
  filteredCommands.forEach((command) => {
    if (!grouped.has(command.section)) {
      grouped.set(command.section, []);
    }
    grouped.get(command.section)!.push(command);
  });

  const sections = Array.from(grouped.entries());

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 px-4 pt-20 backdrop-blur-sm" onClick={() => setIsOpen(false)}>
      <div className="w-full max-w-2xl overflow-hidden rounded-2xl border border-border/70 bg-surface shadow-2xl" onClick={(event) => event.stopPropagation()}>
        <div className="flex items-center gap-3 border-b border-border/60 px-4 py-3">
          <Command className="h-4 w-4 text-textTertiary" />
          <input
            autoFocus
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setSelectedIndex(0);
            }}
            placeholder="Search traces, actions, or projects"
            className="flex-1 bg-transparent text-sm text-textPrimary outline-none placeholder:text-textTertiary"
          />
          <kbd className="rounded border border-border/60 bg-background px-2 py-0.5 text-[10px] text-textTertiary">
            ESC
          </kbd>
        </div>
        <div className="max-h-[420px] overflow-y-auto">
          {filteredCommands.length === 0 ? (
            <div className="flex flex-col items-center gap-2 px-6 py-12 text-center text-textTertiary">
              <Search className="h-8 w-8" />
              <p>No commands match “{query}”.</p>
            </div>
          ) : (
            sections.map(([section, commands], sectionIndex) => (
              <div key={section} className="border-b border-border/30 last:border-none">
                <p className="px-4 py-2 text-[11px] uppercase tracking-widest text-textTertiary">{section}</p>
                {commands.map((command) => {
                  const index = filteredCommands.findIndex((item) => item.id === command.id);
                  const active = index === selectedIndex;
                  return (
                    <button
                      key={command.id}
                      onClick={() => executeCommand(command)}
                      onMouseEnter={() => setSelectedIndex(index)}
                      className={cn(
                        'flex w-full items-center justify-between gap-3 px-4 py-2 text-left text-sm transition-colors',
                        active ? 'bg-primary/10 text-textPrimary' : 'hover:bg-surface-hover'
                      )}
                    >
                      <div className="flex items-center gap-3">
                        <span className="text-textSecondary">{command.icon}</span>
                        <div className="flex flex-col">
                          <span className="font-medium">{command.label}</span>
                          {command.hint && <span className="text-xs text-textTertiary">{command.hint}</span>}
                        </div>
                      </div>
                      {command.shortcut && (
                        <kbd className="rounded border border-border/60 bg-background px-2 py-0.5 text-[10px] text-textTertiary">
                          {command.shortcut}
                        </kbd>
                      )}
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>
        <div className="flex items-center justify-between border-t border-border/60 px-4 py-3 text-[11px] text-textTertiary">
          <div className="flex items-center gap-4">
            <span>
              <kbd className="rounded bg-surface px-1">↑↓</kbd> Navigate
            </span>
            <span>
              <kbd className="rounded bg-surface px-1">Enter</kbd> Run
            </span>
          </div>
          <div className="flex items-center gap-2">
            <History className="h-3.5 w-3.5" />
            {loadingRecents ? 'Updating activity…' : 'Recently fetched from FlowTrace' }
          </div>
        </div>
      </div>
    </div>
  );
}
