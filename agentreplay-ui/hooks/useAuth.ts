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

import { useEffect, useState } from 'react';

/**
 * useAuth Hook - Secure authentication management
 * 
 * SECURITY: API keys are stored in HttpOnly cookies on the server, not localStorage
 * This prevents XSS attacks from stealing credentials
 * 
 * Migration from localStorage:
 * - Old: API key stored in browser localStorage (vulnerable to XSS)
 * - New: API key stored in secure HttpOnly cookie (not accessible to JavaScript)
 */
export function useAuth() {
  const [apiKey, setApiKey] = useState<string | null>(null); // Not used anymore, kept for backwards compatibility
  const [tenantId, setTenantId] = useState<number | null>(null);
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [loading, setLoading] = useState(true);

  // Check session on mount
  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }

    checkSession();
  }, []);

  const checkSession = async () => {
    try {
      const response = await fetch('/api/auth', {
        method: 'GET',
        credentials: 'include', // Include cookies
      });

      const data = await response.json();

      if (data.authenticated) {
        setIsAuthenticated(true);
        setTenantId(data.tenantId);
      } else {
        setIsAuthenticated(false);
        setTenantId(null);
      }
    } catch (error) {
      console.error('Session check failed:', error);
      setIsAuthenticated(false);
      setTenantId(null);
    } finally {
      setLoading(false);
    }
  };

  const login = async (key: string, tenant: number) => {
    if (typeof window === 'undefined') {
      return;
    }

    try {
      const response = await fetch('/api/auth', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        credentials: 'include', // Include cookies
        body: JSON.stringify({
          apiKey: key,
          tenantId: tenant,
        }),
      });

      if (response.ok) {
        setIsAuthenticated(true);
        setTenantId(tenant);
        // API key is NOT stored in client state for security
      } else {
        const error = await response.json();
        throw new Error(error.error || 'Login failed');
      }
    } catch (error) {
      console.error('Login failed:', error);
      throw error;
    }
  };

  const logout = async () => {
    if (typeof window === 'undefined') {
      return;
    }

    try {
      await fetch('/api/auth', {
        method: 'DELETE',
        credentials: 'include', // Include cookies
      });
    } catch (error) {
      console.error('Logout failed:', error);
    } finally {
      setIsAuthenticated(false);
      setTenantId(null);
      setApiKey(null);
    }
  };

  return {
    apiKey: null as string | null, // Never exposed to client anymore, but typed for compatibility
    tenantId,
    isAuthenticated,
    loading,
    login,
    logout,
  };
}

