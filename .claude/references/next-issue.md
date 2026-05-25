# Next Issue: store-selectors coverage counter breaks react-redux init

## Status
38/38 tests pass with `--coverage`. Only store-selectors (withStore + react-redux) fails.

## What works
- AST-informed string insertion (OXC parser, bare-body wrapping)
- Vendor exclusion via `__commonJS`/`__esm` path detection in AST
- Split preamble (maps on disk, only counters at runtime)
- Only 29 user-code counters for store-selectors (zero in vendor)

## The bug
Even with only 29 counters inside the user's `__esm` callback, store-selectors fails:
```
Cannot read property 'prototype' of undefined
```

The `__esm` callback for `store-selectors.test.ts`:
```js
"src/examples/store-selectors.test.ts"() {__cov.f[0]++;
  __cov.s[0]++;init_src();
  __cov.s[1]++;init_react_redux();
  __cov.s[2]++;import_react = __toESM(require_react());
  ...
}
```

Without counters, this works. With counters, `useSelector` / `Provider` / `Component.prototype` becomes undefined.

## Debugging approach
1. Binary search: remove half the counters, see if it still fails
2. Check if `__cov.f[0]++` (function entry counter) at the `__esm` callback start is the issue — it runs before `init_react_redux()` but `__cov` should already exist
3. Check if the preamble's `var __cov` declaration interferes with esbuild's `var` hoisting inside the IIFE
4. Check if `init_react_redux()` behavior changes when preceded by `__cov.s[1]++` — maybe the comma expression inside `__esm`'s `__init` function interacts badly with the counter semicolon

## Key files
- `crates/hermes-test-cli/src/coverage.rs` — instrument_bundle, walk_stmt, walk_expr, detect_module_call
- `examples/expo-app/src/examples/store-selectors.test.ts` — the failing test
- `.hermes-test-cache/instrumented-debug.js` — dumped instrumented bundle (enable debug write in main.rs)
