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

import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

type Theme = 'light' | 'dark' | 'system';
type ResolvedTheme = 'light' | 'dark';

const isDesktopRuntime = () =>
  typeof window !== 'undefined' && Boolean((window as any).__TAURI_IPC__);

const applyTheme = (theme: ResolvedTheme) => {
  document.documentElement.classList.toggle('dark', theme === 'dark');
};

// Get saved theme from localStorage settings
const getSavedTheme = (): Theme | null => {
  try {
    const savedSettings = localStorage.getItem('agentreplay_settings');
    if (savedSettings) {
      const settings = JSON.parse(savedSettings);
      return settings?.ui?.theme || null;
    }
  } catch (e) {
    console.warn('Failed to read saved theme:', e);
  }
  return null;
};

// ... lines 42-99 unchanged ...

// No saved theme, default to DARK (Principal Engineer Decision)
if (isDesktopRuntime()) {
  try {
    // ... (lines 78-97) ...
    // Actually, if we want to force dark by default, we should override system detection unless explicitly set to system.
    // But let's just make the fallback dark.
  } catch { } // omitted for brevity in thought, but I need to replace the block carefully.
}

// Wait, I can't multi-chunk widely separated lines easily without verify.
// I will replace the getSavedTheme function specifically.


// Resolve theme to light/dark
const resolveTheme = (theme: Theme): ResolvedTheme => {
  if (theme === 'system') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  return theme;
};

export async function initTheme() {
  if (typeof document === 'undefined') {
    return () => undefined;
  }

  // First, check localStorage for user's saved preference
  const savedTheme = getSavedTheme();
  if (savedTheme) {
    applyTheme(resolveTheme(savedTheme));

    // If system theme, still listen for changes
    if (savedTheme === 'system') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      const handleChange = (event: MediaQueryListEvent) => {
        // Re-check saved theme in case it changed
        const currentSaved = getSavedTheme();
        if (currentSaved === 'system') {
          applyTheme(event.matches ? 'dark' : 'light');
        }
      };
      mediaQuery.addEventListener('change', handleChange);
      return () => mediaQuery.removeEventListener('change', handleChange);
    }
    return () => undefined;
  }

  // No saved theme, use system detection
  if (isDesktopRuntime()) {
    try {
      const desktopWindow = getCurrentWindow();
      const systemTheme = await desktopWindow.theme();
      applyTheme(systemTheme ?? (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'));

      const unlisten = await listen<ResolvedTheme>('tauri://theme-changed', (event) => {
        // Only apply if user preference is 'system'
        const currentSaved = getSavedTheme();
        if (!currentSaved || currentSaved === 'system') {
          const nextTheme = event.payload || 'light';
          applyTheme(nextTheme);
        }
      });

      return () => {
        unlisten();
      };
    } catch (error) {
      console.warn('Failed to bind desktop theme listeners, falling back to media queries', error);
    }
  }

  const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
  // Default to Dark Mode as requested
  applyTheme('dark');

  const handleChange = (event: MediaQueryListEvent) => {
    const currentSaved = getSavedTheme();
    if (!currentSaved || currentSaved === 'system') {
      applyTheme(event.matches ? 'dark' : 'light');
    }
  };

  mediaQuery.addEventListener('change', handleChange);
  return () => mediaQuery.removeEventListener('change', handleChange);
}

// Export for use by Settings page
export { applyTheme, resolveTheme, getSavedTheme };
