#!/usr/bin/env node
// hermes-test plugin bundler — runs esbuild via its JS API so an onResolve hook
// can serve mock wrapper files for relative-path mocks. Invoked by the Rust CLI
// with a JSON config path as the only argument.
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
const textWrappers = cfg.textWrappers || {};
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

// Anything imported with a non-code extension is an asset (fonts from @expo/vector-icons,
// images, audio, …). Metro treats those as assets; esbuild would fail with "No loader is
// configured". One catch-all instead of a per-extension loader list.
const CODE_EXT = /\.(m?[jt]sx?|c[jt]s|json)$/i;
const assetPlugin = {
  name: 'ht-assets',
  setup(build) {
    build.onResolve({ filter: /\.[a-z0-9]+$/i }, (args) => {
      if (CODE_EXT.test(args.path) || args.namespace === 'ht-asset') return undefined;
      // Only file-ish specifiers: relative, absolute, or package paths with an extension.
      const ext = args.path.slice(args.path.lastIndexOf('.') + 1).toLowerCase();
      if (opts.loader['.' + ext] || ext.length > 5) return undefined;
      return { path: args.path, namespace: 'ht-asset' };
    });
    build.onLoad({ filter: /.*/, namespace: 'ht-asset' }, () => ({
      contents: 'module.exports = {};',
      loader: 'js',
    }));
  },
};

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
      if (process.env.HT_DEBUG_RESOLVE) {
        console.error(`[ht-resolve] path=${args.path} importer=${args.importer}`);
      }
      if (args.pluginData === 'ht-real') return undefined;
      // Text-matched mocks (alias specifiers, packages, barrel ancestors):
      // EXACT import-text match — the legacy shadow-tree/package-shim boundary.
      // Differently-spelled routes to the same file (production-internal
      // relative imports) intentionally get the real module, so module-level
      // init-time reads can't capture another test file's mocks.
      if (textWrappers[args.path]) return { path: textWrappers[args.path] };
      // Relative-mock targets: identity matching — resolve against the importer
      // so the mock applies however the consumer spells the path.
      if (args.path[0] === '.') {
        if (!args.importer) return undefined;
        const base = path.resolve(path.dirname(args.importer), args.path);
        const hit = wrapperFor(base);
        return hit ? { path: hit } : undefined;
      }
      return undefined;
    });
  },
};

// No mocked targets → no interception needed → skip the plugin entirely so no
// import pays a Go→JS round trip.
// The asset catch-all is cheap (onResolve filter is a plain extension regex) and always on.
opts.plugins = [assetPlugin];
if (Object.keys(wrappers).length > 0 || Object.keys(textWrappers).length > 0) opts.plugins.push(mockPlugin);

esbuild
  .build(opts)
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(String((err && err.message) || err));
    process.exit(1);
  });
