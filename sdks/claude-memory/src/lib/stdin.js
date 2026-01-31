/**
 * Stdin/Stdout utilities for Claude Code plugin hooks
 */

async function readStdin() {
  return new Promise((resolve, reject) => {
    let data = '';
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', (chunk) => {
      data += chunk;
    });
    process.stdin.on('end', () => {
      try {
        resolve(data.trim() ? JSON.parse(data) : {});
      } catch (err) {
        reject(new Error(`Failed to parse stdin JSON: ${err.message}`));
      }
    });
    process.stdin.on('error', reject);
    if (process.stdin.isTTY) resolve({});
  });
}

function writeOutput(data) {
  console.log(JSON.stringify(data));
}

function outputSuccess(additionalContext = null) {
  if (additionalContext) {
    writeOutput({
      hookSpecificOutput: { hookEventName: 'SessionStart', additionalContext },
    });
  } else {
    writeOutput({ continue: true, suppressOutput: true });
  }
}

function outputError(message) {
  console.error(`AgentReplay: ${message}`);
  writeOutput({ continue: true, suppressOutput: true });
}

module.exports = { readStdin, writeOutput, outputSuccess, outputError };
