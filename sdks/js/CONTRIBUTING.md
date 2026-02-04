# Contributing to @agentreplay/agentreplay

Thank you for your interest in contributing! This guide will help you get started.

## Development Setup

### Prerequisites

- Node.js 18+
- npm 9+

### Getting Started

```bash
# Clone the repository
git clone https://github.com/agentreplay/agentreplay.git
cd agentreplay/sdks/js

# Install dependencies
npm install

# Build the package
npm run build

# Run tests
npm test

# Type check
npm run typecheck

# Lint
npm run lint
```

## Project Structure

```
sdks/js/
├── src/
│   ├── index.ts          # Main exports
│   ├── client.ts         # AgentreplayClient class
│   ├── config.ts         # init() and configuration
│   ├── context.ts        # AsyncLocalStorage context
│   ├── transport.ts      # Batching and HTTP transport
│   ├── traceable.ts      # traceable(), withSpan(), startSpan()
│   ├── privacy.ts        # Redaction and scrubbers
│   ├── sampling.ts       # Sampling logic
│   ├── types.ts          # TypeScript types
│   └── wrappers/         # Auto-instrumentation
│       ├── openai.ts     # OpenAI wrapper
│       └── fetch.ts      # Fetch wrapper
├── examples/
│   ├── basic.ts          # Original API example
│   └── modern-api.ts     # New API example
├── dist/                 # Build output (generated)
├── package.json
├── tsconfig.json
└── README.md
```

## Development Workflow

### Making Changes

1. Create a feature branch: `git checkout -b feature/my-feature`
2. Make your changes
3. Run checks: `npm run typecheck && npm test && npm run lint`
4. Commit with conventional commits: `feat: add new feature`
5. Push and create a PR

### Commit Message Format

We use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation only
- `refactor:` - Code refactoring
- `test:` - Adding tests
- `chore:` - Maintenance

### Running Tests

```bash
# Run all tests
npm test

# Run in watch mode
npm run test:watch

# Run with coverage
npm run test:coverage
```

### Building

```bash
# Build for production
npm run build

# Build in watch mode (development)
npm run dev
```

## Code Style

- TypeScript strict mode
- Prefer `const` over `let`
- Use explicit return types for public APIs
- Document public APIs with JSDoc
- Keep files focused and small

## Adding a New Wrapper

To add auto-instrumentation for a new library:

1. Create `src/wrappers/mylib.ts`
2. Export `wrapMyLib()` function
3. Add to `src/wrappers/index.ts`
4. Add to `src/index.ts` exports
5. Add example in `examples/`
6. Update README.md

## Questions?

Open an issue or discussion on GitHub!
