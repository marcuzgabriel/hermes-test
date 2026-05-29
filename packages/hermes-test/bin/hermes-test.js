#!/usr/bin/env node
"use strict";

const { execFileSync } = require("child_process");
const path = require("path");
const os = require("os");

const platform = os.platform();
const arch = os.arch();

const pkgMap = {
  "darwin-arm64": "@hermes-test/darwin-arm64",
  "darwin-x64": "@hermes-test/darwin-x64",
  "linux-x64": "@hermes-test/linux-x64",
};

const key = `${platform}-${arch}`;
const pkg = pkgMap[key];

if (!pkg) {
  console.error(`hermes-test: unsupported platform ${key}`);
  console.error(`Supported: ${Object.keys(pkgMap).join(", ")}`);
  process.exit(1);
}

let binPath;
try {
  // Search from cwd and script dir so the platform package is found
  // even when hoisted to a different node_modules in monorepos (bun/pnpm/yarn workspaces)
  const resolved = require.resolve(`${pkg}/package.json`, {
    paths: [process.cwd(), path.join(__dirname, "..")]
  });
  binPath = path.join(path.dirname(resolved), "bin", "hermes-test");
} catch {
  console.error(`hermes-test: platform package ${pkg} not installed.`);
  console.error(`Run: npm install --save-dev ${pkg}`);
  process.exit(1);
}

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: "inherit" });
} catch (e) {
  process.exit(e.status ?? 1);
}
