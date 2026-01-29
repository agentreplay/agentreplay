/**
 * Agentreplay Claude Code Plugin
 * 
 * Automatic tracing of all Claude Code tool calls and sessions.
 * 
 * Installation:
 *   npm install -g @sochdb/agentreplay-claude-code
 * 
 * Or manually run:
 *   npx @sochdb/agentreplay-claude-code
 */

module.exports = {
  name: 'agentreplay-claude-code',
  version: '0.1.0',
  install: require('./bin/install').install,
};
