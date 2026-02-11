/**
 * Task 7 — Connection Health Monitor & Auto-Discovery
 *
 * Polls /mcp/health on configurable interval. Auto-discovers capabilities.
 * EMA latency: ema_new = α × sample + (1-α) × ema_old, α = 0.3
 * Jitter via Welford's online algorithm: O(1) per update.
 */

// ─── Health Types ──────────────────────────────────────────────────────────────

export interface McpHealthStatus {
  status: string;
  protocol_version: string;
  server_name: string;
  server_version: string;
  connected_clients: number;
  capabilities: McpCapabilities;
}

export interface McpCapabilities {
  tools?: { listChanged?: boolean };
  resources?: { subscribe?: boolean; listChanged?: boolean };
  prompts?: { listChanged?: boolean };
  logging?: Record<string, unknown>;
}

export type HealthState = 'healthy' | 'degraded' | 'unhealthy' | 'unknown';

export interface HealthSnapshot {
  state: HealthState;
  status?: McpHealthStatus;
  lastChecked: number;
  latencyMs: number;
  emaLatencyMs: number;
  jitterMs: number;
  error?: string;
}

// ─── Welford's Online Algorithm for Jitter ─────────────────────────────────────

interface WelfordState {
  count: number;
  mean: number;
  m2: number;
}

function welfordInit(): WelfordState {
  return { count: 0, mean: 0, m2: 0 };
}

/** O(1) per update */
function welfordUpdate(state: WelfordState, sample: number): WelfordState {
  const count = state.count + 1;
  const delta = sample - state.mean;
  const mean = state.mean + delta / count;
  const delta2 = sample - mean;
  const m2 = state.m2 + delta * delta2;
  return { count, mean, m2 };
}

function welfordStdDev(state: WelfordState): number {
  if (state.count < 2) return 0;
  return Math.sqrt(state.m2 / (state.count - 1));
}

// ─── Health Monitor ────────────────────────────────────────────────────────────

export type HealthChangeHandler = (snapshot: HealthSnapshot) => void;

export class HealthMonitor {
  private _endpoint: string;
  private _intervalMs: number;
  private _timer: ReturnType<typeof setInterval> | null = null;
  private _handlers: Set<HealthChangeHandler> = new Set();
  private _snapshot: HealthSnapshot;
  private _emaLatency = 0;
  private _welford: WelfordState;
  private _alpha = 0.3;

  constructor(endpoint: string, intervalMs = 5000) {
    this._endpoint = endpoint;
    this._intervalMs = intervalMs;
    this._welford = welfordInit();
    this._snapshot = {
      state: 'unknown',
      lastChecked: 0,
      latencyMs: 0,
      emaLatencyMs: 0,
      jitterMs: 0,
    };
  }

  get snapshot(): HealthSnapshot {
    return { ...this._snapshot };
  }

  start(): void {
    if (this._timer) return;
    this._check(); // Initial check
    this._timer = setInterval(() => this._check(), this._intervalMs);
  }

  stop(): void {
    if (this._timer) {
      clearInterval(this._timer);
      this._timer = null;
    }
  }

  setInterval(ms: number): void {
    this._intervalMs = ms;
    if (this._timer) {
      this.stop();
      this.start();
    }
  }

  setEndpoint(endpoint: string): void {
    this._endpoint = endpoint;
  }

  onChange(handler: HealthChangeHandler): () => void {
    this._handlers.add(handler);
    return () => this._handlers.delete(handler);
  }

  async checkNow(): Promise<HealthSnapshot> {
    await this._check();
    return this._snapshot;
  }

  private async _check(): Promise<void> {
    const healthUrl = this._endpoint.replace(/\/mcp\/?$/, '/mcp/health');
    const start = performance.now();

    try {
      const timeoutMs = Math.max(this._intervalMs - 500, 2000);
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
      const resp = await fetch(healthUrl, {
        method: 'GET',
        signal: controller.signal,
      });
      clearTimeout(timeoutId);

      const elapsed = performance.now() - start;

      if (!resp.ok) {
        this._snapshot = {
          state: 'degraded',
          lastChecked: Date.now(),
          latencyMs: elapsed,
          emaLatencyMs: this._updateEma(elapsed),
          jitterMs: this._updateJitter(elapsed),
          error: `HTTP ${resp.status}`,
        };
      } else {
        const data = (await resp.json()) as McpHealthStatus;
        this._snapshot = {
          state: 'healthy',
          status: data,
          lastChecked: Date.now(),
          latencyMs: elapsed,
          emaLatencyMs: this._updateEma(elapsed),
          jitterMs: this._updateJitter(elapsed),
        };
      }
    } catch (err) {
      const elapsed = performance.now() - start;
      this._snapshot = {
        state: 'unhealthy',
        lastChecked: Date.now(),
        latencyMs: elapsed,
        emaLatencyMs: this._emaLatency,
        jitterMs: welfordStdDev(this._welford),
        error: err instanceof Error ? err.message : 'Connection failed',
      };
    }

    this._handlers.forEach((h) => h(this._snapshot));
  }

  private _updateEma(sample: number): number {
    this._emaLatency = this._alpha * sample + (1 - this._alpha) * this._emaLatency;
    return this._emaLatency;
  }

  private _updateJitter(sample: number): number {
    this._welford = welfordUpdate(this._welford, sample);
    return welfordStdDev(this._welford);
  }
}

// ─── Capability Discovery ──────────────────────────────────────────────────────

export interface DiscoveredCapabilities {
  capabilities: McpCapabilities;
  serverName: string;
  serverVersion: string;
  protocolVersion: string;
  discoveredAt: number;
  ttlMs: number;
}

let _cachedCapabilities: DiscoveredCapabilities | null = null;

export function getCachedCapabilities(): DiscoveredCapabilities | null {
  if (!_cachedCapabilities) return null;
  if (Date.now() - _cachedCapabilities.discoveredAt > _cachedCapabilities.ttlMs) {
    _cachedCapabilities = null;
    return null;
  }
  return _cachedCapabilities;
}

export function setCachedCapabilities(caps: DiscoveredCapabilities): void {
  _cachedCapabilities = caps;
}

export function clearCachedCapabilities(): void {
  _cachedCapabilities = null;
}

export function isCapabilitySupported(
  caps: McpCapabilities | undefined,
  feature: 'tools' | 'resources' | 'prompts' | 'logging'
): boolean {
  if (!caps) return false;
  return caps[feature] !== undefined && caps[feature] !== null;
}
