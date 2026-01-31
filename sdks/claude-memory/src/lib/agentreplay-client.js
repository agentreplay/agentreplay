/**
 * Agent Replay Client for Claude Code Plugin
 * 
 * Provides memory storage and retrieval via the local Agent Replay server.
 * Unlike cloud-based solutions, all data stays on your machine.
 */

const {
  validateUrl,
  validateContainerTag,
} = require('./validate.js');

const DEFAULT_URL = 'http://localhost:9600';
const DEFAULT_TENANT_ID = 1;
const DEFAULT_PROJECT_ID = 1;

class AgentReplayClient {
  constructor(options = {}) {
    this.url = (options.url || process.env.AGENTREPLAY_URL || DEFAULT_URL).replace(/\/$/, '');
    this.tenantId = options.tenantId || parseInt(process.env.AGENTREPLAY_TENANT_ID || DEFAULT_TENANT_ID, 10);
    this.projectId = options.projectId || parseInt(process.env.AGENTREPLAY_PROJECT_ID || DEFAULT_PROJECT_ID, 10);
    this.containerTag = options.containerTag || 'default';
    this.timeout = options.timeout || 30000;
    
    const urlCheck = validateUrl(this.url);
    if (!urlCheck.valid) {
      console.warn(`URL warning: ${urlCheck.reason}`);
    }
  }

  /**
   * Check if Agent Replay server is running
   */
  async healthCheck() {
    try {
      const response = await this._request('GET', '/api/v1/health');
      return { healthy: true, ...response };
    } catch (err) {
      return { healthy: false, error: err.message };
    }
  }

  /**
   * Add a memory/observation to Agent Replay
   */
  async addMemory(content, containerTag, metadata = {}, customId = null) {
    const payload = {
      content,
      collection: containerTag || this.containerTag,
      metadata: {
        source: 'claude-code-plugin',
        ...metadata,
      },
    };

    if (customId) {
      payload.custom_id = customId;
    }

    const result = await this._request('POST', '/api/v1/memory/ingest', payload);
    return {
      id: result.document_id,
      success: result.success,
      containerTag: containerTag || this.containerTag,
      chunksCreated: result.chunks_created,
      vectorsStored: result.vectors_stored,
    };
  }

  /**
   * Search memories by semantic similarity
   */
  async search(query, containerTag, options = {}) {
    const payload = {
      query,
      collection: containerTag || this.containerTag,
      limit: options.limit || 10,
      min_score: options.minScore || 0.0,
    };

    const result = await this._request('POST', '/api/v1/memory/retrieve', payload);
    return {
      results: (result.results || []).map((r) => ({
        id: r.document_id,
        content: r.content || '',
        score: r.score,
        metadata: r.metadata,
        chunkIndex: r.chunk_index,
      })),
      total: result.total_results,
      query: result.query,
      collection: result.collection,
    };
  }

  /**
   * Get profile/context for a workspace
   * Returns both static preferences and recent dynamic context
   */
  async getProfile(containerTag, query) {
    // For Agent Replay, we do a semantic search and categorize results
    const searchResult = await this.search(query || '', containerTag, { limit: 20 });
    
    // Categorize results into static (persistent) and dynamic (recent)
    const staticFacts = [];
    const dynamicFacts = [];
    
    for (const r of searchResult.results) {
      const metadata = r.metadata || {};
      if (metadata.type === 'preference' || metadata.type === 'convention') {
        staticFacts.push(r.content);
      } else {
        dynamicFacts.push(r.content);
      }
    }
    
    return {
      profile: {
        static: staticFacts.slice(0, 10),
        dynamic: dynamicFacts.slice(0, 10),
      },
      searchResults: searchResult,
    };
  }

  /**
   * List memories in a collection
   */
  async listMemories(containerTag, limit = 20) {
    const result = await this.search('', containerTag, { limit });
    return { memories: result.results };
  }

  /**
   * Get memory statistics
   */
  async getStats() {
    return this._request('GET', '/api/v1/memory/stats');
  }

  /**
   * Get memory info and collections
   */
  async getInfo() {
    return this._request('GET', '/api/v1/memory/info');
  }

  /**
   * Internal HTTP request helper
   */
  async _request(method, path, body = null) {
    const url = `${this.url}${path}`;
    
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeout);
    
    try {
      const options = {
        method,
        headers: {
          'Content-Type': 'application/json',
          'X-Tenant-ID': String(this.tenantId),
          'X-Project-ID': String(this.projectId),
        },
        signal: controller.signal,
      };
      
      if (body) {
        options.body = JSON.stringify(body);
      }
      
      const response = await fetch(url, options);
      
      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`Agent Replay API error (${response.status}): ${errorText}`);
      }
      
      return await response.json();
    } finally {
      clearTimeout(timeoutId);
    }
  }
}

module.exports = { AgentReplayClient };
