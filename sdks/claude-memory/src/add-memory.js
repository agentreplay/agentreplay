/**
 * Add memory - CLI tool to manually add content to Agent Replay
 */

const { AgentReplayClient } = require('./lib/agentreplay-client');
const { getContainerTag, getProjectName } = require('./lib/container-tag');
const { loadSettings, getConfig } = require('./lib/settings');

async function main() {
  const content = process.argv.slice(2).join(' ');

  if (!content || !content.trim()) {
    console.log(
      'No content provided. Usage: node add-memory.cjs "content to save"',
    );
    return;
  }

  const settings = loadSettings();
  const config = getConfig(settings);

  const cwd = process.cwd();
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
    console.log('Agent Replay is not running.');
    console.log(`Start Agent Replay at ${config.url} to save memories.`);
    return;
  }

  try {
    const result = await client.addMemory(content, containerTag, {
      type: 'manual',
      project: projectName,
      timestamp: new Date().toISOString(),
    });

    console.log(`Memory saved to project: ${projectName}`);
    console.log(`ID: ${result.id}`);
    console.log('Data stored locally on this machine.');
  } catch (err) {
    console.log(`Error saving memory: ${err.message}`);
  }
}

main().catch((err) => {
  console.error(`Fatal error: ${err.message}`);
  process.exit(1);
});
