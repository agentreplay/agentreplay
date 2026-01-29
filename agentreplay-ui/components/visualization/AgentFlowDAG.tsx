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

import { useMemo, useState, useRef, useCallback, useEffect } from 'react';
import { 
  ZoomIn, 
  ZoomOut, 
  Maximize2, 
  Move,
  Bot,
  Cpu,
  Database,
  Globe,
  Server,
  Wrench,
  MessageSquare,
  ArrowRight,
  Play,
} from 'lucide-react';
import { cn } from '../../lib/utils';

export interface DAGNode {
  id: string;
  type: 'agent' | 'llm' | 'tool' | 'service' | 'database' | 'external' | 'input' | 'output';
  label: string;
  status?: 'active' | 'completed' | 'error' | 'pending';
  metadata?: Record<string, any>;
  // Positioning (optional - will be auto-calculated if not provided)
  x?: number;
  y?: number;
  // Metrics
  calls?: number;
  duration?: number;
  tokens?: number;
  cost?: number;
}

export interface DAGEdge {
  source: string;
  target: string;
  label?: string;
  value?: number;
  animated?: boolean;
}

interface AgentFlowDAGProps {
  nodes: DAGNode[];
  edges: DAGEdge[];
  onNodeClick?: (node: DAGNode) => void;
  onEdgeClick?: (edge: DAGEdge) => void;
  title?: string;
  showMinimap?: boolean;
  interactive?: boolean;
  className?: string;
}

// Auto-layout using layered approach
function calculateLayout(nodes: DAGNode[], edges: DAGEdge[]): Map<string, { x: number; y: number }> {
  const positions = new Map<string, { x: number; y: number }>();
  
  // Build adjacency list
  const incoming = new Map<string, string[]>();
  const outgoing = new Map<string, string[]>();
  
  nodes.forEach(n => {
    incoming.set(n.id, []);
    outgoing.set(n.id, []);
  });
  
  edges.forEach(e => {
    const inList = incoming.get(e.target);
    if (inList) inList.push(e.source);
    const outList = outgoing.get(e.source);
    if (outList) outList.push(e.target);
  });
  
  // Find root nodes (no incoming edges)
  const roots = nodes.filter(n => (incoming.get(n.id)?.length || 0) === 0);
  
  // BFS to assign layers
  const layers = new Map<string, number>();
  const visited = new Set<string>();
  const queue: { id: string; layer: number }[] = roots.map(r => ({ id: r.id, layer: 0 }));
  
  while (queue.length > 0) {
    const { id, layer } = queue.shift()!;
    if (visited.has(id)) continue;
    visited.add(id);
    layers.set(id, Math.max(layers.get(id) || 0, layer));
    
    const children = outgoing.get(id) || [];
    children.forEach(child => {
      if (!visited.has(child)) {
        queue.push({ id: child, layer: layer + 1 });
      }
    });
  }
  
  // Handle disconnected nodes
  nodes.forEach(n => {
    if (!layers.has(n.id)) {
      layers.set(n.id, 0);
    }
  });
  
  // Group nodes by layer
  const nodesByLayer = new Map<number, string[]>();
  layers.forEach((layer, id) => {
    if (!nodesByLayer.has(layer)) nodesByLayer.set(layer, []);
    nodesByLayer.get(layer)!.push(id);
  });
  
  // Calculate positions
  const layerCount = Math.max(...Array.from(layers.values())) + 1;
  const horizontalSpacing = 200;
  const verticalSpacing = 120;
  const startX = 100;
  const centerY = 250;
  
  nodesByLayer.forEach((nodeIds, layer) => {
    const x = startX + layer * horizontalSpacing;
    const totalHeight = (nodeIds.length - 1) * verticalSpacing;
    const startY = centerY - totalHeight / 2;
    
    nodeIds.forEach((id, index) => {
      positions.set(id, {
        x,
        y: startY + index * verticalSpacing,
      });
    });
  });
  
  return positions;
}

