/**
 * Format context for injection into Claude Code sessions
 */

function formatRelativeTime(isoTimestamp) {
  try {
    const dt = new Date(isoTimestamp);
    const now = new Date();
    const seconds = (now.getTime() - dt.getTime()) / 1000;
    const minutes = seconds / 60;
    const hours = seconds / 3600;
    const days = seconds / 86400;

    if (minutes < 30) return 'just now';
    if (minutes < 60) return `${Math.floor(minutes)}mins ago`;
    if (hours < 24) return `${Math.floor(hours)}hrs ago`;
    if (days < 7) return `${Math.floor(days)}d ago`;

    const month = dt.toLocaleString('en', { month: 'short' });
    if (dt.getFullYear() === now.getFullYear()) {
      return `${dt.getDate()} ${month}`;
    }
    return `${dt.getDate()} ${month}, ${dt.getFullYear()}`;
  } catch {
    return '';
  }
}

function deduplicateMemories(staticFacts, dynamicFacts, searchResults) {
  const seen = new Set();

  const uniqueStatic = staticFacts.filter((m) => {
    if (seen.has(m)) return false;
    seen.add(m);
    return true;
  });

  const uniqueDynamic = dynamicFacts.filter((m) => {
    if (seen.has(m)) return false;
    seen.add(m);
    return true;
  });

  const uniqueSearch = searchResults.filter((r) => {
    const content = r.content ?? '';
    if (!content || seen.has(content)) return false;
    seen.add(content);
    return true;
  });

  return {
    static: uniqueStatic,
    dynamic: uniqueDynamic,
    searchResults: uniqueSearch,
  };
}

function formatContext(
  profileResult,
  includeProfile = true,
  includeRelevantMemories = false,
  maxResults = 10,
) {
  if (!profileResult) return null;

  const staticFacts = profileResult.profile?.static || [];
  const dynamicFacts = profileResult.profile?.dynamic || [];
  const searchResults = profileResult.searchResults?.results || [];

  const deduped = deduplicateMemories(
    includeProfile ? staticFacts : [],
    includeProfile ? dynamicFacts : [],
    includeRelevantMemories ? searchResults : [],
  );

  const statics = deduped.static.slice(0, maxResults);
  const dynamics = deduped.dynamic.slice(0, maxResults);
  const search = deduped.searchResults.slice(0, maxResults);

  if (statics.length === 0 && dynamics.length === 0 && search.length === 0) {
    return null;
  }

  const sections = [];

  if (statics.length > 0) {
    sections.push(
      '## User Preferences (Persistent)\n' +
        statics.map((f) => `- ${f}`).join('\n'),
    );
  }

  if (dynamics.length > 0) {
    sections.push(
      '## Recent Context\n' + dynamics.map((f) => `- ${f}`).join('\n'),
    );
  }

  if (search.length > 0) {
    const lines = search.map((r) => {
      const content = r.content ?? '';
      const score = r.score != null ? `[${Math.round(r.score * 100)}%]` : '';
      const line = content.length > 200 ? content.slice(0, 200) + '...' : content;
      return `- ${score} ${line}`;
    });
    sections.push('## Relevant Memories\n' + lines.join('\n'));
  }

  return `<agentreplay-context>
The following is recalled context from your local Agent Replay memory.
Data stored locally on this machine.

${sections.join('\n\n')}

</agentreplay-context>`;
}

module.exports = { formatContext, formatRelativeTime, deduplicateMemories };
