/**
 * UserPromptSubmit hook - Placeholder for future enhancements
 */

const { loadSettings, debugLog } = require('./lib/settings');
const { readStdin, outputSuccess } = require('./lib/stdin');

async function main() {
  const settings = loadSettings();

  try {
    const input = await readStdin();
    const sessionId = input.session_id;

    debugLog(settings, 'UserPromptSubmit', { sessionId });

    outputSuccess();
  } catch (err) {
    debugLog(settings, 'Error', { error: err.message });
    outputSuccess();
  }
}

main().catch((err) => {
  console.error(`AgentReplay fatal: ${err.message}`);
  process.exit(1);
});
