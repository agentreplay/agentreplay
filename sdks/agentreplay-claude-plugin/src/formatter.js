/*
 * Context formatting for memory injection
 */

function humanizeTime(isoStr) {
  if (!isoStr) return '';
  try {
    const diff = Math.floor((Date.now() - new Date(isoStr).getTime()) / 1000);
    if (diff < 120) return 'moments ago';
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
    const d = new Date(isoStr);
    const mon = d.toLocaleString('en', { month: 'short' });
    return d.getFullYear() === new Date().getFullYear()
      ? `${mon} ${d.getDate()}`
      : `${mon} ${d.getDate()}, ${d.getFullYear()}`;
  } catch {
    return '';
  }
}

function dedupe(prefs, ctx, matches) {
  const seen = new Set();
  const uniqPrefs = prefs.filter((x) => {
    const k = String(x).trim().toLowerCase();
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
  const uniqCtx = ctx.filter((x) => {
    const k = String(x).trim().toLowerCase();
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
  const uniqMatches = matches.filter((m) => {
    const k = String(m.text || '').trim().toLowerCase();
    if (!k || seen.has(k)) return false;
    seen.add(k);
    return true;
  });
  return { prefs: uniqPrefs, ctx: uniqCtx, matches: uniqMatches };
}

function buildContextXml(profile, limit = 5) {
  if (!profile) return null;

  const d = dedupe(
    profile.preferences || [],
    profile.context || [],
    profile.search?.matches || []
  );

  const prefs = d.prefs.slice(0, limit);
  const ctx = d.ctx.slice(0, limit);
  const matches = d.matches.slice(0, limit);

  if (prefs.length === 0 && ctx.length === 0 && matches.length === 0) {
    return null;
  }

  const parts = [];

  if (prefs.length > 0) {
    parts.push('## Preferences\n' + prefs.map((p) => `• ${p}`).join('\n'));
  }

  if (ctx.length > 0) {
    parts.push('## Context\n' + ctx.map((c) => `• ${c}`).join('\n'));
  }

  if (matches.length > 0) {
    const lines = matches.map((m) => {
      const pct = m.score != null ? `${Math.round(m.score * 100)}%` : '';
      const preview = String(m.text || '').slice(0, 150);
      return `• [${pct}] ${preview}${m.text?.length > 150 ? '...' : ''}`;
    });
    parts.push('## Related\n' + lines.join('\n'));
  }

  return `<agentreplay-memory>
Recalled from local Agent Replay.

${parts.join('\n\n')}
</agentreplay-memory>`;
}

module.exports = { buildContextXml, humanizeTime, dedupe };
