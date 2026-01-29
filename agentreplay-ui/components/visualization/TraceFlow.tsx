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

import { useMemo } from 'react';
import { AgentFlowDAG, DAGNode, DAGEdge } from './AgentFlowDAG';

interface Span {
  span_id: string;
  trace_id: string;
  parent_span_id?: string;
  name: string;
  start_time: string | number;
  end_time: string | number;
  attributes?: Record<string, any>;
  status?: string;
}

interface TraceFlowProps {
  spans: Span[];
  onSpanClick?: (span: Span) => void;
  className?: string;
}

export function TraceFlow({ spans, onSpanClick, className }: TraceFlowProps) {
  const { nodes, edges, spanMap } = useMemo(() => {
    const spanMap = new Map<string, Span>();
    const nodes: DAGNode[] = [];
    const edges: DAGEdge[] = [];
    
    // Build span map
    spans.forEach(span => {
      spanMap.set(span.span_id, span);
    });
    
    // Convert spans to DAG nodes
    spans.forEach(span => {
      const attrs = span.attributes || {};
      
      // Determine node type based on span attributes
      let type: DAGNode['type'] = 'service';
      if (attrs['gen_ai.system'] || attrs['llm.model'] || span.name.toLowerCase().includes('llm')) {
        type = 'llm';
      } else if (attrs['tool.name'] || span.name.toLowerCase().includes('tool')) {
        type = 'tool';
      } else if (attrs['agent.name'] || span.name.toLowerCase().includes('agent')) {
        type = 'agent';
      } else if (span.name.toLowerCase().includes('db') || span.name.toLowerCase().includes('database')) {
        type = 'database';
      } else if (attrs['http.url'] || span.name.toLowerCase().includes('http')) {
        type = 'external';
      }
      
      // Calculate duration
      const startTime = typeof span.start_time === 'string' ? new Date(span.start_time).getTime() : span.start_time;
      const endTime = typeof span.end_time === 'string' ? new Date(span.end_time).getTime() : span.end_time;
      const duration = Math.round((endTime - startTime) / 1000); // ms
      
      nodes.push({
        id: span.span_id,
        type,
        label: span.name.length > 20 ? span.name.slice(0, 20) + '…' : span.name,
        status: span.status === 'error' ? 'error' : 'completed',
        duration,
        tokens: attrs['gen_ai.usage.total_tokens'] || attrs['llm.token_count'],
        metadata: {
          ...attrs,
          span_id: span.span_id,
          trace_id: span.trace_id,
        },
      });
      
      // Create edge from parent to this span
      if (span.parent_span_id && spanMap.has(span.parent_span_id)) {
        edges.push({
          source: span.parent_span_id,
          target: span.span_id,
        });
      }
    });
    
    // If no edges were created (flat structure), create a linear flow
    if (edges.length === 0 && nodes.length > 1) {
      for (let i = 0; i < nodes.length - 1; i++) {
        edges.push({
          source: nodes[i].id,
          target: nodes[i + 1].id,
        });
      }
    }
    
    return { nodes, edges, spanMap };
  }, [spans]);

  const handleNodeClick = (node: DAGNode) => {
    const span = spanMap.get(node.id);
    if (span && onSpanClick) {
      onSpanClick(span);
    }
  };

  if (spans.length === 0) {
    return null;
  }

  return (
    <AgentFlowDAG
      nodes={nodes}
      edges={edges}
      onNodeClick={handleNodeClick}
      title="Trace Flow"
      className={className}
    />
  );
}
