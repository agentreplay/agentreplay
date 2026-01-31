/**
 * Stop hook - Saves conversation turn to Agent Replay memory
 */

const { AgentReplayClient } = require('./lib/agentreplay-client');
const { getContainerTag, getProjectName } = require('./lib/container-tag');
const { loadSettings, getConfig, debugLog } = require('./lib/settings');
const { readStdin, writeOutput } = require('./lib/stdin');
const { formatNewEntries } = require('./lib/transcript-formatter');

async function main() {
  const settings = loadSettings();

  try {
    const input = await readStdin();
    const cwd = input.cwd || process.cwd();
    const sessionId = input.session_id;
    const transcriptPath = input.transcript_path;

    debugLog(settings, 'Stop', { sessionId, transcriptPath });

    if (!transcriptPath || !sessionId) {
      debugLog(settings, 'Missing transcript path or session id');
      writeOutput({ continue: true });
      return;
    }

    const config = getConfig(settings);
    const containerTag = getContainerTag(cwd);
    const projectName = getProjectName(cwd);

    const client = new AgentReplayClient({
      url: config.url,
      tenantId: config.tenantId,
      projectId: config.projectId,
      containerTag,
    });

    // Check if Agent Replay is running
    const health = await client.healthCheck();
    if (!health.healthy) {
      debugLog(settings, 'Agent Replay not running, skipping save');
      writeOutput({ continue: true });
      return;
    }

    // Format new entries from transcript
    const formatted = formatNewEntries(transcriptPath, sessionId);

    if (!formatted) {
      debugLog(settings, 'No new content to save');
      writeOutput({ continue: true });
      return;
    }

    // Save to Agent Replay
    await client.addMemory(
      formatted,
      containerTag,
      {
        type: 'session_turn',
        project: projectName,
        timestamp: new Date().toISOString(),
        session_id: sessionId,
      },
      sessionId,
    );

    debugLog(settings, 'Session turn saved', { length: formatted.length });
    writeOutput({ continue: true });
  } catch (err) {
    debugLog(settings, 'Error', { error: err.message });
    console.error(`AgentReplay: ${err.message}`);
    writeOutput({ continue: true });
  }
}

main().catch((err) => {
  console.error(`AgentReplay fatal: ${err.message}`);
  process.exit(1);
});
