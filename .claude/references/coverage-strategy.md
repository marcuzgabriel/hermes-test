# Coverage Strategy — Final Analysis

## What we tried and what happened

| Approach | Result | Problem |
|----------|--------|---------|
| Heuristic string insertion (original) | 537/1054 on large bundles | `is_safe_insert_point` guesses wrong on bare bodies, object literals |
| OXC VisitMut + Codegen | Same pass rate | Codegen **reformats source** (changes quotes, whitespace), breaks runtime |
| AST-informed string insertion | **38/38 pass** (medium bundles) | Large bundles (store-selectors + redux 300KB) still fail |
| Per-file overlay directory | 38/38 but overlay broke store-selectors | **Fought the bundler** — node_modules resolution, aliases, shadow wrappers all broken |

## What's actually working now

- OXC parser (read-only) detects bare bodies from AST context
- String insertion preserves source byte-for-byte
- Bare bodies wrapped: `if (x) stmt;` → `if (x) {__cov.s[N]++;stmt;}`
- Concise arrows skipped
- Post-bundle: bundler handles all resolution, coverage just adds counters

## The REAL root cause of the large-bundle failure

**It's the preamble size, not the counters.**

For store-selectors (300KB bundle, ~3900 statements):
- `statementMap` alone: ~3900 entries × ~100 bytes = **~400KB of JSON**
- Plus `fnMap`, `branchMap`, `s`, `f`, `b` zero maps
- Total preamble: **618KB on a single line**
- Instrumented bundle: 300KB source + 618KB preamble + counters = **~1MB**

Hermes chokes on this. The original 300KB bundle runs fine. The counters add maybe 80KB. But the **618KB JSON preamble** embedded as a JS object literal on one line is the killer.

## How Istanbul actually solves this

Istanbul instruments **per-file, before bundling**. Each file gets:

```js
function cov_abc123() {
  var coverageData = {
    path: "src/utils.ts",
    statementMap: { /* ~20 entries, ~2KB */ },
    s: { "0": 0, "1": 0, ... },
    // ...
  };
  var coverage = global.__coverage__ || (global.__coverage__ = {});
  if (!coverage[path]) coverage[path] = coverageData;
  return coverage[path];
}
```

Each file's preamble is **2-5KB** (20-50 statements per file). Even with 100 files, total overhead is ~300KB distributed across the bundle, not concentrated in one spot.

Vitest does the same: `istanbul-lib-instrument` in `onFileTransform` hook, per-file, before Vite bundles.

## Why we can't do per-file easily

- esbuild CLI has **no plugin API** (only JS API has plugins)
- The overlay approach broke module resolution (fought the bundler)
- The bundler's resolution logic (aliases, shadow wrappers, shims, mock require) is complex and battle-tested — can't bypass it

## The fix: Split the preamble from the counters

**Don't embed the coverage map in the preamble. Only embed the counters.**

### Current approach (broken at scale):
```js
var __cov=(function(){
  var g=globalThis;
  g.__coverage__=g.__coverage__||{};
  g.__coverage__["bundle.js"]={
    path:"bundle.js",
    statementMap:{/* 400KB */},
    fnMap:{/* 100KB */},
    branchMap:{/* 50KB */},
    s:{"0":0,"1":0,...},  // 30KB
    f:{"0":0,...},         // 5KB
    b:{"0":[0,0],...}      // 10KB
  };
  return g.__coverage__["bundle.js"];
})()
```
**Total preamble: ~618KB** ← this kills Hermes

### New approach:
```js
var __cov=(function(){
  var g=globalThis;
  g.__coverage__=g.__coverage__||{};
  if(!g.__coverage__["bundle.js"])g.__coverage__["bundle.js"]={
    s:{"0":0,"1":0,...},
    f:{"0":0,...},
    b:{"0":[0,0],...}
  };
  return g.__coverage__["bundle.js"];
})()
```
**Total preamble: ~50KB** ← Hermes handles this fine

The `statementMap`, `fnMap`, `branchMap` are written to a **separate JSON file** (`.hermes-test-cache/coverage-map.json`). After test execution:

1. Collect `__coverage__` from Hermes (only has `s`, `f`, `b` counters)
2. Read the coverage map JSON from disk
3. Merge counters + map → produce lcov

### Why this works:
- Preamble shrinks from 618KB to ~50KB (just zero-initialized counters)
- The coverage map JSON never enters Hermes at all
- Counter insertions are unchanged (already working)
- lcov generation merges at the end (already have the code for this)
- No bundler changes needed — still post-bundle instrumentation

### Implementation (small change):
1. `instrument_bundle()`: split preamble generation — embed only `s`/`f`/`b`, return map data separately
2. `main.rs`: write map data to `.hermes-test-cache/coverage-map.json`
3. `collect_coverage()`: read map from disk, merge with runtime counters, then produce lcov

## Performance target

- Instrumentation: <100ms (OXC parse + string insertion on 300KB = fast)
- Preamble: ~50KB instead of 618KB
- Total bundle overhead: ~130KB (50KB preamble + 80KB counters) instead of 700KB
- Should keep total under 15 seconds even for large test suites

## References
- [How Istanbul works](https://gist.github.com/robertknight/834452c3b06963ff2a8b9682fd4189cb) — per-file cov_HASH() function pattern
- [Vitest coverage collection](https://deepwiki.com/vitest-dev/vitest/4.2-coverage-collection) — onFileTransform hook
- [esbuild coverage issue #184](https://github.com/evanw/esbuild/issues/184) — out of scope for esbuild
- [Istanbul instrumenter API](https://github.com/istanbuljs/istanbuljs/blob/main/packages/istanbul-lib-instrument/src/instrumenter.js)
- [V8 vs Istanbul comparison](https://dev.to/stevez/v8-coverage-vs-istanbul-performance-and-accuracy-3ei8)
