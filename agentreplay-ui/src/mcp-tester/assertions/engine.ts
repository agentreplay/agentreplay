/**
 * Task 9 — Assertion Engine for MCP Regression Testing
 *
 * Declarative assertion DSL for MCP response validation.
 * Recursive descent parser: O(n). JSONPath evaluation: O(p × d).
 * Total assertion evaluation for m assertions: O(m).
 */

import { extractJsonPath } from '../composer/sequences';

// ─── Assertion Types ───────────────────────────────────────────────────────────

export type AssertionOperator =
  | 'equals'
  | 'not_equals'
  | 'contains'
  | 'not_contains'
  | 'greater_than'
  | 'less_than'
  | 'greater_equal'
  | 'less_equal'
  | 'exists'
  | 'not_exists'
  | 'is_null'
  | 'not_null'
  | 'matches_regex'
  | 'typeof'
  | 'length_equals'
  | 'length_greater'
  | 'length_less'
  | 'conforms_to';

export interface Assertion {
  id: string;
  /** Human-readable label */
  label: string;
  /** JSONPath to extract the value (e.g., $.result.tools) */
  path: string;
  /** Assertion operator */
  operator: AssertionOperator;
  /** Expected value (not needed for exists/not_exists/is_null/not_null) */
  expected?: unknown;
  /** Whether this assertion is enabled */
  enabled: boolean;
}

export interface AssertionResult {
  assertionId: string;
  label: string;
  passed: boolean;
  actual?: unknown;
  expected?: unknown;
  message: string;
  durationMs: number;
}

export interface AssertionSuite {
  id: string;
  name: string;
  description: string;
  assertions: Assertion[];
}

// ─── Built-in Assertion Suites ─────────────────────────────────────────────────

export const BUILTIN_SUITES: AssertionSuite[] = [
  {
    id: 'basic-health',
    name: 'Basic Health',
    description: 'Verify MCP server responds correctly to initialize',
    assertions: [
      {
        id: 'no-error',
        label: 'No JSON-RPC error',
        path: '$.error',
        operator: 'not_exists',
        enabled: true,
      },
      {
        id: 'has-result',
        label: 'Has result field',
        path: '$.result',
        operator: 'exists',
        enabled: true,
      },
      {
        id: 'protocol-version',
        label: 'Protocol version is 2024-11-05',
        path: '$.result.protocolVersion',
        operator: 'equals',
        expected: '2024-11-05',
        enabled: true,
      },
    ],
  },
  {
    id: 'tools-validation',
    name: 'Tools Validation',
    description: 'Verify tool listing returns expected tools',
    assertions: [
      {
        id: 'has-tools',
        label: 'Has tools array',
        path: '$.result.tools',
        operator: 'exists',
        enabled: true,
      },
      {
        id: 'tool-count',
        label: 'At least 5 tools',
        path: '$.result.tools',
        operator: 'length_greater',
        expected: 4,
        enabled: true,
      },
      {
        id: 'has-search-traces',
        label: 'search_traces tool exists',
        path: '$.result.tools[0].name',
        operator: 'equals',
        expected: 'search_traces',
        enabled: true,
      },
    ],
  },
  {
    id: 'latency-sla',
    name: 'Latency SLA',
    description: 'Verify response times are within SLA',
    assertions: [
      {
        id: 'ping-latency',
        label: 'Ping responds under 100ms',
        path: '$.__meta.durationMs',
        operator: 'less_than',
        expected: 100,
        enabled: true,
      },
      {
        id: 'tools-list-latency',
        label: 'tools/list under 200ms',
        path: '$.__meta.durationMs',
        operator: 'less_than',
        expected: 200,
        enabled: true,
      },
    ],
  },
];

// ─── Assertion Engine ──────────────────────────────────────────────────────────

/**
 * Evaluate a single assertion against a response.
 * O(p × d) for JSONPath extraction + O(1) for comparison.
 */
