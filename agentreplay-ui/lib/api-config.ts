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
 * API Configuration
 * 
 * Centralized API base URL detection for both Tauri and browser environments.
 */

/**
 * Determine the API base URL based on environment
 * - In Tauri: Use direct connection to embedded server
 * - In Vite dev: Use proxy (empty string)
 * - In production browser: Use direct connection
 */
export function getApiBaseUrl(): string {
  // In Tauri, window.__TAURI__ is defined
  const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
  
  if (isTauri) {
    // Tauri app: connect directly to embedded server
    return 'http://127.0.0.1:9600';
  }
  
  // Development with Vite proxy (port 5173)
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return ''; // Use Vite proxy
  }
  
  // Fallback: direct connection to server
  return 'http://127.0.0.1:9600';
}

// Export the base URL as a constant for convenience
export const API_BASE_URL = getApiBaseUrl();

/**
 * Check if we're running in Tauri
 */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window;
}

/**
 * Build a full API URL
 */
export function apiUrl(path: string): string {
  const base = getApiBaseUrl();
  // Ensure path starts with /
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  return `${base}${normalizedPath}`;
}
