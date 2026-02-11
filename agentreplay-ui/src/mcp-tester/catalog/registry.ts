/**
 * Task 3 — Method Catalog & Schema Registry
 *
 * Exhaustive MCP Method Catalog with JSON Schema Validation.
 * Registry keyed by method name. Schema validation is O(d × k) → O(1) for
 * fixed schemas. Autocomplete for enum fields uses static sets — O(1).
 */

// ─── Schema Types ──────────────────────────────────────────────────────────────

export interface JsonSchema {
  type: string;
  properties?: Record<string, JsonSchemaProperty>;
  required?: string[];
  description?: string;
}

export interface JsonSchemaProperty {
  type: string;
  description?: string;
  default?: unknown;
  enum?: string[];
  items?: JsonSchemaProperty;
  properties?: Record<string, JsonSchemaProperty>;
  required?: string[];
  minimum?: number;
  maximum?: number;
}

export type MethodCategory = 'lifecycle' | 'resources' | 'tools' | 'prompts';

export interface McpMethodDefinition {
  method: string;
  category: MethodCategory;
  description: string;
  isNotification?: boolean;
  paramsSchema?: JsonSchema;
  resultSchema?: JsonSchema;
  defaultParams?: Record<string, unknown>;
}

// ─── Category Colors (matches the spec) ────────────────────────────────────────

export const CATEGORY_COLORS: Record<MethodCategory, {
  bg: string;
  text: string;
  dot: string;
  border: string;
}> = {
  lifecycle: { bg: 'bg-violet-500/15', text: 'text-violet-300', dot: 'bg-violet-400', border: 'border-violet-500/30' },
  resources: { bg: 'bg-sky-500/15', text: 'text-sky-300', dot: 'bg-sky-400', border: 'border-sky-500/30' },
  tools: { bg: 'bg-amber-500/15', text: 'text-amber-300', dot: 'bg-amber-400', border: 'border-amber-500/30' },
  prompts: { bg: 'bg-emerald-500/15', text: 'text-emerald-300', dot: 'bg-emerald-400', border: 'border-emerald-500/30' },
};

// ─── Enum Value Sets (for autocomplete) ────────────────────────────────────────

export const SPAN_TYPES = [
  'Root', 'LLM', 'ToolCall', 'ToolResponse', 'Planning', 'Reasoning',
  'Error', 'Retrieval', 'Embedding', 'HttpCall', 'Database', 'Function',
  'Reranking', 'Parsing', 'Generation', 'Custom',
] as const;

export const CONTEXT_TYPES = ['error', 'debug', 'trace', 'general'] as const;
export const DIRECTIONS = ['ancestors', 'descendants', 'both'] as const;
export const TIME_RANGES = ['1h', '6h', '24h', '7d', '30d', 'all'] as const;
export const GROUP_BY_OPTIONS = ['operation', 'span_type', 'model', 'agent'] as const;

// ─── Tool Input Schemas ────────────────────────────────────────────────────────

const searchTracesSchema: JsonSchema = {
  type: 'object',
  properties: {
    query: { type: 'string', description: 'Semantic search query for traces' },
    limit: { type: 'number', description: 'Maximum number of results', default: 10, minimum: 1, maximum: 100 },
    start_ts: { type: 'number', description: 'Start timestamp in microseconds (optional)' },
    end_ts: { type: 'number', description: 'End timestamp in microseconds (optional)' },
    span_types: {
      type: 'array',
      description: 'Filter by span types',
      items: { type: 'string', enum: [...SPAN_TYPES] },
    },
    include_payload: { type: 'boolean', description: 'Include full payload in results', default: false },
    include_related: { type: 'boolean', description: 'Include related traces', default: false },
  },
  required: ['query'],
};

const getContextSchema: JsonSchema = {
  type: 'object',
  properties: {
    query: { type: 'string', description: 'Context query for error resolution' },
    context_type: { type: 'string', description: 'Type of context', enum: [...CONTEXT_TYPES] },
    limit: { type: 'number', description: 'Maximum number of context items', default: 5 },
  },
  required: ['query'],
};

const getTraceDetailsSchema: JsonSchema = {
  type: 'object',
  properties: {
    edge_id: { type: 'string', description: 'The edge ID of the trace to retrieve' },
  },
  required: ['edge_id'],
};

const getRelatedTracesSchema: JsonSchema = {
  type: 'object',
  properties: {
    edge_id: { type: 'string', description: 'The edge ID to find related traces for' },
    direction: { type: 'string', description: 'Direction to traverse', enum: [...DIRECTIONS] },
    max_depth: { type: 'number', description: 'Maximum traversal depth', default: 3 },
  },
  required: ['edge_id'],
};

