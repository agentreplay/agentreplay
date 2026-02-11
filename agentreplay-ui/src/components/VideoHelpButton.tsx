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

import React from 'react';
import { Youtube } from 'lucide-react';

const YOUTUBE_CHANNEL_URL = 'https://www.youtube.com/channel/UCooI5ooJQzRtrlZk0nRt-ow';

interface VideoHelpButtonProps {
  pageId: string;
  className?: string;
  size?: 'sm' | 'md' | 'lg';
}

// ============================================================================
// Video Help Button - Opens YouTube channel in a new tab
// ============================================================================
export function VideoHelpButton({ pageId, className = '', size = 'md' }: VideoHelpButtonProps) {
  const sizeClasses = {
    sm: 'w-6 h-6',
    md: 'w-8 h-8',
    lg: 'w-10 h-10',
  };

  const iconSizes = {
    sm: 'w-3 h-3',
    md: 'w-4 h-4',
    lg: 'w-5 h-5',
  };

  return (
    <a
      href={YOUTUBE_CHANNEL_URL}
      target="_blank"
      rel="noopener noreferrer"
      className={`
        ${sizeClasses[size]} 
        rounded-lg 
        flex items-center justify-center 
        transition-all duration-200
        group
        ${className}
      `}
      style={{ border: '1px solid rgba(239,68,68,0.25)', backgroundColor: 'rgba(239,68,68,0.06)' }}
      title="Watch on YouTube"
      onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'rgba(239,68,68,0.12)'; e.currentTarget.style.borderColor = 'rgba(239,68,68,0.4)'; }}
      onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'rgba(239,68,68,0.06)'; e.currentTarget.style.borderColor = 'rgba(239,68,68,0.25)'; }}
    >
      <Youtube className={`${iconSizes[size]} group-hover:scale-110 transition-transform`} style={{ color: '#ef4444' }} />
    </a>
  );
}

export default VideoHelpButton;
