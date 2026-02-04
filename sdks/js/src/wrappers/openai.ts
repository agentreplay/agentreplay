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
 * OpenAI SDK wrapper for automatic tracing.
 *
 * Wraps the OpenAI client to automatically trace all API calls.
 */

import { withSpan, type ActiveSpan } from '../traceable';
import type { SpanKind } from '../types';

/**
 * Wrapper options
 */
export interface WrapOpenAIOptions {
  /** Whether to record input messages (default: true) */
  recordInput?: boolean;
  /** Whether to record output messages (default: true) */
  recordOutput?: boolean;
  /** Whether to record model parameters (default: true) */
  recordParameters?: boolean;
  /** Custom metadata to add to all spans */
  metadata?: Record<string, unknown>;
}

/**
 * Chat completion message type
 */
interface ChatMessage {
  role: string;
  content?: string | null;
  tool_calls?: Array<{
    id: string;
    type: string;
    function: {
      name: string;
      arguments: string;
    };
  }>;
}

/**
 * Chat completion response type
 */
interface ChatCompletionResponse {
  id: string;
  object: string;
  created: number;
  model: string;
  choices: Array<{
    index: number;
    message: ChatMessage;
    finish_reason: string | null;
  }>;
  usage?: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
}

/**
 * Embedding response type
 */
interface EmbeddingResponse {
  object: string;
  data: Array<{
    object: string;
    embedding: number[];
    index: number;
  }>;
  model: string;
  usage: {
    prompt_tokens: number;
    total_tokens: number;
  };
}

/**
 * Detect provider system from model name
 */
function detectSystem(model: string): string {
  const modelLower = model.toLowerCase();
  if (modelLower.includes('gpt') || modelLower.includes('o1') || modelLower.includes('o3')) {
    return 'openai';
  }
  if (modelLower.includes('claude') || modelLower.includes('anthropic')) {
    return 'anthropic';
  }
  if (modelLower.includes('llama') || modelLower.includes('meta')) {
    return 'meta';
  }
  if (modelLower.includes('gemini') || modelLower.includes('palm')) {
    return 'google';
  }
  if (modelLower.includes('mistral')) {
    return 'mistral';
  }
  return 'openai'; // Default for OpenAI SDK
}

/**
 * Wrap OpenAI chat.completions.create
 */
function wrapChatCompletions(
  original: any,
  options: WrapOpenAIOptions
): any {
  return async function (this: any, params: any) {
    const model = params.model ?? 'unknown';
    const spanName = `chat-${model}`;

    return withSpan(
      spanName,
      {
        kind: 'llm' as SpanKind,
        metadata: {
          ...options.metadata,
          'gen_ai.system': detectSystem(model),
          'gen_ai.operation.name': 'chat',
        },
      },
      async (span: ActiveSpan) => {
        // Record input
        if (options.recordInput !== false) {
          span.setTag('gen_ai.request.model', model);
          if (params.messages) {
            span.attributes['gen_ai.prompt.messages'] = JSON.stringify(
              params.messages.map((m: ChatMessage) => ({
                role: m.role,
                content: m.content,
              }))
            );
          }
        }

        // Record parameters
        if (options.recordParameters !== false) {
          if (params.temperature !== undefined) {
            span.attributes['gen_ai.request.temperature'] = String(params.temperature);
          }
          if (params.max_tokens !== undefined) {
            span.attributes['gen_ai.request.max_tokens'] = String(params.max_tokens);
          }
          if (params.top_p !== undefined) {
            span.attributes['gen_ai.request.top_p'] = String(params.top_p);
          }
        }

        try {
          // Call original
          const response: ChatCompletionResponse = await original.call(this, params);

          // Record output
          if (options.recordOutput !== false) {
            const choice = response.choices[0];
            if (choice?.message) {
              span.attributes['gen_ai.completion.message'] = JSON.stringify(choice.message);
              span.attributes['gen_ai.response.finish_reasons'] = JSON.stringify(
                response.choices.map((c) => c.finish_reason)
              );
            }
            span.attributes['gen_ai.response.model'] = response.model;
            span.attributes['gen_ai.response.id'] = response.id;
          }

          // Record usage
          if (response.usage) {
            span.setTokenUsage({
              prompt: response.usage.prompt_tokens,
              completion: response.usage.completion_tokens,
              total: response.usage.total_tokens,
            });
          }

          return response;
        } catch (error) {
          span.setError(error);
          throw error;
        }
      }
    );
  };
}

/**
 * Wrap OpenAI embeddings.create
 */
