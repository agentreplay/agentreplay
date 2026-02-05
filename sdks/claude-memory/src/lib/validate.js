/*
 * Input validation helpers
 */

// Check server URL validity
function checkEndpoint(urlStr) {
  if (!urlStr || typeof urlStr !== 'string') {
    return { ok: false, msg: 'Endpoint URL required' };
  }

  try {
    const u = new URL(urlStr);
    const allowedProtocols = ['http:', 'https:'];
    if (!allowedProtocols.includes(u.protocol)) {
      return { ok: false, msg: 'Endpoint must be http or https' };
    }
    return { ok: true };
  } catch {
    return { ok: false, msg: 'Malformed URL' };
  }
}

// Check collection identifier validity
function checkCollectionId(id) {
  if (!id || typeof id !== 'string') {
    return { ok: false, msg: 'Collection ID required' };
  }

  if (id.length > 100) {
    return { ok: false, msg: 'Collection ID exceeds 100 characters' };
  }

  const validPattern = /^[a-zA-Z0-9][a-zA-Z0-9_-]*$/;
  if (!validPattern.test(id)) {
    return { ok: false, msg: 'Collection ID must start with alphanumeric and contain only letters, numbers, hyphens, underscores' };
  }

  return { ok: true };
}

module.exports = { checkEndpoint, checkCollectionId };
