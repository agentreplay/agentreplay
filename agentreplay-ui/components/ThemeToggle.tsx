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

import { useEffect, useState } from 'react';
import { Moon, Sun, Monitor } from 'lucide-react';

type Theme = 'light' | 'dark' | 'system';

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>('system');
  const [mounted, setMounted] = useState(false);
  
  // Ensure mounted to avoid hydration mismatch
  useEffect(() => {
    setMounted(true);
    const stored = localStorage.getItem('agentreplay-theme') as Theme;
    if (stored) {
      setTheme(stored);
      applyTheme(stored);
    } else {
      applyTheme('system');
    }
  }, []);
  
  const applyTheme = (newTheme: Theme) => {
    const root = document.documentElement;
    
    if (newTheme === 'system') {
      const systemPrefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      root.classList.toggle('dark', systemPrefersDark);
    } else {
      root.classList.toggle('dark', newTheme === 'dark');
    }
  };
  
  const changeTheme = (newTheme: Theme) => {
    setTheme(newTheme);
    localStorage.setItem('agentreplay-theme', newTheme);
    applyTheme(newTheme);
  };
  
  if (!mounted) {
    return <div className="w-10 h-10" />; // Placeholder
  }
  
  return (
    <div className="flex items-center gap-1 bg-surface-elevated rounded-lg p-1 border border-border">
      <button
        onClick={() => changeTheme('light')}
        className={`p-2 rounded transition-colors ${
          theme === 'light' 
            ? 'bg-primary text-white' 
            : 'text-textTertiary hover:text-textPrimary hover:bg-surface-hover'
        }`}
        title="Light mode"
      >
        <Sun className="w-4 h-4" />
      </button>
      <button
        onClick={() => changeTheme('system')}
        className={`p-2 rounded transition-colors ${
          theme === 'system' 
            ? 'bg-primary text-white' 
            : 'text-textTertiary hover:text-textPrimary hover:bg-surface-hover'
        }`}
        title="System mode"
      >
        <Monitor className="w-4 h-4" />
      </button>
      <button
        onClick={() => changeTheme('dark')}
        className={`p-2 rounded transition-colors ${
          theme === 'dark' 
            ? 'bg-primary text-white' 
            : 'text-textTertiary hover:text-textPrimary hover:bg-surface-hover'
        }`}
        title="Dark mode"
      >
        <Moon className="w-4 h-4" />
      </button>
    </div>
  );
}
