// Pattern: user-provided shim for a package the bundler cannot parse
// Demonstrates: "shims": { "flow-only-lib": "./src/shims/flow-only-lib.js" } in
// hermes-test.config.json externalizes the real package AND serves the shim at runtime.
// Before 1.3.1 a user shim only registered the runtime replacement — the real package was
// still bundled, so Flow syntax / font assets inside it failed the build and the shim never
// took effect.
import { test, group, expect } from 'hermes-test';
import FlowOnlyThing from 'flow-only-lib';

group('user shims', () => {
  test('a configured shim replaces an unbundleable package', () => {
    expect(FlowOnlyThing()).toBe('shimmed-flow-only-lib');
  });
});
