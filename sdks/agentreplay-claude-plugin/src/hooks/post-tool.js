/*
 * PostToolUse hook
 * Sends tool completion trace with timing
 */

const { AgentReplayAPI } = require('../api');
const { loadConfig, log, parseStdin, respond, readState } = require('../common');

(async () => {
  const cfg = loadConfig();

  try {
    const input = await parseStdin();
    const toolName = input.tool_name || 'unknown';
    const toolInput = input.tool_input || {};
    const toolOutput = input.tool_response || {};

    log(cfg, 'PostToolUse', { tool: toolName });

    // Skip ignored tools
    if (cfg.ignoredTools.includes(toolName)) {
      respond({ continue: true, suppressOutput: true });
      return;
    }

    if (cfg.tracingEnabled) {
      const api = new AgentReplayAPI(cfg);
      const health = await api.ping();

      if (health.ok) {
        // Calculate duration
        const toolState = readState(`tool_${toolName}`);
        const duration = toolState.startTime ? Date.now() - toolState.startTime : null;

        // Get parent span
        const parentState = readState('parent_span');

        // Check for errors
        const isError = typeof toolOutput === 'object' && toolOutput.is_error === true;

        try {
          await api.sendToolSpan(
            toolName,
            toolInput,
            toolOutput,
            duration,
            parentState.spanId
          );
          log(cfg, 'Tool traced', { tool: toolName, duration, error: isError });
        } catch (e) {
          log(cfg, 'Trace error', e.message);
        }
      }
    }

    respond({ continue: true, suppressOutput: true });
  } catch (err) {
    log(cfg, 'Hook error', err.message);
    respond({ continue: true, suppressOutput: true });
  }
})();
