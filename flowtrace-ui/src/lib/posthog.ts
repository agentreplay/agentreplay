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
 * PostHog Analytics Initialization for Flowtrace
 * 
 * This module initializes PostHog using the JavaScript SDK.
 * It's imported once in the app entry point (main.tsx).
 */

import posthog from 'posthog-js';

// PostHog configuration via environment variables
const POSTHOG_KEY = import.meta.env.VITE_POSTHOG_KEY || 'phc_Xpg4nGBpfHTzSqaXWyv5cibnX1TxkGdU8cBi7JV0fIJ';
const POSTHOG_HOST = import.meta.env.VITE_POSTHOG_HOST || 'https://us.i.posthog.com';

// Initialize PostHog
// Note: Always initialize - user consent is handled via opt_out_capturing() in analytics.ts
posthog.init(POSTHOG_KEY, {
    api_host: POSTHOG_HOST,
    // Manual control for Tauri SPA navigation
    capture_pageview: false,
    // Enable autocapture for clicks, inputs, etc.
    autocapture: true,
    // Use localStorage for persistence (works well in desktop apps)
    persistence: 'localStorage',
    // Disable session recording by default (can enable later)
    disable_session_recording: true,
    // Respect Do Not Track browser setting
    respect_dnt: true,
    // Don't load toolbar in Tauri app
    advanced_disable_toolbar_metrics: true,
    // Debug mode in development
    loaded: (posthog) => {
        if (import.meta.env.DEV) {
            console.debug('[PostHog] Initialized in dev mode');
        }
    },
});

console.log('[PostHog] Initialized with host:', POSTHOG_HOST, 'key:', POSTHOG_KEY.slice(0, 10) + '...');

export { posthog };
