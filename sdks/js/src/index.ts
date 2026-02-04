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
 * @example Quick Start
 * ```typescript
 * import { init, traceable, wrapOpenAI, flush } from '@agentreplay/sdk';
 * import OpenAI from 'openai';
 *
 * // Initialize (reads env vars by default)
 * init();
 *
 * // Wrap OpenAI client for automatic tracing
 * const openai = wrapOpenAI(new OpenAI());
 *
 * // All calls are now traced
 * const response = await openai.chat.completions.create({
 *   model: 'gpt-4o',
 *   messages: [{ role: 'user', content: 'Hello!' }]
 * });
 *
 * // Or use traceable for custom functions
 * const result = await traceable(async () => {
 *   return someOperation();
 * }, { name: 'my_operation', kind: 'tool' })();
 *
 * // Flush before serverless function exits
 * await flush();
 * ```
 *
 * @packageDocumentation
 */

// ==================== Core ====================
export { AgentreplayClient } from './client';

// ==================== Configuration ====================
export {
  init,
  getConfig,
  isSDKInitialized,
  resetConfig,
  type InitOptions,
  type ResolvedConfig,
} from './config';

// ==================== Tracing ====================
export {
  traceable,
  withSpan,
  startSpan,
  captureException,
  type TraceableOptions,
  type WithSpanOptions,
  type ActiveSpan,
} from './traceable';

// ==================== Context ====================
export {
  getCurrentContext,
  getCurrentSpan,
  getCurrentTraceId,
  withContext,
  setGlobalContext,
  getGlobalContext,
  clearGlobalContext,
  bindContext,
  type SpanContext,
} from './context';

// ==================== Transport ====================
export {
  flush,
  shutdown,
  getTransportStats,
  type TransportStats,
} from './transport';

// ==================== Wrappers ====================
export {
  wrapOpenAI,
  wrapAnthropic,
  wrapFetch,
  installFetchTracing,
  type WrapOpenAIOptions,
  type WrapFetchOptions,
} from './wrappers';

// ==================== Privacy ====================
export {
  redactPayload,
  hashPII,
  truncateValue,
  safeStringify,
  emailScrubber,
  creditCardScrubber,
  apiKeyScrubber,
  phoneScrubber,
  ssnScrubber,
  builtInScrubbers,
} from './privacy';

// ==================== Sampling ====================
export {
  shouldSample,
  createSampler,
  alwaysSample,
  neverSample,
} from './sampling';

// ==================== Types ====================
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
  // New types
  Span,
  SpanKind,
  SamplingConfig,
  SamplingRule,
  PrivacyConfig,
  TransportConfig,
  Scrubber,
} from './types';
