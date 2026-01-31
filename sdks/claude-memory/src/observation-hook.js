/**
 * PostToolUse hook - Placeholder for observation capture
 */

const { loadSettings, debugLog } = require('./lib/settings');
const { readStdin, outputSuccess } = require('./lib/stdin');

async function main() {
  const settings = loadSettings();

  try {
    const input = await readStdin();
    const sessionId = input.session_id;
    const toolName = input.tool_name;

    debugLog(settings, 'PostToolUse', { sessionId, toolName });

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
