# Changelog

All notable changes to the @agentreplay/agentreplay package will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-02-04

### Added

#### Core Features
- `init()` - Environment-first configuration with explicit overrides
- `shutdown()` / `flush()` - Graceful shutdown and serverless flush
- `traceable(fn, options)` - Wrap any function for automatic tracing
- `withSpan(name, options, fn)` - Scoped spans with full access
- `startSpan(name, options)` - Manual span control
- `captureException(err, context)` - Error capture helper

#### Auto-Instrumentation
- `wrapOpenAI(client)` - Automatic tracing for OpenAI SDK
- `wrapAnthropic(client)` - Automatic tracing for Anthropic SDK
- `wrapFetch(fetch)` - HTTP request tracing
- `installFetchTracing()` - Global fetch instrumentation

#### Context Propagation
- AsyncLocalStorage-based context for Node.js
- `setGlobalContext()` - Attach userId, sessionId, tags to all spans
- `withContext()` - Run functions with specific context
- `bindContext()` - Bind context to callbacks
- Automatic span nesting under concurrent requests

#### Batching & Transport
- Batch queue with configurable size and time-based flushing
- Exponential backoff with jitter for retries
- Bounded queue with backpressure (configurable maxQueueSize)
- Graceful shutdown with SIGTERM/SIGINT handlers
- Console mode for local debugging (`transport.mode: 'console'`)

#### Privacy & Redaction
- Path-based redaction (e.g., `messages.*.content`)
- Built-in scrubbers: email, credit card, API key, phone, SSN
- `hashPII()` helper for anonymization
- Payload size limits with truncation
- Safe JSON serialization (handles circular refs)

#### Sampling
- Base rate sampling (0.0 - 1.0)
- Conditional rules (error, tags, path prefix)
- Deterministic sampling by userId for stable cohorts
- Always/never sample helpers

#### Types
- Full TypeScript support with strict types
- ESM and CommonJS dual builds
- Span kinds: request, chain, llm, tool, retriever, embedding, guardrail, cache, http, db

### Configuration Options

```typescript
init({
  apiKey: 'ar_xxx',
  baseUrl: 'https://api.agentreplay.dev',
  tenantId: 1,
  projectId: 0,
  environment: 'production',
  debug: false,
  strict: true,
  timeout: 30000,
  sampling: { rate: 0.1, rules: [...] },
  privacy: { mode: 'redact', redact: [...], scrubbers: [...] },
  transport: { mode: 'batch', batchSize: 100, ... },
});
```

### Environment Variables

- `AGENTREPLAY_API_KEY` - API key
- `AGENTREPLAY_URL` - Server URL
- `AGENTREPLAY_TENANT_ID` - Tenant ID
- `AGENTREPLAY_PROJECT_ID` - Project ID
- `AGENTREPLAY_ENVIRONMENT` - Environment name
- `AGENTREPLAY_DEBUG` - Enable debug logging
- `AGENTREPLAY_SAMPLING_RATE` - Base sampling rate

[Unreleased]: https://github.com/agentreplay/agentreplay/compare/js-v0.1.0...HEAD
[0.1.0]: https://github.com/agentreplay/agentreplay/releases/tag/js-v0.1.0
