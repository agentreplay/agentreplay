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

import { useEffect, useRef, useState } from 'react';

import { useAuth } from './useAuth';

export interface TraceEvent {
  edge_id: string;
  timestamp_us: number;
  operation: string;
  span_type: string;
  duration_ms: number;
  tokens: number;
  cost: number;
  status: 'success' | 'error';
  agent_id: number;
  session_id: number;
}

export interface UseRealtimeTracesOptions {
  maxBufferSize?: number;
  reconnectInterval?: number;
  maxReconnectInterval?: number;
  maxRetries?: number;
}

interface CircuitBreakerState {
  failureCount: number;
  lastFailureTime: number;
  state: 'closed' | 'open' | 'half-open';
}

export function useRealtimeTraces(options: UseRealtimeTracesOptions = {}) {
  const { 
    maxBufferSize = 100, 
    reconnectInterval = 1_000,
    maxReconnectInterval = 30_000,
    maxRetries = 10
  } = options;

  const [traces, setTraces] = useState<TraceEvent[]>([]);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [connectionState, setConnectionState] = useState<'connecting' | 'connected' | 'disconnected' | 'failed'>('disconnected');

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reconnectAttemptsRef = useRef(0);
  const currentBackoffRef = useRef(reconnectInterval);
  
  // Circuit breaker state
  const circuitBreakerRef = useRef<CircuitBreakerState>({
    failureCount: 0,
    lastFailureTime: 0,
    state: 'closed'
  });
  
  // Pending traces buffer (for when disconnected)
  const pendingTracesRef = useRef<TraceEvent[]>([]);
  
  // Circular buffer to avoid memory allocation in hot path
  const bufferRef = useRef<TraceEvent[]>([]);
  const writeIndexRef = useRef(0);
  const countRef = useRef(0);

  const { apiKey } = useAuth();

  useEffect(() => {
    let cancelled = false;

    const checkCircuitBreaker = (): boolean => {
      const cb = circuitBreakerRef.current;
      const now = Date.now();
      
      if (cb.state === 'open') {
        // Check if we should transition to half-open (30 seconds after last failure)
        if (now - cb.lastFailureTime > 30_000) {
          cb.state = 'half-open';
          console.log('[Flowtrace] Circuit breaker transitioning to half-open, attempting connection');
          return true;
        }
        console.log('[Flowtrace] Circuit breaker is open, blocking connection attempt');
        return false;
      }
      
      return true; // closed or half-open allows requests
    };

    const recordSuccess = () => {
      const cb = circuitBreakerRef.current;
      cb.failureCount = 0;
      cb.state = 'closed';
      reconnectAttemptsRef.current = 0;
      currentBackoffRef.current = reconnectInterval;
      
      // CRITICAL FIX (Task 8): Actually flush pending traces to state
      if (pendingTracesRef.current.length > 0) {
        console.log(`[Flowtrace] Flushing ${pendingTracesRef.current.length} pending traces`);
        
        // Add all pending traces to buffer and update state
        pendingTracesRef.current.forEach(trace => {
          bufferRef.current[writeIndexRef.current] = trace;
          writeIndexRef.current = (writeIndexRef.current + 1) % maxBufferSize;
          countRef.current = Math.min(countRef.current + 1, maxBufferSize);
        });
        
        // Trigger UI update with flushed traces
        const currentBuffer = bufferRef.current;
        const writeIdx = writeIndexRef.current;
        const count = countRef.current;
        
        if (count < maxBufferSize) {
          setTraces([...currentBuffer.slice(0, count)].reverse());
        } else {
          const newest = currentBuffer.slice(writeIdx);
          const oldest = currentBuffer.slice(0, writeIdx);
          setTraces([...newest, ...oldest].reverse());
        }
        
        // Now clear
        pendingTracesRef.current = [];
      }
    };

    const recordFailure = (errorType: 'network' | 'auth' | 'unknown') => {
      const cb = circuitBreakerRef.current;
      cb.failureCount += 1;
      cb.lastFailureTime = Date.now();
      
      // Auth failures are permanent - open circuit immediately
      if (errorType === 'auth') {
        cb.state = 'open';
        cb.failureCount = 10; // Max out failures
        setError(new Error('Authentication failed - check API key'));
        setConnectionState('failed');
        
        // CRITICAL: Clean up reconnect timer to prevent memory leak
        if (reconnectTimeoutRef.current) {
          clearTimeout(reconnectTimeoutRef.current);
          reconnectTimeoutRef.current = null;
        }
        return;
      }
      
      // Open circuit after 5 failures
      if (cb.failureCount >= 5) {
        cb.state = 'open';
        console.error('[Flowtrace] Circuit breaker opened after 5 failures');
        setConnectionState('failed');
      }
    };

    const connect = () => {
      if (typeof window === 'undefined') {
        return;
      }

      // Check circuit breaker
      if (!checkCircuitBreaker()) {
        setConnectionState('failed');
        // Schedule retry after backoff
        reconnectTimeoutRef.current = setTimeout(connect, currentBackoffRef.current);
        return;
      }

      // Check max retries
      if (reconnectAttemptsRef.current >= maxRetries) {
        console.error('[Flowtrace] Max reconnection attempts reached');
        setConnectionState('failed');
        setError(new Error('Max reconnection attempts reached'));
        return;
      }

      setConnectionState('connecting');

      try {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        // Connect directly to Flowtrace server for WebSocket (Next.js rewrites don't support WS)
        const wsHost = process.env.NEXT_PUBLIC_WS_URL || 'localhost:9600';
        const baseUrl = `${protocol}//${wsHost}/ws/traces`;
        // apiKey is always null in current implementation (HttpOnly cookie auth)
        const wsUrl =
          apiKey && typeof apiKey === 'string' && apiKey.length > 0
            ? `${baseUrl}?api_key=${encodeURIComponent(apiKey)}`
            : baseUrl;

        const ws = new WebSocket(wsUrl);
        wsRef.current = ws;

        ws.onopen = () => {
          if (cancelled) {
            ws.close();
            return;
          }
          console.log('[Flowtrace] WebSocket connected');
          setConnected(true);
          setError(null);
          setConnectionState('connected');
          recordSuccess();
        };

        ws.onmessage = (event) => {
          if (cancelled) {
            return;
          }
          try {
            const data = JSON.parse(event.data);
            if (data.type === 'Connected') {
              return;
            }

            const trace: TraceEvent = data;
            
            // Use circular buffer to avoid array spread/slice allocation
            bufferRef.current[writeIndexRef.current] = trace;
            writeIndexRef.current = (writeIndexRef.current + 1) % maxBufferSize;
            countRef.current = Math.min(countRef.current + 1, maxBufferSize);
            
            // Create view of buffer for rendering (in newest-first order)
            const currentBuffer = bufferRef.current;
            const writeIdx = writeIndexRef.current;
            const count = countRef.current;
            
            if (count < maxBufferSize) {
              // Buffer not full yet - just slice from beginning
              setTraces([...currentBuffer.slice(0, count)].reverse());
            } else {
              // Buffer is full - reconstruct newest-first order
              const newest = currentBuffer.slice(writeIdx);
              const oldest = currentBuffer.slice(0, writeIdx);
              setTraces([...newest, ...oldest].reverse());
            }
          } catch (err) {
            console.error('[Flowtrace] Failed to parse trace event', err);
            recordFailure('unknown');
          }
        };

        ws.onerror = (evt) => {
          console.error('[Flowtrace] WebSocket error', evt);
          setConnected(false);
          setConnectionState('disconnected');
          
          // Categorize error (limited info available in browser)
          const errorType = 'network';
          recordFailure(errorType);
          setError(new Error('WebSocket connection error'));
        };

        ws.onclose = (evt) => {
          if (cancelled) {
            return;
          }
          setConnected(false);
          setConnectionState('disconnected');
          console.log('[Flowtrace] WebSocket closed', { code: evt.code, reason: evt.reason, wasClean: evt.wasClean });
          
          // Categorize close reason
          let errorType: 'network' | 'auth' | 'unknown' = 'network';
          if (evt.code === 1008 || evt.code === 4001) {
            // Policy violation or authentication error
            errorType = 'auth';
          } else if (evt.code === 1000 || evt.code === 1001) {
            // Normal or going away - don't treat as error
            return;
          }
          
          // Only reconnect if it wasn't a clean close
          if (!evt.wasClean) {
            recordFailure(errorType);
            reconnectAttemptsRef.current += 1;
            
            // Exponential backoff with jitter
            const jitter = Math.random() * 1000;
            currentBackoffRef.current = Math.min(
              currentBackoffRef.current * 2 + jitter,
              maxReconnectInterval
            );
            
            console.log(`[Flowtrace] Reconnecting in ${Math.round(currentBackoffRef.current)}ms (attempt ${reconnectAttemptsRef.current}/${maxRetries})`);
            reconnectTimeoutRef.current = setTimeout(connect, currentBackoffRef.current);
          }
        };
      } catch (err) {
        console.error('[Flowtrace] Failed to establish WebSocket', err);
        setError(err as Error);
      }
    };

    connect();

    return () => {
      cancelled = true;
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
        reconnectTimeoutRef.current = null;
      }
      if (wsRef.current) {
        wsRef.current.close(1000, 'component unmounted');
        wsRef.current = null;
      }
    };
  }, [apiKey, maxBufferSize, reconnectInterval]);

  const clearTraces = () => {
    setTraces([]);
    bufferRef.current = [];
    writeIndexRef.current = 0;
    countRef.current = 0;
  };

  return {
    traces,
    connected,
    error,
    connectionState,
    clearTraces,
  };
}
