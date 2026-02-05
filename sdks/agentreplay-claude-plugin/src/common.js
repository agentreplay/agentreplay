/*
 * Shared utilities for Agent Replay Claude plugin
 */

const nodeOs = require('node:os');
const nodePath = require('node:path');
const nodeFs = require('node:fs');
const nodeCrypto = require('node:crypto');

// ============================================================================
// Configuration
// ============================================================================

const CONFIG_DIR = nodePath.join(nodeOs.homedir(), '.agentreplay-claude');
const CONFIG_FILE = nodePath.join(CONFIG_DIR, 'config.json');
const STATE_DIR = nodePath.join(CONFIG_DIR, 'state');

const DEFAULTS = Object.freeze({
  serverUrl: 'http://localhost:47100',
  tenantId: 1,
  projectId: 1,
  tracingEnabled: true,
  memoryEnabled: true,
  verbose: false,
  contextLimit: 5,
  ignoredTools: ['Read', 'Glob', 'Grep', 'TodoWrite', 'LS'],
});

function loadConfig() {
  const cfg = { ...DEFAULTS };

  // Load from file
  try {
    if (nodeFs.existsSync(CONFIG_FILE)) {
      Object.assign(cfg, JSON.parse(nodeFs.readFileSync(CONFIG_FILE, 'utf8')));
    }
  } catch {}

  // Environment overrides
  const env = process.env;
  if (env.AGENTREPLAY_URL) cfg.serverUrl = env.AGENTREPLAY_URL;
  if (env.AGENTREPLAY_TENANT_ID) cfg.tenantId = Number(env.AGENTREPLAY_TENANT_ID);
  if (env.AGENTREPLAY_PROJECT_ID) cfg.projectId = Number(env.AGENTREPLAY_PROJECT_ID);
  if (env.AGENTREPLAY_TRACING === 'false') cfg.tracingEnabled = false;
  if (env.AGENTREPLAY_MEMORY === 'false') cfg.memoryEnabled = false;
  if (env.AGENTREPLAY_DEBUG === 'true' || env.AGENTREPLAY_DEBUG === '1') cfg.verbose = true;

  return cfg;
}

function log(cfg, tag, data = null) {
  if (!cfg.verbose) return;
  const ts = new Date().toISOString();
  const msg = data ? `[${ts}] ${tag}: ${JSON.stringify(data)}` : `[${ts}] ${tag}`;
  process.stderr.write(msg + '\n');
}

// ============================================================================
// Workspace Identification
// ============================================================================

function computeWorkspaceId(dirPath) {
  if (!dirPath) return 'ws_default';
  const abs = nodePath.resolve(dirPath);
  const hash = nodeCrypto.createHash('sha1').update(abs).digest('hex').slice(0, 12);
  const name = nodePath.basename(abs).toLowerCase().replace(/[^a-z0-9]+/g, '_').slice(0, 24);
  return name ? `ws_${name}_${hash}` : `ws_${hash}`;
}

function extractProjectName(dirPath) {
  if (!dirPath) return 'Untitled';
  return nodePath.basename(nodePath.resolve(dirPath));
}

// ============================================================================
// State Management
// ============================================================================

function ensureStateDir() {
  if (!nodeFs.existsSync(STATE_DIR)) {
    nodeFs.mkdirSync(STATE_DIR, { recursive: true, mode: 0o700 });
  }
}

function readState(key) {
  try {
    const fp = nodePath.join(STATE_DIR, `${key}.json`);
    if (nodeFs.existsSync(fp)) {
      return JSON.parse(nodeFs.readFileSync(fp, 'utf8'));
    }
  } catch {}
  return {};
}

function writeState(key, data) {
  ensureStateDir();
  const fp = nodePath.join(STATE_DIR, `${key}.json`);
  nodeFs.writeFileSync(fp, JSON.stringify(data), 'utf8');
}

// ============================================================================
// Hook I/O
// ============================================================================

function parseStdin() {
  return new Promise((resolve, reject) => {
    if (process.stdin.isTTY) {
      resolve({});
      return;
    }
    const chunks = [];
    process.stdin.setEncoding('utf8');
    process.stdin.on('readable', () => {
      let c;
      while ((c = process.stdin.read()) !== null) chunks.push(c);
    });
    process.stdin.on('end', () => {
      const raw = chunks.join('');
      if (!raw.trim()) { resolve({}); return; }
      try { resolve(JSON.parse(raw)); }
      catch (e) { reject(new Error(`Invalid JSON: ${e.message}`)); }
    });
    process.stdin.on('error', reject);
  });
}

function respond(payload) {
  process.stdout.write(JSON.stringify(payload) + '\n');
}

function done(additionalContext = null) {
  if (additionalContext) {
    respond({ hookSpecificOutput: { hookEventName: 'SessionStart', additionalContext } });
  } else {
    respond({ continue: true, suppressOutput: true });
  }
}

// ============================================================================
// Exports
// ============================================================================

module.exports = {
  loadConfig,
  log,
  computeWorkspaceId,
  extractProjectName,
  readState,
  writeState,
  parseStdin,
  respond,
  done,
  CONFIG_DIR,
  STATE_DIR,
};
