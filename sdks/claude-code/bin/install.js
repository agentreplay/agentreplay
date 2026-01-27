#!/usr/bin/env node
/**
 * Flowtrace Claude Code Plugin Installer
 * 
 * Copies the plugin to ~/.claude/plugins/flowtrace
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

const PLUGIN_NAME = 'flowtrace';

function copyRecursive(src, dest) {
  const stats = fs.statSync(src);
  
  if (stats.isDirectory()) {
    if (!fs.existsSync(dest)) {
      fs.mkdirSync(dest, { recursive: true });
    }
    const files = fs.readdirSync(src);
    for (const file of files) {
      copyRecursive(path.join(src, file), path.join(dest, file));
    }
  } else {
    fs.copyFileSync(src, dest);
  }
}

function install() {
  const homeDir = os.homedir();
  const claudePluginsDir = path.join(homeDir, '.claude', 'plugins');
  const destDir = path.join(claudePluginsDir, PLUGIN_NAME);
  
  // Source is the plugin directory in the npm package
  const srcDir = path.join(__dirname, '..', 'plugin');
  
  if (!fs.existsSync(srcDir)) {
    console.error('❌ Plugin source directory not found:', srcDir);
    process.exit(1);
  }
  
  console.log('📦 Installing Flowtrace plugin for Claude Code...');
  console.log(`   Source: ${srcDir}`);
  console.log(`   Destination: ${destDir}`);
  
  try {
    // Create plugins directory if needed
    if (!fs.existsSync(claudePluginsDir)) {
      fs.mkdirSync(claudePluginsDir, { recursive: true });
    }
    
    // Remove old version if exists
    if (fs.existsSync(destDir)) {
      fs.rmSync(destDir, { recursive: true });
    }
    
    // Copy plugin files
    copyRecursive(srcDir, destDir);
    
    console.log('');
    console.log('✅ Flowtrace plugin installed successfully!');
    console.log('');
    console.log('📊 Configuration (via environment variables):');
    console.log('   FLOWTRACE_URL=http://localhost:9600');
    console.log('   FLOWTRACE_TENANT_ID=1');
    console.log('   FLOWTRACE_PROJECT_ID=1');
    console.log('');
    console.log('🚀 Start Claude Code to begin tracing!');
    console.log('');
  } catch (err) {
    console.error('❌ Failed to install plugin:', err.message);
    process.exit(1);
  }
}

// Run if called directly
if (require.main === module) {
  install();
}

module.exports = { install };
