/*
 * Stop hook
 * - Sends session end trace
 * - Persists conversation to memory
 */

const nodeFs = require('node:fs');
const { AgentReplayAPI } = require('../api');
const { loadConfig, log, computeWorkspaceId, extractProjectName, parseStdin, respond, readState, writeState } = require('../common');

function extractTranscript(transcriptPath, sessionId) {
  if (!transcriptPath || !nodeFs.existsSync(transcriptPath)) return null;

  try {
    const data = JSON.parse(nodeFs.readFileSync(transcriptPath, 'utf8'));
    const entries = data.entries || data.messages || [];
    if (entries.length === 0) return null;

    const stateKey = `transcript_${sessionId}`;
    const state = readState(stateKey);
    const cursor = state.cursor || 0;

    const newEntries = entries.slice(cursor);
    if (newEntries.length === 0) return null;

    const lines = newEntries.map((e) => {
      const role = e.role || e.type || 'system';
      const body = String(e.content || e.message || '').slice(0, 400);
      const tool = e.tool_name || e.toolName;
      return tool ? `[${role}/${tool}] ${body}` : `[${role}] ${body}`;
    });

    writeState(stateKey, { cursor: entries.length, ts: Date.now() });
    return lines.join('\n---\n');
  } catch {
    return null;
  }
}

(async () => {
  const cfg = loadConfig();

  try {
    const input = await parseStdin();
    const cwd = input.cwd || process.cwd();
    const sessionId = input.session_id;
    const transcriptPath = input.transcript_path;

    log(cfg, 'Stop', { sessionId, transcriptPath });

    const api = new AgentReplayAPI(cfg);
    const health = await api.ping();

    if (!health.ok) {
      log(cfg, 'Server offline');
      respond({ continue: true });
      return;
    }

    const wsId = computeWorkspaceId(cwd);
    const projectName = extractProjectName(cwd);

    // Tracing: send session end span
    if (cfg.tracingEnabled) {
      try {
        const parentState = readState('parent_span');
        await api.sendEndSpan(input.reason || 'normal', parentState.spanId);
        log(cfg, 'Session end traced');
      } catch (e) {
        log(cfg, 'Trace error', e.message);
      }
    }

    // Memory: save conversation
    if (cfg.memoryEnabled && sessionId && transcriptPath) {
      try {
        const content = extractTranscript(transcriptPath, sessionId);
        if (content) {
          await api.storeMemory(content, wsId, {
            kind: 'conversation',
            project: projectName,
            when: new Date().toISOString(),
            session: sessionId,
          }, sessionId);
          log(cfg, 'Conversation saved', { chars: content.length });
        }
      } catch (e) {
        log(cfg, 'Memory error', e.message);
      }
    }

    // Clear session state
    writeState('parent_span', {});

    respond({ continue: true });
  } catch (err) {
    log(cfg, 'Hook error', err.message);
    respond({ continue: true });
  }
})();
