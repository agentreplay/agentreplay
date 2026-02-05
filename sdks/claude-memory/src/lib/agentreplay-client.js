/*
 * MemoryService - HTTP client for Agent Replay local server
 * Handles all communication with the memory backend
 */

const http = require('node:http');
const https = require('node:https');

// Server connection defaults
const DEFAULTS = Object.freeze({
  endpoint: 'http://localhost:47100',
  tenant: 1,
  project: 1,
  requestTimeout: 30000,
  collection: 'default',
});

class MemoryService {
  #baseUrl;
  #tenantId;
  #projectId;
  #defaultCollection;
  #timeoutMs;

  constructor(config = {}) {
    this.#baseUrl = this.#normalizeEndpoint(
      config.endpoint ?? process.env.AGENTREPLAY_URL ?? DEFAULTS.endpoint
    );
    this.#tenantId = Number(config.tenant ?? process.env.AGENTREPLAY_TENANT_ID ?? DEFAULTS.tenant);
    this.#projectId = Number(config.project ?? process.env.AGENTREPLAY_PROJECT_ID ?? DEFAULTS.project);
    this.#defaultCollection = config.collection ?? DEFAULTS.collection;
    this.#timeoutMs = config.timeout ?? DEFAULTS.requestTimeout;
  }

  #normalizeEndpoint(url) {
    return String(url).replace(/\/+$/, '');
  }

  get endpoint() {
    return this.#baseUrl;
  }

  // Verify server is reachable
  async ping() {
    try {
      await this.#httpCall('GET', '/api/v1/health');
      return { ok: true };
    } catch (e) {
      return { ok: false, reason: e.message };
    }
  }

  // Store content in memory
  async store(text, collection, meta = {}, docId = null) {
    const requestBody = {
      content: text,
      collection: collection ?? this.#defaultCollection,
      metadata: { origin: 'claude-plugin', ...meta },
    };
    if (docId) requestBody.custom_id = docId;

    const response = await this.#httpCall('POST', '/api/v1/memory/ingest', requestBody);
    return {
      documentId: response.document_id,
      stored: response.success === true,
      collection: collection ?? this.#defaultCollection,
      chunks: response.chunks_created ?? 0,
      vectors: response.vectors_stored ?? 0,
    };
  }

  // Find similar memories
  async find(queryText, collection, opts = {}) {
    const requestBody = {
      query: queryText,
      collection: collection ?? this.#defaultCollection,
      limit: opts.maxResults ?? 10,
      min_score: opts.threshold ?? 0.0,
    };

    const response = await this.#httpCall('POST', '/api/v1/memory/retrieve', requestBody);
    const items = Array.isArray(response.results) ? response.results : [];

    return {
      matches: items.map((item) => ({
        docId: item.document_id,
        text: item.content ?? '',
        relevance: item.score,
        meta: item.metadata,
        segment: item.chunk_index,
      })),
      count: response.total_results ?? items.length,
      query: queryText,
    };
  }

  // Build context profile from memories
  async buildProfile(collection, searchQuery) {
    const searchResult = await this.find(searchQuery ?? '', collection, { maxResults: 20 });

    const preferences = [];
    const recentContext = [];

    for (const match of searchResult.matches) {
      const metaType = match.meta?.type;
      const isPreference = metaType === 'preference' || metaType === 'convention';
      if (isPreference) {
        preferences.push(match.text);
      } else {
        recentContext.push(match.text);
      }
    }

    return {
      preferences: preferences.slice(0, 10),
      context: recentContext.slice(0, 10),
      related: searchResult,
    };
  }

  // Get all memories from collection
  async browse(collection, count = 20) {
    const result = await this.find('', collection, { maxResults: count });
    return { items: result.matches };
  }

  // Server statistics
  async statistics() {
    return this.#httpCall('GET', '/api/v1/memory/stats');
  }

  // Collection metadata
  async metadata() {
    return this.#httpCall('GET', '/api/v1/memory/info');
  }

  // Execute HTTP request
  async #httpCall(verb, path, payload = null) {
    const fullUrl = `${this.#baseUrl}${path}`;
    const abort = new AbortController();
    const timer = setTimeout(() => abort.abort(), this.#timeoutMs);

    try {
      const reqOptions = {
        method: verb,
        headers: {
          'Content-Type': 'application/json',
          'X-Tenant-ID': String(this.#tenantId),
          'X-Project-ID': String(this.#projectId),
        },
        signal: abort.signal,
      };

      if (payload !== null) {
        reqOptions.body = JSON.stringify(payload);
      }

      const res = await fetch(fullUrl, reqOptions);
      if (!res.ok) {
        const errBody = await res.text();
        throw new Error(`HTTP ${res.status}: ${errBody}`);
      }
      return res.json();
    } finally {
      clearTimeout(timer);
    }
  }
}

module.exports = { MemoryService };
