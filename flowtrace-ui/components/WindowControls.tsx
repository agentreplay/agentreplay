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
import { Minus, Square, X, AppWindow } from 'lucide-react';

export default function WindowControls() {
    const [appWindow, setAppWindow] = useState<any>(null);

    useEffect(() => {
        // Dynamically import Tauri API to avoid Server-Side Rendering errors
        import('@tauri-apps/api/window').then((mod) => {
            setAppWindow(mod.getCurrentWindow());
        });
    }, []);

    if (!appWindow) return null; // Don't render until Tauri is ready

    return (
        <div
            // CRITICAL: This attribute makes the window movable!
            data-tauri-drag-region
            className="fixed top-0 left-0 right-0 h-10 bg-black/90 border-b border-white/10 flex justify-between items-center z-50 select-none"
        >
            {/* Left Side: Title / Icon (Clicking here also drags the window) */}
            <div className="flex items-center gap-3 pl-4 text-gray-400 pointer-events-none">
                <AppWindow size={16} className="text-blue-500" />
                <span className="text-sm font-medium font-mono">FlowTrace</span>
            </div>

            {/* Right Side: Window Controls */}
            <div className="flex h-full">
                <button
                    onClick={() => appWindow.minimize()}
                    className="h-full px-4 hover:bg-gray-800 text-gray-400 hover:text-white transition-colors"
                >
                    <Minus size={16} />
                </button>
                <button
                    onClick={() => appWindow.toggleMaximize()}
                    className="h-full px-4 hover:bg-gray-800 text-gray-400 hover:text-white transition-colors"
                >
                    <Square size={14} />
                </button>
                <button
                    onClick={() => appWindow.close()}
                    className="h-full px-4 hover:bg-red-500 text-gray-400 hover:text-white transition-colors"
                >
                    <X size={16} />
                </button>
            </div>
        </div>
    );
}