function wrapEmbeddings(
  original: any,
  options: WrapOpenAIOptions
): any {
  return async function (this: any, params: any) {
    const model = params.model ?? 'text-embedding-ada-002';
    const spanName = `embedding-${model}`;

    return withSpan(
      spanName,
      {
        kind: 'embedding' as SpanKind,
        metadata: {
          ...options.metadata,
          'gen_ai.system': 'openai',
          'gen_ai.operation.name': 'embedding',
        },
      },
      async (span: ActiveSpan) => {
        // Record input
        if (options.recordInput !== false) {
          span.setTag('gen_ai.request.model', model);
          const inputCount = Array.isArray(params.input) ? params.input.length : 1;
          span.attributes['input.count'] = String(inputCount);
        }

        try {
          // Call original
          const response: EmbeddingResponse = await original.call(this, params);

          // Record output
          if (options.recordOutput !== false) {
            span.attributes['gen_ai.response.model'] = response.model;
            span.attributes['output.count'] = String(response.data.length);
            span.attributes['output.dimensions'] = String(response.data[0]?.embedding.length ?? 0);
          }

          // Record usage
          if (response.usage) {
            span.setTokenUsage({
              prompt: response.usage.prompt_tokens,
              total: response.usage.total_tokens,
            });
          }

          return response;
        } catch (error) {
          span.setError(error);
          throw error;
        }
      }
    );
  };
}

/**
 * Wrap an OpenAI client instance for automatic tracing.
 *
 * All chat.completions.create and embeddings.create calls will be
 * automatically traced with full context.
 *
 * @example
 * ```typescript
 * import OpenAI from 'openai';
 * import { init, wrapOpenAI } from '@agentreplay/sdk';
 *
 * init();
 *
 * const openai = wrapOpenAI(new OpenAI());
 *
 * // All calls are now automatically traced
 * const response = await openai.chat.completions.create({
 *   model: 'gpt-4o',
 *   messages: [{ role: 'user', content: 'Hello!' }]
 * });
 * ```
 */
export function wrapOpenAI<T extends object>(
  client: T,
  options: WrapOpenAIOptions = {}
): T {
  const wrapped = Object.create(Object.getPrototypeOf(client));

  // Copy all properties
  for (const key of Object.keys(client)) {
    wrapped[key] = (client as any)[key];
  }

  // Wrap chat.completions
  if ((client as any).chat?.completions?.create) {
    wrapped.chat = {
      ...wrapped.chat,
      completions: {
        ...(client as any).chat.completions,
        create: wrapChatCompletions(
          (client as any).chat.completions.create.bind((client as any).chat.completions),
          options
        ),
      },
    };
  }

  // Wrap embeddings
  if ((client as any).embeddings?.create) {
    wrapped.embeddings = {
      ...(client as any).embeddings,
      create: wrapEmbeddings(
        (client as any).embeddings.create.bind((client as any).embeddings),
        options
      ),
    };
  }

  return wrapped as T;
}

/**
 * Wrap an Anthropic client instance for automatic tracing.
 *
 * @example
 * ```typescript
 * import Anthropic from '@anthropic-ai/sdk';
 * import { init, wrapAnthropic } from '@agentreplay/sdk';
 *
 * init();
 *
 * const anthropic = wrapAnthropic(new Anthropic());
 *
 * const response = await anthropic.messages.create({
 *   model: 'claude-3-opus-20240229',
 *   messages: [{ role: 'user', content: 'Hello!' }]
 * });
 * ```
 */
export function wrapAnthropic<T extends object>(
  client: T,
  options: WrapOpenAIOptions = {}
): T {
  const wrapped = Object.create(Object.getPrototypeOf(client));

  // Copy all properties
  for (const key of Object.keys(client)) {
    wrapped[key] = (client as any)[key];
  }

  // Wrap messages.create
  if ((client as any).messages?.create) {
    const originalCreate = (client as any).messages.create.bind((client as any).messages);

    wrapped.messages = {
      ...(client as any).messages,
      create: async function (params: any) {
        const model = params.model ?? 'unknown';
        const spanName = `chat-${model}`;

        return withSpan(
          spanName,
          {
            kind: 'llm' as SpanKind,
            metadata: {
              ...options.metadata,
              'gen_ai.system': 'anthropic',
              'gen_ai.operation.name': 'chat',
            },
          },
          async (span: ActiveSpan) => {
            // Record input
            if (options.recordInput !== false) {
              span.setTag('gen_ai.request.model', model);
              if (params.messages) {
                span.attributes['gen_ai.prompt.messages'] = JSON.stringify(params.messages);
              }
              if (params.system) {
                span.attributes['gen_ai.prompt.system'] = params.system;
              }
            }

            // Record parameters
            if (options.recordParameters !== false) {
              if (params.temperature !== undefined) {
                span.attributes['gen_ai.request.temperature'] = String(params.temperature);
              }
              if (params.max_tokens !== undefined) {
                span.attributes['gen_ai.request.max_tokens'] = String(params.max_tokens);
              }
            }

            try {
              const response = await originalCreate(params);

              // Record output
              if (options.recordOutput !== false) {
                span.attributes['gen_ai.response.model'] = response.model;
                span.attributes['gen_ai.response.id'] = response.id;
                span.attributes['gen_ai.response.stop_reason'] = response.stop_reason;
                if (response.content) {
                  span.attributes['gen_ai.completion.message'] = JSON.stringify(response.content);
                }
              }

              // Record usage
              if (response.usage) {
                span.setTokenUsage({
                  prompt: response.usage.input_tokens,
                  completion: response.usage.output_tokens,
                  total: response.usage.input_tokens + response.usage.output_tokens,
                });
              }

              return response;
            } catch (error) {
              span.setError(error);
              throw error;
            }
          }
        );
      },
    };
  }

  return wrapped as T;
}
