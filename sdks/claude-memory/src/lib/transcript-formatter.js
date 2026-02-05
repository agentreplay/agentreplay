/*
 * Session transcript processor
 * Extracts and formats new conversation content for memory storage
 */

const nodeFs = require('node:fs');
const nodePath = require('node:path');
const nodeOs = require('node:os');

const STATE_DIR = nodePath.join(nodeOs.homedir(), '.agentreplay-claude', 'state');

function initStateDir() {
  if (!nodeFs.existsSync(STATE_DIR)) {
    nodeFs.mkdirSync(STATE_DIR, { recursive: true, mode: 0o700 });
  }
}

function stateFilePath(sid) {
  initStateDir();
  const safeSid = String(sid).replace(/[^a-zA-Z0-9_-]/g, '_');
  return nodePath.join(STATE_DIR, `${safeSid}.state`);
}

function readState(sid) {
  try {
    const fp = stateFilePath(sid);
    if (nodeFs.existsSync(fp)) {
      const raw = nodeFs.readFileSync(fp, 'utf8');
      return JSON.parse(raw);
    }
  } catch {
    // Fresh state on error
  }
  return { cursor: 0, ts: 0 };
}

function writeState(sid, state) {
  const fp = stateFilePath(sid);
  initStateDir();
  nodeFs.writeFileSync(fp, JSON.stringify(state), 'utf8');
}

// Truncate long strings
function clip(text, max) {
  if (!text) return '';
  return text.length > max ? text.slice(0, max) + '…' : text;
}

// Process transcript and return only new entries
function extractNewContent(transcriptFile, sessionId) {
  if (!transcriptFile || !nodeFs.existsSync(transcriptFile)) {
    return null;
  }

  let data;
  try {
    const raw = nodeFs.readFileSync(transcriptFile, 'utf8');
    data = JSON.parse(raw);
  } catch {
    return null;
  }

  const allEntries = data.entries ?? data.messages ?? [];
  if (allEntries.length === 0) return null;

  const state = readState(sessionId);
  const unprocessed = allEntries.slice(state.cursor);
  if (unprocessed.length === 0) return null;

  // Build formatted output
  const lines = [];
  for (const entry of unprocessed) {
    const actor = entry.role ?? entry.type ?? 'system';
    const body = entry.content ?? entry.message ?? '';
    const tool = entry.tool_name ?? entry.toolName;

    if (tool) {
      lines.push(`[${actor}/${tool}] ${clip(body, 400)}`);
    } else {
      lines.push(`[${actor}] ${clip(body, 400)}`);
    }
  }

  // Update state
  writeState(sessionId, { cursor: allEntries.length, ts: Date.now() });

  return lines.join('\n---\n');
}

module.exports = {
  extractNewContent,
  readState,
  writeState,
};