const nodeColors: Record<DAGNode['type'], { bg: string; border: string; icon: string }> = {
  agent: { bg: 'bg-blue-500/20', border: 'border-blue-500', icon: 'text-blue-500' },
  llm: { bg: 'bg-purple-500/20', border: 'border-purple-500', icon: 'text-purple-500' },
  tool: { bg: 'bg-orange-500/20', border: 'border-orange-500', icon: 'text-orange-500' },
  service: { bg: 'bg-green-500/20', border: 'border-green-500', icon: 'text-green-500' },
  database: { bg: 'bg-cyan-500/20', border: 'border-cyan-500', icon: 'text-cyan-500' },
  external: { bg: 'bg-yellow-500/20', border: 'border-yellow-500', icon: 'text-yellow-500' },
  input: { bg: 'bg-emerald-500/20', border: 'border-emerald-500', icon: 'text-emerald-500' },
  output: { bg: 'bg-rose-500/20', border: 'border-rose-500', icon: 'text-rose-500' },
};

const NodeIcon = ({ type }: { type: DAGNode['type'] }) => {
  const icons: Record<string, React.ReactNode> = {
    agent: <Bot className="w-4 h-4" />,
    llm: <Cpu className="w-4 h-4" />,
    tool: <Wrench className="w-4 h-4" />,
    service: <Server className="w-4 h-4" />,
    database: <Database className="w-4 h-4" />,
    external: <Globe className="w-4 h-4" />,
    input: <Play className="w-4 h-4" />,
    output: <ArrowRight className="w-4 h-4" />,
  };
  return <>{icons[type] || <MessageSquare className="w-4 h-4" />}</>;
};

