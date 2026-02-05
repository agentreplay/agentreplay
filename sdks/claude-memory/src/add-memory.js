/*
 * CLI: Store content in memory
 * Usage: node add-memory.js "content to remember"
 */

const { MemoryService } = require('./lib/agentreplay-client');
const { computeWorkspaceId, extractProjectLabel } = require('./lib/container-tag');
const { loadConfig, getServerConfig } = require('./lib/settings');

(async function main() {
  const args = process.argv.slice(2);
  const text = args.join(' ').trim();

  if (!text) {
    console.log('Usage: add-memory "text to store"');
    console.log('Example: add-memory "User prefers dark mode"');
    return;
  }

  const cfg = loadConfig();
  const serverCfg = getServerConfig(cfg);
  const cwd = process.cwd();
  const wsId = computeWorkspaceId(cwd);
  const projectLabel = extractProjectLabel(cwd);

  const memService = new MemoryService({
    endpoint: serverCfg.endpoint,
    tenant: serverCfg.tenant,
    project: serverCfg.project,
    collection: wsId,
  });

  const pingResult = await memService.ping();
  if (!pingResult.ok) {
    console.log(`Cannot reach memory server at ${serverCfg.endpoint}`);
    console.log('Ensure Agent Replay is running.');
    return;
  }

  try {
    const result = await memService.store(text, wsId, {
      kind: 'user_input',
      project: projectLabel,
      when: new Date().toISOString(),
    });

    console.log(`Stored in: ${projectLabel}`);
    console.log(`Document: ${result.documentId}`);
    console.log('Data kept locally on this machine.');
  } catch (err) {
    console.log(`Store failed: ${err.message}`);
  }
})();
