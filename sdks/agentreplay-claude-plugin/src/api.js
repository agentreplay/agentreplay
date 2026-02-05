/*
 * HTTP client for Agent Replay server
 * Handles both tracing and memory APIs
 */

const crypto = require('node:crypto');
const nodeFs = require('node:fs');
const nodePath = require('node:path');
const nodeOs = require('node:os');

// Generate random hex ID
function randomHexId(bytes = 8) {
  return '0x' + crypto.randomBytes(bytes).toString('hex');
}

// Get current time in microseconds
function nowMicros() {
  return BigInt(Date.now()) * 1000n;
}

// Cache file for project ID
function getProjectCachePath() {
  const cacheDir = nodePath.join(nodeOs.homedir(), '.agentreplay');
  if (!nodeFs.existsSync(cacheDir)) {
    nodeFs.mkdirSync(cacheDir, { recursive: true });
  }
  return nodePath.join(cacheDir, 'claude-code-project.json');
}

// Deterministic project ID for "Claude Code" - same on all installations
// Hash "Claude Code" to a consistent u16 value
function getClaudeCodeProjectId() {
  const name = 'Claude Code';
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = ((hash << 5) - hash + name.charCodeAt(i)) & 0xFFFF;
  }
  // Ensure non-zero (reserve 0 for "unassigned")
  return hash || 1;
}

// Well-known project ID for Claude Code sessions
const CLAUDE_CODE_PROJECT_ID = getClaudeCodeProjectId(); // = 46947

class AgentReplayAPI {
  #endpoint;
  #tenantId;
  #projectId;
  #timeout;
  #currentTraceId;
  #currentSessionId;
  #projectInitialized;

  constructor(config) {
    this.#endpoint = String(config.serverUrl || 'http://localhost:47100').replace(/\/+$/, '');
    this.#tenantId = Number(config.tenantId || 1);
    this.#projectId = null; // Will be resolved lazily
    this.#timeout = config.timeout || 30000;
    this.#currentTraceId = randomHexId(16);
    this.#currentSessionId = process.env.CLAUDE_SESSION_ID || randomHexId(8);
    this.#projectInitialized = false;
  }

  get baseUrl() {
    return this.#endpoint;
  }

  get traceId() {
    return this.#currentTraceId;
  }

  get projectId() {
    return this.#projectId;
  }

  // -------------------------------------------------------------------------
  // Project Management - Use deterministic "Claude Code" project ID
  // -------------------------------------------------------------------------

  async ensureProject() {
    if (this.#projectInitialized && this.#projectId) {
      return this.#projectId;
    }

    // Use deterministic project ID (same on all installations)
    this.#projectId = CLAUDE_CODE_PROJECT_ID;
    this.#projectInitialized = true;

    // Try to register the project if it doesn't exist (fire and forget)
    this.#registerProjectIfNeeded().catch(() => {});

    return this.#projectId;
  }

