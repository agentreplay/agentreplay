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

import useSWR from 'swr';

export interface TimeseriesBucket {
  timestamp: number;
  request_count: number;
  total_tokens: number;
  total_cost: number;
  avg_duration: number;
  error_count: number;
  p50_duration: number;
  p95_duration: number;
  p99_duration: number;
}

interface TimeseriesResponse {
  data: TimeseriesBucket[];
  metadata: {
    start_ts: number;
    end_ts: number;
    interval_seconds: number;
    bucket_count: number;
  };
}

const fetcher = async (url: string) => {
  const headers = new Headers();
  if (typeof window !== 'undefined') {
    const apiKey = window.localStorage.getItem('flowtrace_api_key');
    if (apiKey) {
      headers.set('X-API-Key', apiKey);
    }
  }

  const res = await fetch(url, { headers });
  if (!res.ok) {
    throw new Error(`Timeseries fetch failed (${res.status})`);
  }

  return res.json() as Promise<TimeseriesResponse>;
};

export function useTimeseriesMetrics(
  startTs: number,
  endTs: number,
  intervalSeconds = 300,
) {
  const { data, error, isLoading, mutate } = useSWR<TimeseriesResponse>(
    `/api/v1/metrics/timeseries?start_ts=${startTs}&end_ts=${endTs}&interval_seconds=${intervalSeconds}`,
    fetcher,
    {
      refreshInterval: 30_000,
      revalidateOnFocus: false,
      dedupingInterval: 10_000,
    },
  );

  return {
    data: data?.data ?? [],
    metadata: data?.metadata,
    error,
    isLoading,
    refresh: mutate,
  };
}
