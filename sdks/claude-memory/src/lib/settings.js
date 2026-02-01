const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');

const SETTINGS_DIR = path.join(os.homedir(), '.agentreplay-claude');
const SETTINGS_FILE = path.join(SETTINGS_DIR, 'settings.json');

const DEFAULT_SETTINGS = {
  skipTools: ['Read', 'Glob', 'Grep', 'TodoWrite', 'AskUserQuestion'],
  captureTools: ['Edit', 'Write', 'Bash', 'Task'],
  maxProfileItems: 5,
  debug: false,
  injectProfile: true,
  url: 'http://localhost:47100',
  tenantId: 1,
  projectId: 1,
};

function ensureSettingsDir() {
  if (!fs.existsSync(SETTINGS_DIR)) {
    fs.mkdirSync(SETTINGS_DIR, { recursive: true });
  }
}

function loadSettings() {
  const settings = { ...DEFAULT_SETTINGS };
  try {
    if (fs.existsSync(SETTINGS_FILE)) {
      const fileContent = fs.readFileSync(SETTINGS_FILE, 'utf-8');
      Object.assign(settings, JSON.parse(fileContent));
    }
  } catch (err) {
    console.error(`Settings: Failed to load ${SETTINGS_FILE}: ${err.message}`);
  }
  
  // Environment variable overrides
  if (process.env.AGENTREPLAY_URL) {
    settings.url = process.env.AGENTREPLAY_URL;
  }
  if (process.env.AGENTREPLAY_TENANT_ID) {
    settings.tenantId = parseInt(process.env.AGENTREPLAY_TENANT_ID, 10);
  }
  if (process.env.AGENTREPLAY_PROJECT_ID) {
    settings.projectId = parseInt(process.env.AGENTREPLAY_PROJECT_ID, 10);
  }
  if (process.env.AGENTREPLAY_SKIP_TOOLS) {
    settings.skipTools = process.env.AGENTREPLAY_SKIP_TOOLS.split(',').map((s) => s.trim());
  }
  if (process.env.AGENTREPLAY_DEBUG === 'true') {
    settings.debug = true;
  }
  
  return settings;
}

function saveSettings(settings) {
  ensureSettingsDir();
  const toSave = { ...settings };
  fs.writeFileSync(SETTINGS_FILE, JSON.stringify(toSave, null, 2));
}

function getConfig(settings) {
  return {
    url: settings.url,
    tenantId: settings.tenantId,
    projectId: settings.projectId,
  };
}

function shouldCaptureTool(toolName, settings) {
  if (settings.skipTools.includes(toolName)) return false;
  if (settings.captureTools && settings.captureTools.length > 0) {
    return settings.captureTools.includes(toolName);
  }
  return true;
}

function debugLog(settings, message, data) {
  if (settings.debug) {
    const timestamp = new Date().toISOString();
    console.error(
      data
        ? `[${timestamp}] ${message}: ${JSON.stringify(data)}`
        : `[${timestamp}] ${message}`,
    );
  }
}

module.exports = {
  SETTINGS_DIR,
  SETTINGS_FILE,
  DEFAULT_SETTINGS,
  loadSettings,
  saveSettings,
  getConfig,
  shouldCaptureTool,
  debugLog,
};
