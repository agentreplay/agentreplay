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
 * PostHog Analytics Wrapper for Flowtrace
 * 
 * This module provides a wrapper around the PostHog JS SDK with
 * automatic respect for user privacy preferences.
 * 
 * Analytics is enabled by default but can be disabled in Settings.
 */

import { posthog } from './posthog';

// Settings storage key
const SETTINGS_KEY = 'flowtrace_settings';

/**
 * Check if analytics is enabled in user settings
 */
function isAnalyticsEnabled(): boolean {
  try {
    const settings = localStorage.getItem(SETTINGS_KEY);
    if (!settings) {
      // Default to enabled if no settings exist
      return true;
    }
    const parsed = JSON.parse(settings);
    // Default to enabled if analytics section doesn't exist
    return parsed?.analytics?.enabled ?? true;
  } catch {
    return true;
  }
}

/**
 * PostHog Analytics wrapper
 * All methods check if analytics is enabled before sending events
 */
export const Analytics = {
  /**
   * Capture an event with optional properties
   */
  capture(eventName: string, properties?: Record<string, unknown>): void {
    console.log('[Analytics] capture called:', eventName, 'enabled:', isAnalyticsEnabled());

    if (!isAnalyticsEnabled()) {
      console.log('[Analytics] Skipped - analytics disabled');
      return;
    }

    try {
      posthog.capture(eventName, properties);
      console.log('[Analytics] Event sent:', eventName);
    } catch (error) {
      console.debug('[Analytics] Failed to capture event:', error);
    }
  },

  /**
   * Identify a user (anonymous by default, use for user ID if provided)
   */
  identify(distinctId: string, properties?: Record<string, unknown>): void {
    if (!isAnalyticsEnabled()) {
      return;
    }

    try {
      posthog.identify(distinctId, properties);
    } catch (error) {
      console.debug('[Analytics] Failed to identify:', error);
    }
  },

  /**
   * Capture an anonymous event (no user identification)
   */
  captureAnonymous(eventName: string, properties?: Record<string, unknown>): void {
    if (!isAnalyticsEnabled()) {
      return;
    }

    try {
      posthog.capture(eventName, { ...properties, anonymous: true });
    } catch (error) {
      console.debug('[Analytics] Failed to capture anonymous event:', error);
    }
  },

  /**
   * Capture a page view (call on route changes)
   */
  capturePageView(pageName?: string, properties?: Record<string, unknown>): void {
    if (!isAnalyticsEnabled()) {
      return;
    }

    try {
      posthog.capture('$pageview', {
        $current_url: pageName || window.location.pathname,
        ...properties
      });
    } catch (error) {
      console.debug('[Analytics] Failed to capture page view:', error);
    }
  },

  /**
   * Reset the user (call on logout/clear)
   */
  reset(): void {
    try {
      posthog.reset();
    } catch (error) {
      console.debug('[Analytics] Failed to reset:', error);
    }
  },

  /**
   * Opt out of capturing (user disabled analytics)
   */
  optOut(): void {
    try {
      posthog.opt_out_capturing();
    } catch (error) {
      console.debug('[Analytics] Failed to opt out:', error);
    }
  },

  /**
   * Opt in to capturing (user enabled analytics)
   */
  optIn(): void {
    try {
      posthog.opt_in_capturing();
    } catch (error) {
      console.debug('[Analytics] Failed to opt in:', error);
    }
  },

  /**
   * Check if analytics is currently enabled
   */
  isEnabled(): boolean {
    return isAnalyticsEnabled();
  },
};

// Common events used throughout the app
export const AnalyticsEvents = {
  // App lifecycle
  APP_OPENED: 'app_opened',
  APP_CLOSED: 'app_closed',

  // Trace events  
  TRACE_VIEWED: 'trace_viewed',
  TRACE_DELETED: 'trace_deleted',
  TRACE_EXPORTED: 'trace_exported',

  // Search & Query
  SEARCH_PERFORMED: 'search_performed',
  FILTER_APPLIED: 'filter_applied',

  // Evaluation events
  EVAL_RUN_STARTED: 'eval_run_started',
  EVAL_RUN_COMPLETED: 'eval_run_completed',

  // Settings
  SETTINGS_CHANGED: 'settings_changed',
  THEME_CHANGED: 'theme_changed',
  ANALYTICS_TOGGLED: 'analytics_toggled',

  // Features
  FEATURE_USED: 'feature_used',
  ERROR_OCCURRED: 'error_occurred',
};

export default Analytics;
