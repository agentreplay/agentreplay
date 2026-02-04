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
 * Privacy and redaction module.
 *
 * Provides client-side redaction to prevent sensitive data from leaving the app.
 */

import type { PrivacyConfig, Scrubber } from './types';
import { getConfigOrNull } from './config';

/**
 * Built-in scrubber for email addresses
 */
export const emailScrubber: Scrubber = {
  name: 'email',
  pattern: /[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/g,
  replacement: '[EMAIL_REDACTED]',
};

/**
 * Built-in scrubber for credit card numbers
 */
export const creditCardScrubber: Scrubber = {
  name: 'credit_card',
  pattern: /\b(?:\d[ -]*?){13,16}\b/g,
  replacement: '[CREDIT_CARD_REDACTED]',
};

/**
 * Built-in scrubber for API keys (common patterns)
 */
export const apiKeyScrubber: Scrubber = {
  name: 'api_key',
  pattern: /\b(sk-[a-zA-Z0-9]{20,}|api[_-]?key[=:]\s*["']?[a-zA-Z0-9_-]+["']?|bearer\s+[a-zA-Z0-9._-]+)\b/gi,
  replacement: '[API_KEY_REDACTED]',
};

/**
 * Built-in scrubber for phone numbers
 */
export const phoneScrubber: Scrubber = {
  name: 'phone',
  pattern: /\b(?:\+?1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b/g,
  replacement: '[PHONE_REDACTED]',
};

/**
 * Built-in scrubber for SSN
 */
export const ssnScrubber: Scrubber = {
  name: 'ssn',
  pattern: /\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b/g,
  replacement: '[SSN_REDACTED]',
};

/**
 * All built-in scrubbers
 */
export const builtInScrubbers: Scrubber[] = [
  emailScrubber,
  creditCardScrubber,
  apiKeyScrubber,
  phoneScrubber,
  ssnScrubber,
];

/**
 * Hash a value using a simple hash function (for PII hashing).
 * Note: This is not cryptographically secure, use for anonymization only.
 */
export function hashPII(value: string): string {
  let hash = 0;
  for (let i = 0; i < value.length; i++) {
    const char = value.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash = hash & hash; // Convert to 32bit integer
  }
  return `hash_${Math.abs(hash).toString(16)}`;
}

/**
 * Check if a path matches a redaction pattern.
 * Supports wildcards: messages.*.content, input.*
 */
function pathMatches(path: string, pattern: string): boolean {
  const pathParts = path.split('.');
  const patternParts = pattern.split('.');

  if (patternParts.length > pathParts.length) {
    return false;
  }

  for (let i = 0; i < patternParts.length; i++) {
    if (patternParts[i] === '*') {
      continue;
    }
    if (patternParts[i] !== pathParts[i]) {
      return false;
    }
  }

  return true;
}

/**
 * Check if a path should be redacted based on config
 */
function shouldRedactPath(path: string, redactPaths: string[]): boolean {
  return redactPaths.some((pattern) => pathMatches(path, pattern));
}

/**
 * Apply scrubbers to a string value
 */
function applyScrubbers(value: string, scrubbers: Scrubber[]): string {
  let result = value;
  for (const scrubber of scrubbers) {
    result = result.replace(scrubber.pattern, scrubber.replacement);
  }
  return result;
}

/**
 * Recursively redact a payload based on privacy config.
 */
function redactObject(
  obj: unknown,
  config: PrivacyConfig,
  currentPath: string = ''
): unknown {
  if (obj === null || obj === undefined) {
    return obj;
  }

  // Handle strings
  if (typeof obj === 'string') {
    // Check path-based redaction
    if (config.redact && shouldRedactPath(currentPath, config.redact)) {
      return '[REDACTED]';
    }

    // Apply scrubbers
    if (config.scrubbers && config.scrubbers.length > 0) {
      return applyScrubbers(obj, config.scrubbers);
    }

    return obj;
  }

  // Handle arrays
  if (Array.isArray(obj)) {
    return obj.map((item, index) =>
      redactObject(item, config, `${currentPath}.${index}`)
    );
  }

  // Handle objects
  if (typeof obj === 'object') {
    const result: Record<string, unknown> = {};

    for (const [key, value] of Object.entries(obj)) {
      const newPath = currentPath ? `${currentPath}.${key}` : key;

      // Check if this key should be completely removed
      if (config.removeKeys && config.removeKeys.includes(key)) {
        continue;
      }

      // Check path-based redaction
      if (config.redact && shouldRedactPath(newPath, config.redact)) {
        result[key] = '[REDACTED]';
      } else {
        result[key] = redactObject(value, config, newPath);
      }
    }

    return result;
  }

  // Return primitives as-is
  return obj;
}

/**
 * Redact a payload based on the current privacy configuration.
 *
 * @example
 * ```typescript
 * const data = {
 *   messages: [
 *     { role: 'user', content: 'My email is john@example.com' }
 *   ],
 *   apiKey: 'sk-abc123'
 * };
 *
 * const redacted = redactPayload(data);
 * // {
 * //   messages: [{ role: 'user', content: 'My email is [EMAIL_REDACTED]' }],
 * //   apiKey: '[REDACTED]'
 * // }
 * ```
 */
export function redactPayload<T>(payload: T): T {
  const config = getConfigOrNull();
  const privacyConfig = config?.privacy;

  // No privacy config or mode is 'none'
  if (!privacyConfig || privacyConfig.mode === 'none') {
    return payload;
  }

  // Allowlist mode - only keep allowed fields
  if (privacyConfig.mode === 'allowlist' && privacyConfig.allowlist) {
    if (typeof payload !== 'object' || payload === null) {
      return payload;
    }

    const result: Record<string, unknown> = {};
    for (const allowedPath of privacyConfig.allowlist) {
      const value = getValueAtPath(payload as Record<string, unknown>, allowedPath);
      if (value !== undefined) {
        setValueAtPath(result, allowedPath, value);
      }
    }
    return result as T;
  }

  // Redact mode - redact specified paths and apply scrubbers
  return redactObject(payload, privacyConfig, '') as T;
}

/**
 * Get value at a dot-separated path
 */
function getValueAtPath(obj: Record<string, unknown>, path: string): unknown {
  const parts = path.split('.');
  let current: unknown = obj;

  for (const part of parts) {
    if (current === null || current === undefined || typeof current !== 'object') {
      return undefined;
    }
    current = (current as Record<string, unknown>)[part];
  }

  return current;
}

/**
 * Set value at a dot-separated path
 */
function setValueAtPath(obj: Record<string, unknown>, path: string, value: unknown): void {
  const parts = path.split('.');
  let current = obj;

  for (let i = 0; i < parts.length - 1; i++) {
    const part = parts[i];
    if (!(part in current)) {
      current[part] = {};
    }
    current = current[part] as Record<string, unknown>;
  }

  current[parts[parts.length - 1]] = value;
}

/**
 * Truncate a value to a maximum size.
 * Useful for preventing huge payloads.
 */
export function truncateValue(
  value: unknown,
  maxBytes: number = 10000
): unknown {
  if (typeof value === 'string') {
    if (value.length > maxBytes) {
      return value.slice(0, maxBytes) + `... [truncated ${value.length - maxBytes} chars]`;
    }
    return value;
  }

  if (typeof value === 'object' && value !== null) {
    const json = JSON.stringify(value);
    if (json.length > maxBytes) {
      // Truncate and mark as truncated
      return {
        __truncated: true,
        __originalSize: json.length,
        __preview: json.slice(0, Math.min(1000, maxBytes)),
      };
    }
    return value;
  }

  return value;
}

/**
 * Safe JSON stringify that handles circular references and limits size.
 */
export function safeStringify(
  value: unknown,
  maxBytes: number = 100000
): string {
  const seen = new WeakSet();

  const replacer = (_key: string, val: unknown): unknown => {
    if (typeof val === 'object' && val !== null) {
      if (seen.has(val)) {
        return '[Circular]';
      }
      seen.add(val);
    }
    return val;
  };

  try {
    let json = JSON.stringify(value, replacer);
    if (json.length > maxBytes) {
      json = json.slice(0, maxBytes) + '..."[TRUNCATED]"';
    }
    return json;
  } catch {
    return '"[Unserializable]"';
  }
}
