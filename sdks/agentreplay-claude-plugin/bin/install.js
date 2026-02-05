#!/usr/bin/env node
/*
 * Plugin installer for Claude Code
 * Copies plugin files to ~/.claude/plugins/agentreplay
 */

const nodeFs = require('node:fs');
const nodePath = require('node:path');
const nodeOs = require('node:os');

const TARGET_NAME = 'agentreplay';

function copyTree(from, to) {
  const stat = nodeFs.statSync(from);
  
  if (stat.isDirectory()) {
    nodeFs.mkdirSync(to, { recursive: true });
    for (const entry of nodeFs.readdirSync(from)) {
      copyTree(nodePath.join(from, entry), nodePath.join(to, entry));
    }
  } else {
    nodeFs.copyFileSync(from, to);
  }
}

function run() {
  const home = nodeOs.homedir();
  const pluginsRoot = nodePath.join(home, '.claude', 'plugins');
  const targetPath = nodePath.join(pluginsRoot, TARGET_NAME);
  const sourcePath = nodePath.join(__dirname, '..', 'plugin');

  if (!nodeFs.existsSync(sourcePath)) {
    console.error('Plugin source not found:', sourcePath);
    process.exit(1);
  }

  console.log('Installing Agent Replay plugin for Claude Code...');
  console.log(`  From: ${sourcePath}`);
  console.log(`  To:   ${targetPath}`);

  try {
    nodeFs.mkdirSync(pluginsRoot, { recursive: true });

    if (nodeFs.existsSync(targetPath)) {
      nodeFs.rmSync(targetPath, { recursive: true });
    }

    copyTree(sourcePath, targetPath);

    console.log('');
    console.log('Installation complete!');
    console.log('');
    console.log('Environment variables (optional):');
    console.log('  AGENTREPLAY_URL=http://localhost:47100');
    console.log('  AGENTREPLAY_TENANT_ID=1');
    console.log('  AGENTREPLAY_PROJECT_ID=1');
    console.log('  AGENTREPLAY_DEBUG=true');
    console.log('');
    console.log('Features enabled:');
    console.log('  - Session tracing (observability)');
    console.log('  - Persistent memory (context injection)');
    console.log('');
  } catch (err) {
    console.error('Installation failed:', err.message);
    process.exit(1);
  }
}

if (require.main === module) {
  run();
}

module.exports = { run };
