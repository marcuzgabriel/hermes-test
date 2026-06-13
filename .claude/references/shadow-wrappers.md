# Shadow Wrappers — Mock Isolation via Proxy Shims

## The Problem

In a single esbuild bundle, a module is resolved ONCE and inlined. Multiple test files
that mock the SAME module with DIFFERENT implementations all get the same real module.

Two variants:
1. **Aliased paths**: `@scope/hooks/redux/useRedux` via tsconfig alias → `./src/hooks/...`
2. **npm packages**: `jwt-decode`, `react-redux` via node_modules

## The Solution: One Strategy — Shadow Wrappers Everywhere

### For Aliased Paths: Shadow Directory Tree

Create a shadow directory tree mirroring the alias target:
- **Non-mocked paths**: symlinked to real source
- **Mocked paths**: CJS Proxy wrapper files

```js
// Shadow wrapper for @scope/hooks/redux/useRedux
var _loaded = null; var _loading = false;
function _getReal() {
  if (_loaded) return _loaded;
  if (_loading) return {};
  _loading = true;
  _loaded = require("@__ht_real/scope/hooks/redux/useRedux");
  _loading = false;
  return _loaded;
}
module.exports = new Proxy({}, {
  get: function(t, p) {
    if (p === '__esModule') return true;
    var fm = globalThis.__HT_file_mocks;
    var f = globalThis.__currentTestFile;
    var m = fm && f && fm[f] && fm[f]['@scope/hooks/redux/useRedux'];
    if (m && p in m) return m[p];  // mock registered → use it
    return _getReal()[p];           // no mock → real module
  }
});
```

esbuild alias: `@scope/hooks` → shadow directory
Real module alias: `@__ht_real/scope/hooks` → real `./src/hooks`

### For npm Packages: Package Shims

Auto-generated CJS Proxy wrappers — same pattern as shadow wrappers:

```js
// Auto-generated shim for jwt-decode
// Same Proxy pattern, different alias prefix
_loaded = require("@__ht_real_pkg/jwt-decode");
// ... same Proxy get/ownKeys/getOwnPropertyDescriptor ...
```

esbuild alias: `jwt-decode` → shim file
Real module alias: `@__ht_real_pkg/jwt-decode` → real `jwt-decode` package

**This replaces externalization.** The real module stays in the bundle. No `__HT_noop` fallback.

### Why It Works — ESM vs CJS and esbuild's handling

**ESM imports are hoisted and statically resolved.** esbuild turns them into direct variable bindings at build time — by runtime, the import is gone. There's no function call or object access to intercept:

```js
// What you write (ESM):
import { useSelector } from 'react-redux';

// What esbuild outputs in the bundle:
var useSelector = /* direct reference to the function */;
// → No interception point. The import is baked in.
```

**CJS stays as a runtime operation.** It's an object with properties, accessed dynamically. esbuild wraps it with `__toESM`, which creates property getters that execute at access time:

```js
// What you write (CJS):
const { useSelector } = require('react-redux');

// What esbuild outputs (with __toESM wrapper):
var react_redux = __toESM(require_react_redux());
// access: react_redux.useSelector → goes through getter → hits our Proxy
```

Shadow wrappers and package shims are deliberately written as CJS (`module.exports = new Proxy(...)`) to force esbuild into the `__toESM` path. This preserves runtime property access, which is where the Proxy intercepts and checks per-file mocks.

This is why relative imports (`../redux/useRedux`) can't be mocked — esbuild resolves them as ESM top-level vars at build time, so there's nothing to intercept at runtime.

In detail:
1. **CJS `module.exports = Proxy`** — esbuild sees CJS syntax, triggers `__toESM` wrapping
2. **`__toESM` patch** — returns Proxy directly when `__esModule` is true
3. **Live getters** — every `import { X }` access goes through `Proxy.get` at runtime
4. **Lazy `_getReal()`** — avoids circular dependency crashes at init time
5. **Per-file scoping** — `__currentTestFile` set before each file's require, Proxy reads it at access time

### What It Doesn't Handle

**Relative imports** (`../redux/useRedux`): esbuild resolves these to shared top-level
variables at bundle time. No filesystem-level or Proxy interception point exists.
Fix: use `withStore` pattern (real Redux store, no mocking needed).

## Split Mode

- **Vendor bundle**: all npm packages + aliased source (aliases resolved, real code bundled)
- **Group bundles**: 10 test files each with `--packages=external`, shadow wrappers + package shims applied
- Vendor passes empty `mock_modules` so aliases are NOT skipped

## Results

| Metric | Before package shims | After package shims | Final (Day 21) |
|---|---|---|---|
| Topdanmark | 1315/1458 (90.2%) | 1367/1458 (93.8%) | **1472/1472 (100%)** |
| Split mode | Working | Working, 3.3s | Working, ~2s |
| expo-app | 1411/1411 | 1411/1411 | 1411/1411 |

The remaining failures after package shims were solved by function Proxy apply traps, ecosystem wrapper shims, RTK contamination fixes, and withApiStore+mockFetch rewrites (see `mock-strategies.md` and `challenges.md`).

## Implementation

- `create_shadow_wrappers()` in `bundler.rs`: shadow directory tree for aliased mocks
- `create_package_shims()` in `bundler.rs`: auto-generated CJS wrappers for npm packages
- `create_shadow_tree()`: recursive, handles symlinks + Proxy wrapper generation
- `bundle_split()`: vendor with resolved aliases, groups with shadow wrappers + shims
- Cleanup: shadow dirs + shim dir deleted after bundling
