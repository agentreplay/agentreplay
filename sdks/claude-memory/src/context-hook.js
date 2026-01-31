/**
 * SessionStart hook - Injects relevant memories into Claude Code context
 */

const { AgentReplayClient } = require('./lib/agentreplay-client');
const { getContainerTag, getProjectName } = require('./lib/container-tag');
const { loadSettings, getConfig, debugLog } = require('./lib/settings');
const { readStdin, writeOutput } = require('./lib/stdin');
const { formatContext } = require('./lib/format-context');

async function main() {
  const settings = loadSettings();

  try {
    const input = await readStdin();
    const cwd = input.cwd || process.cwd();
    const containerTag = getContainerTag(cwd);
    const projectName = getProjectName(cwd);

    debugLog(settings, 'SessionStart', { cwd, containerTag, projectName });

    const config = getConfig(settings);
    const client = new AgentReplayClient({
      url: config.url,
      tenantId: config.tenantId,
      projectId: config.projectId,
      containerTag,
    });

    // Check if Agent Replay is running
    const health = await client.healthCheck();
    if (!health.healthy) {
      debugLog(settings, 'Agent Replay not running', { error: health.error });
      writeOutput({
        hookSpecificOutput: {
          hookEventName: 'SessionStart',
          additionalContext: `<agentreplay-status>
Agent Replay is not running at ${config.url}
Start Agent Replay to enable persistent memory.
Memories will be saved once Agent Replay is running.
</agentreplay-status>`,
        },
      });
      return;
    }

    // Get profile/context for this workspace
    const profileResult = await client
      .getProfile(containerTag, projectName)
      .catch(() => null);

    const additionalContext = formatContext(
      profileResult,
      true,
      false,
      settings.maxProfileItems,
    );

    if (!additionalContext) {
      writeOutput({
        hookSpecificOutput: {
          hookEventName: 'SessionStart',
          additionalContext: `<agentreplay-context>
No previous memories found for ${projectName}.
Memories will be saved locally as you work.
All data stays on your machine.
</agentreplay-context>`,
        },
      });
      return;
    }

    debugLog(settings, 'Context generated', {
      length: additionalContext.length,
    });

    writeOutput({
      hookSpecificOutput: { hookEventName: 'SessionStart', additionalContext },
    });
  } catch (err) {
    debugLog(settings, 'Error', { error: err.message });
    console.error(`AgentReplay: ${err.message}`);
    writeOutput({
      hookSpecificOutput: {
        hookEventName: 'SessionStart',
        additionalContext: `<agentreplay-status>
Failed to load memories: ${err.message}
Session will continue without memory context.
</agentreplay-status>`,
      },
    });
  }
}

main().catch((err) => {
  console.error(`AgentReplay fatal: ${err.message}`);
  process.exit(1);
});
