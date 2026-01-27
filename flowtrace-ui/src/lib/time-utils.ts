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
 * Time Utilities for Flowtrace
 * 
 * CRITICAL FIX (Task 10): Standardize timestamp handling across frontend and backend
 * 
 * Backend uses MICROSECONDS (μs) since Unix epoch
 * Frontend JavaScript Date uses MILLISECONDS (ms) since Unix epoch
 * 
 * This utility ensures consistent conversions and prevents off-by-1000x errors
 */

// ============================================================================
// Constants
// ============================================================================

/** 1 second in microseconds */
export const SECOND_IN_MICROS = 1_000_000;

/** 1 minute in microseconds */
export const MINUTE_IN_MICROS = 60 * SECOND_IN_MICROS;

/** 1 hour in microseconds */
export const HOUR_IN_MICROS = 60 * MINUTE_IN_MICROS;

/** 1 day in microseconds */
export const DAY_IN_MICROS = 24 * HOUR_IN_MICROS;

/** 1 week in microseconds */
export const WEEK_IN_MICROS = 7 * DAY_IN_MICROS;

// ============================================================================
// Core Conversion Functions
// ============================================================================

/**
 * Convert microseconds (backend format) to milliseconds (JavaScript Date format)
 * 
 * @example
 * const timestampUs = 1700000000000000; // From backend
 * const date = new Date(microsToMillis(timestampUs));
 */
export function microsToMillis(micros: number): number {
  return Math.floor(micros / 1000);
}

/**
 * Convert milliseconds (JavaScript Date format) to microseconds (backend format)
 * 
 * @example
 * const now = Date.now(); // milliseconds
 * const nowUs = millisToMicros(now); // Send to backend
 */
export function millisToMicros(millis: number): number {
  return millis * 1000;
}

/**
 * Convert duration in microseconds to milliseconds
 */
export function durationMicrosToMillis(durationUs: number): number {
  return durationUs / 1000;
}

/**
 * Convert duration in microseconds to seconds
 */
export function durationMicrosToSeconds(durationUs: number): number {
  return durationUs / 1_000_000;
}

// ============================================================================
// Formatting Functions
// ============================================================================

/**
 * Format microsecond timestamp as human-readable date
 * 
 * @example
 * formatTimestamp(1700000000000000) // "Nov 15, 2023, 12:00:00 AM"
 */
export function formatTimestamp(timestampUs: number, options?: Intl.DateTimeFormatOptions): string {
  const date = new Date(microsToMillis(timestampUs));
  return date.toLocaleString(undefined, options);
}

/**
 * Format microsecond duration as human-readable string
 * 
 * @example
 * formatDuration(1500000) // "1.50s"
 * formatDuration(500) // "500μs"
 */
export function formatDuration(durationUs: number): string {
  if (durationUs < 1000) {
    return `${durationUs}μs`;
  } else if (durationUs < 1_000_000) {
    return `${(durationUs / 1000).toFixed(2)}ms`;
  } else if (durationUs < 60_000_000) {
    return `${(durationUs / 1_000_000).toFixed(2)}s`;
  } else if (durationUs < 3600_000_000) {
    const minutes = Math.floor(durationUs / 60_000_000);
    const seconds = ((durationUs % 60_000_000) / 1_000_000).toFixed(0);
    return `${minutes}m ${seconds}s`;
  } else {
    const hours = Math.floor(durationUs / 3600_000_000);
    const minutes = Math.floor((durationUs % 3600_000_000) / 60_000_000);
    return `${hours}h ${minutes}m`;
  }
}

/**
 * Format relative time from microsecond timestamp (e.g., "2 hours ago")
 * 
 * @example
 * formatRelativeTime(Date.now() * 1000 - 3600 * 1000000) // "1 hour ago"
 */
