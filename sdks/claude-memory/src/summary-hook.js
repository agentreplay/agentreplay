/*
 * Session end hook
 * Persists new conversation content to memory storage
 */

const { MemoryService } = require('./lib/agentreplay-client');
const { computeWorkspaceId, extractProjectLabel } = require('./lib/container-tag');
const { loadConfig, getServerConfig, logDebug } = require('./lib/settings');
const { parseInput, respond } = require('./lib/stdin');
const { extractNewContent } = require('./lib/transcript-formatter');

(async function run() {
  const cfg = loadConfig();

  try {
    const hookInput = await parseInput();
    const workDir = hookInput.cwd ?? process.cwd();
    const sid = hookInput.session_id;
    const transcriptFile = hookInput.transcript_path;

    logDebug(cfg, 'Session end', { sid, transcriptFile });

    if (!transcriptFile || !sid) {
      logDebug(cfg, 'Missing session data, skipping');
      respond({ continue: true });
      return;
    }

    const serverCfg = getServerConfig(cfg);
    const wsId = computeWorkspaceId(workDir);
    const projectLabel = extractProjectLabel(workDir);

    const memService = new MemoryService({
      endpoint: serverCfg.endpoint,
      tenant: serverCfg.tenant,
      project: serverCfg.project,
      collection: wsId,
    });

    // Skip if server unavailable
    const pingResult = await memService.ping();
    if (!pingResult.ok) {
      logDebug(cfg, 'Server offline, session not saved');
      respond({ continue: true });
      return;
    }

    // Get new conversation content
    const newContent = extractNewContent(transcriptFile, sid);
    if (!newContent) {
      logDebug(cfg, 'No new content');
      respond({ continue: true });
      return;
    }

    // Persist to memory
    await memService.store(
      newContent,
      wsId,
      {
        kind: 'conversation_turn',
        project: projectLabel,
        when: new Date().toISOString(),
        sid: sid,
      },
      sid
    );

    logDebug(cfg, 'Content saved', { chars: newContent.length });
    respond({ continue: true });
  } catch (err) {
    logDebug(cfg, 'Save error', { err: err.message });
    process.stderr.write(`[memory-hook] ${err.message}\n`);
    respond({ continue: true });
  }
})();
