#!/usr/bin/env node
'use strict';

const { execFileSync } = require('child_process');
const path = require('path');
const os = require('os');
const fs = require('fs');

const platform = os.platform();
const arch = os.arch();
const args = process.argv.slice(2);

function resolveEngine(argv) {
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--engine' && i + 1 < argv.length) {
      return argv[i + 1].toLowerCase();
    }
    if (arg.startsWith('--engine=')) {
      return arg.slice('--engine='.length).toLowerCase();
    }
  }
  return 'hermes';
}

// Check for local development build first (target/release/hermes-test next to the package)
const localBin = path.join(__dirname, '..', '..', '..', 'target', 'release', 'hermes-test');
if (fs.existsSync(localBin)) {
  try {
    execFileSync(localBin, process.argv.slice(2), { stdio: 'inherit' });
  } catch (e) {
    process.exit(e.status ?? 1);
  }
  process.exit(0);
}

const hermesPkgMap = {
  'darwin-arm64': '@hermes-test/darwin-arm64',
  'darwin-x64': '@hermes-test/darwin-x64',
  'linux-x64': '@hermes-test/linux-x64',
};
const v8PkgMap = {
  'darwin-arm64': '@hermes-test-v8/darwin-arm64',
  'darwin-x64': '@hermes-test-v8/darwin-x64',
  'linux-x64': '@hermes-test-v8/linux-x64',
};

const key = `${platform}-${arch}`;
const engine = resolveEngine(args);
const pkgMap = engine === 'v8' ? v8PkgMap : hermesPkgMap;
const pkg = pkgMap[key];

if (!pkg) {
  console.error(`hermes-test: unsupported platform ${key}`);
  console.error(`Supported: ${Object.keys(hermesPkgMap).join(', ')}`);
  process.exit(1);
}

let binPath;
try {
  // Search from cwd and script dir so the platform package is found
  // even when hoisted to a different node_modules in monorepos (bun/pnpm/yarn workspaces)
  const resolved = require.resolve(`${pkg}/package.json`, {
    paths: [process.cwd(), path.join(__dirname, '..')],
  });
  binPath = path.join(path.dirname(resolved), 'bin', 'hermes-test');
} catch {
  console.error(`hermes-test: ${engine} platform package ${pkg} not installed.`);
  if (engine === 'v8') {
    console.error(`Run: npm install --save-dev ${pkg}`);
  } else {
    console.error(`Run: npm install --save-dev ${pkg}`);
  }
  process.exit(1);
}

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: 'inherit' });
} catch (e) {
  process.exit(e.status ?? 1);
}
