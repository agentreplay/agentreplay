/*
 * SessionStart hook
 * - Creates root trace for observability
 * - Injects relevant memories into context
 */

const { AgentReplayAPI } = require('../api');
const { loadConfig, log, computeWorkspaceId, extractProjectName, parseStdin, respond, writeState } = require('../common');
const { buildContextXml } = require('../formatter');

(async () => {
  const cfg = loadConfig();

  try {
    const input = await parseStdin();
    const cwd = input.cwd || process.cwd();
    const wsId = computeWorkspaceId(cwd);
    const projectName = extractProjectName(cwd);

    log(cfg, 'SessionStart', { cwd, wsId, projectName });

    const api = new AgentReplayAPI(cfg);
    const health = await api.ping();

    if (!health.ok) {
      log(cfg, 'Server offline', health);
      respond({
        hookSpecificOutput: {
          hookEventName: 'SessionStart',
          additionalContext: `<agentreplay-status>
Agent Replay server not available at ${cfg.serverUrl}
Start Agent Replay to enable tracing and memory.
</agentreplay-status>`,
        },
      });
      return;
    }

    const outputs = [];

    // Tracing: create root span
    if (cfg.tracingEnabled) {
      try {
        const spanId = await api.sendRootSpan(cwd, projectName);
        if (spanId) {
          writeState('parent_span', { spanId, traceId: api.traceId });
          log(cfg, 'Root span created', { spanId, traceId: api.traceId });
        }
        outputs.push(`Tracing: ${cfg.serverUrl}`);
      } catch (e) {
        log(cfg, 'Trace error', e.message);
      }
    }

    // Memory: fetch and inject context
    if (cfg.memoryEnabled) {
      try {
        const profile = await api.getProfile(wsId, projectName);
        const contextXml = buildContextXml(profile, cfg.contextLimit);

        if (contextXml) {
          log(cfg, 'Memory injected', { bytes: contextXml.length });
          respond({
            hookSpecificOutput: {
              hookEventName: 'SessionStart',
              additionalContext: contextXml,
            },
          });
          return;
        }
        outputs.push('Memory: no prior context');
      } catch (e) {
        log(cfg, 'Memory error', e.message);
      }
    }

    // No memory context - just show status
    respond({
      hookSpecificOutput: {
        hookEventName: 'SessionStart',
        additionalContext: outputs.length > 0
          ? `<agentreplay-status>\n${outputs.join('\n')}\n</agentreplay-status>`
          : null,
      },
    });
  } catch (err) {
    log(cfg, 'Hook error', err.message);
    respond({ continue: true, suppressOutput: true });
  }
})();
