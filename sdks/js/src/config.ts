/**
 * Copyright 2025 Sushanth (https://github.com/sushanthpy)
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/**
 * Configuration module for Agentreplay SDK
 *
 * Supports env-first configuration with explicit overrides.
 */

import type { SamplingConfig, PrivacyConfig, TransportConfig } from './types';

/**
 * SDK initialization options
 */
export interface InitOptions {
  /** API key for authentication */
  apiKey?: string;
  /** Base URL of Agentreplay server */
  baseUrl?: string;
  /** Tenant identifier */
  tenantId?: number;
  /** Project identifier */
  projectId?: number;
  /** Default agent identifier */
  agentId?: number;
  /** Environment name */
  environment?: string;
  /** Enable debug mode */
  debug?: boolean;
  /** Strict mode - throws on missing API key */
  strict?: boolean;
  /** Request timeout in milliseconds */
  timeout?: number;
  /** Sampling configuration */
  sampling?: SamplingConfig;
  /** Privacy/redaction configuration */
  privacy?: PrivacyConfig;
  /** Transport configuration */
  transport?: TransportConfig;
  /** Custom headers */
  headers?: Record<string, string>;
  /** Custom fetch implementation */
  fetch?: typeof fetch;
}

/**
 * Resolved configuration after merging env vars and options
 */
export interface ResolvedConfig {
  apiKey: string | undefined;
  baseUrl: string;
  tenantId: number;
  projectId: number;
  agentId: number;
  environment: string;
  debug: boolean;
  strict: boolean;
  timeout: number;
  sampling: SamplingConfig;
  privacy: PrivacyConfig;
  transport: TransportConfig;
  headers: Record<string, string>;
  fetch: typeof fetch;
}

/**
 * Global SDK state
 */
let globalConfig: ResolvedConfig | null = null;
let isInitialized = false;

/**
 * Get environment variable (works in Node.js and edge runtimes)
 */
function getEnv(key: string): string | undefined {
  if (typeof process !== 'undefined' && process.env) {
    return process.env[key];
  }
  return undefined;
}

/**
 * Get environment variable as number
 */
function getEnvNumber(key: string, defaultValue: number): number {
  const value = getEnv(key);
  if (value === undefined) return defaultValue;
  const parsed = parseInt(value, 10);
  return isNaN(parsed) ? defaultValue : parsed;
}

/**
 * Get environment variable as boolean
 */
function getEnvBool(key: string): boolean {
  const value = getEnv(key);
  return value === '1' || value === 'true';
}

/**
 * Default configuration values
 */
const DEFAULT_CONFIG: Omit<ResolvedConfig, 'fetch'> = {
  apiKey: undefined,
  baseUrl: 'http://localhost:8080',
  tenantId: 1,
  projectId: 0,
  agentId: 1,
  environment: 'development',
  debug: false,
  strict: false,
  timeout: 30000,
  sampling: {
    rate: 1.0,
    rules: [],
  },
  privacy: {
    mode: 'none',
    redact: [],
    scrubbers: [],
  },
  transport: {
    mode: 'batch',
    batchSize: 100,
    flushIntervalMs: 5000,
    maxQueueSize: 10000,
    maxRetries: 3,
    retryDelayMs: 1000,
    compression: false,
  },
  headers: {},
};

/**
 * Initialize the Agentreplay SDK.
 *
 * Reads configuration from environment variables by default:
 * - AGENTREPLAY_API_KEY
 * - AGENTREPLAY_URL
 * - AGENTREPLAY_TENANT_ID
 * - AGENTREPLAY_PROJECT_ID
 * - AGENTREPLAY_AGENT_ID
 * - AGENTREPLAY_ENVIRONMENT
 * - AGENTREPLAY_DEBUG
 * - AGENTREPLAY_SAMPLING_RATE
 *
 * @example
 * ```typescript
 * import { init } from '@agentreplay/sdk';
 *
 * // Use env vars
 * init();
 *
 * // Or explicit config
 * init({
 *   apiKey: 'ar_xxx',
 *   baseUrl: 'https://api.agentreplay.dev',
 *   tenantId: 1,
 *   environment: 'production',
 *   sampling: { rate: 0.1 },
 *   privacy: { redact: ['messages.*.content'] }
 * });
 * ```
 */
