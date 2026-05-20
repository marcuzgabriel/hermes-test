#!/usr/bin/env node
// Bin shim: locates the platform-specific hermes-test binary.
// 1. Checks for local dev binary (cargo build symlink)
// 2. Falls back to platform-specific optionalDependency package
const { execFileSync } = require('child_process');
const fs = require('fs');
const path = require('path');

// Local dev: check for the cargo-built binary symlink
const localBin = path.join(__dirname, 'hermes-test');
if (fs.existsSync(localBin)) {
  try {
    execFileSync(localBin, process.argv.slice(2), { stdio: 'inherit' });
    process.exit(0);
  } catch (e) {
    process.exit(e.status || 1);
  }
}

// Published: resolve from platform-specific optionalDependency
const PLATFORM_MAP = {
  'darwin-arm64': '@marcuzgabriel/hermes-test-darwin-arm64',
  'linux-x64': '@marcuzgabriel/hermes-test-linux-x64',
};

const key = `${process.platform}-${process.arch}`;
const pkg = PLATFORM_MAP[key];

if (!pkg) {
  console.error(`hermes-test: unsupported platform ${key}`);
  console.error('Supported: darwin-arm64, linux-x64');
  process.exit(1);
}

let binPath;
try {
  binPath = path.join(require.resolve(`${pkg}/package.json`), '..', 'bin', 'hermes-test');
} catch {
  console.error(`hermes-test: platform package ${pkg} not found`);
  console.error('Try reinstalling: npm install @marcuzgabriel/hermes-test');
  process.exit(1);
}

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: 'inherit' });
} catch (e) {
  process.exit(e.status || 1);
}
