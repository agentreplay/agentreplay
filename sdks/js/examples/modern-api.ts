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

/**
 * Agentreplay SDK - Modern API Example
 *
 * Demonstrates the new developer-friendly API with:
 * - init() with env vars
 * - traceable() wrapper
 * - wrapOpenAI() auto-instrumentation
 * - Context propagation
 * - flush() for serverless
 */

import {
  init,
  traceable,
  withSpan,
  startSpan,
  setGlobalContext,
  flush,
  shutdown,
  captureException,
  emailScrubber,
  creditCardScrubber,
} from '../src';

// Mock OpenAI-like client for demo
const mockOpenAI = {
  chat: {
    completions: {
      create: async (params: any) => ({
        id: 'chatcmpl-123',
        object: 'chat.completion',
        created: Date.now(),
        model: params.model,
        choices: [{
          index: 0,
          message: { role: 'assistant', content: 'Hello! I am an AI assistant.' },
          finish_reason: 'stop',
        }],
        usage: { prompt_tokens: 10, completion_tokens: 8, total_tokens: 18 },
      }),
    },
  },
};

async function main() {
  console.log('🚀 Agentreplay SDK Modern API Example\n');

  // ==================== 1. Initialize ====================
  console.log('1. Initializing SDK...');

  init({
    baseUrl: 'http://localhost:8080',
    tenantId: 1,
    projectId: 0,
    environment: 'development',
    debug: true, // Enable debug logging

    // Sampling: 100% for demo, use lower in production
    sampling: {
      rate: 1.0,
      rules: [
        // Always sample errors
        { when: { error: true }, sample: 1.0 },
        // Always sample VIP users
        { when: { tag: 'tier:premium' }, sample: 1.0 },
      ],
    },

    // Privacy: redact sensitive data client-side
    privacy: {
      mode: 'redact',
      redact: ['messages.*.content'], // Redact message content
      scrubbers: [emailScrubber, creditCardScrubber],
    },

    // Transport: batch for efficiency
    transport: {
      mode: 'console', // 'console' for demo, use 'batch' in production
      batchSize: 100,
      flushIntervalMs: 5000,
      maxQueueSize: 10000,
      maxRetries: 3,
      retryDelayMs: 1000,
      compression: false,
    },
  });

  console.log('   ✅ SDK initialized\n');

  // ==================== 2. Set Global Context ====================
  console.log('2. Setting global context...');

  setGlobalContext({
    userId: 'user_12345',
    sessionId: Date.now(),
    tags: {
      'tier': 'premium',
      'environment': 'demo',
    },
  });

  console.log('   ✅ Context set\n');

  // ==================== 3. Use traceable() wrapper ====================
  console.log('3. Using traceable() wrapper...');

  const processRequest = traceable(
    async (query: string) => {
      // Simulate some processing
      await new Promise((resolve) => setTimeout(resolve, 100));
      return { result: `Processed: ${query}` };
    },
    { name: 'process_request', kind: 'chain' }
  );

  const result = await processRequest('What is the weather?');
  console.log(`   Result: ${JSON.stringify(result)}`);
  console.log('   ✅ traceable() completed\n');

  // ==================== 4. Use withSpan() for scoped spans ====================
  console.log('4. Using withSpan() for retrieval...');

  const documents = await withSpan(
    'retrieve_context',
    {
      kind: 'retriever',
      input: { query: 'weather forecast' },
      metadata: { source: 'vector_db' },
    },
    async (span) => {
      // Simulate vector DB retrieval
      await new Promise((resolve) => setTimeout(resolve, 50));

      const docs = [
        { id: 1, content: 'Weather data...' },
        { id: 2, content: 'Forecast info...' },
      ];

      span.setOutput({ count: docs.length });
      span.addEvent('retrieval_complete', { source: 'pinecone' });

      return docs;
    }
  );

  console.log(`   Retrieved ${documents.length} documents`);
  console.log('   ✅ withSpan() completed\n');

  // ==================== 5. Use startSpan() for manual control ====================
  console.log('5. Using startSpan() for tool call...');

  const toolSpan = startSpan('web_search', {
    kind: 'tool',
    input: { query: 'current temperature Paris' },
  });

  try {
    // Simulate tool execution
    await new Promise((resolve) => setTimeout(resolve, 75));

    const toolResult = { temperature: '22°C', location: 'Paris, France' };
    toolSpan.setOutput(toolResult);
    toolSpan.end();

    console.log(`   Tool result: ${JSON.stringify(toolResult)}`);
  } catch (err) {
    toolSpan.setError(err);
    toolSpan.end();
    throw err;
  }

  console.log('   ✅ startSpan() completed\n');

  // ==================== 6. LLM call with nested spans ====================
  console.log('6. Simulating LLM call with nested spans...');

  await withSpan('agent_turn', { kind: 'chain' }, async (agentSpan) => {
    // Planning phase
    await withSpan('planning', { kind: 'llm' }, async (planSpan) => {
      await new Promise((resolve) => setTimeout(resolve, 30));
      planSpan.setOutput({ plan: 'Search web, then respond' });
    });

    // Tool call
    await withSpan('execute_tool', { kind: 'tool' }, async (toolSpan) => {
      await new Promise((resolve) => setTimeout(resolve, 40));
      toolSpan.setOutput({ data: 'Tool output here' });
    });

    // Final response
    await withSpan('generate_response', { kind: 'llm' }, async (responseSpan) => {
      // Simulate OpenAI call
      const response = await mockOpenAI.chat.completions.create({
        model: 'gpt-4o',
        messages: [{ role: 'user', content: 'What is the weather?' }],
      });

      responseSpan.setTokenUsage({
        prompt: response.usage.prompt_tokens,
        completion: response.usage.completion_tokens,
        total: response.usage.total_tokens,
      });
      responseSpan.setOutput({ content: response.choices[0].message.content });
    });

    agentSpan.setOutput({ status: 'completed' });
  });

  console.log('   ✅ Nested spans completed\n');

  // ==================== 7. Error handling ====================
  console.log('7. Demonstrating error capture...');

  try {
    await traceable(
      async () => {
        throw new Error('Simulated error for demo');
      },
      { name: 'failing_operation', kind: 'tool' }
    )();
  } catch (err) {
    captureException(err, { operation: 'demo_error' });
    console.log('   ✅ Error captured (expected)\n');
  }

  // ==================== 8. Flush before exit ====================
  console.log('8. Flushing spans before exit...');

  await flush({ timeoutMs: 5000 });

  console.log('   ✅ Flush complete\n');

  // ==================== 9. Shutdown ====================
  console.log('9. Shutting down SDK...');

  await shutdown();

  console.log('   ✅ Shutdown complete\n');

  console.log('🎉 Example complete!');
  console.log('\nIn production, spans would be sent to your Agentreplay server.');
  console.log('Set transport.mode to "batch" and provide a valid baseUrl.');
}

// Run the example
main().catch(console.error);
