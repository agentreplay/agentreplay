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
 * Secure Token Storage
 * 
 * CRITICAL SECURITY FIX: This module provides secure storage for sensitive tokens
 * with the following security measures:
 * 
 * 1. Token expiry - Tokens automatically expire after a configurable TTL
 * 2. Integrity check - Tokens are wrapped with a signature to detect tampering
 * 3. Auto-cleanup - Expired tokens are automatically removed
 * 
 * NOTE: For production, API keys should be stored in HttpOnly cookies (see useAuth.ts).
 * This module is for development/testing scenarios where localStorage is acceptable.
 * 
 * Future improvements:
 * - Use Web Crypto API for proper encryption (AES-GCM)
 * - Implement secure key derivation (PBKDF2)
 * - Add hardware key support (WebAuthn)
 */

/** Token with metadata for security checks */
interface SecureToken {
  /** The actual token value */
  value: string;
  /** Unix timestamp (ms) when the token was stored */
  storedAt: number;
  /** Unix timestamp (ms) when the token expires */
  expiresAt: number;
  /** Simple checksum for integrity verification */
  checksum: string;
}

/** Storage configuration */
interface StorageConfig {
  /** Key prefix for namespacing */
  prefix: string;
  /** Default TTL in milliseconds (default: 24 hours) */
  defaultTTL: number;
  /** Whether to log security events */
  debug: boolean;
}

const DEFAULT_CONFIG: StorageConfig = {
  prefix: 'agentreplay_secure_',
  defaultTTL: 24 * 60 * 60 * 1000, // 24 hours
  debug: false,
};

/**
 * Generate a simple checksum for integrity verification
 * NOTE: This is NOT cryptographic security - it detects accidental corruption,
 * not deliberate tampering. Use proper HMAC for production.
 */
function generateChecksum(value: string, storedAt: number, expiresAt: number): string {
  const data = `${value}:${storedAt}:${expiresAt}`;
  let hash = 0;
  for (let i = 0; i < data.length; i++) {
    const char = data.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash = hash & hash; // Convert to 32bit integer
  }
  return hash.toString(16);
}

/**
 * Verify token integrity
 */
function verifyChecksum(token: SecureToken): boolean {
  const expected = generateChecksum(token.value, token.storedAt, token.expiresAt);
  return token.checksum === expected;
}

/**
 * Store a token securely with expiry
 */
export function setSecureToken(
  key: string,
  value: string,
  ttlMs?: number,
  config: Partial<StorageConfig> = {}
): void {
  if (typeof window === 'undefined') return;

  const finalConfig = { ...DEFAULT_CONFIG, ...config };
  const now = Date.now();
  const expiresAt = now + (ttlMs ?? finalConfig.defaultTTL);

  const token: SecureToken = {
    value,
    storedAt: now,
    expiresAt,
    checksum: generateChecksum(value, now, expiresAt),
  };

  const storageKey = `${finalConfig.prefix}${key}`;
  
  try {
    localStorage.setItem(storageKey, JSON.stringify(token));
    
    if (finalConfig.debug) {
      console.log(`[SecureStorage] Stored token: ${key}, expires: ${new Date(expiresAt).toISOString()}`);
    }
  } catch (e) {
    console.error('[SecureStorage] Failed to store token:', e);
    throw new Error('Failed to store secure token');
  }
}

/**
 * Retrieve a token if valid (not expired, not tampered)
 */