export function formatRelativeTime(timestampUs: number): string {
  const nowUs = Date.now() * 1000;
  const diffUs = nowUs - timestampUs;
  
  if (diffUs < 60 * SECOND_IN_MICROS) {
    const seconds = Math.floor(diffUs / SECOND_IN_MICROS);
    return `${seconds}s ago`;
  } else if (diffUs < 60 * MINUTE_IN_MICROS) {
    const minutes = Math.floor(diffUs / MINUTE_IN_MICROS);
    return `${minutes}m ago`;
  } else if (diffUs < 24 * HOUR_IN_MICROS) {
    const hours = Math.floor(diffUs / HOUR_IN_MICROS);
    return `${hours}h ago`;
  } else if (diffUs < 30 * DAY_IN_MICROS) {
    const days = Math.floor(diffUs / DAY_IN_MICROS);
    return `${days}d ago`;
  } else {
    const months = Math.floor(diffUs / (30 * DAY_IN_MICROS));
    return `${months}mo ago`;
  }
}

// ============================================================================
// Utility Functions for API Parameters
// ============================================================================

/**
 * Get current time in microseconds (for backend API calls)
 * 
 * @example
 * const params = {
 *   start_ts: nowMicros() - DAY_IN_MICROS,
 *   end_ts: nowMicros()
 * };
 */
export function nowMicros(): number {
  return Date.now() * 1000;
}

/**
 * Get time N days ago in microseconds
 */
export function daysAgoMicros(days: number): number {
  return nowMicros() - (days * DAY_IN_MICROS);
}

/**
 * Get time N hours ago in microseconds
 */
export function hoursAgoMicros(hours: number): number {
  return nowMicros() - (hours * HOUR_IN_MICROS);
}

/**
 * Get time N minutes ago in microseconds
 */
export function minutesAgoMicros(minutes: number): number {
  return nowMicros() - (minutes * MINUTE_IN_MICROS);
}

// ============================================================================
// Type Guards
// ============================================================================

/**
 * Check if a timestamp is in microseconds (vs milliseconds)
 * Heuristic: microsecond timestamps are > 1 trillion
 */
export function isMicroseconds(timestamp: number): boolean {
  // Microsecond timestamps since 2001-09-09 are > 1 trillion
  // Millisecond timestamps won't reach 1 trillion until year 33658
  return timestamp > 1_000_000_000_000;
}

/**
 * Auto-detect and convert timestamp to microseconds
 * Useful for handling mixed timestamp formats
 */
export function ensureMicroseconds(timestamp: number): number {
  return isMicroseconds(timestamp) ? timestamp : millisToMicros(timestamp);
}

/**
 * Auto-detect and convert timestamp to milliseconds
 * Useful for handling mixed timestamp formats
 */
export function ensureMilliseconds(timestamp: number): number {
  return isMicroseconds(timestamp) ? microsToMillis(timestamp) : timestamp;
}

// ============================================================================
// Validation
// ============================================================================

/**
 * Validate that a timestamp is within reasonable bounds
 * Backend enforces 2020-01-01 to 2099-12-31
 */
export function isValidTimestamp(timestampUs: number): boolean {
  const MIN_VALID = 1_577_836_800_000_000; // 2020-01-01
  const MAX_VALID = 4_102_444_800_000_000; // 2099-12-31
  return timestampUs >= MIN_VALID && timestampUs <= MAX_VALID;
}

// ============================================================================
// Chart/Visualization Helpers
// ============================================================================

/**
 * Format timestamp for chart x-axis labels
 */
export function formatChartTimestamp(timestampUs: number, showTime: boolean = true): string {
  const date = new Date(microsToMillis(timestampUs));
  
  if (showTime) {
    return date.toLocaleTimeString(undefined, { 
      hour: '2-digit', 
      minute: '2-digit',
      second: '2-digit'
    });
  } else {
    return date.toLocaleDateString(undefined, { 
      month: 'short', 
      day: 'numeric' 
    });
  }
}

/**
 * Format full timestamp for chart tooltip
 */
export function formatChartTooltipTimestamp(timestampUs: number): string {
  return formatTimestamp(timestampUs, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}
