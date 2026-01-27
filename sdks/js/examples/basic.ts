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
 * Flowtrace SDK Basic Example
 *
 * Demonstrates core functionality of the Flowtrace JavaScript SDK.
 */

import { FlowtraceClient, SpanType } from '../src';

async function main() {
  // Initialize client
  const client = new FlowtraceClient({
    url: 'http://localhost:8080',
    tenantId: 1,
    projectId: 0,
  });

  console.log('Flowtrace SDK Example\n');

  // Generate a session ID for this example
  const sessionId = Date.now();

  // 1. Create a root trace
  console.log('1. Creating root trace...');
  const rootTrace = await client.createTrace({
    agentId: 1,
    sessionId,
    spanType: SpanType.Root,
    metadata: { name: 'example-agent' },
  });
  console.log(`   Created: ${rootTrace.edgeId}\n`);

  // 2. Create a planning span (child of root)
  console.log('2. Creating planning span...');
  const planningTrace = await client.createTrace({
    agentId: 1,
    sessionId,
    spanType: SpanType.Planning,
    parentId: rootTrace.edgeId,
    metadata: { step: 'analyze_request' },
  });
  console.log(`   Created: ${planningTrace.edgeId}\n`);

  // 3. Track an LLM call with GenAI attributes
  console.log('3. Creating GenAI trace (LLM call)...');
  const llmTrace = await client.createGenAITrace({
    agentId: 1,
    sessionId,
    model: 'gpt-4o',
    inputMessages: [
      { role: 'system', content: 'You are a helpful assistant.' },
      { role: 'user', content: 'What is the capital of France?' },
    ],
    output: { role: 'assistant', content: 'The capital of France is Paris.' },
    modelParameters: {
      temperature: 0.7,
      max_tokens: 1000,
    },
    inputUsage: 25,
    outputUsage: 12,
    totalUsage: 37,
    parentId: planningTrace.edgeId,
    finishReason: 'stop',
  });
  console.log(`   Created: ${llmTrace.edgeId}`);
  console.log(`   Model: ${llmTrace.model}\n`);

  // 4. Track a tool call
  console.log('4. Creating tool trace...');
  const toolTrace = await client.createToolTrace({
    agentId: 1,
    sessionId,
    toolName: 'web_search',
    toolInput: { query: 'Paris population 2024' },
    toolOutput: { result: 'Paris has a population of approximately 2.1 million' },
    toolDescription: 'Search the web for information',
    parentId: llmTrace.edgeId,
  });
  console.log(`   Created: ${toolTrace.edgeId}`);
  console.log(`   Tool: ${toolTrace.toolName}\n`);

  // 5. Create a response span
  console.log('5. Creating response span...');
  const responseTrace = await client.createTrace({
    agentId: 1,
    sessionId,
    spanType: SpanType.Response,
    parentId: rootTrace.edgeId,
    metadata: {
      final_answer: 'The capital of France is Paris, with a population of about 2.1 million.',
    },
  });
  console.log(`   Created: ${responseTrace.edgeId}\n`);

  // 6. Update trace with final metrics
  console.log('6. Updating trace with completion info...');
  await client.updateTrace({
    edgeId: rootTrace.edgeId,
    sessionId,
    tokenCount: 50,
    durationMs: 1500,
  });
  console.log('   Updated successfully\n');

  // 7. Query traces
  console.log('7. Querying traces for session...');
  const traces = await client.filterBySession(sessionId);
  console.log(`   Found ${traces.length} traces in session ${sessionId}\n`);

  // 8. Submit feedback
  console.log('8. Submitting user feedback...');
  try {
    await client.submitFeedback(rootTrace.edgeId, 1); // thumbs up
    console.log('   Feedback submitted (thumbs up)\n');
  } catch {
    console.log('   Feedback endpoint not available (expected in demo)\n');
  }

  // 9. Health check
  console.log('9. Checking server health...');
  try {
    const health = await client.health();
    console.log(`   Status: ${health.status}\n`);
  } catch {
    console.log('   Server not running (expected in demo)\n');
  }

  console.log('Example complete!');
}

// Run the example
main().catch(console.error);