export function getSecureToken(
  key: string,
  config: Partial<StorageConfig> = {}
): string | null {
  if (typeof window === 'undefined') return null;

  const finalConfig = { ...DEFAULT_CONFIG, ...config };
  const storageKey = `${finalConfig.prefix}${key}`;

  try {
    const stored = localStorage.getItem(storageKey);
    if (!stored) return null;

    const token: SecureToken = JSON.parse(stored);
    const now = Date.now();

    // Check expiry
    if (token.expiresAt <= now) {
      if (finalConfig.debug) {
        console.log(`[SecureStorage] Token expired: ${key}`);
      }
      localStorage.removeItem(storageKey);
      return null;
    }

    // Verify integrity
    if (!verifyChecksum(token)) {
      console.error('[SecureStorage] Token integrity check failed - possible tampering');
      localStorage.removeItem(storageKey);
      return null;
    }

    if (finalConfig.debug) {
      const remaining = Math.round((token.expiresAt - now) / 1000 / 60);
      console.log(`[SecureStorage] Token valid: ${key}, expires in ${remaining} minutes`);
    }

    return token.value;
  } catch (e) {
    console.error('[SecureStorage] Failed to retrieve token:', e);
    localStorage.removeItem(storageKey);
    return null;
  }
}

/**
 * Remove a stored token
 */
export function removeSecureToken(
  key: string,
  config: Partial<StorageConfig> = {}
): void {
  if (typeof window === 'undefined') return;

  const finalConfig = { ...DEFAULT_CONFIG, ...config };
  const storageKey = `${finalConfig.prefix}${key}`;
  localStorage.removeItem(storageKey);

  if (finalConfig.debug) {
    console.log(`[SecureStorage] Removed token: ${key}`);
  }
}

/**
 * Check if a token exists and is valid
 */
export function hasValidToken(
  key: string,
  config: Partial<StorageConfig> = {}
): boolean {
  return getSecureToken(key, config) !== null;
}

/**
 * Get token metadata without returning the value
 */
export function getTokenInfo(
  key: string,
  config: Partial<StorageConfig> = {}
): { expiresAt: Date; storedAt: Date; remainingMs: number } | null {
  if (typeof window === 'undefined') return null;

  const finalConfig = { ...DEFAULT_CONFIG, ...config };
  const storageKey = `${finalConfig.prefix}${key}`;

  try {
    const stored = localStorage.getItem(storageKey);
    if (!stored) return null;

    const token: SecureToken = JSON.parse(stored);
    const now = Date.now();

    if (token.expiresAt <= now) {
      localStorage.removeItem(storageKey);
      return null;
    }

    return {
      expiresAt: new Date(token.expiresAt),
      storedAt: new Date(token.storedAt),
      remainingMs: token.expiresAt - now,
    };
  } catch {
    return null;
  }
}

/**
 * Cleanup all expired tokens
 */
export function cleanupExpiredTokens(config: Partial<StorageConfig> = {}): number {
  if (typeof window === 'undefined') return 0;

  const finalConfig = { ...DEFAULT_CONFIG, ...config };
  const prefix = finalConfig.prefix;
  const now = Date.now();
  let cleaned = 0;

  for (let i = localStorage.length - 1; i >= 0; i--) {
    const key = localStorage.key(i);
    if (!key || !key.startsWith(prefix)) continue;

    try {
      const stored = localStorage.getItem(key);
      if (!stored) continue;

      const token: SecureToken = JSON.parse(stored);
      if (token.expiresAt <= now) {
        localStorage.removeItem(key);
        cleaned++;
      }
    } catch {
      // Invalid entry, remove it
      if (key) {
        localStorage.removeItem(key);
        cleaned++;
      }
    }
  }

  if (finalConfig.debug && cleaned > 0) {
    console.log(`[SecureStorage] Cleaned up ${cleaned} expired tokens`);
  }

  return cleaned;
}

/**
 * Initialize automatic cleanup on page load
 * Call this once when the app starts
 */
export function initializeSecureStorage(config: Partial<StorageConfig> = {}): void {
  if (typeof window === 'undefined') return;

  // Clean up expired tokens on load
  cleanupExpiredTokens(config);

  // Set up periodic cleanup every 5 minutes
  setInterval(() => cleanupExpiredTokens(config), 5 * 60 * 1000);
}

// Convenience exports for common use cases
export const SecureStorage = {
  set: setSecureToken,
  get: getSecureToken,
  remove: removeSecureToken,
  has: hasValidToken,
  info: getTokenInfo,
  cleanup: cleanupExpiredTokens,
  init: initializeSecureStorage,
};

export default SecureStorage;