const getTraceSummarySchema: JsonSchema = {
  type: 'object',
  properties: {
    time_range: { type: 'string', description: 'Time range for summary', enum: [...TIME_RANGES] },
    group_by: { type: 'string', description: 'Group results by field', enum: [...GROUP_BY_OPTIONS] },
  },
};

const saveMemorySchema: JsonSchema = {
  type: 'object',
  properties: {
    content: { type: 'string', description: 'Content to save as memory/observation' },
    category: { type: 'string', description: 'Memory category' },
    tags: { type: 'array', description: 'Tags for the memory', items: { type: 'string' } },
  },
  required: ['content'],
};

// ─── Method Registry ───────────────────────────────────────────────────────────

export const MCP_METHODS: McpMethodDefinition[] = [
  // ── Lifecycle ──
  {
    method: 'ping',
    category: 'lifecycle',
    description: 'Ping the MCP server to verify connectivity.',
    defaultParams: {},
  },
  {
    method: 'initialize',
    category: 'lifecycle',
    description: 'Initialize the MCP session. Must be called before any other method.',
    paramsSchema: {
      type: 'object',
      properties: {
        protocolVersion: { type: 'string', description: 'MCP protocol version', default: '2024-11-05' },
        capabilities: {
          type: 'object',
          description: 'Client capabilities',
          properties: {
            roots: { type: 'object', properties: { listChanged: { type: 'boolean' } } },
            sampling: { type: 'object', properties: {} },
          },
        },
        clientInfo: {
          type: 'object',
          description: 'Client identification',
          properties: {
            name: { type: 'string', description: 'Client name' },
            version: { type: 'string', description: 'Client version' },
          },
          required: ['name', 'version'],
        },
      },
      required: ['protocolVersion', 'capabilities', 'clientInfo'],
    },
    defaultParams: {
      protocolVersion: '2024-11-05',
      capabilities: { roots: { listChanged: true }, sampling: {} },
      clientInfo: { name: 'MCP Tester', version: '1.0.0' },
    },
  },
  {
    method: 'initialized',
    category: 'lifecycle',
    description: 'Notify the server that initialization is complete. Sent as a notification (no id).',
    isNotification: true,
    defaultParams: {},
  },

  // ── Resources ──
  {
    method: 'resources/list',
    category: 'resources',
    description: 'List all available MCP resources.',
    defaultParams: {},
  },
  {
    method: 'resources/read',
    category: 'resources',
    description: 'Read a specific resource by URI.',
    paramsSchema: {
      type: 'object',
      properties: {
        uri: { type: 'string', description: 'Resource URI to read (e.g., agentreplay://traces/recent)' },
      },
      required: ['uri'],
    },
    defaultParams: { uri: 'agentreplay://traces/recent' },
  },

  // ── Tools ──
  {
    method: 'tools/list',
    category: 'tools',
    description: 'List all available MCP tools with their input schemas.',
    defaultParams: {},
  },
  {
    method: 'tools/call',
    category: 'tools',
    description: 'Call an MCP tool by name with arguments.',
    paramsSchema: {
      type: 'object',
      properties: {
        name: {
          type: 'string',
          description: 'Tool name',
          enum: ['search_traces', 'get_context', 'get_trace_details', 'get_related_traces', 'get_trace_summary', 'save_memory'],
        },
        arguments: {
          type: 'object',
          description: 'Tool-specific arguments (varies by tool)',
        },
      },
      required: ['name', 'arguments'],
    },
    defaultParams: {
      name: 'search_traces',
      arguments: { query: 'error', limit: 10 },
    },
  },

  // ── Prompts ──
  {
    method: 'prompts/list',
    category: 'prompts',
    description: 'List all available MCP prompts.',
    defaultParams: {},
  },
  {
    method: 'prompts/get',
    category: 'prompts',
    description: 'Get a specific prompt by name with arguments.',
    paramsSchema: {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'Prompt name' },
        arguments: { type: 'object', description: 'Prompt arguments' },
      },
      required: ['name'],
    },
    defaultParams: { name: 'analyze_trace', arguments: {} },
  },
];

// ─── Tool-Specific Schema Lookup ───────────────────────────────────────────────

export const TOOL_SCHEMAS: Record<string, JsonSchema> = {
  search_traces: searchTracesSchema,
  get_context: getContextSchema,
  get_trace_details: getTraceDetailsSchema,
  get_related_traces: getRelatedTracesSchema,
  get_trace_summary: getTraceSummarySchema,
  save_memory: saveMemorySchema,
};

