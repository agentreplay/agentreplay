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

import React, { CSSProperties } from 'react';

// Extend CSSProperties to include WebKit-specific properties
interface ExtendedCSSProperties extends CSSProperties {
  WebkitAppRegion?: 'drag' | 'no-drag';
}

export default function Titlebar() {
  const headerStyle: ExtendedCSSProperties = {
    // Subtle separator
    borderBottom: '1px solid rgba(255, 255, 255, 0.06)',
    // Slight blur effect for depth
    backdropFilter: 'blur(10px)',
    WebkitBackdropFilter: 'blur(10px)',
    backgroundColor: 'rgba(0, 0, 0, 0.4)',
    // Ensure drag region works
    WebkitAppRegion: 'drag',
  };

  const noDragStyle: ExtendedCSSProperties = {
    WebkitAppRegion: 'no-drag',
  };

  const dragStyle: ExtendedCSSProperties = {
    WebkitAppRegion: 'drag',
  };

  return (
    <header
      data-tauri-drag-region
      className="fixed top-0 left-0 right-0 h-10 flex items-center px-4 select-none z-50 cursor-grab active:cursor-grabbing"
      style={headerStyle}
    >
      {/* macOS traffic lights need space on the left */}
      <div 
        className="flex items-center gap-3 ml-16"
        style={noDragStyle}
      >
        {/* AgentReplay branding with actual app icon */}
        <div 
          className="flex items-center gap-2 text-sm font-medium text-white/90 pointer-events-none"
          data-tauri-drag-region
        >
          <img
            src="/icons/32x32.png"
            alt="AgentReplay"
            className="w-5 h-5 rounded-md"
          />
          <span>AgentReplay</span>
        </div>
      </div>
      
      {/* Invisible drag region to fill the rest of the titlebar */}
      <div 
        data-tauri-drag-region 
        className="flex-1 h-full"
        style={dragStyle}
      />
    </header>
  );
}
