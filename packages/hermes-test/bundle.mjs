// Bundle the harness into a single JS file that can be injected into Hermes
import { build } from 'esbuild';
import { readFileSync, writeFileSync } from 'fs';

const polyfills = readFileSync('src/polyfills.js', 'utf8');

await build({
  entryPoints: ['src/harness.ts'],
  bundle: true,
  format: 'iife',
  globalName: '__metroTestHarness',
  outfile: 'dist/harness.bundle.js',
  target: 'es2020',
  minify: false,
  platform: 'neutral',
  banner: { js: polyfills },
  external: ['react'],
  // react is externalized — resolved at runtime from globalThis.__HT_React.
  // react-reconciler is NOT imported by the harness — it's loaded at runtime
  // via globalThis.__HT_Reconciler (set by the CLI entry from the user's node_modules).
  // This ensures the reconciler always matches the user's React version.
  alias: {
    react: './src/shims/react.js',
  },
});

// Apply Hermes patches (same as metro.rs patch_esbuild_for_hermes)
let code = readFileSync('dist/harness.bundle.js', 'utf8');
code = code.replace(
  `for (let key of __getOwnPropNames(from))\n        if (!__hasOwnProp.call(to, key) && key !== except)\n          __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });`,
  `var keys = __getOwnPropNames(from);\n      for (var i = 0; i < keys.length; i++) {\n        var key = keys[i];\n        if (!__hasOwnProp.call(to, key) && key !== except)\n          __defProp(to, key, { get: ((k) => from[k]).bind(null, key), enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable, configurable: true });\n      }`,
);
code = code.replace(
  '{ get: all[name], enumerable: true }',
  '{ get: all[name], enumerable: true, configurable: true }',
);
writeFileSync('dist/harness.bundle.js', code);

console.log('Harness bundled to dist/harness.bundle.js');
console.log('Size:', readFileSync('dist/harness.bundle.js').length, 'bytes');
