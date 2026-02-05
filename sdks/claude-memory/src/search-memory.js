/*
 * CLI: Query stored memories
 * Usage: node search-memory.js "search terms"
 */

const { MemoryService } = require('./lib/agentreplay-client');
const { computeWorkspaceId, extractProjectLabel } = require('./lib/container-tag');
const { loadConfig, getServerConfig } = require('./lib/settings');

(async function main() {
  const args = process.argv.slice(2);
  const queryText = args.join(' ').trim();

  if (!queryText) {
    console.log('Usage: search-memory "your query"');
    console.log('Example: search-memory "authentication setup"');
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
    const profile = await memService.buildProfile(wsId, queryText);

    console.log(`\n# Query: "${queryText}"`);
    console.log(`# Project: ${projectLabel}\n`);

    // Show preferences
    if (profile.preferences?.length > 0) {
      console.log('## Preferences');
      profile.preferences.forEach((p) => console.log(`  • ${p}`));
      console.log('');
    }

    // Show context
    if (profile.context?.length > 0) {
      console.log('## Context');
      profile.context.forEach((c) => console.log(`  • ${c}`));
      console.log('');
    }

    // Show matches
    const matches = profile.related?.matches ?? [];
    if (matches.length > 0) {
      console.log('## Matches');
      matches.slice(0, 8).forEach((m, idx) => {
        const pct = m.relevance != null ? Math.round(m.relevance * 100) : 0;
        const preview = (m.text ?? '').substring(0, 300);
        console.log(`\n[${idx + 1}] ${pct}% relevance`);
        console.log(preview);
      });
    }

    if (profile.preferences?.length === 0 && profile.context?.length === 0 && matches.length === 0) {
      // Fallback direct search
      const directSearch = await memService.find(queryText, wsId, { maxResults: 8 });
      if (directSearch.matches?.length > 0) {
        console.log('## Matches');
        directSearch.matches.forEach((m, idx) => {
          const pct = m.relevance != null ? Math.round(m.relevance * 100) : 0;
          const preview = (m.text ?? '').substring(0, 300);
          console.log(`\n[${idx + 1}] ${pct}% relevance`);
          console.log(preview);
        });
      } else {
        console.log('No matches found.');
        console.log('Memories accumulate as you work in this project.');
      }
    }
  } catch (err) {
    console.log(`Query failed: ${err.message}`);
  }
})();
