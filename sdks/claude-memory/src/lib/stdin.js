/*
 * Plugin I/O handlers for Claude Code hook communication
 */

// Parse JSON from standard input stream
function parseInput() {
  return new Promise((resolve, reject) => {
    if (process.stdin.isTTY) {
      resolve({});
      return;
    }

    const chunks = [];
    process.stdin.setEncoding('utf8');

    process.stdin.on('readable', () => {
      let chunk;
      while ((chunk = process.stdin.read()) !== null) {
        chunks.push(chunk);
      }
    });

    process.stdin.on('end', () => {
      const raw = chunks.join('');
      if (!raw.trim()) {
        resolve({});
        return;
      }
      try {
        resolve(JSON.parse(raw));
      } catch (parseErr) {
        reject(new Error(`Invalid JSON input: ${parseErr.message}`));
      }
    });

    process.stdin.on('error', reject);
  });
}

// Send structured response to stdout
function respond(payload) {
  process.stdout.write(JSON.stringify(payload) + '\n');
}

// Signal successful hook completion
function complete(injectedContext = null) {
  if (injectedContext !== null) {
    respond({
      hookSpecificOutput: {
        hookEventName: 'SessionStart',
        additionalContext: injectedContext,
      },
    });
  } else {
    respond({ continue: true, suppressOutput: true });
  }
}

// Log error and continue
function fail(msg) {
  process.stderr.write(`[AgentReplay] ${msg}\n`);
  respond({ continue: true, suppressOutput: true });
}

module.exports = { parseInput, respond, complete, fail };
