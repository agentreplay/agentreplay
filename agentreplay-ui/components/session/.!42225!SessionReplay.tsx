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

import { useState, useEffect, useRef, useCallback } from 'react';
import { 
  Play, 
  Pause, 
  SkipBack, 
  SkipForward, 
  RotateCcw,
  FastForward,
  Rewind,
  Clock,
  Activity,
  MessageCircle,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react';
import { cn } from '../../lib/utils';

interface SessionMessage {
  id: string;
  role: 'system' | 'user' | 'assistant' | 'tool';
  content: string;
  timestamp: number;
  traceId: string;
  cost?: number;
  tokens?: number;
}

interface SessionReplayProps {
  messages: SessionMessage[];
  onMessageSelect?: (message: SessionMessage) => void;
  onTraceClick?: (traceId: string) => void;
}

type PlaybackSpeed = 0.5 | 1 | 1.5 | 2 | 4;

const PLAYBACK_SPEEDS: PlaybackSpeed[] = [0.5, 1, 1.5, 2, 4];

export function SessionReplay({ messages, onMessageSelect, onTraceClick }: SessionReplayProps) {
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [playbackSpeed, setPlaybackSpeed] = useState<PlaybackSpeed>(1);
  const [visibleMessages, setVisibleMessages] = useState<SessionMessage[]>([]);
  const timerRef = useRef<NodeJS.Timeout | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Calculate time between messages for realistic replay
  const getDelay = useCallback((currentMsg: SessionMessage, nextMsg?: SessionMessage) => {
    if (!nextMsg) return 1000;
    const timeDiff = (nextMsg.timestamp - currentMsg.timestamp) / 1000; // Convert to ms
    // Cap delays between 500ms and 5000ms for UX
    const cappedDelay = Math.min(Math.max(timeDiff, 500), 5000);
    return cappedDelay / playbackSpeed;
  }, [playbackSpeed]);

  // Play/pause effect
  useEffect(() => {
    if (isPlaying && currentIndex < messages.length) {
      const currentMsg = messages[currentIndex];
      const nextMsg = messages[currentIndex + 1];
      const delay = getDelay(currentMsg, nextMsg);

      timerRef.current = setTimeout(() => {
        setVisibleMessages(prev => [...prev, currentMsg]);
        setCurrentIndex(prev => prev + 1);
        onMessageSelect?.(currentMsg);
      }, currentIndex === 0 ? 500 : delay);
    } else if (currentIndex >= messages.length) {
      setIsPlaying(false);
    }

    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
    };
  }, [isPlaying, currentIndex, messages, getDelay, onMessageSelect]);

  // Auto-scroll to bottom when new messages appear
  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [visibleMessages]);

  const handlePlay = () => {
    if (currentIndex >= messages.length) {
      // Restart from beginning
      setCurrentIndex(0);
      setVisibleMessages([]);
    }
    setIsPlaying(true);
  };

  const handlePause = () => {
    setIsPlaying(false);
  };

  const handleReset = () => {
    setIsPlaying(false);
    setCurrentIndex(0);
    setVisibleMessages([]);
  };

  const handleStepBack = () => {
    setIsPlaying(false);
    if (currentIndex > 0) {
      setCurrentIndex(prev => prev - 1);
      setVisibleMessages(prev => prev.slice(0, -1));
    }
  };

  const handleStepForward = () => {
    setIsPlaying(false);
    if (currentIndex < messages.length) {
      const currentMsg = messages[currentIndex];
      setVisibleMessages(prev => [...prev, currentMsg]);
      setCurrentIndex(prev => prev + 1);
      onMessageSelect?.(currentMsg);
    }
  };

  const handleSpeedChange = () => {
    const currentIdx = PLAYBACK_SPEEDS.indexOf(playbackSpeed);
    const nextIdx = (currentIdx + 1) % PLAYBACK_SPEEDS.length;
    setPlaybackSpeed(PLAYBACK_SPEEDS[nextIdx]);
  };

  const handleSeek = (index: number) => {
    setIsPlaying(false);
    setCurrentIndex(index + 1);
    setVisibleMessages(messages.slice(0, index + 1));
    onMessageSelect?.(messages[index]);
  };

  const progress = messages.length > 0 ? (currentIndex / messages.length) * 100 : 0;

  return (
    <div className="flex flex-col h-full bg-background rounded-2xl border border-border overflow-hidden">
      {/* Replay Controls Header */}
      <div className="flex items-center justify-between px-4 py-3 bg-surface-elevated border-b border-border">
        <div className="flex items-center gap-2">
          <Activity className="w-4 h-4 text-primary" />
          <span className="text-sm font-medium text-textPrimary">Session Replay</span>
          <span className="text-xs text-textTertiary">
            {currentIndex} / {messages.length} turns
          </span>
        </div>

        {/* Playback Controls */}
        <div className="flex items-center gap-1">
          {/* Reset */}
          <button
            onClick={handleReset}
            className="p-1.5 rounded-lg hover:bg-surface transition-colors text-textSecondary hover:text-textPrimary"
            title="Reset"
          >
            <RotateCcw className="w-4 h-4" />
          </button>

          {/* Step Back */}
          <button
            onClick={handleStepBack}
            disabled={currentIndex === 0}
            className="p-1.5 rounded-lg hover:bg-surface transition-colors text-textSecondary hover:text-textPrimary disabled:opacity-50 disabled:cursor-not-allowed"
            title="Previous message"
          >
            <ChevronLeft className="w-4 h-4" />
          </button>

          {/* Play/Pause */}
          <button
            onClick={isPlaying ? handlePause : handlePlay}
            className="p-2 rounded-lg bg-primary hover:bg-primary-hover text-white transition-colors"
            title={isPlaying ? 'Pause' : 'Play'}
          >
            {isPlaying ? <Pause className="w-4 h-4" /> : <Play className="w-4 h-4" />}
          </button>

          {/* Step Forward */}
          <button
            onClick={handleStepForward}
            disabled={currentIndex >= messages.length}
            className="p-1.5 rounded-lg hover:bg-surface transition-colors text-textSecondary hover:text-textPrimary disabled:opacity-50 disabled:cursor-not-allowed"
            title="Next message"
          >
            <ChevronRight className="w-4 h-4" />
          </button>

          {/* Speed Control */}
          <button
            onClick={handleSpeedChange}
            className="px-2 py-1 rounded-lg hover:bg-surface transition-colors text-xs font-mono text-textSecondary hover:text-textPrimary min-w-[48px]"
            title="Playback speed"
          >
            {playbackSpeed}x
          </button>
        </div>
      </div>

      {/* Progress Bar */}
      <div className="relative h-1.5 bg-background">
        <div 
          className="absolute h-full bg-primary transition-all duration-300 ease-out"
          style={{ width: `${progress}%` }}
        />
        {/* Clickable timeline markers */}
        <div className="absolute inset-0 flex">
          {messages.map((msg, idx) => (
            <button
              key={msg.id}
              onClick={() => handleSeek(idx)}
              className={cn(
                "h-full hover:bg-primary/50 transition-colors",
                idx < currentIndex && "bg-primary/30"
              )}
              style={{ width: `${100 / messages.length}%` }}
              title={`Turn ${idx + 1}: ${msg.role}`}
            />
          ))}
        </div>
      </div>

      {/* Timeline Strip */}
      <div className="flex items-center gap-1 px-4 py-2 bg-surface/50 border-b border-border overflow-x-auto">
        {messages.map((msg, idx) => (
          <button
            key={msg.id}
            onClick={() => handleSeek(idx)}
            className={cn(
              "flex-shrink-0 px-2 py-1 rounded text-xs transition-all",
              idx < currentIndex 
                ? "bg-primary/20 text-primary border border-primary/30"
                : idx === currentIndex && visibleMessages.length > 0
                ? "bg-primary text-white ring-2 ring-primary/50"
                : "bg-surface border border-border text-textTertiary hover:border-primary/50"
            )}
          >
            {msg.role === 'user' ? '👤' : msg.role === 'assistant' ? '🤖' : '⚙️'}
          </button>
        ))}
      </div>

      {/* Messages Area */}
      <div 
        ref={containerRef}
        className="flex-1 overflow-y-auto p-4 space-y-3"
      >
        {visibleMessages.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-textTertiary">
            <Play className="w-8 h-8 mb-2 opacity-50" />
            <p className="text-sm">Press play to replay the conversation</p>
            <p className="text-xs mt-1">Or click a turn marker above to jump to it</p>
          </div>
        ) : (
          visibleMessages.map((message, idx) => (
            <div
              key={message.id}
              className={cn(
                "rounded-xl p-3 transition-all animate-slide-in",
                message.role === 'user' 
                  ? "bg-surface border border-border ml-8" 
                  : message.role === 'assistant'
                  ? "bg-primary/10 border border-primary/20 mr-8"
                  : "bg-surface-elevated border border-border mx-4"
              )}
              style={{ animationDelay: `${idx * 50}ms` }}
            >
              <div className="flex items-center justify-between mb-2">
                <span className="flex items-center gap-2 text-xs font-medium text-textSecondary">
                  {message.role === 'user' ? '👤 User' : message.role === 'assistant' ? '🤖 Assistant' : '⚙️ System'}
                  <span className="text-textTertiary font-normal">
                    {new Date(message.timestamp / 1000).toLocaleTimeString()}
                  </span>
                </span>
                <div className="flex items-center gap-2 text-xs text-textTertiary">
                  {message.tokens !== undefined && (
                    <span>{message.tokens.toLocaleString()} tokens</span>
                  )}
                  {message.cost !== undefined && (
                    <span>${message.cost.toFixed(4)}</span>
                  )}
                  {onTraceClick && (
                    <button
                      onClick={() => onTraceClick(message.traceId)}
                      className="text-primary hover:underline"
                    >
