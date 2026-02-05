/*
 * Tool observation hook
 * Placeholder for capturing tool usage events
 */

const { loadConfig, logDebug } = require('./lib/settings');
const { parseInput, complete } = require('./lib/stdin');

(async function run() {
  const cfg = loadConfig();

  try {
    const hookInput = await parseInput();
    logDebug(cfg, 'Tool used', {
      session: hookInput.session_id,
      tool: hookInput.tool_name,
    });
    complete();
  } catch (err) {
    logDebug(cfg, 'Observation failed', { err: err.message });
    complete();
  }
})();
