# Agent Replay JavaScript/TypeScript SDK

High-performance observability SDK for LLM agents and AI applications.

## Installation

```bash
npm install @agentreplay/sdk
# or
yarn add @agentreplay/sdk
# or
pnpm add @agentreplay/sdk
```

## Quick Start

```typescript
import { Agent ReplayClient, SpanType } from '@agentreplay/sdk';

// Initialize the client
const client = new Agent ReplayClient({
  url: 'http://localhost:8080',
  tenantId: 1,
  projectId: 0  // optional
});

// Create a basic trace
const trace = await client.createTrace({
  agentId: 1,
  sessionId: 123,
  spanType: SpanType.Root,
  metadata: { name: 'my-agent' }
});

console.log(`Created trace: ${trace.edgeId}`);
```

## Tracking LLM Calls

The SDK supports OpenTelemetry GenAI semantic conventions for comprehensive LLM observability:

```typescript
// Track an LLM call with full context
const llmTrace = await client.createGenAITrace({
  agentId: 1,
  sessionId: 123,
  model: 'gpt-4o',
  inputMessages: [
    { role: 'system', content: 'You are a helpful assistant.' },
    { role: 'user', content: 'What is the capital of France?' }
  ],
  output: { role: 'assistant', content: 'The capital of France is Paris.' },
  modelParameters: {
    temperature: 0.7,
    max_tokens: 1000
  },
  inputUsage: 25,
  outputUsage: 12,
  totalUsage: 37,
  finishReason: 'stop'
});
```

## Tracking Tool Calls

```typescript
// Track a tool/function call
const toolTrace = await client.createToolTrace({
  agentId: 1,
  sessionId: 123,
  toolName: 'web_search',
  toolInput: { query: 'weather in Paris' },
  toolOutput: { results: [...] },
  toolDescription: 'Search the web for information',
  parentId: llmTrace.edgeId  // Link to parent LLM trace
});
```

## Querying Traces

```typescript
// Query traces with filters
const results = await client.queryTraces({
  sessionId: 123,
  limit: 100
});

// Query within a time range
const rangeResults = await client.queryTemporalRange(
  Date.now() * 1000 - 3600_000_000,  // 1 hour ago (microseconds)
  Date.now() * 1000,                   // now
  { agentId: 1 }
);

// Get a specific trace with payload
const trace = await client.getTrace('abc123');

// Get trace hierarchy
const tree = await client.getTraceTree('abc123');
```

## User Feedback

Capture user satisfaction signals for building evaluation datasets:

```typescript
// Submit thumbs up/down feedback
await client.submitFeedback(trace.edgeId, 1);   // thumbs up
await client.submitFeedback(trace.edgeId, -1);  // thumbs down

// Add to evaluation dataset
await client.addToDataset(trace.edgeId, 'bad_responses', {
  inputData: { prompt: 'Hello' },
  outputData: { response: '...' }
});
```

## Span Types

```typescript
import { SpanType } from '@agentreplay/sdk';

SpanType.Root        // 0 - Root span
SpanType.Planning    // 1 - Planning phase
SpanType.Reasoning   // 2 - Reasoning/thinking
SpanType.ToolCall    // 3 - Tool/function call
SpanType.ToolResponse // 4 - Tool response
SpanType.Synthesis   // 5 - Result synthesis
SpanType.Response    // 6 - Final response
SpanType.Error       // 7 - Error state
SpanType.Retrieval   // 8 - Vector DB retrieval
SpanType.Embedding   // 9 - Text embedding
SpanType.HttpCall    // 10 - HTTP API call
SpanType.Database    // 11 - Database query
SpanType.Function    // 12 - Generic function
SpanType.Reranking   // 13 - Result reranking
SpanType.Parsing     // 14 - Document parsing
SpanType.Generation  // 15 - Content generation
SpanType.Custom      // 255 - Custom types
```

## Configuration Options

```typescript
interface Agent ReplayClientOptions {
  url: string;                      // Agent Replay server URL
  tenantId: number;                 // Tenant identifier
  projectId?: number;               // Project identifier (default: 0)
  agentId?: number;                 // Default agent ID (default: 1)
  timeout?: number;                 // Request timeout in ms (default: 30000)
  headers?: Record<string, string>; // Additional headers
  fetch?: typeof fetch;             // Custom fetch implementation
}
```

## Framework Integrations

### With OpenAI

```typescript
import OpenAI from 'openai';
import { Agent ReplayClient } from '@agentreplay/sdk';

const openai = new OpenAI();
const agentreplay = new Agent ReplayClient({ url: '...', tenantId: 1 });

async function chat(messages: OpenAI.ChatCompletionMessageParam[]) {
  const response = await openai.chat.completions.create({
    model: 'gpt-4o',
    messages,
    temperature: 0.7
  });

  // Track the call
  await agentreplay.createGenAITrace({
    agentId: 1,
    sessionId: Date.now(),
    model: response.model,
    inputMessages: messages.map(m => ({ role: m.role, content: String(m.content) })),
    output: { role: 'assistant', content: response.choices[0].message.content ?? '' },
    inputUsage: response.usage?.prompt_tokens,
    outputUsage: response.usage?.completion_tokens,
    totalUsage: response.usage?.total_tokens,
    finishReason: response.choices[0].finish_reason
  });

  return response;
}
```

### With LangChain.js

```typescript
import { ChatOpenAI } from '@langchain/openai';
import { Agent ReplayClient, SpanType } from '@agentreplay/sdk';

const agentreplay = new Agent ReplayClient({ url: '...', tenantId: 1 });

// Create a callback handler
class Agent ReplayHandler {
  private sessionId = Date.now();

  async handleLLMStart(llm: any, prompts: string[]) {
    return agentreplay.createTrace({
      agentId: 1,
      sessionId: this.sessionId,
      spanType: SpanType.Generation,
      metadata: { prompts }
    });
  }

  async handleLLMEnd(output: any, runId: string) {
    await agentreplay.updateTrace({
      edgeId: runId,
      sessionId: this.sessionId,
      tokenCount: output.llmOutput?.tokenUsage?.totalTokens
    });
  }
}
```

## TypeScript Support

The SDK is written in TypeScript and provides full type definitions:

```typescript
import type {
  Agent ReplayClientOptions,
  QueryFilter,
  QueryResponse,
  TraceView,
  SpanInput,
  GenAIAttributes
} from '@agentreplay/sdk';
```

## Error Handling

```typescript
try {
  const trace = await client.createTrace({...});
} catch (error) {
  if (error instanceof Error) {
    console.error(`Agent Replay error: ${error.message}`);
  }
}
```

## License

MIT
