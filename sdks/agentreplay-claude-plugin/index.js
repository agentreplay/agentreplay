/*
 * Agent Replay Plugin for Claude Code
 * 
 * Provides:
 * - Observability: Automatic tracing of tool calls and sessions
 * - Memory: Persistent context storage and retrieval
 */

module.exports = {
  name: 'agentreplay-claude-plugin',
  version: '0.2.0',
  
  // Programmatic installation
  install: require('./bin/install').run,
  
  // Feature flags
  features: {
    tracing: true,
    memory: true,
  },
};
