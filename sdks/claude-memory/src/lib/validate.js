/**
 * Validation utilities for Agent Replay client
 */

function validateUrl(url) {
  if (!url) {
    return { valid: false, reason: 'URL is required' };
  }
  
  try {
    const parsed = new URL(url);
    if (!['http:', 'https:'].includes(parsed.protocol)) {
      return { valid: false, reason: 'URL must use http or https protocol' };
    }
    return { valid: true };
  } catch {
    return { valid: false, reason: 'Invalid URL format' };
  }
}

function validateContainerTag(tag) {
  if (!tag) {
    return { valid: false, reason: 'Container tag is required' };
  }
  
  if (tag.length > 128) {
    return { valid: false, reason: 'Container tag too long (max 128 chars)' };
  }
  
  // Allow alphanumeric, hyphens, underscores
  if (!/^[a-zA-Z0-9_-]+$/.test(tag)) {
    return { valid: false, reason: 'Container tag can only contain alphanumeric characters, hyphens, and underscores' };
  }
  
  return { valid: true };
}

module.exports = { validateUrl, validateContainerTag };
