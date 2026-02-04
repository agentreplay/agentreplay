# Agentreplay JavaScript/TypeScript SDK

[![npm version](https://badge.fury.io/js/@agentreplay%2Fagentreplay.svg)](https://badge.fury.io/js/@agentreplay%2Fagentreplay)
[![Node.js 18+](https://img.shields.io/badge/node-18+-green.svg)](https://nodejs.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.0+-blue.svg)](https://www.typescriptlang.org/)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![CI](https://github.com/agentreplay/agentreplay/actions/workflows/ci-nodejs.yaml/badge.svg)](https://github.com/agentreplay/agentreplay/actions/workflows/ci-nodejs.yaml)

**The observability platform for LLM agents and AI applications.** Trace every LLM call, tool invocation, and agent step with minimal code changes.

---

## ✨ Features

| Feature | Description |
|---------|-------------|
| 🚀 **Zero-Config Setup** | Works out of the box with environment variables |
| 🎯 **One-Liner Instrumentation** | Wrap OpenAI/Anthropic clients in one line |
| 🔧 **Function Wrapping** | `traceable()` for any function |
| 🔄 **Async Native** | Full support for async/await and Promises |
| 🔒 **Privacy First** | Built-in PII redaction and scrubbing |
| 📊 **Token Tracking** | Automatic token usage capture |
| 🌐 **Framework Agnostic** | Works with LangChain.js, Vercel AI SDK, etc. |
| ⚡ **Batched Transport** | Efficient background sending with retry |
| 📦 **Dual Package** | ESM and CommonJS support |
| 🎨 **TypeScript First** | Full type safety and IntelliSense |

---

## 📦 Installation

```bash
# npm
npm install @agentreplay/agentreplay

# yarn
yarn add @agentreplay/agentreplay

# pnpm
pnpm add @agentreplay/agentreplay
```

---

## 🚀 Quick Start

### 1. Set Environment Variables

```bash
export AGENTREPLAY_API_KEY="your-api-key"
export AGENTREPLAY_PROJECT_ID="my-project"
# Optional
export AGENTREPLAY_BASE_URL="https://api.agentreplay.io"
```

### 2. Initialize and Trace

```typescript
import { init, traceable, flush } from '@agentreplay/agentreplay';

// Initialize (reads from env vars automatically)
init();

// Wrap any function for tracing
const myAiFunction = traceable(
  async (query: string) => {
    // Your AI logic here
    return `Response to: ${query}`;
  },
  { name: 'myAiFunction' }
);

// Call your function - it's automatically traced!
const result = await myAiFunction("What is the capital of France?");

// Ensure all traces are sent before exit
await flush();
```

That's it! Your function calls are now being traced and sent to Agentreplay.

---

## 🔧 Core API Reference

### Initialization

```typescript
import { init, getConfig, resetConfig } from '@agentreplay/agentreplay';

// Option 1: Environment variables (recommended for production)
init();

// Option 2: Explicit configuration
init({
  apiKey: 'your-api-key',
  projectId: 'my-project',
  baseUrl: 'https://api.agentreplay.io',
  
  // Optional settings
  tenantId: 'default',        // Multi-tenant identifier
  agentId: 'default',         // Default agent ID
  enabled: true,              // Set false to disable in tests
  captureInput: true,         // Capture function inputs
  captureOutput: true,        // Capture function outputs
  batchSize: 100,             // Batch size before sending
  flushInterval: 5000,        // Auto-flush interval in ms
  debug: false,               // Enable debug logging
});

// Get current configuration
const config = getConfig();
console.log(`Project: ${config.projectId}`);

// Reset to defaults
resetConfig();
```

---

## 🎯 The `traceable()` Function

The primary way to instrument your code:

### Basic Usage

```typescript
import { traceable } from '@agentreplay/agentreplay';

// Wrap any async function
const processQuery = traceable(
  async (query: string) => {
    return await callLlm(query);
  },
  { name: 'processQuery' }
);

// Wrap sync functions too
const parseInput = traceable(
  (input: string) => {
    return JSON.parse(input);
  },
  { name: 'parseInput' }
);
```

### With Options

```typescript
import { traceable, SpanKind } from '@agentreplay/agentreplay';

// Custom span kind for LLM calls
const callOpenAI = traceable(
  async (messages: Message[]) => {
    return await openai.chat.completions.create({
      model: 'gpt-4',
      messages,
    });
  },
  { 
    name: 'callOpenAI',
    kind: SpanKind.LLM,
  }
);

// Disable input capture for sensitive functions
const authenticate = traceable(
  async (password: string) => {
    return await verifyPassword(password);
  },
  { 
    name: 'authenticate',
    captureInput: false,
  }
);

// Add static metadata
const enhancedQuery = traceable(
  async (query: string) => {
    return await process(query);
  },
  {
    name: 'enhancedQuery',
    metadata: { version: '2.0', model: 'gpt-4', team: 'ml' },
  }
);
```

### Type Safety

Full TypeScript support with preserved function signatures:

```typescript
import { traceable } from '@agentreplay/agentreplay';

interface ChatMessage {
  role: 'user' | 'assistant';
  content: string;
}

// Types are preserved
const chat = traceable(
  async (messages: ChatMessage[]): Promise<string> => {
    // Implementation
    return response;
  },
  { name: 'chat' }
);

// TypeScript knows the types!
const result: string = await chat([
  { role: 'user', content: 'Hello' }
]);
```

---

## 📐 Context Manager: `withSpan()`

For more control over span attributes and timing:

```typescript
import { withSpan, SpanKind } from '@agentreplay/agentreplay';

async function complexOperation(query: string) {
  return await withSpan('process_query', async (span) => {
    // Set input data
    span.setInput({ query, timestamp: Date.now() });
    
    // Nested span for document retrieval
    const docs = await withSpan('retrieve_documents', async (retrieverSpan) => {
      const results = await vectorDb.search(query, { topK: 5 });
      retrieverSpan.setOutput({ documentCount: results.length });
      retrieverSpan.setAttribute('vectorDb', 'pinecone');
      return results;
    }, { kind: SpanKind.RETRIEVER });
    
    // Nested span for LLM generation
    const response = await withSpan('generate_response', async (llmSpan) => {
      llmSpan.setModel('gpt-4', 'openai');
      const result = await generateResponse(query, docs);
      llmSpan.setTokenUsage({
        promptTokens: 150,
        completionTokens: 200,
        totalTokens: 350,
      });
      return result;
    }, { kind: SpanKind.LLM });
    
    // Add events for debugging
    span.addEvent('processing_complete', { docCount: docs.length });
    
    // Set final output
    span.setOutput({ response, sourceCount: docs.length });
    
    return { response, sources: docs };
  }, { kind: SpanKind.CHAIN });
}
```

### Manual Span Control

For cases where you need explicit control over span lifecycle:

```typescript
import { startSpan, SpanKind } from '@agentreplay/agentreplay';

async function longRunningOperation() {
  const span = startSpan('background_job', {
    kind: SpanKind.TOOL,
    input: { jobType: 'data_sync' },
  });
  
  try {
    // Long running work...
    for (let i = 0; i < 100; i++) {
      await processItem(i);
      if (i % 10 === 0) {
        span.addEvent('progress', { completed: i });
      }
    }
    
    span.setOutput({ itemsProcessed: 100 });
    span.setStatus('ok');
    
  } catch (error) {
    span.captureException(error as Error);
    span.setStatus('error');
    throw error;
    
  } finally {
    span.end();  // Always call end()
  }
}
```

---

## 🔌 LLM Client Wrappers

### OpenAI (Recommended)

One line to instrument all OpenAI calls:

```typescript
import OpenAI from 'openai';
import { init, wrapOpenAI, flush } from '@agentreplay/agentreplay';

init();

// Wrap the client - all calls are now traced automatically!
const openai = wrapOpenAI(new OpenAI());

// Use normally - tracing happens in the background
const response = await openai.chat.completions.create({
  model: 'gpt-4',
  messages: [
    { role: 'system', content: 'You are a helpful assistant.' },
    { role: 'user', content: 'Explain quantum computing in simple terms.' },
  ],
  temperature: 0.7,
});

console.log(response.choices[0].message.content);

// Embeddings are traced too
const embedding = await openai.embeddings.create({
  model: 'text-embedding-ada-002',
  input: 'Hello world',
});

await flush();
```

**Automatically captured:**
- Model name
- Input messages
- Output content
- Token usage (prompt, completion, total)
- Latency
- Finish reason
- Errors

### Anthropic

```typescript
import Anthropic from '@anthropic-ai/sdk';
import { init, wrapAnthropic, flush } from '@agentreplay/agentreplay';

init();

// Wrap the Anthropic client
const anthropic = wrapAnthropic(new Anthropic());

// Use normally
const message = await anthropic.messages.create({
  model: 'claude-3-opus-20240229',
  max_tokens: 1024,
  messages: [
    { role: 'user', content: 'Explain the theory of relativity.' },
  ],
});

console.log(message.content[0].text);
await flush();
```

### Disable Content Capture

For privacy-sensitive applications:

```typescript
// Don't capture message content, only metadata
const openai = wrapOpenAI(new OpenAI(), { captureContent: false });

// Traces will still include:
// - Model name
// - Token counts
// - Latency
// - Error information
// But NOT the actual messages or responses
```

### Fetch Instrumentation

Trace all HTTP requests:

```typescript
import { init, wrapFetch, installFetchTracing } from '@agentreplay/agentreplay';

init();

// Option 1: Wrap specific fetch instance
const tracedFetch = wrapFetch(fetch);
const response = await tracedFetch('https://api.example.com/data');

// Option 2: Install globally (affects all fetch calls)
installFetchTracing();

// Now all fetch calls are traced automatically
const data = await fetch('https://api.example.com/users');
```

---

## 🏷️ Context Management

### Global Context

Set context that applies to ALL subsequent traces:

```typescript
import { setGlobalContext, getGlobalContext } from '@agentreplay/agentreplay';

// Set user context (persists until cleared)
setGlobalContext({
  userId: 'user-123',
  sessionId: 'session-456',
  agentId: 'support-bot',
});

// Add more context later (merges with existing)
setGlobalContext({
  environment: 'production',
  version: '1.2.0',
  region: 'us-west-2',
});

// Get current global context
const context = getGlobalContext();
console.log(context);
// { userId: 'user-123', sessionId: 'session-456', ... }
```

### Request-Scoped Context

For web applications with per-request context:

```typescript
import { withContext } from '@agentreplay/agentreplay';

async function handleApiRequest(request: Request) {
  // Context only applies within this callback
  return await withContext(
    {
      userId: request.userId,
      requestId: request.headers.get('X-Request-ID'),
      path: new URL(request.url).pathname,
    },
    async () => {
      // All traces in here include this context
      const result = await processRequest(request);
      return result;
    }
  );
  // Context automatically cleared after callback
}
```

### Bind Context for Callbacks

Preserve context across async boundaries:

```typescript
import { bindContext, setGlobalContext } from '@agentreplay/agentreplay';

setGlobalContext({ requestId: 'req-123' });

// Bind current context to a callback
const boundCallback = bindContext(async () => {
  // This runs with the context from when bindContext was called
  // Even if called later in a different async context
  return await processAsync();
});

// Later, even in a different async context
setTimeout(boundCallback, 1000);

// Or with event emitters
emitter.on('data', bindContext(async (data) => {
  // Context preserved here
  await handleData(data);
}));
```

---

## 🔒 Privacy & Data Redaction

### Configure Privacy Settings

```typescript
import { configurePrivacy } from '@agentreplay/agentreplay';

configurePrivacy({
  // Enable built-in scrubbers for common PII
  enableBuiltinScrubbers: true,  // Emails, credit cards, SSNs, phones, API keys
  
  // Add custom regex patterns
  customPatterns: [
    /secret-\w+/gi,           // Custom secret format
    /internal-id-\d+/gi,      // Internal IDs
    /password:\s*\S+/gi,      // Password fields
  ],
  
  // Completely scrub these JSON paths
  scrubPaths: [
    'input.password',
    'input.credentials.apiKey',
    'output.user.ssn',
    'metadata.internalToken',
  ],
  
  // Hash PII instead of replacing with [REDACTED]
  // Allows tracking unique values without exposing data
  hashPii: true,
  hashSalt: 'your-secret-salt-here',
});
```

### Built-in Scrubbers

The SDK includes patterns for:

| Type | Example | Redacted As |
|------|---------|-------------|
| Email | user@example.com | [REDACTED] |
| Credit Card | 4111-1111-1111-1111 | [REDACTED] |
| SSN | 123-45-6789 | [REDACTED] |
| Phone (US) | +1-555-123-4567 | [REDACTED] |
| Phone (Intl) | +44-20-1234-5678 | [REDACTED] |
| API Key | sk-proj-abc123... | [REDACTED] |
| Bearer Token | Bearer eyJ... | [REDACTED] |
| JWT | eyJhbG... | [REDACTED] |
| IP Address | 192.168.1.1 | [REDACTED] |

### Manual Redaction

```typescript
import { redactPayload, hashPII } from '@agentreplay/agentreplay';

// Redact an entire payload
const data = {
  user: {
    email: 'john@example.com',
    phone: '+1-555-123-4567',
  },
  message: 'My credit card is 4111-1111-1111-1111',
  apiKey: 'sk-proj-abcdefghijk',
};

const safeData = redactPayload(data);
// Result:
// {
//   user: {
//     email: '[REDACTED]',
//     phone: '[REDACTED]',
//   },
//   message: 'My credit card is [REDACTED]',
//   apiKey: '[REDACTED]',
// }

// Hash for consistent anonymization (same input = same hash)
const userHash = hashPII('user@example.com');
// '[HASH:a1b2c3d4]'

// Useful for analytics without exposing PII
console.log(`User ${userHash} performed action`);
```

---

## 📊 Sampling

Control which traces are captured to manage costs and volume:

```typescript
import { configureSampling } from '@agentreplay/agentreplay';

configureSampling({
  // Sample 10% of traces (0.0 to 1.0)
  sampleRate: 0.1,
  
  // Always sample errors regardless of rate
  alwaysSampleErrors: true,
  
  // Always sample slow operations (>5 seconds)
  alwaysSampleSlowThreshold: 5000,
  
  // Fine-grained rules for specific operations
  rules: [
    // Always sample LLM calls (100%)
    { match: { kind: 'llm' }, sampleRate: 1.0 },
    
    // Sample 50% of retriever calls
    { match: { kind: 'retriever' }, sampleRate: 0.5 },
    
    // Never sample health checks
    { match: { name: /health|ping|ready/i }, sampleRate: 0 },
    
    // Sample by user for consistent experience
    { match: { userId: '*' }, sampleRate: 0.1, deterministic: true },
  ],
});
```

---

## 📊 Span Kinds

Use semantic span kinds for better visualization and filtering:

```typescript
import { SpanKind } from '@agentreplay/agentreplay';

// Available span kinds
SpanKind.CHAIN       // Orchestration, workflows, pipelines
SpanKind.LLM         // LLM API calls (OpenAI, Anthropic, etc.)
SpanKind.TOOL        // Tool/function calls, actions
SpanKind.RETRIEVER   // Vector DB search, document retrieval
SpanKind.EMBEDDING   // Embedding generation
SpanKind.GUARDRAIL   // Safety checks, content filtering
SpanKind.CACHE       // Cache operations
SpanKind.HTTP        // HTTP requests
SpanKind.DB          // Database queries
```

Example usage:

```typescript
import { traceable, SpanKind } from '@agentreplay/agentreplay';

const searchDocuments = traceable(
  async (query: string) => {
    return await vectorDb.similaritySearch(query, { k: 5 });
  },
  { name: 'searchDocuments', kind: SpanKind.RETRIEVER }
);

const generateAnswer = traceable(
  async (query: string, docs: Document[]) => {
    return await llm.generate(query, { context: docs });
  },
  { name: 'generateAnswer', kind: SpanKind.LLM }
);

const ragPipeline = traceable(
  async (query: string) => {
    const docs = await searchDocuments(query);
    return await generateAnswer(query, docs);
  },
  { name: 'ragPipeline', kind: SpanKind.CHAIN }
);
```

---

## ⚙️ Lifecycle Management

### Flushing Traces

Always ensure traces are sent before your application exits:

```typescript
import { init, flush, shutdown } from '@agentreplay/agentreplay';

init();

// Your application code...

// Option 1: Manual flush with timeout
await flush(10000);  // Wait up to 10 seconds

// Option 2: Full graceful shutdown
await shutdown(30000);  // Flush and cleanup

// Option 3: Process handlers (added automatically by init())
process.on('beforeExit', async () => {
  await flush();
});
```

### Serverless / AWS Lambda

**Critical**: Always flush explicitly before the function returns!

```typescript
import { init, traceable, flush } from '@agentreplay/agentreplay';

init();

const processEvent = traceable(
  async (event: any) => {
    // Your logic here
    return { processed: true };
  },
  { name: 'processEvent' }
);

export const handler = async (event: any, context: any) => {
  try {
    const result = await processEvent(event);
    return {
      statusCode: 200,
      body: JSON.stringify(result),
    };
  } finally {
    // CRITICAL: Flush before Lambda freezes
    await flush(5000);
  }
};
```

### Next.js / Vercel

```typescript
// instrumentation.ts (Next.js 13+)
import { init } from '@agentreplay/agentreplay';

export function register() {
  init();
}

// In your API route or Server Component
import { flush, withSpan } from '@agentreplay/agentreplay';

export async function POST(request: Request) {
  try {
    return await withSpan('api_chat', async (span) => {
      const body = await request.json();
      span.setInput(body);
      
      const result = await processChat(body);
      span.setOutput(result);
      
      return Response.json(result);
    });
  } finally {
    // Flush in edge/serverless
    await flush(5000);
  }
}
```

### Express.js Middleware

```typescript
import express from 'express';
import { init, withContext, flush } from '@agentreplay/agentreplay';

init();

const app = express();

// Add tracing context for each request
app.use((req, res, next) => {
  const requestId = req.headers['x-request-id'] as string || crypto.randomUUID();
  
  withContext(
    { 
      requestId, 
      path: req.path,
      method: req.method,
    },
    () => next()
  );
});

// Graceful shutdown
const server = app.listen(3000);

process.on('SIGTERM', async () => {
  server.close();
  await flush(10000);
  process.exit(0);
});
```

---

## 🔗 Framework Integrations

### LangChain.js

```typescript
import { ChatOpenAI } from '@langchain/openai';
import { init, traceable, SpanKind, flush } from '@agentreplay/agentreplay';

init();

const answerQuestion = traceable(
  async (question: string) => {
    const llm = new ChatOpenAI({ modelName: 'gpt-4', temperature: 0 });
    const response = await llm.invoke(question);
    return response.content;
  },
  { name: 'langchain_qa', kind: SpanKind.CHAIN }
);

const result = await answerQuestion('What is machine learning?');
await flush();
```

### Vercel AI SDK

```typescript
import { streamText } from 'ai';
import { openai } from '@ai-sdk/openai';
import { init, withSpan, SpanKind, flush } from '@agentreplay/agentreplay';

init();

export async function POST(request: Request) {
  const { prompt } = await request.json();
  
  return await withSpan('ai_stream', async (span) => {
    span.setInput({ prompt });
    span.setModel('gpt-4', 'openai');
    
    const result = await streamText({
      model: openai('gpt-4'),
      prompt,
      onFinish: async ({ usage }) => {
        span.setTokenUsage({
          promptTokens: usage.promptTokens,
          completionTokens: usage.completionTokens,
          totalTokens: usage.totalTokens,
        });
        await flush(2000);
      },
    });
    
    return result.toDataStreamResponse();
  }, { kind: SpanKind.LLM });
}
```

### Hono (Edge Runtime)

```typescript
import { Hono } from 'hono';
import { init, traceable, flush } from '@agentreplay/agentreplay';

init();

const app = new Hono();

const processMessage = traceable(
  async (message: string) => {
    return await callLLM(message);
  },
  { name: 'processMessage' }
);

app.post('/chat', async (c) => {
  try {
    const { message } = await c.req.json();
    const response = await processMessage(message);
    return c.json({ response });
  } finally {
    await flush(3000);
  }
});

export default app;
```

---

## 🌐 Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `AGENTREPLAY_API_KEY` | API key for authentication | **Required** |
| `AGENTREPLAY_PROJECT_ID` | Project identifier | **Required** |
| `AGENTREPLAY_BASE_URL` | API base URL | `https://api.agentreplay.io` |
| `AGENTREPLAY_TENANT_ID` | Tenant identifier | `default` |
| `AGENTREPLAY_AGENT_ID` | Default agent ID | `default` |
| `AGENTREPLAY_ENABLED` | Enable/disable tracing | `true` |
| `AGENTREPLAY_DEBUG` | Enable debug logging | `false` |
| `AGENTREPLAY_BATCH_SIZE` | Spans per batch | `100` |
| `AGENTREPLAY_FLUSH_INTERVAL` | Auto-flush interval (ms) | `5000` |
| `AGENTREPLAY_CAPTURE_INPUT` | Capture function inputs | `true` |
| `AGENTREPLAY_CAPTURE_OUTPUT` | Capture function outputs | `true` |

---

## 🧪 Testing

### Disable Tracing in Tests

```typescript
import { init, resetConfig } from '@agentreplay/agentreplay';

beforeAll(() => {
  init({ enabled: false });
});

afterAll(() => {
  resetConfig();
});

test('my function works', async () => {
  // Tracing is disabled, no network calls
  const result = await myTracedFunction('test');
  expect(result).toBe(expected);
});
```

Or use environment variable:

```bash
AGENTREPLAY_ENABLED=false npm test
```

### Mock the SDK

```typescript
import { jest } from '@jest/globals';

jest.mock('@agentreplay/agentreplay', () => ({
  init: jest.fn(),
  traceable: (fn: Function) => fn,  // Pass-through
  flush: jest.fn().mockResolvedValue(undefined),
}));
```

---

## 📦 Package Exports

The SDK provides multiple entry points for different use cases:

```typescript
// Main entry (recommended)
import { 
  init, 
  traceable, 
  wrapOpenAI,
  flush,
} from '@agentreplay/agentreplay';

// Types only (for TypeScript)
import type { 
  Span, 
  SpanKind, 
  Config,
  SamplingConfig,
  PrivacyConfig,
} from '@agentreplay/agentreplay';
```

---

## 📚 Complete API Reference

### Initialization

| Function | Description |
|----------|-------------|
| `init(config?)` | Initialize the SDK with configuration |
| `getConfig()` | Get current configuration |
| `resetConfig()` | Reset to defaults |

### Tracing

| Function | Description |
|----------|-------------|
| `traceable(fn, opts)` | Wrap a function for tracing |
| `withSpan(name, fn, opts)` | Execute callback with a span context |
| `startSpan(name, opts)` | Create a manual span |
| `captureException(error)` | Capture an error in current span |

### Client Wrappers

| Function | Description |
|----------|-------------|
| `wrapOpenAI(client, opts)` | Wrap OpenAI client |
| `wrapAnthropic(client, opts)` | Wrap Anthropic client |
| `wrapFetch(fetch, opts)` | Wrap fetch function |
| `installFetchTracing()` | Install global fetch tracing |

### Context

| Function | Description |
|----------|-------------|
| `setGlobalContext(ctx)` | Set global context |
| `getGlobalContext()` | Get current global context |
| `withContext(ctx, fn)` | Run callback with scoped context |
| `bindContext(fn)` | Bind current context to callback |

### Transport

| Function | Description |
|----------|-------------|
| `flush(timeout?)` | Flush pending spans |
| `shutdown(timeout?)` | Graceful shutdown |

### Privacy

| Function | Description |
|----------|-------------|
| `configurePrivacy(opts)` | Configure redaction settings |
| `redactPayload(data)` | Redact sensitive data from object |
| `hashPII(value, salt?)` | Hash PII for anonymization |

### Sampling

| Function | Description |
|----------|-------------|
| `configureSampling(opts)` | Configure sampling rules |

### Span Methods

| Method | Description |
|--------|-------------|
| `setInput(data)` | Set span input data |
| `setOutput(data)` | Set span output data |
| `setAttribute(key, value)` | Set a single attribute |
| `setAttributes(obj)` | Set multiple attributes |
| `addEvent(name, attrs)` | Add a timestamped event |
| `captureException(error)` | Record an error |
| `setTokenUsage(usage)` | Set LLM token counts |
| `setModel(model, provider)` | Set model information |
| `setStatus(status)` | Set span status |
| `end()` | End the span |

---

## 🤝 Contributing

We welcome contributions! See [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.

```bash
# Clone the repository
git clone https://github.com/agentreplay/agentreplay.git
cd agentreplay/sdks/js

# Install dependencies
npm install

# Build
npm run build

# Run tests
npm test

# Run linter
npm run lint

# Type check
npm run typecheck

# Format code
npm run format
```

---

## 📄 License

Apache 2.0 - see [LICENSE](../../LICENSE) for details.

---

## 🔗 Links

- 📖 [Documentation](https://docs.agentreplay.io)
- 💻 [GitHub Repository](https://github.com/agentreplay/agentreplay)
- 📦 [npm Package](https://www.npmjs.com/package/@agentreplay/agentreplay)
- 💬 [Discord Community](https://discord.gg/agentreplay)
- 🐦 [Twitter](https://twitter.com/agentreplay)

---

<p align="center">
  Made with ❤️ by the Agentreplay team
</p>
