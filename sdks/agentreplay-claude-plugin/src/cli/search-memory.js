/*
 * CLI: Search local memory
 * Usage: node search-memory.js "query"
 */

const { AgentReplayAPI } = require('../api');
const { loadConfig, computeWorkspaceId, extractProjectName } = require('../common');

(async () => {
  const query = process.argv.slice(2).join(' ').trim();

  if (!query) {
    console.log('Usage: search-memory "your query"');
    console.log('Example: search-memory "authentication setup"');
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
    console.log('Start the Agent Replay server to search memories.');
    return;
  }

  try {
    const profile = await api.getProfile(wsId, query);

    console.log(`\n# Search: "${query}"`);
    console.log(`# Project: ${projectName}\n`);

    if (profile.preferences?.length > 0) {
      console.log('## Preferences');
      profile.preferences.forEach((p) => console.log(`  • ${p}`));
      console.log('');
    }

    if (profile.context?.length > 0) {
      console.log('## Context');
      profile.context.forEach((c) => console.log(`  • ${c}`));
      console.log('');
    }

    const matches = profile.search?.matches || [];
    if (matches.length > 0) {
      console.log('## Matches');
      matches.slice(0, 8).forEach((m, i) => {
        const pct = m.score != null ? Math.round(m.score * 100) : 0;
        const preview = (m.text || '').slice(0, 300);
        console.log(`\n[${i + 1}] ${pct}% match`);
        console.log(preview);
      });
    }

    if (!profile.preferences?.length && !profile.context?.length && !matches.length) {
      console.log('No matches found.');
      console.log('Memories build up as you work in this project.');
    }
  } catch (err) {
    console.log(`Search failed: ${err.message}`);
  }
})();
