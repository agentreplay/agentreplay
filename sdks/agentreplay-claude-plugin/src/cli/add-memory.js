/*
 * CLI: Add content to memory
 * Usage: node add-memory.js "content to store"
 */

const { AgentReplayAPI } = require('../api');
const { loadConfig, computeWorkspaceId, extractProjectName } = require('../common');

(async () => {
  const content = process.argv.slice(2).join(' ').trim();

  if (!content) {
    console.log('Usage: add-memory "content to store"');
    console.log('Example: add-memory "User prefers dark mode"');
    return;
  }

  const cfg = loadConfig();
  const cwd = process.cwd();
  const wsId = computeWorkspaceId(cwd);
  const projectName = extractProjectName(cwd);

  const api = new AgentReplayAPI(cfg);
  const health = await api.ping();

  if (!health.ok) {
    console.log(`Cannot reach Agent Replay at ${cfg.serverUrl}`);
    console.log('Start the Agent Replay server to store memories.');
    return;
  }

  try {
    const result = await api.storeMemory(content, wsId, {
      kind: 'manual',
      project: projectName,
      when: new Date().toISOString(),
    });

    console.log(`Stored in: ${projectName}`);
    console.log(`Document: ${result.docId}`);
    console.log('Data saved locally.');
  } catch (err) {
    console.log(`Store failed: ${err.message}`);
  }
})();
