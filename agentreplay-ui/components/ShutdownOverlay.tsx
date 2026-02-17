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

/**
 * Full-screen overlay shown during app shutdown.
 * Listens for the 'app-shutting-down' Tauri event emitted by the backend
 * when the window close is requested and graceful cleanup begins.
 */
export default function ShutdownOverlay() {
  const [isShuttingDown, setIsShuttingDown] = useState(false);

  useEffect(() => {
    // Only listen in Tauri environment
    const isTauri =
      typeof window !== 'undefined' &&
      ('__TAURI__' in window || '__TAURI_INTERNALS__' in window);

    if (!isTauri) return;

    let unlisten: (() => void) | undefined;

    import('@tauri-apps/api/event').then(({ listen }) => {
      listen('app-shutting-down', () => {
        setIsShuttingDown(true);
      }).then((fn) => {
        unlisten = fn;
      });
    });

    return () => {
      unlisten?.();
    };
  }, []);

  if (!isShuttingDown) return null;

  return (
    <div className="fixed inset-0 z-[9999] flex flex-col items-center justify-center bg-background/95 backdrop-blur-sm">
      {/* Spinner */}
      <div className="relative mb-6">
        <div className="h-10 w-10 animate-spin rounded-full border-[3px] border-muted-foreground/20 border-t-blue-500" />
      </div>

      {/* Text */}
      <p className="text-sm font-medium text-foreground">Closing AgentReplay...</p>
      <p className="mt-1 text-xs text-muted-foreground">Saving data &amp; cleaning up</p>
    </div>
  );
}
