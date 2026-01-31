/**
 * Search memory - CLI tool to search Agent Replay memories
 */

const { AgentReplayClient } = require('./lib/agentreplay-client');
const { getContainerTag, getProjectName } = require('./lib/container-tag');
const { loadSettings, getConfig } = require('./lib/settings');

async function main() {
  const query = process.argv.slice(2).join(' ');

  if (!query || !query.trim()) {
    console.log(
      'No search query provided. Please specify what you want to search for.',
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
    console.log(`Start Agent Replay at ${config.url} to enable memory search.`);
    return;
  }

  try {
    const result = await client.getProfile(containerTag, query);

    console.log(`## Memory Search: "${query}"`);
    console.log(`Project: ${projectName}\n`);

    if (result.profile) {
      if (result.profile.static?.length > 0) {
        console.log('### User Preferences');
        result.profile.static.forEach((fact) => console.log(`- ${fact}`));
        console.log('');
      }
      if (result.profile.dynamic?.length > 0) {
        console.log('### Recent Context');
        result.profile.dynamic.forEach((fact) => console.log(`- ${fact}`));
        console.log('');
      }
    }

    if (result.searchResults?.results?.length > 0) {
      console.log('### Relevant Memories');
      result.searchResults.results.forEach((mem, i) => {
        const score = mem.score != null ? Math.round(mem.score * 100) : 0;
        const content = mem.content || '';
        console.log(`\n**Memory ${i + 1}** (${score}% match)`);
        console.log(content.slice(0, 500));
      });
    } else {
      // Try direct search
      const searchResult = await client.search(query, containerTag, { limit: 10 });
      if (searchResult.results?.length > 0) {
        console.log('### Relevant Memories');
        searchResult.results.forEach((mem, i) => {
          const score = mem.score != null ? Math.round(mem.score * 100) : 0;
          const content = mem.content || '';
          console.log(`\n**Memory ${i + 1}** (${score}% match)`);
          console.log(content.slice(0, 500));
        });
      } else {
        console.log('No memories found matching your query.');
        console.log('Memories are automatically saved as you work in this project.');
      }
    }
  } catch (err) {
    console.log(`Error searching memories: ${err.message}`);
  }
}

main().catch((err) => {
  console.error(`Fatal error: ${err.message}`);
  process.exit(1);
});