// ─── Registry Lookup ───────────────────────────────────────────────────────────

const _methodMap = new Map<string, McpMethodDefinition>();
MCP_METHODS.forEach((m) => _methodMap.set(m.method, m));

/** O(1) lookup by method name */
export function getMethodDefinition(method: string): McpMethodDefinition | undefined {
  return _methodMap.get(method);
}

/** O(1) category filter via pre-built index */
const _categoryIndex = new Map<MethodCategory, McpMethodDefinition[]>();
MCP_METHODS.forEach((m) => {
  const list = _categoryIndex.get(m.category) || [];
  list.push(m);
  _categoryIndex.set(m.category, list);
});

export function getMethodsByCategory(category: MethodCategory): McpMethodDefinition[] {
  return _categoryIndex.get(category) || [];
}

export function getAllCategories(): MethodCategory[] {
  return ['lifecycle', 'resources', 'tools', 'prompts'];
}

/** Get the tool argument schema when tools/call is selected with a specific tool name */
export function getToolArgumentSchema(toolName: string): JsonSchema | undefined {
  return TOOL_SCHEMAS[toolName];
}

// ─── Schema Validation ─────────────────────────────────────────────────────────

export interface SchemaValidationError {
  path: string;
  message: string;
}

/**
 * Validate params against a JSON Schema. O(d × k) where d = depth, k = keys.
 * For AgentReplay schemas (max depth 3, max 7 keys) this is effectively O(1).
 */
export function validateParams(
  params: Record<string, unknown>,
  schema: JsonSchema
): SchemaValidationError[] {
  const errors: SchemaValidationError[] = [];

  // Check required fields
  if (schema.required) {
    for (const field of schema.required) {
      if (!(field in params) || params[field] === undefined) {
        errors.push({ path: field, message: `Required field "${field}" is missing.` });
      }
    }
  }

  // Check types and enums
  if (schema.properties) {
    for (const [key, prop] of Object.entries(schema.properties)) {
      if (!(key in params)) continue;
      const value = params[key];

      if (value === null || value === undefined) continue;

      // Type check
      if (prop.type === 'string' && typeof value !== 'string') {
        errors.push({ path: key, message: `Field "${key}" must be a string.` });
      } else if (prop.type === 'number' && typeof value !== 'number') {
        errors.push({ path: key, message: `Field "${key}" must be a number.` });
      } else if (prop.type === 'boolean' && typeof value !== 'boolean') {
        errors.push({ path: key, message: `Field "${key}" must be a boolean.` });
      } else if (prop.type === 'array' && !Array.isArray(value)) {
        errors.push({ path: key, message: `Field "${key}" must be an array.` });
      } else if (prop.type === 'object' && (typeof value !== 'object' || Array.isArray(value))) {
        errors.push({ path: key, message: `Field "${key}" must be an object.` });
      }

      // Enum check
      if (prop.enum && typeof value === 'string' && !prop.enum.includes(value)) {
        errors.push({ path: key, message: `Field "${key}" must be one of: ${prop.enum.join(', ')}.` });
      }

      // Range check
      if (prop.minimum !== undefined && typeof value === 'number' && value < prop.minimum) {
        errors.push({ path: key, message: `Field "${key}" must be >= ${prop.minimum}.` });
      }
      if (prop.maximum !== undefined && typeof value === 'number' && value > prop.maximum) {
        errors.push({ path: key, message: `Field "${key}" must be <= ${prop.maximum}.` });
      }
    }
  }

  return errors;
}

/** Generate a template params object from a schema with defaults */
export function generateTemplate(schema: JsonSchema): Record<string, unknown> {
  const template: Record<string, unknown> = {};
  if (!schema.properties) return template;

  for (const [key, prop] of Object.entries(schema.properties)) {
    if (prop.default !== undefined) {
      template[key] = prop.default;
    } else if (prop.enum && prop.enum.length > 0) {
      template[key] = prop.enum[0];
    } else if (prop.type === 'string') {
      template[key] = '';
    } else if (prop.type === 'number') {
      template[key] = 0;
    } else if (prop.type === 'boolean') {
      template[key] = false;
    } else if (prop.type === 'array') {
      template[key] = [];
    } else if (prop.type === 'object') {
      if (prop.properties) {
        template[key] = generateTemplate({ type: 'object', properties: prop.properties, required: prop.required });
      } else {
        template[key] = {};
      }
    }
  }

  return template;
}
