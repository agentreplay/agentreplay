/**
 * Task 4 — Request Composer: Preset Sequences
 *
 * Multi-step MCP test sequences with value extraction (JSONPath).
 * Sequence execution is a topological sort over a DAG of dependencies.
 * For linear sequences this is O(n). JSONPath evaluation is O(p × d).
 */

import type { JsonRpcRequest, JsonRpcResponse } from '../protocol/codec';

// ─── JSONPath Extraction ───────────────────────────────────────────────────────

/**
 * Simple JSONPath evaluator for extracting values from responses.
 * Supports: $.field.nested, $.array[0], $.array[*].field
 * Complexity: O(p × d) where p = path segments, d = document depth.
 */
export function extractJsonPath(obj: unknown, path: string): unknown {
  if (!path.startsWith('$')) return undefined;

  const segments = path
    .slice(1)
    .split(/\.|\[/)
    .filter(Boolean)
    .map((s) => s.replace(/\]$/, ''));

  let current: unknown = obj;

  for (const segment of segments) {
    if (current === null || current === undefined) return undefined;

    if (segment === '*' && Array.isArray(current)) {
      // Wildcard — return all items
      return current;
    }

    if (Array.isArray(current)) {
      const idx = parseInt(segment, 10);
      if (!isNaN(idx)) {
        current = current[idx];
      } else {
        // Map over array
        current = current.map((item: Record<string, unknown>) => item?.[segment]).filter((v) => v !== undefined);
      }
    } else if (typeof current === 'object') {
      current = (current as Record<string, unknown>)[segment];
    } else {
      return undefined;
    }
  }

  return current;
}

// ─── Sequence Types ────────────────────────────────────────────────────────────

export interface ValueExtraction {
  /** Variable name to bind */
  variable: string;
  /** JSONPath to extract from the response */
  path: string;
}

export interface SequenceStep {
  id: string;
  method: string;
  params: Record<string, unknown>;
  /** Whether this step is a notification (no response expected) */
  isNotification?: boolean;
  /** Extract values from the response for use in later steps */
  extractions?: ValueExtraction[];
  /** Description for the step */
  description?: string;
  /** Delay before executing this step (ms) */
  delayMs?: number;
}

export interface McpSequence {
  id: string;
  name: string;
  description: string;
  category: 'lifecycle' | 'tools' | 'resources' | 'prompts' | 'custom';
  steps: SequenceStep[];
}

export interface SequenceExecutionResult {
  sequenceId: string;
  startedAt: number;
  completedAt: number;
  totalDurationMs: number;
  steps: StepExecutionResult[];
  variables: Record<string, unknown>;
  success: boolean;
}

export interface StepExecutionResult {
  stepId: string;
  method: string;
  request: JsonRpcRequest;
  response?: JsonRpcResponse;
  durationMs: number;
  error?: string;
  extractedValues?: Record<string, unknown>;
}

// ─── Variable Substitution ─────────────────────────────────────────────────────

/**
 * Replace {{variable}} placeholders in params with values from the variables map.
 */
export function substituteVariables(
  params: Record<string, unknown>,
  variables: Record<string, unknown>
): Record<string, unknown> {
  const result: Record<string, unknown> = {};

  for (const [key, value] of Object.entries(params)) {
    if (typeof value === 'string' && value.startsWith('{{') && value.endsWith('}}')) {
      const varName = value.slice(2, -2).trim();
      result[key] = variables[varName] ?? value;
    } else if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
      result[key] = substituteVariables(value as Record<string, unknown>, variables);
    } else {
      result[key] = value;
    }
  }

  return result;
}

// ─── Preset Sequences ──────────────────────────────────────────────────────────

