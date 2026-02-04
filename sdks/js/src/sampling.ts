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
 * Sampling module for controlling trace collection rate.
 *
 * Supports:
 * - Base sampling rate
 * - Conditional rules (error, tags, paths)
 * - Deterministic sampling by key (e.g., userId)
 */

import type { SamplingConfig, SamplingRule } from './types';
import { getConfigOrNull } from './config';
import { getCurrentContext, getGlobalContext } from './context';

/**
 * Simple hash function for deterministic sampling
 */
function hashString(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash = hash & hash;
  }
  return Math.abs(hash);
}

/**
 * Check if a sampling rule matches the current context
 */
function ruleMatches(rule: SamplingRule): boolean {
  const ctx = getCurrentContext();
  const globalCtx = getGlobalContext();

  // Check error condition
  if (rule.when.error !== undefined) {
    // We can't know if there's an error at sampling time
    // This rule is evaluated after the span completes
    return false;
  }

  // Check tag condition
  if (rule.when.tag) {
    const tags = { ...globalCtx.tags, ...ctx?.tags };
    if (!(rule.when.tag in tags)) {
      return false;
    }
  }

  // Check path prefix
  if (rule.when.pathPrefix) {
    // Would need request path context - skip for now
    return false;
  }

  // Check custom condition
  if (rule.when.custom) {
    try {
      if (!rule.when.custom()) {
        return false;
      }
    } catch {
      return false;
    }
  }

  return true;
}

/**
 * Get effective sampling rate based on config and rules
 */
function getEffectiveSampleRate(): number {
  const config = getConfigOrNull();
  const samplingConfig = config?.sampling;

  if (!samplingConfig) {
    return 1.0; // Sample everything by default
  }

  // Check rules first (they take precedence)
  if (samplingConfig.rules && samplingConfig.rules.length > 0) {
    for (const rule of samplingConfig.rules) {
      if (ruleMatches(rule)) {
        return rule.sample;
      }
    }
  }

  return samplingConfig.rate ?? 1.0;
}

/**
 * Get deterministic key for stable sampling
 */
function getDeterministicKey(): string | undefined {
  const config = getConfigOrNull();
  const samplingConfig = config?.sampling;

  if (!samplingConfig?.deterministicKey) {
    return undefined;
  }

  const ctx = getCurrentContext();
  const globalCtx = getGlobalContext();

  // Look for the key in context
  switch (samplingConfig.deterministicKey) {
    case 'userId':
      return globalCtx.userId ?? ctx?.userId;
    case 'sessionId':
      return String(globalCtx.sessionId ?? ctx?.sessionId ?? '');
    case 'traceId':
      return ctx?.traceId;
    default:
      // Look in tags
      const tags = { ...globalCtx.tags, ...ctx?.tags };
      return tags[samplingConfig.deterministicKey];
  }
}

/**
 * Determine if the current span should be sampled.
 *
 * @example
 * ```typescript
 * if (shouldSample()) {
 *   // Create and send span
 * }
 * ```
 */
export function shouldSample(): boolean {
  const rate = getEffectiveSampleRate();

  // Always sample
  if (rate >= 1.0) {
    return true;
  }

  // Never sample
  if (rate <= 0.0) {
    return false;
  }

  // Deterministic sampling
  const deterministicKey = getDeterministicKey();
  if (deterministicKey) {
    const hash = hashString(deterministicKey);
    const threshold = Math.floor(rate * 0xffffffff);
    return (hash % 0xffffffff) < threshold;
  }

  // Random sampling
  return Math.random() < rate;
}

/**
 * Create a sampler function with custom logic.
 *
 * @example
 * ```typescript
 * const sampler = createSampler({
 *   rate: 0.1,
 *   rules: [
 *     { when: { tag: 'vip' }, sample: 1.0 },
 *     { when: { error: true }, sample: 1.0 }
 *   ]
 * });
 *
 * if (sampler()) {
 *   // Create span
 * }
 * ```
 */
export function createSampler(config: SamplingConfig): () => boolean {
  return () => {
    // Check rules
    if (config.rules && config.rules.length > 0) {
      for (const rule of config.rules) {
        if (ruleMatches(rule)) {
          return Math.random() < rule.sample;
        }
      }
    }

    const rate = config.rate ?? 1.0;

    // Deterministic sampling
    if (config.deterministicKey) {
      const ctx = getCurrentContext();
      const globalCtx = getGlobalContext();
      let key: string | undefined;

      switch (config.deterministicKey) {
        case 'userId':
          key = globalCtx.userId ?? ctx?.userId;
          break;
        case 'sessionId':
          key = String(globalCtx.sessionId ?? ctx?.sessionId ?? '');
          break;
        case 'traceId':
          key = ctx?.traceId;
          break;
      }

      if (key) {
        const hash = hashString(key);
        const threshold = Math.floor(rate * 0xffffffff);
        return (hash % 0xffffffff) < threshold;
      }
    }

    return Math.random() < rate;
  };
}

/**
 * Always-sample sampler (for critical operations)
 */
export function alwaysSample(): boolean {
  return true;
}

/**
 * Never-sample sampler (for explicitly excluded operations)
 */
export function neverSample(): boolean {
  return false;
}
