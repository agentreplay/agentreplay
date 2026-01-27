/**
 * Flowtrace Claude Code Plugin
 * 
 * Automatic tracing of all Claude Code tool calls and sessions.
 * 
 * Installation:
 *   npm install -g @sochdb/flowtrace-claude-code
 * 
 * Or manually run:
 *   npx @sochdb/flowtrace-claude-code
 */

module.exports = {
  name: 'flowtrace-claude-code',
  version: '0.1.0',
  install: require('./bin/install').install,
};
