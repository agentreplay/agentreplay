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

import React, { useState } from 'react';
import { Play, X, Youtube } from 'lucide-react';
import { getVideoForPage, getYouTubeVideoId, VideoConfig } from '../config/videos';

interface VideoHelpButtonProps {
  pageId: string;
  className?: string;
  size?: 'sm' | 'md' | 'lg';
}

// ============================================================================
// Video Help Button - Shows play icon that opens YouTube video in modal
// ============================================================================
export function VideoHelpButton({ pageId, className = '', size = 'md' }: VideoHelpButtonProps) {
  const [isOpen, setIsOpen] = useState(false);
  const video = getVideoForPage(pageId);

  if (!video || !video.youtubeUrl) {
    return null;
  }

  const videoId = getYouTubeVideoId(video.youtubeUrl);
  if (!videoId) {
    return null;
  }

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
    <>
      {/* Help Button */}
      <button
        onClick={() => setIsOpen(true)}
        className={`
          ${sizeClasses[size]} 
          rounded-lg 
          bg-red-500/10 hover:bg-red-500/20 
          border border-red-500/30 hover:border-red-500/50
          flex items-center justify-center 
          transition-all duration-200
          group
          ${className}
        `}
        title={`Watch: ${video.title}`}
      >
        <Youtube className={`${iconSizes[size]} text-red-500 group-hover:scale-110 transition-transform`} />
      </button>

      {/* Video Modal */}
      {isOpen && (
        <VideoModal
          video={video}
          videoId={videoId}
          onClose={() => setIsOpen(false)}
        />
      )}
    </>
  );
}

// ============================================================================
// Video Modal - Fullscreen overlay with YouTube embed
// ============================================================================
interface VideoModalProps {
  video: VideoConfig;
  videoId: string;
  onClose: () => void;
}

function VideoModal({ video, videoId, onClose }: VideoModalProps) {
  // Close on escape key
  React.useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handleEscape);
    return () => window.removeEventListener('keydown', handleEscape);
  }, [onClose]);

  return (
    <div 
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm"
      onClick={onClose}
    >
      <div 
        className="relative w-full max-w-4xl bg-surface border border-border rounded-xl overflow-hidden shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-border bg-background">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-lg bg-red-500/10 flex items-center justify-center">
              <Play className="w-5 h-5 text-red-500" />
            </div>
            <div>
              <h3 className="font-semibold text-textPrimary">{video.title}</h3>
              <p className="text-sm text-textSecondary">{video.description}</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="w-8 h-8 rounded-lg bg-surface hover:bg-red-500/10 flex items-center justify-center transition-colors"
          >
            <X className="w-4 h-4 text-textSecondary hover:text-red-500" />
          </button>
        </div>

        {/* YouTube Embed */}
        <div className="relative pt-[56.25%] bg-black">
          <iframe
            className="absolute inset-0 w-full h-full"
            src={`https://www.youtube.com/embed/${videoId}?autoplay=1&rel=0`}
            title={video.title}
            frameBorder="0"
            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
            allowFullScreen
          />
        </div>

        {/* Footer */}
        <div className="p-3 border-t border-border bg-background flex items-center justify-between">
          <a
            href={video.youtubeUrl}
            target="_blank"
            rel="noopener noreferrer"
            className="text-xs text-textSecondary hover:text-red-500 flex items-center gap-1"
          >
            <Youtube className="w-3 h-3" />
            Open in YouTube
          </a>
          <span className="text-xs text-textTertiary">Press ESC to close</span>
        </div>
      </div>
    </div>
  );
}

export default VideoHelpButton;