export const PRESET_SEQUENCES: McpSequence[] = [
  {
    id: 'full-handshake',
    name: 'Full Handshake',
    description: 'Complete MCP initialization: ping → initialize → initialized → tools/list',
    category: 'lifecycle',
    steps: [
      {
        id: 'ping',
        method: 'ping',
        params: {},
        description: 'Verify server connectivity',
      },
      {
        id: 'initialize',
        method: 'initialize',
        params: {
          protocolVersion: '2024-11-05',
          capabilities: { roots: { listChanged: true }, sampling: {} },
          clientInfo: { name: 'MCP Tester', version: '1.0.0' },
        },
        extractions: [
          { variable: 'serverName', path: '$.result.serverInfo.name' },
          { variable: 'serverVersion', path: '$.result.serverInfo.version' },
          { variable: 'protocolVersion', path: '$.result.protocolVersion' },
        ],
        description: 'Initialize the MCP session',
      },
      {
        id: 'initialized',
        method: 'initialized',
        params: {},
        isNotification: true,
        description: 'Confirm initialization complete',
      },
      {
        id: 'tools-list',
        method: 'tools/list',
        params: {},
        extractions: [
          { variable: 'toolCount', path: '$.result.tools.length' },
        ],
        description: 'List all available tools',
      },
    ],
  },
  {
    id: 'search-details',
    name: 'Search → Details',
    description: 'Search for traces then get details of the first result',
    category: 'tools',
    steps: [
      {
        id: 'search',
        method: 'tools/call',
        params: {
          name: 'search_traces',
          arguments: { query: 'error', limit: 5 },
        },
        extractions: [
          { variable: 'firstEdgeId', path: '$.result.content[0].text' },
        ],
        description: 'Search for error traces',
      },
      {
        id: 'details',
        method: 'tools/call',
        params: {
          name: 'get_trace_details',
          arguments: { edge_id: '{{firstEdgeId}}' },
        },
        description: 'Get details of the first result',
        delayMs: 100,
      },
    ],
  },
  {
    id: 'error-analysis',
    name: 'Error Analysis',
    description: 'Search for errors and get context for resolution',
    category: 'tools',
    steps: [
      {
        id: 'search-errors',
        method: 'tools/call',
        params: {
          name: 'search_traces',
          arguments: { query: 'error', limit: 10, span_types: ['Error'] },
        },
        extractions: [
          { variable: 'errorTraces', path: '$.result.content' },
        ],
        description: 'Search for error traces',
      },
      {
        id: 'get-context',
        method: 'tools/call',
        params: {
          name: 'get_context',
          arguments: { query: 'error resolution', context_type: 'error' },
        },
        description: 'Get error resolution context',
      },
      {
        id: 'summary',
        method: 'tools/call',
        params: {
          name: 'get_trace_summary',
          arguments: { time_range: '24h', group_by: 'span_type' },
        },
        description: 'Get summary statistics',
      },
    ],
  },
  {
    id: 'causal-graph',
    name: 'Causal Graph Walk',
    description: 'Get a trace and explore its causal relationships',
    category: 'tools',
    steps: [
      {
        id: 'search',
        method: 'tools/call',
        params: {
          name: 'search_traces',
          arguments: { query: 'LLM call', limit: 1 },
        },
        extractions: [
          { variable: 'edgeId', path: '$.result.content[0].text' },
        ],
        description: 'Find a trace to explore',
      },
      {
        id: 'related',
        method: 'tools/call',
        params: {
          name: 'get_related_traces',
          arguments: { edge_id: '{{edgeId}}', direction: 'both', max_depth: 3 },
        },
        extractions: [
          { variable: 'ancestors', path: '$.result.content[0].text' },
        ],
        description: 'Get related traces (ancestors + descendants)',
      },
      {
        id: 'details',
        method: 'tools/call',
        params: {
          name: 'get_trace_details',
          arguments: { edge_id: '{{edgeId}}' },
        },
        description: 'Get full trace details',
      },
    ],
  },
  {
    id: 'full-discovery',
    name: 'Full Discovery',
    description: 'Discover all server capabilities: resources, tools, and prompts',
    category: 'lifecycle',
    steps: [
      {
        id: 'init',
        method: 'initialize',
        params: {
          protocolVersion: '2024-11-05',
          capabilities: { roots: { listChanged: true }, sampling: {} },
          clientInfo: { name: 'MCP Tester', version: '1.0.0' },
        },
        description: 'Initialize session',
      },
      {
        id: 'initialized',
        method: 'initialized',
        params: {},
        isNotification: true,
        description: 'Confirm initialization',
      },
      {
        id: 'resources',
        method: 'resources/list',
        params: {},
        description: 'List all resources',
      },
      {
        id: 'tools',
        method: 'tools/list',
        params: {},
        description: 'List all tools',
      },
      {
        id: 'prompts',
        method: 'prompts/list',
        params: {},
        description: 'List all prompts',
      },
    ],
  },
];

export function getPresetSequence(id: string): McpSequence | undefined {
  return PRESET_SEQUENCES.find((s) => s.id === id);
}

export function getSequencesByCategory(category: McpSequence['category']): McpSequence[] {
  return PRESET_SEQUENCES.filter((s) => s.category === category);
}
