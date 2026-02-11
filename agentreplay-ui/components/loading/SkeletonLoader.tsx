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

"use client";

import React from 'react';

export function SkeletonCard() {
  return (
    <div className="bg-card rounded-lg border border-border p-6 animate-pulse">
      <div className="h-4 bg-secondary rounded w-1/3 mb-4"></div>
      <div className="h-8 bg-muted rounded w-2/3"></div>
    </div>
  );
}

export function SkeletonTable() {
  return (
    <div className="bg-card rounded-lg border border-border overflow-hidden animate-pulse">
      {/* Header */}
      <div className="bg-gray-50 border-b border-gray-200 px-6 py-4">
        <div className="flex gap-4">
          <div className="h-4 bg-muted rounded w-24"></div>
          <div className="h-4 bg-muted rounded w-32"></div>
          <div className="h-4 bg-muted rounded w-20"></div>
          <div className="h-4 bg-muted rounded w-28"></div>
        </div>
      </div>
      
      {/* Rows */}
      {[...Array(8)].map((_, i) => (
        <div key={i} className="border-b border-gray-100 px-6 py-4">
          <div className="flex gap-4 items-center">
            <div className="h-6 bg-secondary rounded w-24"></div>
            <div className="h-6 bg-secondary rounded w-32"></div>
            <div className="h-6 bg-secondary rounded w-20"></div>
            <div className="h-6 bg-secondary rounded w-28"></div>
            <div className="h-6 bg-secondary rounded w-16 ml-auto"></div>
          </div>
        </div>
      ))}
    </div>
  );
}

export function SkeletonChart() {
  return (
    <div className="bg-card rounded-lg border border-border p-6 animate-pulse">
      <div className="h-4 bg-secondary rounded w-1/4 mb-6"></div>
      <div className="space-y-3">
        {[...Array(6)].map((_, i) => (
          <div key={i} className="flex items-end gap-2" style={{ height: '120px' }}>
            <div className="flex-1 bg-secondary rounded" style={{ height: `${30 + Math.random() * 70}%` }}></div>
            <div className="flex-1 bg-secondary rounded" style={{ height: `${30 + Math.random() * 70}%` }}></div>
            <div className="flex-1 bg-secondary rounded" style={{ height: `${30 + Math.random() * 70}%` }}></div>
          </div>
        ))}
      </div>
    </div>
  );
}

export function SkeletonKPI() {
  return (
    <div className="bg-card rounded-lg border border-border p-6 animate-pulse">
      <div className="flex items-center justify-between mb-3">
        <div className="h-4 bg-secondary rounded w-24"></div>
        <div className="h-8 w-8 bg-secondary rounded"></div>
      </div>
      <div className="h-8 bg-muted rounded w-1/2 mb-2"></div>
      <div className="h-3 bg-secondary rounded w-1/3"></div>
    </div>
  );
}

export function SkeletonDashboard() {
  return (
    <div className="space-y-6">
      {/* KPI Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <SkeletonKPI />
        <SkeletonKPI />
        <SkeletonKPI />
        <SkeletonKPI />
      </div>
      
      {/* Chart */}
      <SkeletonChart />
      
      {/* Table */}
      <SkeletonTable />
    </div>
  );
}

export function SkeletonTraceTree() {
  return (
    <div className="bg-card rounded-lg border border-border overflow-hidden animate-pulse">
      <div className="bg-gray-50 border-b border-gray-200 px-4 py-3">
        <div className="h-3 bg-muted rounded w-32"></div>
      </div>
      <div className="divide-y divide-gray-100">
        {[...Array(6)].map((_, i) => (
          <div key={i} className="px-4 py-3 flex items-center gap-3">
            <div className="h-4 w-4 bg-secondary rounded"></div>
            <div className="h-6 bg-secondary rounded w-20"></div>
            <div className="h-4 bg-secondary rounded flex-1"></div>
            <div className="h-4 bg-secondary rounded w-16"></div>
          </div>
        ))}
      </div>
    </div>
  );
}

export default function SkeletonLoader({ type = 'dashboard' }: { type?: 'dashboard' | 'table' | 'chart' | 'card' | 'kpi' | 'tree' }) {
  switch (type) {
    case 'dashboard':
      return <SkeletonDashboard />;
    case 'table':
      return <SkeletonTable />;
    case 'chart':
      return <SkeletonChart />;
    case 'card':
      return <SkeletonCard />;
    case 'kpi':
      return <SkeletonKPI />;
    case 'tree':
      return <SkeletonTraceTree />;
    default:
      return <SkeletonDashboard />;
  }
}
