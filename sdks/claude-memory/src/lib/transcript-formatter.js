/**
 * Transcript formatter for extracting new content from Claude Code sessions
 */

const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');

const PROCESSED_DIR = path.join(os.homedir(), '.agentreplay-claude', 'processed');

function ensureProcessedDir() {
  if (!fs.existsSync(PROCESSED_DIR)) {
    fs.mkdirSync(PROCESSED_DIR, { recursive: true });
  }
}

function getProcessedPath(sessionId) {
  ensureProcessedDir();
  return path.join(PROCESSED_DIR, `${sessionId}.json`);
}

function loadProcessedState(sessionId) {
  const processedPath = getProcessedPath(sessionId);
  try {
    if (fs.existsSync(processedPath)) {
      return JSON.parse(fs.readFileSync(processedPath, 'utf-8'));
    }
  } catch {
    // Ignore errors
  }
  return { lastProcessedIndex: 0, lastTimestamp: 0 };
}

function saveProcessedState(sessionId, state) {
  const processedPath = getProcessedPath(sessionId);
  ensureProcessedDir();
  fs.writeFileSync(processedPath, JSON.stringify(state, null, 2));
}

/**
 * Format new entries from transcript for storage
 */
function formatNewEntries(transcriptPath, sessionId) {
  if (!transcriptPath || !fs.existsSync(transcriptPath)) {
    return null;
  }

  try {
    const transcript = JSON.parse(fs.readFileSync(transcriptPath, 'utf-8'));
    const entries = transcript.entries || transcript.messages || [];
    
    if (entries.length === 0) {
      return null;
    }

    const state = loadProcessedState(sessionId);
    const newEntries = entries.slice(state.lastProcessedIndex);
    
    if (newEntries.length === 0) {
      return null;
    }

    // Format entries for storage
    const formatted = newEntries.map((entry) => {
      const role = entry.role || entry.type || 'unknown';
      const content = entry.content || entry.message || '';
      const toolName = entry.tool_name || entry.toolName;
      
      if (toolName) {
        return `[${role}:${toolName}] ${truncate(content, 500)}`;
      }
      return `[${role}] ${truncate(content, 500)}`;
    }).join('\n\n');

    // Update processed state
    saveProcessedState(sessionId, {
      lastProcessedIndex: entries.length,
      lastTimestamp: Date.now(),
    });

    return formatted;
  } catch (err) {
    console.error(`Error formatting transcript: ${err.message}`);
    return null;
  }
}

function truncate(str, maxLen) {
  if (!str) return '';
  if (str.length <= maxLen) return str;
  return str.slice(0, maxLen) + '...';
}

module.exports = { formatNewEntries, loadProcessedState, saveProcessedState };
