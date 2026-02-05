/*
 * Session initialization hook
 * Retrieves relevant memories and injects them into the new session
 */

const { MemoryService } = require('./lib/agentreplay-client');
const { computeWorkspaceId, extractProjectLabel } = require('./lib/container-tag');
const { loadConfig, getServerConfig, logDebug } = require('./lib/settings');
const { parseInput, respond, complete } = require('./lib/stdin');
const { buildContextBlock } = require('./lib/format-context');

(async function run() {
  const cfg = loadConfig();

  try {
    const hookInput = await parseInput();
    const workDir = hookInput.cwd ?? process.cwd();
    const wsId = computeWorkspaceId(workDir);
    const projectLabel = extractProjectLabel(workDir);

    logDebug(cfg, 'Session init', { workDir, wsId, projectLabel });

    const serverCfg = getServerConfig(cfg);
    const memService = new MemoryService({
      endpoint: serverCfg.endpoint,
      tenant: serverCfg.tenant,
      project: serverCfg.project,
      collection: wsId,
    });

    // Verify server connectivity
    const pingResult = await memService.ping();
    if (!pingResult.ok) {
      logDebug(cfg, 'Server offline', { reason: pingResult.reason });
      respond({
        hookSpecificOutput: {
          hookEventName: 'SessionStart',
          additionalContext: `<memory-status>
Memory server unavailable at ${serverCfg.endpoint}
Launch Agent Replay to enable persistent memory.
Session continues without historical context.
</memory-status>`,
        },
      });
      return;
    }

    // Fetch profile data
    const profile = await memService.buildProfile(wsId, projectLabel).catch(() => null);
    const contextXml = buildContextBlock(profile, true, false, cfg.contextLimit);

    if (!contextXml) {
      respond({
        hookSpecificOutput: {
          hookEventName: 'SessionStart',
          additionalContext: `<memory-status>
No stored memories for "${projectLabel}".
Context will accumulate as you work.
</memory-status>`,
        },
      });
      return;
    }

    logDebug(cfg, 'Context injected', { bytes: contextXml.length });
    respond({
      hookSpecificOutput: {
        hookEventName: 'SessionStart',
        additionalContext: contextXml,
      },
    });
  } catch (err) {
    logDebug(cfg, 'Hook error', { err: err.message });
    process.stderr.write(`[memory-hook] ${err.message}\n`);
    respond({
      hookSpecificOutput: {
        hookEventName: 'SessionStart',
        additionalContext: `<memory-status>
Memory retrieval failed: ${err.message}
</memory-status>`,
      },
    });
  }
})();
