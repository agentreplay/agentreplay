/**
 * Copyright 2025 Sushanth (https://github.com/sushanthpy)
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/**
 * Agentreplay SDK for JavaScript/TypeScript
 *
 * High-performance observability for LLM agents.
 *
 * @example
 * ```typescript
 * import { AgentreplayClient, SpanType } from '@agentreplay/sdk';
 *
 * const client = new AgentreplayClient({
 *   url: 'http://localhost:8080',
 *   tenantId: 1
 * });
 *
 * // Create a trace
 * const trace = await client.createTrace({
 *   agentId: 1,
 *   sessionId: 123,
 *   spanType: SpanType.Root
 * });
 *
 * // Track LLM call
 * await client.createGenAITrace({
 *   agentId: 1,
 *   sessionId: 123,
 *   model: 'gpt-4o',
 *   inputMessages: [{ role: 'user', content: 'Hello!' }],
 *   output: { role: 'assistant', content: 'Hi!' },
 *   inputUsage: 10,
 *   outputUsage: 5
 * });
 * ```
 *
 * @packageDocumentation
 */

export { AgentreplayClient } from './client';

export {
  SpanType,
  SensitivityFlags,
} from './types';

export type {
  AgentreplayClientOptions,
  QueryFilter,
  QueryResponse,
  TraceView,
  SpanInput,
  IngestResponse,
  TraceTreeNode,
  GenAIAttributes,
  FeedbackValue,
  AgentFlowEdge,
  Environment,
  MetricsDataPoint,
  MetricsResponse,
} from './types';
