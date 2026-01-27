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

import React, { useEffect } from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';
import { ProjectProvider } from './context/project-context';
import { ToastProvider } from './context/toast-context';

// Initialize PostHog analytics (must be imported early)
import './lib/posthog';
import { Analytics, AnalyticsEvents } from './lib/analytics';

function Root() {
  useEffect(() => {
    // Block right-click context menu
    const blockContext = (event: MouseEvent) => {
      event.preventDefault();
    };
    document.addEventListener('contextmenu', blockContext);

    // Capture app opened event
    Analytics.capture(AnalyticsEvents.APP_OPENED, {
      platform: navigator.platform,
      userAgent: navigator.userAgent,
    });

    return () => document.removeEventListener('contextmenu', blockContext);
  }, []);

  return (
    <ToastProvider>
      <ProjectProvider>
        <App />
      </ProjectProvider>
    </ToastProvider>
  );
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>
);
