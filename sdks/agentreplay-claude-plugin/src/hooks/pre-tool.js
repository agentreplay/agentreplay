/*
 * PreToolUse hook
 * Records tool start time for duration calculation
 */

const { loadConfig, log, parseStdin, respond, writeState } = require('../common');

(async () => {
  const cfg = loadConfig();

  try {
    const input = await parseStdin();
    const toolName = input.tool_name || 'unknown';

    log(cfg, 'PreToolUse', { tool: toolName });

    // Record start time
    if (cfg.tracingEnabled) {
      writeState(`tool_${toolName}`, { startTime: Date.now() });
    }

    respond({ continue: true, suppressOutput: true });
  } catch (err) {
    log(cfg, 'Hook error', err.message);
    respond({ continue: true, suppressOutput: true });
  }
})();