export function init(options: InitOptions = {}): ResolvedConfig {
  // Read from env vars first
  const envApiKey = getEnv('AGENTREPLAY_API_KEY');
  const envUrl = getEnv('AGENTREPLAY_URL');
  const envTenantId = getEnvNumber('AGENTREPLAY_TENANT_ID', DEFAULT_CONFIG.tenantId);
  const envProjectId = getEnvNumber('AGENTREPLAY_PROJECT_ID', DEFAULT_CONFIG.projectId);
  const envAgentId = getEnvNumber('AGENTREPLAY_AGENT_ID', DEFAULT_CONFIG.agentId);
  const envEnvironment = getEnv('AGENTREPLAY_ENVIRONMENT');
  const envDebug = getEnvBool('AGENTREPLAY_DEBUG');
  const envSamplingRate = getEnv('AGENTREPLAY_SAMPLING_RATE');

  // Merge: defaults < env vars < explicit options
  const config: ResolvedConfig = {
    apiKey: options.apiKey ?? envApiKey ?? DEFAULT_CONFIG.apiKey,
    baseUrl: (options.baseUrl ?? envUrl ?? DEFAULT_CONFIG.baseUrl).replace(/\/$/, ''),
    tenantId: options.tenantId ?? envTenantId,
    projectId: options.projectId ?? envProjectId,
    agentId: options.agentId ?? envAgentId,
    environment: options.environment ?? envEnvironment ?? DEFAULT_CONFIG.environment,
    debug: options.debug ?? envDebug ?? DEFAULT_CONFIG.debug,
    strict: options.strict ?? DEFAULT_CONFIG.strict,
    timeout: options.timeout ?? DEFAULT_CONFIG.timeout,
    sampling: {
      ...DEFAULT_CONFIG.sampling,
      ...options.sampling,
      rate: options.sampling?.rate ?? (envSamplingRate ? parseFloat(envSamplingRate) : DEFAULT_CONFIG.sampling.rate),
    },
    privacy: {
      ...DEFAULT_CONFIG.privacy,
      ...options.privacy,
    },
    transport: {
      ...DEFAULT_CONFIG.transport,
      ...options.transport,
    },
    headers: {
      ...DEFAULT_CONFIG.headers,
      ...options.headers,
    },
    fetch: options.fetch ?? globalThis.fetch,
  };

  // Validate in strict mode
  if (config.strict && !config.apiKey) {
    throw new Error(
      'Agentreplay: API key is required in strict mode. ' +
      'Set AGENTREPLAY_API_KEY environment variable or pass apiKey option.'
    );
  }

  // Warn if no API key (non-strict)
  if (!config.apiKey && config.debug) {
    console.warn(
      '[Agentreplay] No API key configured. Set AGENTREPLAY_API_KEY or pass apiKey option.'
    );
  }

  // Debug logging
  if (config.debug) {
    console.log('[Agentreplay] Initialized with config:', {
      baseUrl: config.baseUrl,
      tenantId: config.tenantId,
      projectId: config.projectId,
      agentId: config.agentId,
      environment: config.environment,
      samplingRate: config.sampling.rate,
      transportMode: config.transport.mode,
      privacyMode: config.privacy.mode,
      apiKey: config.apiKey ? '***' + config.apiKey.slice(-4) : 'not set',
    });
  }

  globalConfig = config;
  isInitialized = true;

  return config;
}

/**
 * Get the current SDK configuration.
 * Throws if SDK not initialized.
 */
export function getConfig(): ResolvedConfig {
  if (!isInitialized || !globalConfig) {
    throw new Error(
      'Agentreplay SDK not initialized. Call init() first.'
    );
  }
  return globalConfig;
}

/**
 * Check if SDK is initialized.
 */
export function isSDKInitialized(): boolean {
  return isInitialized;
}

/**
 * Get config or return null if not initialized.
 */
export function getConfigOrNull(): ResolvedConfig | null {
  return globalConfig;
}

/**
 * Reset SDK state (for testing).
 */
export function resetConfig(): void {
  globalConfig = null;
  isInitialized = false;
}