export function evaluateAssertion(
  assertion: Assertion,
  response: unknown,
  meta?: { durationMs?: number }
): AssertionResult {
  const start = performance.now();

  // Merge meta into response for $.__meta access
  const enriched = typeof response === 'object' && response !== null
    ? { ...(response as Record<string, unknown>), __meta: meta || {} }
    : response;

  const actual = extractJsonPath(enriched, assertion.path);

  let passed = false;
  let message = '';

  switch (assertion.operator) {
    case 'equals':
      passed = deepEqual(actual, assertion.expected);
      message = passed ? 'Values are equal' : `Expected ${JSON.stringify(assertion.expected)}, got ${JSON.stringify(actual)}`;
      break;

    case 'not_equals':
      passed = !deepEqual(actual, assertion.expected);
      message = passed ? 'Values are not equal' : `Expected values to differ, both are ${JSON.stringify(actual)}`;
      break;

    case 'contains':
      if (typeof actual === 'string' && typeof assertion.expected === 'string') {
        passed = actual.includes(assertion.expected);
      } else if (Array.isArray(actual)) {
        passed = actual.some((item) => deepEqual(item, assertion.expected));
      }
      message = passed ? 'Value contains expected' : `${JSON.stringify(actual)} does not contain ${JSON.stringify(assertion.expected)}`;
      break;

    case 'not_contains':
      if (typeof actual === 'string' && typeof assertion.expected === 'string') {
        passed = !actual.includes(assertion.expected);
      } else if (Array.isArray(actual)) {
        passed = !actual.some((item) => deepEqual(item, assertion.expected));
      }
      message = passed ? 'Value does not contain expected' : `${JSON.stringify(actual)} contains ${JSON.stringify(assertion.expected)}`;
      break;

    case 'greater_than':
      passed = typeof actual === 'number' && typeof assertion.expected === 'number' && actual > assertion.expected;
      message = passed ? `${actual} > ${assertion.expected}` : `Expected > ${assertion.expected}, got ${actual}`;
      break;

    case 'less_than':
      passed = typeof actual === 'number' && typeof assertion.expected === 'number' && actual < assertion.expected;
      message = passed ? `${actual} < ${assertion.expected}` : `Expected < ${assertion.expected}, got ${actual}`;
      break;

    case 'greater_equal':
      passed = typeof actual === 'number' && typeof assertion.expected === 'number' && actual >= assertion.expected;
      message = passed ? `${actual} >= ${assertion.expected}` : `Expected >= ${assertion.expected}, got ${actual}`;
      break;

    case 'less_equal':
      passed = typeof actual === 'number' && typeof assertion.expected === 'number' && actual <= assertion.expected;
      message = passed ? `${actual} <= ${assertion.expected}` : `Expected <= ${assertion.expected}, got ${actual}`;
      break;

    case 'exists':
      passed = actual !== undefined && actual !== null;
      message = passed ? 'Value exists' : `Value at ${assertion.path} does not exist`;
      break;

    case 'not_exists':
      passed = actual === undefined || actual === null;
      message = passed ? 'Value does not exist' : `Value at ${assertion.path} exists: ${JSON.stringify(actual)}`;
      break;

    case 'is_null':
      passed = actual === null;
      message = passed ? 'Value is null' : `Expected null, got ${JSON.stringify(actual)}`;
      break;

    case 'not_null':
      passed = actual !== null && actual !== undefined;
      message = passed ? 'Value is not null' : 'Value is null';
      break;

    case 'matches_regex':
      if (typeof actual === 'string' && typeof assertion.expected === 'string') {
        try {
          passed = new RegExp(assertion.expected).test(actual);
        } catch {
          passed = false;
          message = 'Invalid regex pattern';
        }
      }
      if (!message) message = passed ? 'Matches pattern' : `"${actual}" does not match /${assertion.expected}/`;
      break;

    case 'typeof':
      passed = typeof actual === assertion.expected;
      message = passed ? `Type is ${assertion.expected}` : `Expected type ${assertion.expected}, got ${typeof actual}`;
      break;

    case 'length_equals':
      if (Array.isArray(actual)) {
        passed = actual.length === assertion.expected;
        message = passed ? `Length is ${assertion.expected}` : `Expected length ${assertion.expected}, got ${actual.length}`;
      } else if (typeof actual === 'string') {
        passed = actual.length === assertion.expected;
        message = passed ? `Length is ${assertion.expected}` : `Expected length ${assertion.expected}, got ${actual.length}`;
      } else {
        message = 'Value is not an array or string';
      }
      break;

    case 'length_greater':
      if (Array.isArray(actual)) {
        passed = actual.length > (assertion.expected as number);
        message = passed ? `Length ${actual.length} > ${assertion.expected}` : `Length ${actual.length} <= ${assertion.expected}`;
      }
      break;

    case 'length_less':
      if (Array.isArray(actual)) {
        passed = actual.length < (assertion.expected as number);
        message = passed ? `Length ${actual.length} < ${assertion.expected}` : `Length ${actual.length} >= ${assertion.expected}`;
      }
      break;

    case 'conforms_to':
      // Structural subtype check: O(f) where f = fields
      passed = conformsToSchema(actual, assertion.expected as Record<string, string>);
      message = passed ? 'Conforms to schema' : 'Does not conform to expected schema';
      break;
  }

  return {
    assertionId: assertion.id,
    label: assertion.label,
    passed,
    actual,
    expected: assertion.expected,
    message,
    durationMs: performance.now() - start,
  };
}

