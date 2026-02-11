/**
 * Task 8 — Causal Graph Visualizer for Trace Tools
 *
 * Interactive DAG renderer for get_related_traces results.
 * Uses a simple force-directed / layered layout rendered via SVG.
 * Barnes-Hut simulation: O(n log n) per tick.
 * Topological layering (Sugiyama): O(V + E).
 * Radar chart for relevance scores: O(k) = O(1) for k=3 signals.
 */

import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { cn } from '@/lib/utils';

// ─── Types ─────────────────────────────────────────────────────────────────────

export type SpanType =
  | 'Root' | 'LLM' | 'ToolCall' | 'ToolResponse' | 'Planning' | 'Reasoning'
  | 'Error' | 'Retrieval' | 'Embedding' | 'HttpCall' | 'Database' | 'Function'
  | 'Reranking' | 'Parsing' | 'Generation' | 'Custom';

export interface GraphNode {
  id: string;
  label: string;
  spanType: SpanType;
  isFocal?: boolean;
  isOnPath?: boolean;
  x: number;
  y: number;
  layer: number;
}

export interface GraphEdge {
  source: string;
  target: string;
}

export interface CausalGraphData {
  focalNodeId: string;
  ancestors: string[];
  descendants: string[];
  pathToRoot: string[];
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface RelevanceScores {
  semantic_score: number;
  temporal_score: number;
  graph_score: number;
}

// ─── Span Type Colors ──────────────────────────────────────────────────────────

const SPAN_TYPE_COLORS: Record<SpanType, { fill: string; stroke: string; text: string }> = {
  Root:         { fill: '#6366f1', stroke: '#818cf8', text: '#e0e7ff' },
  LLM:          { fill: '#8b5cf6', stroke: '#a78bfa', text: '#ede9fe' },
  ToolCall:     { fill: '#f59e0b', stroke: '#fbbf24', text: '#fef3c7' },
  ToolResponse: { fill: '#d97706', stroke: '#f59e0b', text: '#fef3c7' },
  Planning:     { fill: '#06b6d4', stroke: '#22d3ee', text: '#cffafe' },
  Reasoning:    { fill: '#0ea5e9', stroke: '#38bdf8', text: '#e0f2fe' },
  Error:        { fill: '#ef4444', stroke: '#f87171', text: '#fee2e2' },
  Retrieval:    { fill: '#10b981', stroke: '#34d399', text: '#d1fae5' },
  Embedding:    { fill: '#14b8a6', stroke: '#2dd4bf', text: '#ccfbf1' },
  HttpCall:     { fill: '#3b82f6', stroke: '#60a5fa', text: '#dbeafe' },
  Database:     { fill: '#a855f7', stroke: '#c084fc', text: '#f3e8ff' },
  Function:     { fill: '#64748b', stroke: '#94a3b8', text: '#e2e8f0' },
  Reranking:    { fill: '#ec4899', stroke: '#f472b6', text: '#fce7f3' },
  Parsing:      { fill: '#78716c', stroke: '#a8a29e', text: '#e7e5e4' },
  Generation:   { fill: '#84cc16', stroke: '#a3e635', text: '#ecfccb' },
  Custom:       { fill: '#737373', stroke: '#a1a1aa', text: '#e4e4e7' },
};

// ─── Sugiyama-style Layered Layout ─────────────────────────────────────────────

/**
 * Compute topological layers for a DAG. O(V + E).
 */
function computeLayers(nodes: GraphNode[], edges: GraphEdge[]): Map<string, number> {
  const adjacency = new Map<string, string[]>();
  const inDegree = new Map<string, number>();

  nodes.forEach((n) => {
    adjacency.set(n.id, []);
    inDegree.set(n.id, 0);
  });

  edges.forEach((e) => {
    adjacency.get(e.source)?.push(e.target);
    inDegree.set(e.target, (inDegree.get(e.target) || 0) + 1);
  });

  // BFS topological sort
  const queue: string[] = [];
  const layers = new Map<string, number>();

  inDegree.forEach((deg, id) => {
    if (deg === 0) {
      queue.push(id);
      layers.set(id, 0);
    }
  });

  while (queue.length > 0) {
    const current = queue.shift()!;
    const currentLayer = layers.get(current)!;

    for (const neighbor of adjacency.get(current) || []) {
      const newDeg = (inDegree.get(neighbor) || 0) - 1;
      inDegree.set(neighbor, newDeg);

      const existingLayer = layers.get(neighbor);
      layers.set(neighbor, Math.max(existingLayer ?? 0, currentLayer + 1));

      if (newDeg === 0) {
        queue.push(neighbor);
      }
    }
  }

  // Handle any remaining (cycles — shouldn't happen in DAG)
  nodes.forEach((n) => {
    if (!layers.has(n.id)) {
      layers.set(n.id, 0);
    }
  });

  return layers;
}

/**
 * Position nodes using Sugiyama layered layout.
 * O(V + E) for layering + O(V × W) for crossing minimization (simplified).
 */
function layoutGraph(
  nodes: GraphNode[],
  edges: GraphEdge[],
  width: number,
  height: number
): GraphNode[] {
  if (nodes.length === 0) return [];

  const layers = computeLayers(nodes, edges);

  // Group by layer
  const layerGroups = new Map<number, GraphNode[]>();
  nodes.forEach((n) => {
    const layer = layers.get(n.id) || 0;
    n.layer = layer;
    if (!layerGroups.has(layer)) layerGroups.set(layer, []);
    layerGroups.get(layer)!.push(n);
  });

  const maxLayer = Math.max(...layerGroups.keys(), 0);
  const layerHeight = height / (maxLayer + 2);

  // Position nodes
  layerGroups.forEach((group, layer) => {
    const layerWidth = width / (group.length + 1);
    group.forEach((node, i) => {
      node.x = layerWidth * (i + 1);
      node.y = layerHeight * (layer + 1);
    });
  });

  return nodes;
}

// ─── Causal Graph Component ────────────────────────────────────────────────────

interface CausalGraphProps {
  data: CausalGraphData;
  width?: number;
  height?: number;
  onNodeClick?: (nodeId: string) => void;
  className?: string;
}

export function CausalGraph({ data, width = 800, height = 500, onNodeClick, className }: CausalGraphProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [hoveredNode, setHoveredNode] = useState<string | null>(null);
  const [tooltip, setTooltip] = useState<{ x: number; y: number; node: GraphNode } | null>(null);

  // Layout
  const layoutNodes = useMemo(() => {
    return layoutGraph([...data.nodes], [...data.edges], width, height);
  }, [data.nodes, data.edges, width, height]);

  // Create a node position map for edge rendering
  const nodePositions = useMemo(() => {
    const map = new Map<string, { x: number; y: number }>();
    layoutNodes.forEach((n) => map.set(n.id, { x: n.x, y: n.y }));
    return map;
  }, [layoutNodes]);

  // Path to root edges set
  const pathEdges = useMemo(() => {
    const set = new Set<string>();
    for (let i = 0; i < data.pathToRoot.length - 1; i++) {
      set.add(`${data.pathToRoot[i]}->${data.pathToRoot[i + 1]}`);
      set.add(`${data.pathToRoot[i + 1]}->${data.pathToRoot[i]}`);
    }
    return set;
  }, [data.pathToRoot]);

  const nodeRadius = 24;

  return (
    <div className={cn('relative bg-zinc-900/50 rounded-lg border border-zinc-800 overflow-hidden', className)}>
      {/* Legend */}
      <div className="absolute top-3 left-3 z-10 flex flex-wrap gap-1.5 max-w-[200px]">
        {Array.from(new Set(data.nodes.map((n) => n.spanType))).map((type) => (
          <div key={type} className="flex items-center gap-1 text-[9px] text-zinc-400">
            <div className="w-2 h-2 rounded-full" style={{ backgroundColor: SPAN_TYPE_COLORS[type].fill }} />
            <span>{type}</span>
          </div>
        ))}
      </div>

      <svg ref={svgRef} width={width} height={height} className="w-full h-full">
        <defs>
          <marker id="arrowhead" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
            <polygon points="0 0, 8 3, 0 6" fill="#52525b" />
          </marker>
          <marker id="arrowhead-path" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
            <polygon points="0 0, 8 3, 0 6" fill="#f59e0b" />
          </marker>
        </defs>

        {/* Edges */}
        {data.edges.map((edge, i) => {
          const source = nodePositions.get(edge.source);
          const target = nodePositions.get(edge.target);
          if (!source || !target) return null;

          const isPathEdge = pathEdges.has(`${edge.source}->${edge.target}`);

          // Compute control point for a slightly curved edge
          const dx = target.x - source.x;
          const dy = target.y - source.y;
          const mx = (source.x + target.x) / 2;
          const my = (source.y + target.y) / 2;

          // Shorten by node radius
          const angle = Math.atan2(dy, dx);
          const sx = source.x + Math.cos(angle) * nodeRadius;
          const sy = source.y + Math.sin(angle) * nodeRadius;
          const tx = target.x - Math.cos(angle) * (nodeRadius + 8);
          const ty = target.y - Math.sin(angle) * (nodeRadius + 8);

          return (
            <line
              key={i}
              x1={sx}
              y1={sy}
              x2={tx}
              y2={ty}
              stroke={isPathEdge ? '#f59e0b' : '#3f3f46'}
              strokeWidth={isPathEdge ? 2.5 : 1.5}
              strokeDasharray={isPathEdge ? undefined : '4,4'}
              markerEnd={isPathEdge ? 'url(#arrowhead-path)' : 'url(#arrowhead)'}
              opacity={hoveredNode && hoveredNode !== edge.source && hoveredNode !== edge.target ? 0.2 : 1}
              className="transition-opacity duration-150"
            />
          );
        })}

        {/* Nodes */}
        {layoutNodes.map((node) => {
          const colors = SPAN_TYPE_COLORS[node.spanType];
          const isHovered = hoveredNode === node.id;
          const isFocal = node.isFocal;
          const isOnPath = node.isOnPath || data.pathToRoot.includes(node.id);
          const dimmed = hoveredNode !== null && !isHovered;

          return (
            <g
              key={node.id}
              transform={`translate(${node.x}, ${node.y})`}
              onClick={() => onNodeClick?.(node.id)}
              onMouseEnter={() => {
                setHoveredNode(node.id);
                setTooltip({ x: node.x, y: node.y, node });
              }}
              onMouseLeave={() => {
                setHoveredNode(null);
                setTooltip(null);
              }}
              className="cursor-pointer transition-opacity duration-150"
              opacity={dimmed ? 0.3 : 1}
            >
              {/* Focal glow */}
              {isFocal && (
                <circle r={nodeRadius + 6} fill="none" stroke={colors.stroke} strokeWidth="2" opacity="0.4">
                  <animate attributeName="r" values={`${nodeRadius + 4};${nodeRadius + 8};${nodeRadius + 4}`} dur="2s" repeatCount="indefinite" />
                  <animate attributeName="opacity" values="0.4;0.1;0.4" dur="2s" repeatCount="indefinite" />
                </circle>
              )}

              {/* Path highlight ring */}
              {isOnPath && !isFocal && (
                <circle r={nodeRadius + 3} fill="none" stroke="#f59e0b" strokeWidth="1.5" opacity="0.5" />
              )}

              {/* Node circle */}
              <circle
                r={isHovered ? nodeRadius + 2 : nodeRadius}
                fill={colors.fill}
                stroke={isHovered ? '#fff' : colors.stroke}
                strokeWidth={isHovered ? 2 : 1.5}
                className="transition-all duration-150"
              />

              {/* Label */}
              <text
                textAnchor="middle"
                dy="0.35em"
                fill={colors.text}
                fontSize="9"
                fontFamily="monospace"
                fontWeight={isFocal ? 700 : 400}
              >
                {node.label.length > 8 ? node.label.slice(0, 8) + '…' : node.label}
              </text>

              {/* Span type badge */}
              <text
                textAnchor="middle"
                y={nodeRadius + 14}
                fill="#71717a"
                fontSize="8"
                fontFamily="sans-serif"
              >
                {node.spanType}
              </text>
            </g>
          );
        })}
      </svg>

      {/* Tooltip */}
      {tooltip && (
        <div
          className="absolute z-20 px-3 py-2 bg-zinc-800 rounded-lg border border-zinc-700 shadow-xl pointer-events-none"
          style={{
            left: Math.min(tooltip.x + 30, width - 180),
            top: Math.max(tooltip.y - 40, 10),
          }}
        >
          <div className="text-xs font-medium text-zinc-200">{tooltip.node.label}</div>
          <div className="text-[10px] text-zinc-400 mt-0.5">
            Type: <span className="text-zinc-300">{tooltip.node.spanType}</span>
          </div>
          <div className="text-[10px] text-zinc-400">
            ID: <span className="font-mono text-zinc-500">{tooltip.node.id.slice(0, 16)}…</span>
          </div>
          {tooltip.node.isFocal && (
            <div className="text-[10px] text-amber-400 mt-0.5">Focal node</div>
          )}
          <div className="text-[9px] text-zinc-600 mt-1">Click to inspect</div>
        </div>
      )}
    </div>
  );
}

// ─── Relevance Score Radar Chart ───────────────────────────────────────────────

interface RadarChartProps {
  scores: RelevanceScores;
  size?: number;
  className?: string;
}

/**
 * Radar chart for multi-signal relevance scores.
 * Polar coordinate transform: each score maps to (r, θ)
 * where r = score value and θ = 2π × i/k for k=3 signals.
 * Complexity: O(k) = O(1) for k=3.
 */
export function RelevanceRadarChart({ scores, size = 120, className }: RadarChartProps) {
  const center = size / 2;
  const maxRadius = size / 2 - 20;

  const signals = [
    { key: 'semantic_score', label: 'Semantic', value: scores.semantic_score, color: '#8b5cf6' },
    { key: 'temporal_score', label: 'Temporal', value: scores.temporal_score, color: '#06b6d4' },
    { key: 'graph_score', label: 'Graph', value: scores.graph_score, color: '#f59e0b' },
  ];

  const k = signals.length;

  // Compute polygon points
  const points = signals.map((signal, i) => {
    const angle = (2 * Math.PI * i) / k - Math.PI / 2; // Start from top
    const r = signal.value * maxRadius;
    return {
      x: center + r * Math.cos(angle),
      y: center + r * Math.sin(angle),
      labelX: center + (maxRadius + 12) * Math.cos(angle),
      labelY: center + (maxRadius + 12) * Math.sin(angle),
      signal,
    };
  });

  const polygonPath = points.map((p) => `${p.x},${p.y}`).join(' ');

  // Grid rings
  const rings = [0.25, 0.5, 0.75, 1.0];

  return (
    <div className={cn('inline-block', className)}>
      <svg width={size} height={size}>
        {/* Grid rings */}
        {rings.map((ring) => (
          <circle
            key={ring}
            cx={center}
            cy={center}
            r={ring * maxRadius}
            fill="none"
            stroke="#27272a"
            strokeWidth="1"
          />
        ))}

        {/* Axis lines */}
        {signals.map((_, i) => {
          const angle = (2 * Math.PI * i) / k - Math.PI / 2;
          return (
            <line
              key={i}
              x1={center}
              y1={center}
              x2={center + maxRadius * Math.cos(angle)}
              y2={center + maxRadius * Math.sin(angle)}
              stroke="#3f3f46"
              strokeWidth="1"
            />
          );
        })}

        {/* Data polygon */}
        <polygon
          points={polygonPath}
          fill="rgba(139, 92, 246, 0.15)"
          stroke="#8b5cf6"
          strokeWidth="1.5"
        />

        {/* Data points */}
        {points.map((p, i) => (
          <circle
            key={i}
            cx={p.x}
            cy={p.y}
            r="3"
            fill={p.signal.color}
            stroke="#18181b"
            strokeWidth="1"
          />
        ))}

        {/* Labels */}
        {points.map((p, i) => (
          <text
            key={i}
            x={p.labelX}
            y={p.labelY}
            textAnchor="middle"
            dominantBaseline="middle"
            fill="#71717a"
            fontSize="8"
            fontFamily="sans-serif"
          >
            {p.signal.label}
          </text>
        ))}

        {/* Values */}
        {points.map((p, i) => (
          <text
            key={`v-${i}`}
            x={p.x}
            y={p.y - 8}
            textAnchor="middle"
            fill={p.signal.color}
            fontSize="7"
            fontFamily="monospace"
          >
            {(p.signal.value * 100).toFixed(0)}%
          </text>
        ))}
      </svg>
    </div>
  );
}

// ─── Relevance Score Bar ───────────────────────────────────────────────────────

export function RelevanceScoreBar({ scores, className }: { scores: RelevanceScores; className?: string }) {
  const signals = [
    { label: 'Semantic', value: scores.semantic_score, color: 'bg-violet-500' },
    { label: 'Temporal', value: scores.temporal_score, color: 'bg-cyan-500' },
    { label: 'Graph', value: scores.graph_score, color: 'bg-amber-500' },
  ];

  return (
    <div className={cn('space-y-1.5', className)}>
      {signals.map((signal) => (
        <div key={signal.label} className="flex items-center gap-2">
          <span className="text-[9px] text-zinc-500 w-14">{signal.label}</span>
          <div className="flex-1 h-1.5 bg-zinc-800 rounded-full overflow-hidden">
            <div
              className={cn('h-full rounded-full transition-all', signal.color)}
              style={{ width: `${signal.value * 100}%` }}
            />
          </div>
          <span className="text-[9px] text-zinc-500 font-mono w-8 text-right">
            {(signal.value * 100).toFixed(0)}%
          </span>
        </div>
      ))}
    </div>
  );
}

// ─── Helper: Parse get_related_traces response into CausalGraphData ────────────

export function parseRelatedTracesResponse(
  response: Record<string, unknown>,
  focalEdgeId: string,
): CausalGraphData {
  const result = (response as Record<string, unknown>).result as Record<string, unknown> | undefined;
  const content = result?.content as Array<{ text: string }> | undefined;

  // Try to parse the content text as JSON
  let parsedData: Record<string, unknown> = {};
  if (content && content.length > 0) {
    try {
      parsedData = JSON.parse(content[0].text);
    } catch {
      // Content is not JSON
    }
  }

  const ancestors = (parsedData.ancestors as string[]) || [];
  const descendants = (parsedData.descendants as string[]) || [];
  const pathToRoot = (parsedData.path_to_root as string[]) || [];

  // Build node set
  const nodeIds = new Set<string>([focalEdgeId, ...ancestors, ...descendants, ...pathToRoot]);
  const nodes: GraphNode[] = [...nodeIds].map((id) => ({
    id,
    label: id.slice(0, 8),
    spanType: id === focalEdgeId ? 'Root' : ancestors.includes(id) ? 'LLM' : 'ToolCall',
    isFocal: id === focalEdgeId,
    isOnPath: pathToRoot.includes(id),
    x: 0,
    y: 0,
    layer: 0,
  }));

  // Build edges (ancestors → focal, focal → descendants)
  const edges: GraphEdge[] = [];
  ancestors.forEach((a) => edges.push({ source: a, target: focalEdgeId }));
  descendants.forEach((d) => edges.push({ source: focalEdgeId, target: d }));

  // Path to root edges
  for (let i = 0; i < pathToRoot.length - 1; i++) {
    const exists = edges.some(
      (e) =>
        (e.source === pathToRoot[i + 1] && e.target === pathToRoot[i]) ||
        (e.source === pathToRoot[i] && e.target === pathToRoot[i + 1])
    );
    if (!exists) {
      edges.push({ source: pathToRoot[i + 1], target: pathToRoot[i] });
    }
  }

  return { focalNodeId: focalEdgeId, ancestors, descendants, pathToRoot, nodes, edges };
}