export function AgentFlowDAG({
  nodes,
  edges,
  onNodeClick,
  onEdgeClick,
  title = 'Agent Flow',
  showMinimap = true,
  interactive = true,
  className,
}: AgentFlowDAGProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const [selectedNode, setSelectedNode] = useState<string | null>(null);
  const [hoveredNode, setHoveredNode] = useState<string | null>(null);

  // Auto-calculate positions if not provided
  const positions = useMemo(() => {
    const autoPositions = calculateLayout(nodes, edges);
    const result = new Map<string, { x: number; y: number }>();
    
    nodes.forEach(node => {
      if (node.x !== undefined && node.y !== undefined) {
        result.set(node.id, { x: node.x, y: node.y });
      } else {
        result.set(node.id, autoPositions.get(node.id) || { x: 100, y: 100 });
      }
    });
    
    return result;
  }, [nodes, edges]);

  // Canvas dimensions
  const bounds = useMemo(() => {
    let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
    positions.forEach(({ x, y }) => {
      minX = Math.min(minX, x);
      maxX = Math.max(maxX, x);
      minY = Math.min(minY, y);
      maxY = Math.max(maxY, y);
    });
    return {
      width: Math.max(maxX - minX + 300, 600),
      height: Math.max(maxY - minY + 200, 400),
      offsetX: -minX + 100,
      offsetY: -minY + 50,
    };
  }, [positions]);

  // Zoom handlers
  const handleZoomIn = () => setZoom(z => Math.min(z * 1.2, 3));
  const handleZoomOut = () => setZoom(z => Math.max(z / 1.2, 0.3));
  const handleZoomReset = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  };

  // Pan handlers
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (!interactive) return;
    setIsDragging(true);
    setDragStart({ x: e.clientX - pan.x, y: e.clientY - pan.y });
  }, [interactive, pan]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!isDragging) return;
    setPan({
      x: e.clientX - dragStart.x,
      y: e.clientY - dragStart.y,
    });
  }, [isDragging, dragStart]);

  const handleMouseUp = () => setIsDragging(false);

  // Wheel zoom
  const handleWheel = useCallback((e: React.WheelEvent) => {
    if (!interactive) return;
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      setZoom(z => {
        const delta = e.deltaY > 0 ? 0.9 : 1.1;
        return Math.min(Math.max(z * delta, 0.3), 3);
      });
    }
  }, [interactive]);

  const handleNodeClick = (node: DAGNode) => {
    setSelectedNode(node.id);
    onNodeClick?.(node);
  };

  // Get edge path between two nodes
  const getEdgePath = (sourceId: string, targetId: string) => {
    const source = positions.get(sourceId);
    const target = positions.get(targetId);
    if (!source || !target) return '';

    const sx = source.x + bounds.offsetX + 60; // Right side of source node
    const sy = source.y + bounds.offsetY + 25;
    const tx = target.x + bounds.offsetX - 10; // Left side of target node
    const ty = target.y + bounds.offsetY + 25;

    // Bezier curve for smooth edge
    const midX = (sx + tx) / 2;
    return `M ${sx} ${sy} C ${midX} ${sy}, ${midX} ${ty}, ${tx} ${ty}`;
  };

  return (
    <div className={cn("bg-surface rounded-xl border border-border overflow-hidden", className)}>
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 bg-surface-elevated border-b border-border">
        <div className="flex items-center gap-2">
          <Bot className="w-4 h-4 text-primary" />
          <span className="text-sm font-medium text-textPrimary">{title}</span>
          <span className="text-xs text-textTertiary">
            {nodes.length} nodes, {edges.length} edges
          </span>
        </div>

        {interactive && (
          <div className="flex items-center gap-1 bg-background rounded-lg p-1">
            <button
              onClick={handleZoomOut}
              className="p-1.5 rounded hover:bg-surface transition-colors text-textSecondary hover:text-textPrimary"
              title="Zoom out"
            >
              <ZoomOut className="w-4 h-4" />
            </button>
            <span className="px-2 text-xs text-textTertiary font-mono min-w-[48px] text-center">
              {Math.round(zoom * 100)}%
            </span>
            <button
              onClick={handleZoomIn}
              className="p-1.5 rounded hover:bg-surface transition-colors text-textSecondary hover:text-textPrimary"
              title="Zoom in"
            >
              <ZoomIn className="w-4 h-4" />
            </button>
            <div className="w-px h-4 bg-border mx-1" />
            <button
              onClick={handleZoomReset}
              className="p-1.5 rounded hover:bg-surface transition-colors text-textSecondary hover:text-textPrimary"
              title="Reset view"
            >
              <Maximize2 className="w-4 h-4" />
            </button>
          </div>
        )}
      </div>

      {/* Canvas */}
      <div
        ref={containerRef}
        className={cn(
          "relative overflow-hidden",
          isDragging ? "cursor-grabbing" : interactive ? "cursor-grab" : ""
        )}
        style={{ height: '400px' }}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onWheel={handleWheel}
      >
        <svg
          width="100%"
          height="100%"
          viewBox={`0 0 ${bounds.width} ${bounds.height}`}
          style={{
            transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
            transformOrigin: 'center center',
            transition: isDragging ? 'none' : 'transform 0.1s ease-out',
          }}
        >
          {/* Definitions for markers and gradients */}
          <defs>
            <marker
              id="arrowhead"
              markerWidth="10"
              markerHeight="7"
              refX="9"
              refY="3.5"
              orient="auto"
            >
              <polygon
                points="0 0, 10 3.5, 0 7"
                className="fill-textTertiary"
              />
            </marker>
            <marker
              id="arrowhead-active"
              markerWidth="10"
              markerHeight="7"
              refX="9"
              refY="3.5"
              orient="auto"
            >
              <polygon
                points="0 0, 10 3.5, 0 7"
                className="fill-primary"
              />
            </marker>
          </defs>

          {/* Grid pattern */}
          <pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse">
            <path d="M 40 0 L 0 0 0 40" fill="none" stroke="currentColor" strokeWidth="0.5" className="text-border" />
          </pattern>
          <rect width="100%" height="100%" fill="url(#grid)" opacity="0.3" />

          {/* Edges */}
          {edges.map((edge, idx) => {
            const isHighlighted = hoveredNode === edge.source || hoveredNode === edge.target;
            const path = getEdgePath(edge.source, edge.target);
            
            return (
              <g key={`edge-${idx}`}>
                <path
                  d={path}
                  fill="none"
                  stroke={isHighlighted ? 'var(--primary)' : 'var(--border)'}
                  strokeWidth={isHighlighted ? 2 : 1.5}
                  strokeOpacity={isHighlighted ? 1 : 0.6}
                  markerEnd={isHighlighted ? 'url(#arrowhead-active)' : 'url(#arrowhead)'}
                  className={cn(
                    "transition-all",
                    edge.animated && "animate-dash"
                  )}
                  onClick={() => onEdgeClick?.(edge)}
                  style={{ cursor: onEdgeClick ? 'pointer' : 'default' }}
                />
                {edge.label && (
                  <text
                    x={(positions.get(edge.source)!.x + positions.get(edge.target)!.x) / 2 + bounds.offsetX + 30}
                    y={(positions.get(edge.source)!.y + positions.get(edge.target)!.y) / 2 + bounds.offsetY + 15}
                    className="fill-textTertiary text-[10px]"
                    textAnchor="middle"
                  >
                    {edge.label}
                  </text>
                )}
              </g>
            );
          })}

          {/* Nodes */}
          {nodes.map(node => {
            const pos = positions.get(node.id);
            if (!pos) return null;
            
            const colors = nodeColors[node.type];
            const isSelected = selectedNode === node.id;
            const isHovered = hoveredNode === node.id;
            
            return (
              <g
                key={node.id}
                transform={`translate(${pos.x + bounds.offsetX}, ${pos.y + bounds.offsetY})`}
                className="cursor-pointer"
                onClick={() => handleNodeClick(node)}
                onMouseEnter={() => setHoveredNode(node.id)}
                onMouseLeave={() => setHoveredNode(null)}
              >
                {/* Node background */}
                <rect
                  x="-10"
                  y="0"
                  width="120"
                  height="50"
                  rx="8"
                  className={cn(
                    "transition-all",
                    colors.bg,
                    isSelected || isHovered ? colors.border : 'border-transparent',
                    isSelected && 'stroke-2'
                  )}
                  stroke={isSelected || isHovered ? 'var(--primary)' : 'var(--border)'}
                  strokeWidth={isSelected ? 2 : 1}
                />
                
                {/* Status indicator */}
                {node.status && (
                  <circle
                    cx="100"
                    cy="10"
                    r="4"
                    className={cn(
                      node.status === 'completed' && 'fill-green-500',
                      node.status === 'active' && 'fill-blue-500 animate-pulse',
                      node.status === 'error' && 'fill-red-500',
                      node.status === 'pending' && 'fill-gray-400'
                    )}
                  />
                )}
                
                {/* Icon */}
                <foreignObject x="0" y="10" width="24" height="24">
                  <div className={cn("flex items-center justify-center", colors.icon)}>
                    <NodeIcon type={node.type} />
                  </div>
                </foreignObject>
                
                {/* Label */}
                <text
                  x="28"
                  y="22"
                  className="fill-textPrimary text-xs font-medium"
                >
                  {node.label.length > 12 ? node.label.slice(0, 12) + '…' : node.label}
                </text>
                
                {/* Metrics */}
                <text
                  x="28"
                  y="38"
                  className="fill-textTertiary text-[10px]"
                >
                  {node.calls ? `${node.calls} calls` : ''}
                  {node.duration ? ` · ${node.duration}ms` : ''}
                </text>
              </g>
            );
          })}
        </svg>

        {/* Empty state */}
        {nodes.length === 0 && (
          <div className="absolute inset-0 flex items-center justify-center text-textTertiary">
            <div className="text-center">
              <Bot className="w-12 h-12 mx-auto mb-2 opacity-50" />
              <p className="text-sm">No agent flow data available</p>
            </div>
          </div>
        )}
      </div>

      {/* Legend */}
      <div className="px-4 py-2 bg-surface-elevated border-t border-border flex items-center gap-4 text-xs text-textTertiary overflow-x-auto">
        {Object.entries(nodeColors).slice(0, 5).map(([type, colors]) => (
          <div key={type} className="flex items-center gap-1.5 flex-shrink-0">
            <div className={cn("w-3 h-3 rounded", colors.bg, 'border', colors.border)} />
            <span className="capitalize">{type}</span>
          </div>
        ))}
      </div>

      {/* Selected node details */}
      {selectedNode && (
        <div className="absolute bottom-16 right-4 p-3 bg-surface rounded-lg border border-border shadow-lg max-w-xs">
          {(() => {
            const node = nodes.find(n => n.id === selectedNode);
            if (!node) return null;
            return (
              <div>
                <div className="flex items-center gap-2 mb-2">
                  <div className={cn(nodeColors[node.type].icon)}>
                    <NodeIcon type={node.type} />
                  </div>
                  <span className="font-medium text-textPrimary">{node.label}</span>
                </div>
                <div className="text-xs text-textSecondary space-y-1">
                  <p>Type: {node.type}</p>
                  {node.calls && <p>Calls: {node.calls}</p>}
                  {node.duration && <p>Avg Duration: {node.duration}ms</p>}
                  {node.tokens && <p>Tokens: {node.tokens.toLocaleString()}</p>}
                  {node.cost !== undefined && <p>Cost: ${node.cost.toFixed(4)}</p>}
                </div>
              </div>
            );
          })()}
        </div>
      )}
    </div>
  );
}
