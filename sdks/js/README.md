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

## Quick Start (1-Minute Win)

```typescript
import { init, wrapOpenAI, flush } from '@agentreplay/sdk';
import OpenAI from 'openai';

// Initialize - reads AGENTREPLAY_URL, AGENTREPLAY_API_KEY from env
init();

// Wrap OpenAI for automatic tracing
const openai = wrapOpenAI(new OpenAI());

// All calls are now automatically traced!
const response = await openai.chat.completions.create({
  model: 'gpt-4o',
  messages: [{ role: 'user', content: 'Hello!' }]
});

// Flush before serverless function exits
await flush();
```

## Environment Variables

```bash
AGENTREPLAY_API_KEY=ar_xxx          # API key (optional for local)
AGENTREPLAY_URL=http://localhost:8080  # Server URL
AGENTREPLAY_TENANT_ID=1             # Tenant ID
AGENTREPLAY_PROJECT_ID=0            # Project ID
AGENTREPLAY_ENVIRONMENT=production  # Environment name
AGENTREPLAY_DEBUG=1                 # Enable debug logging
AGENTREPLAY_SAMPLING_RATE=0.1       # Sample 10% of traces
```

## Configuration

```typescript
import { init } from '@agentreplay/sdk';

init({
  apiKey: 'ar_xxx',                    // Or use env var
  baseUrl: 'https://api.agentreplay.dev',
  tenantId: 1,
  projectId: 0,
  environment: 'production',
  debug: false,
  strict: true,                        // Throw if API key missing

  // Sampling
  sampling: {
    rate: 0.1,                         // Sample 10%
    rules: [
      { when: { error: true }, sample: 1.0 },      // Always sample errors
      { when: { tag: 'vip' }, sample: 1.0 },       // Always sample VIPs
    ],
    deterministicKey: 'userId',        // Stable sampling per user
  },

  // Privacy - client-side redaction
  privacy: {
    mode: 'redact',
    redact: ['messages.*.content', 'input.apiKey'],
    scrubbers: [emailScrubber, creditCardScrubber],
  },

  // Transport
  transport: {
    mode: 'batch',                     // 'batch' | 'immediate' | 'console'
    batchSize: 100,
    flushIntervalMs: 5000,
    maxQueueSize: 10000,
    maxRetries: 3,
  },
});
```

## Tracing Functions

### traceable() - Wrap any function

```typescript
import { traceable } from '@agentreplay/sdk';

const processQuery = traceable(
  async (query: string) => {
    const result = await someOperation(query);
    return result;
  },
  { name: 'process_query', kind: 'chain' }
);

// Call it like normal
const result = await processQuery('What is the weather?');
```

### withSpan() - Scoped spans with access

```typescript
import { withSpan } from '@agentreplay/sdk';

const docs = await withSpan(
  'retrieve_context',
  { kind: 'retriever', input: { query } },
  async (span) => {
    const results = await vectorDb.search(query);
    span.setOutput({ count: results.length });
    span.addEvent('search_complete');
    return results;
  }
);
```

### startSpan() - Manual control

```typescript
import { startSpan } from '@agentreplay/sdk';

const span = startSpan('web_search', { kind: 'tool' });
try {
  const result = await searchWeb(query);
  span.end({ output: result });
} catch (err) {
  span.end({ error: err });
  throw err;
}
```

## Auto-Instrumentation

### OpenAI

```typescript
import { wrapOpenAI } from '@agentreplay/sdk';
import OpenAI from 'openai';

const openai = wrapOpenAI(new OpenAI(), {
  recordInput: true,      // Record prompts
  recordOutput: true,     // Record completions
  recordParameters: true, // Record temperature, etc.
});

// All chat.completions.create and embeddings.create calls traced
const response = await openai.chat.completions.create({...});
```

### Anthropic

```typescript
import { wrapAnthropic } from '@agentreplay/sdk';
import Anthropic from '@anthropic-ai/sdk';

const anthropic = wrapAnthropic(new Anthropic());

const response = await anthropic.messages.create({...});
```

### Fetch

```typescript
import { wrapFetch, installFetchTracing } from '@agentreplay/sdk';

// Option 1: Wrap specific fetch
const tracedFetch = wrapFetch(fetch, {
  excludeUrls: ['/health', /\.css$/],
});

// Option 2: Install globally
installFetchTracing({ excludeUrls: ['/health'] });
```

## Context Propagation

```typescript
import { setGlobalContext, withContext, bindContext } from '@agentreplay/sdk';

// Set global context (attached to all spans)
setGlobalContext({
  userId: 'user_123',
  sessionId: 456,
  tags: { tier: 'premium' },
});

// Run with specific context
await withContext({ traceId: 'custom-trace-id' }, async () => {
  // Spans created here use this context
});

// Bind context to callbacks
const boundHandler = bindContext(myHandler);
eventEmitter.on('data', boundHandler);
```

## Privacy & Redaction

```typescript
import {
  redactPayload,
  hashPII,
  emailScrubber,
  creditCardScrubber,
  apiKeyScrubber,
} from '@agentreplay/sdk';

// Configure in init()
init({
  privacy: {
    mode: 'redact',
    redact: ['messages.*.content', 'user.email'],
    scrubbers: [emailScrubber, creditCardScrubber, apiKeyScrubber],
  },
});

// Or manually
const redacted = redactPayload(sensitiveData);
const hashed = hashPII('user@example.com'); // -> 'hash_abc123'
```

## Serverless & Edge

```typescript
import { init, flush, shutdown } from '@agentreplay/sdk';

export async function handler(event) {
  // Init once (idempotent)
  init();

  // Your logic here...

  // IMPORTANT: Flush before function exits
  await flush({ timeoutMs: 5000 });

  return response;
}

// For graceful shutdown
process.on('SIGTERM', async () => {
  await shutdown();
  process.exit(0);
});
```

## Debugging

```typescript
// Enable debug mode
init({ debug: true });
// Or: AGENTREPLAY_DEBUG=1

// Dry run (prints to console, doesn't send)
init({
  transport: { mode: 'console' }
});

// Health check
const client = new AgentreplayClient({...});
const result = await client.ping();
if (!result.success) {
  console.error('Connection failed:', result.error);
}
```

## Span Kinds

```typescript
// Available span kinds for categorization
type SpanKind =
  | 'request'    // Root request
  | 'chain'      // Workflow/chain
  | 'llm'        // LLM call
  | 'tool'       // Tool/function
  | 'retriever'  // Vector DB
  | 'embedding'  // Embeddings
  | 'guardrail'  // Safety check
  | 'cache'      // Cache ops
  | 'http'       // HTTP request
  | 'db';        // Database
```
