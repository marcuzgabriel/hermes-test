# Shadow Wrappers — Mock Isolation in a Single Bundle

## The Problem

In a monorepo with tsconfig path aliases (e.g., `@topdanmark/mobile-insurance-app` → `./src`),
multiple test files need to mock the SAME module with DIFFERENT implementations.

Example: `useChooseProfile.test.ts` mocks `@topdanmark/.../hooks` with one spy,
`useServicePills.test.ts` mocks it with another. Both import hook implementations
that internally use the same `@topdanmark/.../hooks` barrel.

In a single esbuild bundle, the module is resolved ONCE via the alias and inlined.
There's no interception point — all consumers get the same real module.

## Approaches Tried and Failed

### 1. Externalize mocked paths (skip alias)
- esbuild processes aliases BEFORE externals
- Skipping the alias externalizes ALL sub-paths, breaking non-mock consumers
- `paymentsUtil` imports `@topdanmark/.../types` — gets noop instead of real module

### 2. OXC AST pre-transform (Vitest-style)
- Parsed test files with OXC, rewrote imports to Proxy wrappers
- OXC semantic analysis correctly found all identifier references
- Problem: only transforms the TEST FILE's imports, not transitive deps
- Hook implementation still gets real module → "react-redux context" error

### 3. Post-bundle mock hoisting
- Regex-based: moved mockModule() before init_*() in bundled output
- Rust regex crate doesn't support lookahead
- Even with correct hoisting, the __require Proxy late-binding already handles this
- Not needed for externalized mocks (Proxy is already lazy)

### 4. Per-file isolation (worked but slow)
- Each file bundled separately with its own esbuild call
- Parallel esbuild via std::thread helped (0.73s → 0.60s)
- Still ~120ms overhead per file (Hermes runtime + esbuild)

## The Solution: Shadow Wrappers

### Concept
Create a shadow directory tree that mirrors the alias target. Non-mocked paths are
symlinks to the real source. Mocked paths are **lazy Proxy wrapper files** that check
`__HT_file_mocks[__currentTestFile]` on every property access.

### How it works

1. **Shadow directory created**: `.hermes-test-shadow-topdanmark-mobile-insurance-app/`
2. **Non-mocked paths**: symlinked → `hooks/carousel/` → real `src/hooks/carousel/`
3. **Mocked paths**: wrapper file → `hooks/index.ts` contains:

```js
var _loaded = null;
function _getReal() { if (!_loaded) _loaded = require("@__ht_real/topdanmark/.../hooks"); return _loaded; }
var _h = { get: function(t, p) {
  if (p === '__esModule') return true;
  var fm = globalThis.__HT_file_mocks;
  var f = globalThis.__currentTestFile;
  var m = fm && f && fm[f] && fm[f]['@topdanmark/.../hooks'];
  if (m && p in m) return m[p];  // mock exists → return it
  return _getReal()[p];           // no mock → return real
}};
module.exports = new Proxy({}, _h);
```

4. **esbuild alias** redirected: `@topdanmark/mobile-insurance-app` → shadow directory
5. **Real module alias** added: `@__ht_real/topdanmark/mobile-insurance-app` → real `./src`
6. **Cleanup**: shadow directory deleted after bundling

### Key Design Decisions

**Lazy loading (`_getReal()`)**: The wrapper doesn't load the real module at init time.
It loads on first property access. This avoids circular dependency crashes — module A
can import wrapped module B which imports A, without deadlocking.

**Symlinks for non-mocked paths**: Only mocked files get wrapper treatment. Everything
else symlinks directly to the real source. No duplication, no resolution changes.

**Per-file scoping via `__currentTestFile`**: The entry generator sets
`globalThis.__currentTestFile = 'useChooseProfile.test.ts'` before each test file's
require. The Proxy reads this at access time, not at init time. Different test files
get different mocks from the SAME Proxy instance.

**Index file matching**: `hooks/index.ts` matches mock path `@scope/hooks` (not just
`@scope/hooks/index`). The code checks both direct path and parent-directory-for-index.

### Comparison with Vitest

| | Vitest | hermes-test |
|---|---|---|
| When | Pre-bundle AST transform | Pre-bundle file generation |
| How | magic-string rewrites imports | Shadow directory + Proxy files |
| Isolation | Worker per file | Single bundle, runtime scoping |
| Bundling | Once per worker | Once total |
| Complexity | AST walker + codegen | Symlinks + 10-line wrappers |

### Results

| Metric | Per-file isolation | Shadow wrappers |
|---|---|---|
| Topdanmark (94 tests) | 0.89s | 0.49s |
| Architecture | 2 bundles (batch+isolated) | 1 bundle |
| esbuild calls | 1 batch + 5 isolated | 1 total |
| Pass rate | 91/94 | 91/94 |

### Implementation

- `create_shadow_wrappers()` in `bundler.rs`: creates shadow tree, returns updated config
- `create_shadow_tree()`: recursive, handles symlinks + wrapper generation
- `make_relative()`: computes relative paths for symlinks
- `bundle_auto_with_config()`: bundles with custom (shadow) alias config
- Cleanup in `run_tests_single()`: removes shadow dirs after bundling
