#!/usr/bin/env node
// hermes-test plugin bundler — runs esbuild via its JS API so an onResolve hook
// can serve mock wrapper files for relative-path mocks. Invoked by the Rust CLI
// (HT_RESOLVER=plugin) with a JSON config path as the only argument.
// The config's `args` array contains the exact CLI flag strings the legacy path
// uses; they are parsed into JS API options here so the two modes cannot drift.
'use strict';

const fs = require('fs');
const path = require('path');

const cfg = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const esbuild = require(cfg.esbuildLib);

const opts = {
  entryPoints: [cfg.entry],
  outfile: cfg.out,
  write: true,
  logLevel: 'silent',
  // The CLI reads NODE_PATH from the environment; the JS API needs it explicitly.
  nodePaths: cfg.nodePaths || [],
  alias: {},
  external: [],
  define: {},
  loader: {},
  supported: {},
};

for (const a of cfg.args) {
  if (a === '--bundle') opts.bundle = true;
  else if (a.startsWith('--format=')) opts.format = a.slice(9);
  else if (a.startsWith('--target=')) opts.target = a.slice(9);
  else if (a.startsWith('--supported:')) {
    const eq = a.indexOf('=', 12);
    opts.supported[a.slice(12, eq)] = a.slice(eq + 1) === 'true';
  } else if (a.startsWith('--define:')) {
    const eq = a.indexOf('=', 9);
    opts.define[a.slice(9, eq)] = a.slice(eq + 1);
  } else if (a.startsWith('--jsx=')) opts.jsx = a.slice(6);
  else if (a.startsWith('--loader:')) {
    const eq = a.indexOf('=', 9);
    opts.loader[a.slice(9, eq)] = a.slice(eq + 1);
  } else if (a.startsWith('--alias:')) {
    const eq = a.indexOf('=', 8);
    opts.alias[a.slice(8, eq)] = a.slice(eq + 1);
  } else if (a.startsWith('--external:')) opts.external.push(a.slice(11));
  else if (a === '--sourcemap=inline') opts.sourcemap = 'inline';
  else if (a === '--packages=external') opts.packages = 'external';
  else {
    console.error('hermes-test plugin bundler: unknown esbuild arg: ' + a);
    process.exit(1);
  }
}

// wrappers: resolved absolute target path (with extension) -> wrapper file path.
// pkgWrappers: bare package specifier -> wrapper file path.
// aliases: [name, targetDir] pairs (longest-prefix matched here, in case esbuild
// hands the hook the pre-substitution specifier).
const wrappers = cfg.wrappers;
const pkgWrappers = cfg.pkgWrappers || {};
const aliases = cfg.aliases || [];
const EXTS = ['.tsx', '.ts', '.jsx', '.js'];

// Given a lexically-resolved base path (usually extensionless), find the wrapper
// registered for it. Mirrors the Rust-side resolution (resolve_relative_file):
// exact, then extension probing, then index files. Pure string lookups against
// the wrapper map — no filesystem access on the hot path.
function wrapperFor(base) {
  if (wrappers[base]) return wrappers[base];
  for (const e of EXTS) {
    const c = base + e;
    if (wrappers[c]) return wrappers[c];
  }
  for (const e of EXTS) {
    const c = base + '/index' + e;
    if (wrappers[c]) return wrappers[c];
  }
  return undefined;
}

function aliasBase(spec) {
  let best;
  for (const [name, target] of aliases) {
    if (spec === name || spec.startsWith(name + '/')) {
      if (!best || name.length > best[0].length) best = [name, target];
    }
  }
  if (!best) return undefined;
  const rem = spec.slice(best[0].length).replace(/^\//, '');
  return rem ? path.join(best[1], rem) : best[1];
}

const mockPlugin = {
  name: 'ht-mocks',
  setup(build) {
    // Pass-through marker: wrappers import the real module as "<spec>?ht-real".
    // Absolute paths load directly; bare package specifiers go through esbuild's
    // own resolver (pluginData guards against re-entering this plugin).
    build.onResolve({ filter: /\?ht-real$/ }, (args) => {
      const stripped = args.path.slice(0, -'?ht-real'.length);
      if (stripped[0] === '/') return { path: stripped };
      return build.resolve(stripped, {
        kind: 'require-call',
        resolveDir: cfg.resolveDir,
        pluginData: 'ht-real',
      });
    });

    // Candidate imports only (cfg.filter pre-screens by mocked basenames and
    // package names on the Go side).
    build.onResolve({ filter: new RegExp(cfg.filter) }, (args) => {
      if (args.pluginData === 'ht-real') return undefined;
      // Bare package mock — exact specifier match.
      if (pkgWrappers[args.path]) return { path: pkgWrappers[args.path] };
      // Relative import — resolve against the importer.
      if (args.path[0] === '.') {
        if (!args.importer) return undefined;
        const base = path.resolve(path.dirname(args.importer), args.path);
        const hit = wrapperFor(base);
        return hit ? { path: hit } : undefined;
      }
      // Absolute path (e.g. after esbuild alias substitution).
      if (args.path[0] === '/') {
        const hit = wrapperFor(args.path);
        return hit ? { path: hit } : undefined;
      }
      // Alias-prefixed bare specifier (pre-substitution).
      const base = aliasBase(args.path);
      if (base) {
        const hit = wrapperFor(base);
        return hit ? { path: hit } : undefined;
      }
      return undefined;
    });
  },
};

// No mocked targets → no interception needed → skip the plugin entirely so no
// import pays a Go→JS round trip.
opts.plugins =
  Object.keys(wrappers).length > 0 || Object.keys(pkgWrappers).length > 0 ? [mockPlugin] : [];

esbuild
  .build(opts)
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(String((err && err.message) || err));
    process.exit(1);
  });
