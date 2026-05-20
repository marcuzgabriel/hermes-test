#!/usr/bin/env node
// Replaces the Node.js shim in node_modules/.bin/hermes-test with the
// native binary from the platform-specific package. Zero overhead at runtime.
// Same pattern as esbuild.
const fs = require('fs');
const path = require('path');

const PLATFORM_MAP = {
  'darwin-arm64': '@marcuzgabriel/hermes-test-darwin-arm64',
  'linux-x64': '@marcuzgabriel/hermes-test-linux-x64',
};

const key = `${process.platform}-${process.arch}`;
const pkg = PLATFORM_MAP[key];
if (!pkg) process.exit(0);

let nativeBin;
try {
  nativeBin = path.join(require.resolve(`${pkg}/package.json`), '..', 'bin', 'hermes-test');
} catch {
  // Platform package not installed (e.g. --no-optional), shim will be used as fallback
  process.exit(0);
}

if (!fs.existsSync(nativeBin)) process.exit(0);

// Find node_modules/.bin/hermes-test and replace it with the native binary
const binLink = path.join(__dirname, '..', '..', '..', '.bin', 'hermes-test');
if (fs.existsSync(binLink)) {
  try {
    fs.unlinkSync(binLink);
    fs.symlinkSync(nativeBin, binLink);
    console.log('hermes-test: using native binary (zero overhead)');
  } catch {
    // If symlinking fails, the Node.js shim still works as fallback
  }
}
