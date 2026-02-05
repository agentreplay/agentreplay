/*
 * Workspace identification utilities
 * Generates stable identifiers for project directories
 */

const nodePath = require('node:path');
const nodeCrypto = require('node:crypto');

// Create a fingerprint from workspace path
function computeWorkspaceId(dirPath) {
  if (!dirPath) return 'unnamed';

  const absolutePath = nodePath.resolve(dirPath);
  const fingerprint = nodeCrypto
    .createHash('sha1')
    .update(absolutePath)
    .digest('hex')
    .substring(0, 12);

  const folderName = nodePath.basename(absolutePath);
  const sanitized = folderName
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_|_$/g, '')
    .substring(0, 24);

  return sanitized ? `ws_${sanitized}_${fingerprint}` : `ws_${fingerprint}`;
}

// Extract display name from path
function extractProjectLabel(dirPath) {
  if (!dirPath) return 'Untitled';
  return nodePath.basename(nodePath.resolve(dirPath));
}

module.exports = {
  computeWorkspaceId,
  extractProjectLabel,
};
