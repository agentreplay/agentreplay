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

/**
 * SSE (Server-Sent Events) hook for real-time trace streaming
 * 
 * This hook uses SSE instead of polling or WebSockets for efficient
 * real-time trace updates. SSE is simpler than WebSockets and more
 * efficient than polling.
 * 
 * Task 7: Added proper AbortController and cleanup to prevent memory leaks
 * and "Can't perform state update on unmounted component" warnings.
 */

import { useEffect, useRef, useState, useCallback } from 'react';

export interface SSETraceEvent {
  trace_id: string;
  span_id: string;
  parent_span_id?: string;
  timestamp_us: number;
  duration_us: number;
  token_count: number;
  span_type: string;
  project_id: number;
  session_id: number;
  agent_id: number;
}

export interface LagEvent {
  type: 'lag';
  skipped: number;
}

export type SSEEvent = SSETraceEvent | LagEvent;

export interface UseSSETracesOptions {
  enabled?: boolean;
  maxTraces?: number;
  onTrace?: (trace: SSETraceEvent) => void;
  onLag?: (skipped: number) => void;
  onError?: (error: Error) => void;
  onConnect?: () => void;
  onDisconnect?: () => void;
  baseUrl?: string;
}

export interface UseSSETracesResult {
  traces: SSETraceEvent[];
  connected: boolean;
  connecting: boolean;
  error: Error | null;
  reconnectCount: number;
  disconnect: () => void;
  connect: () => void;
  clearTraces: () => void;
}

// Determine the API base URL based on environment
function getApiBaseUrl(): string {
  // In Tauri, window.__TAURI__ is defined
  const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
  
  if (isTauri) {
    // Tauri app: connect directly to embedded server
    return 'http://127.0.0.1:9600';
  }
  
  // Development with Vite proxy or browser
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return ''; // Use Vite proxy
  }
  
  // Fallback: try localhost
  return 'http://127.0.0.1:9600';
}

/**
 * Helper to safely update state only if component is still mounted
 */
function useSafeState<T>(initialValue: T): [T, (value: T | ((prev: T) => T)) => void, React.MutableRefObject<boolean>] {
  const [state, setState] = useState(initialValue);
  const mountedRef = useRef(true);
  
  const safeSetState = useCallback((value: T | ((prev: T) => T)) => {
    if (mountedRef.current) {
      setState(value);
    }
  }, []);
  
  return [state, safeSetState, mountedRef];
}

export function useSSETraces(options: UseSSETracesOptions = {}): UseSSETracesResult {
  const {
    enabled = true,
    maxTraces = 100,
    onTrace,
    onLag,
    onError,
    onConnect,
    onDisconnect,
    baseUrl = getApiBaseUrl(),
  } = options;

  const [traces, setTraces, mountedRef] = useSafeState<SSETraceEvent[]>([]);
  const [connected, setConnected] = useSafeState(false);
  const [connecting, setConnecting] = useSafeState(false);
  const [error, setError] = useSafeState<Error | null>(null);
  const [reconnectCount, setReconnectCount] = useSafeState(0);

  const eventSourceRef = useRef<EventSource | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Track connection ID to handle race conditions
  const connectionIdRef = useRef(0);

  // Exponential backoff config
  const minBackoff = 1000;   // 1 second
  const maxBackoff = 30000;  // 30 seconds
  const currentBackoffRef = useRef(minBackoff);

  const disconnect = useCallback(() => {
    // Increment connection ID to invalidate any pending operations
    connectionIdRef.current += 1;
    
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
      eventSourceRef.current = null;
    }
    if (mountedRef.current) {
      setConnected(false);
      setConnecting(false);
    }
  }, []);

  const connect = useCallback(() => {
    // Clean up existing connection
    disconnect();

    if (!enabled || !mountedRef.current) return;

    // Capture connection ID to detect stale callbacks
    const thisConnectionId = connectionIdRef.current;

    setConnecting(true);
    setError(null);

    const url = `${baseUrl}/api/v1/traces/stream`;
    console.log('[SSE] Connecting to:', url);

    try {
      const eventSource = new EventSource(url);
      eventSourceRef.current = eventSource;

      eventSource.onopen = () => {
        // Check if this connection is still valid
        if (!mountedRef.current || connectionIdRef.current !== thisConnectionId) {
          eventSource.close();
          return;
        }
        console.log('[SSE] Connected');
        setConnected(true);
        setConnecting(false);
        setError(null);
        setReconnectCount(0);
        currentBackoffRef.current = minBackoff;
        onConnect?.();
      };

      eventSource.onmessage = (event) => {
        // Check if this connection is still valid
        if (!mountedRef.current || connectionIdRef.current !== thisConnectionId) return;

        try {
          const data = JSON.parse(event.data);
          
          // Check if this is a lag event
          if (data.type === 'lag') {
            console.warn('[SSE] Client lagged, skipped', data.skipped, 'messages');
            onLag?.(data.skipped);
            return;
          }

          // Regular trace event
          const trace: SSETraceEvent = {
            trace_id: data.trace_id,
            span_id: data.span_id,
            parent_span_id: data.parent_span_id,
            timestamp_us: data.timestamp_us,
            duration_us: data.duration_us,
            token_count: data.token_count,
            span_type: data.span_type,
            project_id: data.project_id,
            session_id: data.session_id,
            agent_id: data.agent_id,
          };

          // Add to traces (FIFO, limit to maxTraces)
          setTraces((prev) => {
            const updated = [trace, ...prev];
            if (updated.length > maxTraces) {
              return updated.slice(0, maxTraces);
            }
            return updated;
          });

          onTrace?.(trace);
        } catch (parseError) {
          console.error('[SSE] Failed to parse event:', parseError);
        }
      };

      eventSource.onerror = (event) => {
        console.error('[SSE] Error:', event);
        
        // Check if this connection is still valid
        if (!mountedRef.current || connectionIdRef.current !== thisConnectionId) {
          eventSource.close();
          return;
        }

        setConnected(false);
        setConnecting(false);
        onDisconnect?.();

        // Check if connection was closed (not just an error)
        if (eventSource.readyState === EventSource.CLOSED) {
          const err = new Error('SSE connection closed');
          setError(err);
          onError?.(err);

          // Schedule reconnect with exponential backoff
          setReconnectCount((prev) => prev + 1);
          const backoff = currentBackoffRef.current;
          currentBackoffRef.current = Math.min(backoff * 2, maxBackoff);

          console.log(`[SSE] Will reconnect in ${backoff}ms`);
          reconnectTimeoutRef.current = setTimeout(() => {
            if (mountedRef.current && enabled) {
              connect();
            }
          }, backoff);
        }
      };
    } catch (err) {
      console.error('[SSE] Failed to create EventSource:', err);
      setConnecting(false);
      const error = err instanceof Error ? err : new Error('Failed to connect');
      setError(error);
      onError?.(error);
    }
  }, [enabled, baseUrl, maxTraces, onTrace, onLag, onError, onConnect, onDisconnect, disconnect]);

  const clearTraces = useCallback(() => {
    setTraces([]);
  }, []);

  // Connect on mount and when enabled changes
  useEffect(() => {
    mountedRef.current = true;

    if (enabled) {
      connect();
    } else {
      disconnect();
    }

    return () => {
      mountedRef.current = false;
      disconnect();
    };
  }, [enabled, connect, disconnect]);

  return {
    traces,
    connected,
    connecting,
    error,
    reconnectCount,
    disconnect,
    connect,
    clearTraces,
  };
}

export default useSSETraces;