  async #registerProjectIfNeeded() {
    try {
      // Check if project already registered
      const res = await this.#callRaw('GET', '/api/v1/projects');
      const projects = res.projects || [];
      const existing = projects.find(p => 
        Number(p.project_id) === CLAUDE_CODE_PROJECT_ID ||
        p.name === 'Claude Code'
      );

      if (!existing) {
        // Register the project with our deterministic ID
        // Note: Server will generate its own ID, but traces use our ID from attributes
        await this.#callRaw('POST', '/api/v1/projects', {
          name: 'Claude Code',
          description: 'Claude Code coding sessions'
        });
      }
    } catch (e) {
      // Ignore - traces will still work, just project metadata won't be registered
    }
  }

  // -------------------------------------------------------------------------
  // Health
  // -------------------------------------------------------------------------

  async ping() {
    try {
      await this.#callRaw('GET', '/api/v1/health');
      return { ok: true };
    } catch (e) {
      return { ok: false, error: e.message };
    }
  }

  // -------------------------------------------------------------------------
  // Tracing API - Using proper span format
  // -------------------------------------------------------------------------

  async sendSpan(name, attributes = {}, parentSpanId = null, startTime = null, endTime = null) {
    // Ensure project exists before sending spans
    await this.ensureProject();
    
    const spanId = randomHexId(8);
    const now = nowMicros();
    
    const span = {
      span_id: spanId,
      trace_id: this.#currentTraceId,
      parent_span_id: parentSpanId,
      name: name,
      start_time: Number(startTime || now),
      end_time: endTime ? Number(endTime) : Number(now + 1000n),
      attributes: {
        // Use exact attribute names the server expects
        'agent_id': 'claude-code',
        'session_id': this.#currentSessionId,
        'tenant_id': String(this.#tenantId),
        'project_id': String(this.#projectId),
        ...Object.fromEntries(
          Object.entries(attributes).map(([k, v]) => [k, String(v)])
        ),
      },
    };

    const body = { spans: [span] };
    
    try {
      await this.#call('POST', '/api/v1/traces', body);
      return spanId;
    } catch (e) {
      // Don't fail silently - log but continue
      console.error(`[AgentReplay] Trace send failed: ${e.message}`);
      return null;
    }
  }

  async sendRootSpan(workspace, project) {
    return this.sendSpan('session.start', {
      'event.type': 'session_start',
      'workspace.path': workspace || '',
      'project.name': project || '',
    });
  }

  async sendToolSpan(toolName, input, output, durationMs = null, parentSpanId = null) {
    const startTime = nowMicros();
    const endTime = durationMs 
      ? startTime + BigInt(durationMs * 1000) 
      : startTime + 1000n;
    
    return this.sendSpan(`tool.${toolName}`, {
      'tool.name': toolName,
      'tool.input': this.#truncate(JSON.stringify(input), 2000),
      'tool.output': this.#truncate(JSON.stringify(output), 2000),
      'tool.duration_ms': String(durationMs || 0),
    }, parentSpanId, startTime, endTime);
  }

  async sendEndSpan(reason = 'normal', parentSpanId = null) {
    return this.sendSpan('session.end', {
      'event.type': 'session_end',
      'session.end_reason': reason,
    }, parentSpanId);
  }

  // -------------------------------------------------------------------------
  // Memory API
  // -------------------------------------------------------------------------

  async storeMemory(content, collection, meta = {}, docId = null) {
    const body = {
      content,
      collection,
      metadata: { origin: 'claude-plugin', ...meta },
    };
    if (docId) body.custom_id = docId;

    const res = await this.#call('POST', '/api/v1/memory/ingest', body);
    return {
      docId: res.document_id,
      stored: res.success === true,
      chunks: res.chunks_created || 0,
    };
  }

  async searchMemory(query, collection, limit = 10) {
    const body = { query, collection, limit, min_score: 0.0 };
    const res = await this.#call('POST', '/api/v1/memory/retrieve', body);
    const items = Array.isArray(res.results) ? res.results : [];
    return {
      matches: items.map((r) => ({
        docId: r.document_id,
        text: r.content || '',
        score: r.score,
        meta: r.metadata,
      })),
      total: res.total_results || items.length,
    };
  }

  async getProfile(collection, query = '') {
    const search = await this.searchMemory(query, collection, 20);
    const prefs = [];
    const ctx = [];

    for (const m of search.matches) {
      const kind = m.meta?.type || m.meta?.kind;
      if (kind === 'preference' || kind === 'convention') {
        prefs.push(m.text);
      } else {
        ctx.push(m.text);
      }
    }

    return {
      preferences: prefs.slice(0, 10),
      context: ctx.slice(0, 10),
      search,
    };
  }

  // -------------------------------------------------------------------------
  // Internals
  // -------------------------------------------------------------------------

  // Raw call without project_id (used for project listing/creation)
  async #callRaw(method, path, body = null) {
    const url = `${this.#endpoint}${path}`;
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), this.#timeout);

    try {
      const opts = {
        method,
        headers: {
          'Content-Type': 'application/json',
          'X-Tenant-ID': String(this.#tenantId),
        },
        signal: ctrl.signal,
      };
      if (body) opts.body = JSON.stringify(body);

      const res = await fetch(url, opts);
      if (!res.ok) {
        const txt = await res.text();
        throw new Error(`HTTP ${res.status}: ${txt}`);
      }
      return res.json();
    } finally {
      clearTimeout(timer);
    }
  }

  // Call with project_id header
  async #call(method, path, body = null) {
    // Ensure project is initialized
    if (!this.#projectId) {
      await this.ensureProject();
    }

    const url = `${this.#endpoint}${path}`;
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), this.#timeout);

    try {
      const opts = {
        method,
        headers: {
          'Content-Type': 'application/json',
          'X-Tenant-ID': String(this.#tenantId),
          'X-Project-ID': String(this.#projectId),
        },
        signal: ctrl.signal,
      };
      if (body) opts.body = JSON.stringify(body);

      const res = await fetch(url, opts);
      if (!res.ok) {
        const txt = await res.text();
        throw new Error(`HTTP ${res.status}: ${txt}`);
      }
      return res.json();
    } finally {
      clearTimeout(timer);
    }
  }

  #truncate(s, max) {
    return s && s.length > max ? s.slice(0, max) + '...' : s;
  }
}

module.exports = { AgentReplayAPI };
