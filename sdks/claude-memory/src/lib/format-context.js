/*
 * Context builder for session injection
 * Transforms memory data into Claude-readable context blocks
 */

// Human-readable time difference
function humanizeTimestamp(isoStr) {
  if (!isoStr) return '';
  
  try {
    const then = new Date(isoStr).getTime();
    const now = Date.now();
    const diffSec = Math.floor((now - then) / 1000);

    if (diffSec < 120) return 'moments ago';
    if (diffSec < 3600) return `${Math.floor(diffSec / 60)} min ago`;
    if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
    if (diffSec < 604800) return `${Math.floor(diffSec / 86400)}d ago`;

    const d = new Date(isoStr);
    const mon = d.toLocaleString('en', { month: 'short' });
    const day = d.getDate();
    const yr = d.getFullYear();
    const currYr = new Date().getFullYear();
    
    return yr === currYr ? `${mon} ${day}` : `${mon} ${day}, ${yr}`;
  } catch {
    return '';
  }
}

// Remove duplicate entries across arrays
function removeDuplicates(prefs, ctx, related) {
  const tracker = new Set();

  const uniquePrefs = prefs.filter((item) => {
    const key = String(item).trim().toLowerCase();
    if (tracker.has(key)) return false;
    tracker.add(key);
    return true;
  });

  const uniqueCtx = ctx.filter((item) => {
    const key = String(item).trim().toLowerCase();
    if (tracker.has(key)) return false;
    tracker.add(key);
    return true;
  });

  const uniqueRelated = related.filter((item) => {
    const textContent = String(item.text ?? '').trim().toLowerCase();
    if (!textContent || tracker.has(textContent)) return false;
    tracker.add(textContent);
    return true;
  });

  return { prefs: uniquePrefs, ctx: uniqueCtx, related: uniqueRelated };
}

// Build context XML block from profile data
function buildContextBlock(profileData, includePrefs = true, includeRelated = false, limit = 10) {
  if (!profileData) return null;

  const rawPrefs = profileData.preferences ?? [];
  const rawCtx = profileData.context ?? [];
  const rawRelated = profileData.related?.matches ?? [];

  const deduped = removeDuplicates(
    includePrefs ? rawPrefs : [],
    includePrefs ? rawCtx : [],
    includeRelated ? rawRelated : []
  );

  const prefs = deduped.prefs.slice(0, limit);
  const ctx = deduped.ctx.slice(0, limit);
  const related = deduped.related.slice(0, limit);

  if (prefs.length === 0 && ctx.length === 0 && related.length === 0) {
    return null;
  }

  const blocks = [];

  if (prefs.length > 0) {
    const items = prefs.map((p) => `• ${p}`).join('\n');
    blocks.push(`## Preferences\n${items}`);
  }

  if (ctx.length > 0) {
    const items = ctx.map((c) => `• ${c}`).join('\n');
    blocks.push(`## Context\n${items}`);
  }

  if (related.length > 0) {
    const items = related.map((r) => {
      const pct = r.relevance != null ? `${Math.round(r.relevance * 100)}%` : '';
      const preview = String(r.text ?? '').substring(0, 180);
      const ellipsis = (r.text?.length ?? 0) > 180 ? '...' : '';
      return `• [${pct}] ${preview}${ellipsis}`;
    }).join('\n');
    blocks.push(`## Related\n${items}`);
  }

  const body = blocks.join('\n\n');

  return `<memory-context>
Recalled from local Agent Replay storage.

${body}
</memory-context>`;
}

module.exports = {
  buildContextBlock,
  humanizeTimestamp,
  removeDuplicates,
};
