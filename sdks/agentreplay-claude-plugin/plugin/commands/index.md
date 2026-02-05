---
description: Index codebase into Agent Replay for persistent local context
allowed-tools: ["Read", "Glob", "Grep", "Bash", "Task"]
---

# Codebase Indexing

Analyze this codebase and save findings to Agent Replay's local memory.

## Phase 1: Project Overview

Read:
- `package.json` / `Cargo.toml` / `pyproject.toml` / `go.mod`
- `README.md`
- Config files (tsconfig, eslint, etc.)

Collect: project name, purpose, tech stack, build/run/test commands

## Phase 2: Architecture

Explore:
- Use Glob for folder structure
- Find entry points (index.ts, main.py, App.tsx)
- Identify API routes, database models

Collect: architecture overview, key modules, data flow

## Phase 3: Conventions

Analyze:
- Naming conventions
- File organization patterns
- Import patterns
- Recent commits: `git log --oneline -20`

Collect: coding style, patterns to follow

## Phase 4: Key Components

Read:
- Auth logic
- Database connections
- API clients
- Shared utilities

Collect: where important logic lives

## Final: Save to Memory

Compile findings into a comprehensive summary and save using the memory API.

The summary should include:
- Project name and purpose
- Tech stack
- Architecture overview
- Coding conventions
- Key file locations
