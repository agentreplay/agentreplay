/*
 * User prompt hook
 * Placeholder for prompt-time processing
 */

const { loadConfig, logDebug } = require('./lib/settings');
const { parseInput, complete } = require('./lib/stdin');

(async function run() {
  const cfg = loadConfig();

  try {
    const hookInput = await parseInput();
    logDebug(cfg, 'Prompt received', { session: hookInput.session_id });
    complete();
  } catch (err) {
    logDebug(cfg, 'Prompt hook error', { err: err.message });
    complete();
  }
})();
