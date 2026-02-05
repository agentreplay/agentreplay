/*
 * Configuration management for the Claude memory plugin
 * Handles persistent settings and environment overrides
 */

const nodeFs = require('node:fs');
const nodePath = require('node:path');
const nodeOs = require('node:os');

const CONFIG_FOLDER = nodePath.join(nodeOs.homedir(), '.agentreplay-claude');
const CONFIG_PATH = nodePath.join(CONFIG_FOLDER, 'config.json');

const DEFAULTS = Object.freeze({
  ignoredTools: ['Read', 'Glob', 'Grep', 'TodoWrite', 'AskUserQuestion'],
  trackedTools: ['Edit', 'Write', 'Bash', 'Task'],
  contextLimit: 5,
  verbose: false,
  autoInject: true,
  serverUrl: 'http://localhost:47100',
  tenantId: 1,
  projectId: 1,
});

function ensureConfigFolder() {
  if (!nodeFs.existsSync(CONFIG_FOLDER)) {
    nodeFs.mkdirSync(CONFIG_FOLDER, { recursive: true, mode: 0o700 });
  }
}

function loadConfig() {
  const cfg = { ...DEFAULTS };

  // File-based config
  try {
    if (nodeFs.existsSync(CONFIG_PATH)) {
      const raw = nodeFs.readFileSync(CONFIG_PATH, 'utf8');
      const parsed = JSON.parse(raw);
      Object.assign(cfg, parsed);
    }
  } catch (err) {
    logDebug(cfg, 'Config load failed', err.message);
  }

  // Environment overrides
  const env = process.env;
  if (env.AGENTREPLAY_URL) cfg.serverUrl = env.AGENTREPLAY_URL;
  if (env.AGENTREPLAY_TENANT_ID) cfg.tenantId = parseInt(env.AGENTREPLAY_TENANT_ID, 10);
  if (env.AGENTREPLAY_PROJECT_ID) cfg.projectId = parseInt(env.AGENTREPLAY_PROJECT_ID, 10);
  if (env.AGENTREPLAY_DEBUG === 'true' || env.AGENTREPLAY_DEBUG === '1') cfg.verbose = true;
  if (env.AGENTREPLAY_SKIP_TOOLS) {
    cfg.ignoredTools = env.AGENTREPLAY_SKIP_TOOLS.split(',').map((s) => s.trim()).filter(Boolean);
  }

  return cfg;
}

function persistConfig(cfg) {
  ensureConfigFolder();
  const toWrite = { ...cfg };
  nodeFs.writeFileSync(CONFIG_PATH, JSON.stringify(toWrite, null, 2), 'utf8');
}

function getServerConfig(cfg) {
  return {
    endpoint: cfg.serverUrl,
    tenant: cfg.tenantId,
    project: cfg.projectId,
  };
}

function isToolTracked(toolName, cfg) {
  if (cfg.ignoredTools.includes(toolName)) return false;
  if (cfg.trackedTools?.length > 0) return cfg.trackedTools.includes(toolName);
  return true;
}

function logDebug(cfg, label, detail = null) {
  if (!cfg.verbose) return;
  const ts = new Date().toISOString();
  const msg = detail ? `[${ts}] ${label}: ${JSON.stringify(detail)}` : `[${ts}] ${label}`;
  process.stderr.write(msg + '\n');
}

module.exports = {
  CONFIG_FOLDER,
  CONFIG_PATH,
  DEFAULTS,
  loadConfig,
  persistConfig,
  getServerConfig,
  isToolTracked,
  logDebug,
};
