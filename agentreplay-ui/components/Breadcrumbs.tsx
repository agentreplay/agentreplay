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

'use client';

import { ChevronRight, Home } from 'lucide-react';
import { Link, useLocation, useParams } from 'react-router-dom';
import { cn } from '../lib/utils';

interface BreadcrumbItem {
  label: string;
  href?: string;
  icon?: React.ReactNode;
}

interface BreadcrumbsProps {
  items?: BreadcrumbItem[];
  className?: string;
}

// Auto-generate breadcrumbs from URL if items not provided
function useBreadcrumbsFromPath(): BreadcrumbItem[] {
  const { pathname } = useLocation();
  const { projectId, traceId, sessionId, promptId, runId } = useParams();
  
  const items: BreadcrumbItem[] = [];
  
  // Parse path segments
  const segments = pathname.split('/').filter(Boolean);
  
  if (projectId) {
    // Skip 'projects' and projectId in breadcrumb display
    const projectIndex = segments.indexOf('projects');
    if (projectIndex !== -1) {
      const remainingSegments = segments.slice(projectIndex + 2);
      
      for (let i = 0; i < remainingSegments.length; i++) {
        const segment = remainingSegments[i];
        const href = `/projects/${projectId}/${remainingSegments.slice(0, i + 1).join('/')}`;
        
        // Handle special segments
        if (segment === traceId) {
          items.push({
            label: `Trace ${traceId.slice(0, 8)}...`,
            href: i < remainingSegments.length - 1 ? href : undefined,
          });
        } else if (segment === sessionId) {
          items.push({
            label: `Session ${sessionId.slice(0, 8)}...`,
            href: i < remainingSegments.length - 1 ? href : undefined,
          });
        } else if (segment === promptId) {
          items.push({
            label: `Prompt ${promptId}`,
            href: i < remainingSegments.length - 1 ? href : undefined,
          });
        } else if (segment === runId) {
          items.push({
            label: `Run ${runId.slice(0, 8)}...`,
            href: i < remainingSegments.length - 1 ? href : undefined,
          });
        } else if (segment === 'runs') {
          // Skip 'runs' as it's part of the path structure
          continue;
        } else {
          // Capitalize and format segment name
          const label = segment
            .split('-')
            .map(word => word.charAt(0).toUpperCase() + word.slice(1))
            .join(' ');
          
          items.push({
            label,
            href: i < remainingSegments.length - 1 ? href : undefined,
          });
        }
      }
    }
  }
  
  return items;
}

export function Breadcrumbs({ items: providedItems, className }: BreadcrumbsProps) {
  const autoItems = useBreadcrumbsFromPath();
  const items = providedItems || autoItems;
  
  if (items.length === 0) return null;
  
  return (
    <nav 
      aria-label="Breadcrumb"
      className={cn("flex items-center gap-1 text-sm", className)}
    >
      {items.map((item, index) => (
        <div key={index} className="flex items-center gap-1">
          {index > 0 && (
            <ChevronRight className="w-3.5 h-3.5 text-textTertiary flex-shrink-0" />
          )}
          
          {item.href ? (
            <Link
              to={item.href}
              className="flex items-center gap-1.5 text-textSecondary hover:text-textPrimary transition-colors px-1.5 py-0.5 rounded hover:bg-surface"
            >
              {item.icon}
              <span className="truncate max-w-[150px]">{item.label}</span>
            </Link>
          ) : (
            <span className="flex items-center gap-1.5 text-textPrimary font-medium px-1.5 py-0.5">
              {item.icon}
              <span className="truncate max-w-[200px]">{item.label}</span>
            </span>
          )}
        </div>
      ))}
    </nav>
  );
}

// Compact breadcrumbs for constrained spaces
export function CompactBreadcrumbs({ items: providedItems, className }: BreadcrumbsProps) {
  const autoItems = useBreadcrumbsFromPath();
  const items = providedItems || autoItems;
  
  if (items.length <= 1) return null;
  
  // Show only first and last items with ellipsis
  const firstItem = items[0];
  const lastItem = items[items.length - 1];
  const hasMiddle = items.length > 2;
  
  return (
    <nav 
      aria-label="Breadcrumb"
      className={cn("flex items-center gap-1 text-sm", className)}
    >
      {firstItem.href ? (
        <Link
          to={firstItem.href}
          className="text-textSecondary hover:text-textPrimary transition-colors"
        >
          {firstItem.label}
        </Link>
      ) : (
        <span className="text-textPrimary">{firstItem.label}</span>
      )}
      
      <ChevronRight className="w-3.5 h-3.5 text-textTertiary" />
      
      {hasMiddle && (
        <>
          <span className="text-textTertiary">...</span>
          <ChevronRight className="w-3.5 h-3.5 text-textTertiary" />
        </>
      )}
      
      <span className="text-textPrimary font-medium truncate max-w-[150px]">
        {lastItem.label}
      </span>
    </nav>
  );
}
