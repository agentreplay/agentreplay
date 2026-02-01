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

export interface MetricUpdate {
  type: 'trace_completed' | 'metric_updated' | 'session_created';
  timestamp: number;
  data: {
    total_requests?: number;
    total_cost?: number;
    avg_latency_ms?: number;
    error_rate?: number;
    [key: string]: any;
  };
}

export interface UseRealtimeMetricsOptions {
  reconnectInterval?: number;
  maxReconnectInterval?: number;
  maxRetries?: number;
  onUpdate?: (update: MetricUpdate) => void;
}

interface CircuitBreakerState {
  failureCount: number;
  lastFailureTime: number;
  state: 'closed' | 'open' | 'half-open';
}

export function useRealtimeMetrics(options: UseRealtimeMetricsOptions = {}) {
  const { 
    reconnectInterval = 1_000,
    maxReconnectInterval = 30_000,
    maxRetries = 10,
    onUpdate 
  } = options;

  const [metrics, setMetrics] = useState<MetricUpdate['data']>({
    total_requests: 0,
    total_cost: 0,
    avg_latency_ms: 0,
    error_rate: 0,
  });
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [lastUpdate, setLastUpdate] = useState<number>(Date.now());
  const [connectionState, setConnectionState] = useState<'connecting' | 'connected' | 'disconnected' | 'failed'>('disconnected');

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reconnectAttemptsRef = useRef(0);
  const currentBackoffRef = useRef(reconnectInterval);
  
  const circuitBreakerRef = useRef<CircuitBreakerState>({
    failureCount: 0,
    lastFailureTime: 0,
    state: 'closed'
  });

  const { apiKey } = useAuth();

  useEffect(() => {
    let cancelled = false;

    const checkCircuitBreaker = (): boolean => {
      const cb = circuitBreakerRef.current;
      const now = Date.now();
      
      if (cb.state === 'open') {
        if (now - cb.lastFailureTime > 30_000) {
          cb.state = 'half-open';
          return true;
        }
        return false;
      }
      return true;
    };

    const recordSuccess = () => {
      circuitBreakerRef.current.failureCount = 0;
      circuitBreakerRef.current.state = 'closed';
      reconnectAttemptsRef.current = 0;
      currentBackoffRef.current = reconnectInterval;
    };

    const recordFailure = (errorType: 'network' | 'auth' | 'unknown') => {
      const cb = circuitBreakerRef.current;
      cb.failureCount += 1;
      cb.lastFailureTime = Date.now();
      
      if (errorType === 'auth') {
        cb.state = 'open';
        setError(new Error('Authentication failed'));
        setConnectionState('failed');
        
        // CRITICAL: Clean up reconnect timer to prevent memory leak
        if (reconnectTimeoutRef.current) {
          clearTimeout(reconnectTimeoutRef.current);
          reconnectTimeoutRef.current = null;
        }
        return;
      }
      
      if (cb.failureCount >= 5) {
        cb.state = 'open';
        setConnectionState('failed');
      }
    };

    const connect = () => {
      if (typeof window === 'undefined') {
        return;
      }

      if (!checkCircuitBreaker()) {
        setConnectionState('failed');
        reconnectTimeoutRef.current = setTimeout(connect, currentBackoffRef.current);
        return;
      }

      if (reconnectAttemptsRef.current >= maxRetries) {
        setConnectionState('failed');
        setError(new Error('Max reconnection attempts reached'));
        return;
      }

      setConnectionState('connecting');

      try {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsHost = process.env.NEXT_PUBLIC_WS_URL || 'localhost:47100';
        const baseUrl = `${protocol}//${wsHost}/ws/metrics`;
        // apiKey is always null in current implementation (HttpOnly cookie auth)
        // But we keep the parameter support for future compatibility
        const wsUrl =
          apiKey && typeof apiKey === 'string' && apiKey.length > 0
            ? `${baseUrl}?api_key=${encodeURIComponent(apiKey)}`
            : baseUrl;

        const ws = new WebSocket(wsUrl);
        wsRef.current = ws;

        ws.onopen = () => {
          if (cancelled) {
            return;
          }
          console.log('[Agentreplay] Metrics WebSocket connected');
          setConnected(true);
          setError(null);
          setConnectionState('connected');
          recordSuccess();
        };

        ws.onmessage = (event) => {
          try {
            const update: MetricUpdate = JSON.parse(event.data);
            
            if (update.type === 'metric_updated' && update.data) {
              setMetrics(prev => ({
                ...prev,
                ...update.data,
              }));
              setLastUpdate(Date.now());
              
              if (onUpdate) {
                onUpdate(update);
              }
            }
          } catch (err) {
            console.error('[Agentreplay] Failed to parse metric update', err);
          }
        };

        ws.onerror = (evt) => {
          console.error('[Agentreplay] Metrics WebSocket error', evt);
          setConnected(false);
          setConnectionState('disconnected');
          recordFailure('network');
          setError(new Error('WebSocket connection error'));
        };

        ws.onclose = (evt) => {
          console.log('[Agentreplay] Metrics WebSocket closed', evt.code, evt.reason);
          setConnected(false);
          setConnectionState('disconnected');
          
          if (!cancelled && !evt.wasClean) {
            const errorType = (evt.code === 1008 || evt.code === 4001) ? 'auth' : 'network';
            recordFailure(errorType);
            reconnectAttemptsRef.current += 1;
            
            const jitter = Math.random() * 1000;
            currentBackoffRef.current = Math.min(
              currentBackoffRef.current * 2 + jitter,
              maxReconnectInterval
            );
            
            console.log(`[Agentreplay] Metrics reconnecting in ${Math.round(currentBackoffRef.current)}ms`);
            reconnectTimeoutRef.current = setTimeout(connect, currentBackoffRef.current);
          }
        };
      } catch (err) {
        console.error('[Agentreplay] Failed to establish metrics WebSocket', err);
        setError(err as Error);
      }
    };

    connect();

    return () => {
      cancelled = true;
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      if (wsRef.current) {
        wsRef.current.close();
      }
    };
  }, [apiKey, reconnectInterval, onUpdate]);

  return {
    metrics,
    connected,
    error,
    lastUpdate,
    connectionState,
  };
}
