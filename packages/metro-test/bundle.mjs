// Bundle the harness into a single JS file that can be injected into Hermes
import { build } from 'esbuild';
import { readFileSync } from 'fs';

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
  external: ['react', 'react-test-renderer'],
});

console.log('Harness bundled to dist/harness.bundle.js');
console.log('Size:', readFileSync('dist/harness.bundle.js').length, 'bytes');
