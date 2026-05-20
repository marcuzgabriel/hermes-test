#!/usr/bin/env node
// Selects the correct platform binary from the bundled bin/ directory.

const fs = require('fs');
const path = require('path');
const os = require('os');

// Skip in CI — we're building the binary, not installing it
if (process.env.CI) {
  process.exit(0);
}

const BIN_DIR = path.join(__dirname, '..', 'bin');
const BIN_PATH = path.join(BIN_DIR, 'hermes-test');

// Skip if binary already exists and is executable
if (fs.existsSync(BIN_PATH)) {
  try {
    const stat = fs.lstatSync(BIN_PATH);
    if (!stat.isSymbolicLink()) process.exit(0);
    fs.accessSync(fs.realpathSync(BIN_PATH), fs.constants.X_OK);
    process.exit(0);
  } catch {}
}

const platform = os.platform();
const arch = os.arch();

const PLATFORM_MAP = {
  'darwin-arm64': 'hermes-test-darwin-arm64',
  'linux-x64': 'hermes-test-linux-x64',
};

const key = `${platform}-${arch}`;
const binaryName = PLATFORM_MAP[key];

if (!binaryName) {
  console.error(`hermes-test: unsupported platform ${key}`);
  console.error('Supported: darwin-arm64, linux-x64');
  process.exit(1);
}

const src = path.join(BIN_DIR, binaryName);
if (!fs.existsSync(src)) {
  console.error(`hermes-test: binary not found at ${src}`);
  console.error('This may be a packaging issue. Try reinstalling.');
  process.exit(1);
}

// Copy platform binary to bin/hermes-test
fs.copyFileSync(src, BIN_PATH);
fs.chmodSync(BIN_PATH, 0o755);
console.log(`hermes-test: installed ${binaryName}`);