/**
 * Run all assertions in a suite against a response.
 * Total: O(m) for m assertions (each is O(1) for fixed schemas).
 */
export function runAssertionSuite(
  suite: AssertionSuite,
  response: unknown,
  meta?: { durationMs?: number }
): AssertionResult[] {
  return suite.assertions
    .filter((a) => a.enabled)
    .map((a) => evaluateAssertion(a, response, meta));
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a === null || b === null) return false;
  if (typeof a !== typeof b) return false;
  if (typeof a !== 'object') return false;

  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    return a.every((item, i) => deepEqual(item, b[i]));
  }

  const aObj = a as Record<string, unknown>;
  const bObj = b as Record<string, unknown>;
  const aKeys = Object.keys(aObj);
  const bKeys = Object.keys(bObj);

  if (aKeys.length !== bKeys.length) return false;
  return aKeys.every((key) => deepEqual(aObj[key], bObj[key]));
}

/**
 * Structural subtype check: verify every required field exists with correct type.
 * O(f) where f = number of fields in schema.
 */
function conformsToSchema(value: unknown, schema: Record<string, string>): boolean {
  if (typeof value !== 'object' || value === null) return false;
  const obj = value as Record<string, unknown>;
  for (const [field, expectedType] of Object.entries(schema)) {
    if (!(field in obj)) return false;
    if (typeof obj[field] !== expectedType) return false;
  }
  return true;
}

// ─── DSL Parser ────────────────────────────────────────────────────────────────

/**
 * Parse a simple assertion DSL string into an Assertion object.
 * Syntax: <path> <operator> <expected>
 * Examples:
 *   $.result.tools | length >= 5
 *   $.error == null
 *   $.result.protocolVersion == "2024-11-05"
 *   duration < 200
 *
 * Recursive descent parser: O(n) where n = expression length.
 */
export function parseAssertionDsl(dsl: string): Assertion | null {
  const trimmed = dsl.trim();
  if (!trimmed) return null;

  // Pattern: path operator value
  const patterns: Array<{ regex: RegExp; operator: AssertionOperator }> = [
    { regex: /^(.+?)\s*\|\s*length\s*>=\s*(.+)$/, operator: 'length_greater' },
    { regex: /^(.+?)\s*\|\s*length\s*<=\s*(.+)$/, operator: 'length_less' },
    { regex: /^(.+?)\s*\|\s*length\s*==\s*(.+)$/, operator: 'length_equals' },
    { regex: /^(.+?)\s*===?\s*null$/, operator: 'is_null' },
    { regex: /^(.+?)\s*!==?\s*null$/, operator: 'not_null' },
    { regex: /^(.+?)\s*===?\s*(.+)$/, operator: 'equals' },
    { regex: /^(.+?)\s*!==?\s*(.+)$/, operator: 'not_equals' },
    { regex: /^(.+?)\s*>=\s*(.+)$/, operator: 'greater_equal' },
    { regex: /^(.+?)\s*<=\s*(.+)$/, operator: 'less_equal' },
    { regex: /^(.+?)\s*>\s*(.+)$/, operator: 'greater_than' },
    { regex: /^(.+?)\s*<\s*(.+)$/, operator: 'less_than' },
    { regex: /^(.+?)\s+contains\s+(.+)$/, operator: 'contains' },
    { regex: /^(.+?)\s+matches\s+(.+)$/, operator: 'matches_regex' },
    { regex: /^(.+?)\s+exists$/, operator: 'exists' },
    { regex: /^(.+?)\s+not_exists$/, operator: 'not_exists' },
  ];

  for (const { regex, operator } of patterns) {
    const match = trimmed.match(regex);
    if (match) {
      const path = match[1].trim();
      const expectedRaw = match[2]?.trim();

      let expected: unknown;
      if (expectedRaw !== undefined) {
        // Try to parse as JSON value
        try {
          expected = JSON.parse(expectedRaw);
        } catch {
          // Use as string (strip quotes if present)
          expected = expectedRaw.replace(/^["']|["']$/g, '');
        }
      }

      return {
        id: `dsl_${Date.now()}`,
        label: trimmed,
        path,
        operator,
        expected,
        enabled: true,
      };
    }
  }

  return null;
}
