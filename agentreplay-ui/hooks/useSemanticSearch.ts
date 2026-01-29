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

import { useState } from 'react';
import { API_BASE_URL } from '../src/lib/agentreplay-api';

export interface TraceSearchResult {
  edge_id: string;
  timestamp_us: number;
  operation: string;
  span_type: string;
  duration_ms: number;
  tokens: number;
  cost: number;
  status: string;
  model?: string | null;
  agent_id: number;
  session_id: number;
}

export interface QueryInterpretation {
  model_filter?: string | null;
  error_filter: boolean;
  min_tokens?: number | null;
  time_range: string;
}

interface SearchResponse {
  results: TraceSearchResult[];
  count: number;
  query_interpretation: QueryInterpretation;
}

export function useSemanticSearch() {
  const [loading, setLoading] = useState(false);
  const [results, setResults] = useState<TraceSearchResult[]>([]);
  const [interpretation, setInterpretation] = useState<QueryInterpretation | null>(null);
  const [error, setError] = useState<Error | null>(null);

  const search = async (query: string, limit = 100) => {
    setLoading(true);
    setError(null);

    try {
      const headers = new Headers({ 'Content-Type': 'application/json' });
      if (typeof window !== 'undefined') {
        const apiKey = window.localStorage.getItem('agentreplay_api_key');
        if (apiKey) {
          headers.set('X-API-Key', apiKey);
        }
      }

      const response = await fetch(`${API_BASE_URL}/api/v1/search`, {
        method: 'POST',
        headers,
        body: JSON.stringify({ query, limit }),
      });

      if (!response.ok) {
        throw new Error(`Search failed (${response.status})`);
      }

      const data: SearchResponse = await response.json();
      setResults(data.results);
      setInterpretation(data.query_interpretation);
    } catch (err) {
      console.error('[Agentreplay] Semantic search error', err);
      setError(err as Error);
      setResults([]);
      setInterpretation(null);
    } finally {
      setLoading(false);
    }
  };

  const clear = () => {
    setResults([]);
    setInterpretation(null);
    setError(null);
  };

  return {
    search,
    loading,
    results,
    interpretation,
    error,
    clear,
  };
}
