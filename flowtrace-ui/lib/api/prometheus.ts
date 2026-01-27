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

// Prometheus API client for Flowtrace UI
import axios from 'axios';

const PROMETHEUS_URL = process.env.NEXT_PUBLIC_PROMETHEUS_URL || 'http://localhost:9603';

export interface PrometheusResult {
  metric: Record<string, string>;
  value?: [number, string];
  values?: [number, string][];
}

export interface PrometheusResponse {
  status: string;
  data: {
    resultType: string;
    result: PrometheusResult[];
  };
}

class PrometheusClient {
  private baseURL: string;

  constructor(baseURL: string = PROMETHEUS_URL) {
    this.baseURL = baseURL;
  }

  async query(query: string, time?: number): Promise<PrometheusResult[]> {
    try {
      const params = new URLSearchParams({
        query,
        ...(time && { time: Math.floor(time / 1000).toString() }),
      });

      const response = await axios.get<PrometheusResponse>(
        `${this.baseURL}/api/v1/query?${params}`
      );

      return response.data.data.result;
    } catch (error) {
      console.error('Error querying Prometheus:', error);
      return [];
    }
  }

  async queryRange(
    query: string,
    start: number,
    end: number,
    step: string = '15s'
  ): Promise<PrometheusResult[]> {
    try {
      const params = new URLSearchParams({
        query,
        start: Math.floor(start / 1000).toString(),
        end: Math.floor(end / 1000).toString(),
        step,
      });

      const response = await axios.get<PrometheusResponse>(
        `${this.baseURL}/api/v1/query_range?${params}`
      );

      return response.data.data.result;
    } catch (error) {
      console.error('Error querying Prometheus range:', error);
      return [];
    }
  }
}

// Pre-defined queries for LLM metrics
export const LLMQueries = {
  requestRate: 'rate(flowtrace_llm_requests_total[5m])',
  latencyP95: 'histogram_quantile(0.95, rate(flowtrace_llm_latency_seconds_bucket[5m]))',
  latencyP50: 'histogram_quantile(0.50, rate(flowtrace_llm_latency_seconds_bucket[5m]))',
  costPerHour: 'increase(flowtrace_llm_cost_total[1h])',
  inputTokens: 'rate(flowtrace_llm_tokens_input_total[5m])',
  outputTokens: 'rate(flowtrace_llm_tokens_output_total[5m])',
  errorRate: 'rate(flowtrace_llm_errors_total[5m]) / rate(flowtrace_llm_requests_total[5m])',
  hallucinationRate: 'rate(flowtrace_llm_hallucination_detected_total[5m])',
  avgRelevanceScore: 'avg(flowtrace_llm_relevance_score)',
  avgGroundednessScore: 'avg(flowtrace_llm_groundedness_score)',
};

export const prometheusClient = new PrometheusClient();
