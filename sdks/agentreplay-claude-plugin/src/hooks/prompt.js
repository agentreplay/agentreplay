/*
 * UserPromptSubmit hook
 * Placeholder for prompt-time processing
 */

const { loadConfig, log, parseStdin, respond } = require('../common');

(async () => {
  const cfg = loadConfig();

  try {
    const input = await parseStdin();
    log(cfg, 'UserPrompt', { session: input.session_id });
    respond({ continue: true, suppressOutput: true });
  } catch (err) {
    log(cfg, 'Hook error', err.message);
    respond({ continue: true, suppressOutput: true });
  }
})();
