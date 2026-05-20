#!/usr/bin/env node
// Downloads the hermes-test binary from GitHub Releases for the current platform.
// Runs automatically after `bun add` / `npm install`.

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');
const os = require('os');

// Skip in CI — we're building the binary, not downloading it
if (process.env.CI) {
  process.exit(0);
}

const REPO = 'marcuzgabriel/hermes-test';
const BIN_DIR = path.join(__dirname, '..', 'bin');
const BIN_PATH = path.join(BIN_DIR, 'hermes-test');

// Skip if binary already exists (dev/workspace mode)
if (fs.existsSync(BIN_PATH) && !fs.lstatSync(BIN_PATH).isSymbolicLink()) {
  process.exit(0);
}
// Skip if it's a symlink (workspace dev mode)
if (fs.existsSync(BIN_PATH) && fs.lstatSync(BIN_PATH).isSymbolicLink()) {
  try {
    fs.accessSync(fs.realpathSync(BIN_PATH), fs.constants.X_OK);
    process.exit(0);
  } catch {}
}

function findNpmrcToken() {
  // Walk up from cwd looking for .npmrc with a GitHub Packages token
  let dir = process.cwd();
  while (true) {
    const npmrc = path.join(dir, '.npmrc');
    if (fs.existsSync(npmrc)) {
      const content = fs.readFileSync(npmrc, 'utf8');
      const match = content.match(/\/\/npm\.pkg\.github\.com\/:_authToken=(.+)/);
      if (match) {
        // Resolve env var references like ${NPM_AUTH_TOKEN}
        const val = match[1].trim();
        const envMatch = val.match(/^\$\{(.+)\}$/);
        if (envMatch) return process.env[envMatch[1]] || '';
        return val;
      }
    }
    const parent = path.dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return '';
}

const platform = os.platform();
const arch = os.arch();

const PLATFORM_MAP = {
  'darwin-arm64': 'hermes-test-darwin-arm64',
  'darwin-x64': 'hermes-test-darwin-x64',
  'linux-x64': 'hermes-test-linux-x64',
  'linux-arm64': 'hermes-test-linux-arm64',
};

const key = `${platform}-${arch}`;
const assetName = PLATFORM_MAP[key];

if (!assetName) {
  console.error(`hermes-test: unsupported platform ${key}`);
  console.error('Supported: darwin-arm64, darwin-x64, linux-x64, linux-arm64');
  process.exit(1);
}

// Get latest release URL
const pkg = require('../package.json');
const version = pkg.version;

const url = `https://github.com/${REPO}/releases/download/v${version}/${assetName}`;

console.log(`hermes-test: downloading ${assetName} from v${version}...`);

try {
  fs.mkdirSync(BIN_DIR, { recursive: true });

  // Remove old symlink if it exists
  if (fs.existsSync(BIN_PATH)) {
    fs.unlinkSync(BIN_PATH);
  }

  // Resolve auth token: env vars first, then .npmrc
  const token = process.env.GITHUB_TOKEN || process.env.GH_TOKEN || process.env.NPM_AUTH_TOKEN || findNpmrcToken();
  const authHeader = token ? `-H "Authorization: token ${token}" -H "Accept: application/octet-stream"` : '';
  execSync(`curl -fSL --retry 3 ${authHeader} -o "${BIN_PATH}" "${url}"`, { stdio: 'inherit' });
  fs.chmodSync(BIN_PATH, 0o755);

  console.log('hermes-test: binary installed successfully');
} catch (err) {
  console.error(`hermes-test: failed to download binary from ${url}`);
  console.error('Make sure the release exists. For development, build from source:');
  console.error('  cargo build --release');
  process.exit(1);
}
