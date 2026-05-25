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

## Root cause hypothesis
The `__esm` pattern uses `fn && (res = (0, fn[key])(fn = 0)), res` — a comma expression
that calls the module callback. Inserting `__cov.f[0]++;` at the start of that callback
may change how `__init()` dispatches or how the function body interacts with the comma
expression. The same pattern works fine for simpler test files (no redux/Provider).

## Fix approach — use existing bundler patterns
The shadow wrapper / shimming approach already solves this class of problem:
- Don't instrument INSIDE `__esm`/`__commonJS` callback bodies at all
- These are esbuild's module wrapper functions — their internal structure is fragile
- Instead, instrument at the CALL SITE level: the `init_X()` and `require_X()` calls
  at the top of each module's `__esm` callback are already instrumented as statements
- The test() / group() / beforeEach() callbacks ARE instrumented (they're user functions)
- This means: set `vendor=true` for ALL `__esm`/`__commonJS` callback bodies,
  but keep `vendor=false` for function expressions defined INSIDE user module callbacks
  (test callbacks, hook definitions, etc.)

Alternatively: only skip the function entry counter (`__cov.f[N]++`) for `__esm`/`__commonJS`
method bodies, since those interact with the `__init` dispatch pattern.

## Key files
- `crates/hermes-test-cli/src/coverage.rs` — instrument_bundle, walk_stmt, walk_expr, detect_module_call
- `examples/expo-app/src/examples/store-selectors.test.ts` — the failing test
- `.hermes-test-cache/instrumented-debug.js` — dumped instrumented bundle (enable debug write in main.rs)
