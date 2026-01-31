/**
 * Container tag utilities for workspace identification
 */

const path = require('node:path');
const crypto = require('node:crypto');

/**
 * Generate a container tag from the workspace path
 * Uses a hash of the path to create a unique identifier
 */
function getContainerTag(workspacePath) {
  if (!workspacePath) {
    return 'default';
  }
  
  // Normalize the path
  const normalized = path.resolve(workspacePath);
  
  // Create a short hash for uniqueness
  const hash = crypto.createHash('md5').update(normalized).digest('hex').slice(0, 8);
  
  // Get the directory name for readability
  const dirName = path.basename(normalized).toLowerCase()
    .replace(/[^a-z0-9]/g, '-')
    .slice(0, 32);
  
  return `${dirName}-${hash}`;
}

/**
 * Get a human-readable project name from the path
 */
function getProjectName(workspacePath) {
  if (!workspacePath) {
    return 'Unknown Project';
  }
  
  return path.basename(workspacePath);
}

module.exports = { getContainerTag, getProjectName };
