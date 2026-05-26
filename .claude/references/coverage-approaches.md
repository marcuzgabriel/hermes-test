# Coverage Approaches — What We Tried

## The Goal
`hermes-test --coverage` adds Istanbul-style counters to the bundler's output.
The bundler already produces a perfect bundle (100% test pass rate).

## What Works Now
- 38/38 tests pass with `--coverage` (7 test files, no store-selectors)
- Istanbul hoisted function pattern: `function __cov()` with `__c` cache
- Preamble inside IIFE, split maps to disk
- AST-informed bare-body wrapping, concise arrow skipping
- Post-bundle instrumentation (bundler handles all resolution)

## The One Failure: store-selectors
Uses `withStore` + react-redux + redux. Passes without coverage.
Fails with: `Cannot read property 'prototype' of undefined`

---

## Approach 1: Heuristic string insertion (original)
**What**: Scan characters around each insertion point to decide if safe.
`is_safe_insert_point()` + `is_bare_control_body()` — look at preceding chars.
**Result**: 537/1054 on large bundles. Bare bodies, object literals, template literals break it.
**Why it failed**: Heuristics can't reliably distinguish block `{` from object `{`.

## Approach 2: OXC VisitMut + Codegen
**What**: Parse AST → mutate with VisitMut → re-emit with OXC Codegen.
**Result**: Same pass rate as approach 1.
**Why it failed**: Codegen reformats source (changes quotes, whitespace), breaks runtime behavior. Hermes-specific patterns get mangled.

## Approach 3: AST-informed string insertion (current)
**What**: OXC parser (read-only) for analysis + string insertion for output. Source preserved byte-for-byte.
**Result**: 38/38 pass. This IS the working approach.
**Why store-selectors fails**: Counters in vendor code (react-redux init) break `Component.prototype` chain.

## Approach 4: Per-file overlay directory
**What**: Instrument each source file individually, write to `.hermes-test-cache/cov-src/`, redirect esbuild.
**Result**: 38/38 for non-redux tests. store-selectors fails.
**Why it failed**: esbuild resolves different package entry points from the overlay directory (ESM vs CJS react-redux). Fought the bundler's resolution logic.

## Approach 5: Per-file transforms (shadow wrapper pattern)
**What**: Instrument test files, write temp copies next to originals, use bundler's `transforms` mechanism.
**Result**: Same as overlay. store-selectors fails.
**Why it failed**: esbuild still produces different bundle structure from the temp file (different module name in `__esm`, function hoisting changes). Pre-bundle instrumentation changes how esbuild processes the file.

## Approach 6: `var __cov` preamble before IIFE
**What**: `var __cov = (function(){...})()` prepended to the bundle.
**Result**: `Object is not a function` — ASI merges `var __cov = (...)()` with `(() => {` on next line.
**Fix applied**: Added semicolon. But `var` gets split by esbuild when pre-bundle.

## Approach 7: Istanbul function pattern (current)
**What**: `function __cov(){...}` declaration (hoisted by JS spec). Counters use `__cov().s[N]++`.
**Result**: Works for 38/38. Hoisted functions can call `__cov()` safely.
**store-selectors**: Still fails because vendor code gets counters.

## Approach 8: Comment-based user range detection
**What**: Parse esbuild's `// path/to/module` comments to find vendor vs user sections.
**Result**: Broken. Regular code comments (`// If the importer...`) inside vendor code get misdetected as module boundaries, closing vendor ranges prematurely.

## Approach 9: `__esm`/`__commonJS` path detection (AST-based)
**What**: In AST walker, detect `__commonJS({...})` / `__esm({...})` calls, check path key for `node_modules`.
**Result**: Worked for single-file bundles but failed for multi-file. The walker couldn't reliably propagate vendor/user flag through the AST.

## Approach 10: Quoted path byte scanning
**What**: Scan source bytes for `"src/..."(` patterns (module method keys), brace-count to find body ranges.
**Result**: False positives — strings inside vendor code containing `"src/` matched. Brace counting unreliable with string literals containing braces.

---

## Key Insights
1. **Post-bundle is the only viable path** — pre-bundle (overlay, transforms) changes how esbuild bundles, breaking vendor code resolution.
2. **Istanbul's function pattern is essential** — `function __cov()` is hoisted alongside esbuild's hoisted function declarations. `var __cov` gets split (declaration hoisted, assignment stays).
3. **Preamble must be inside the IIFE** — before the IIFE causes ASI issues or scope problems.
4. **The fundamental unsolved problem**: reliably detecting which byte ranges in the bundle are vendor code (node_modules) vs user code (src/). Every detection method has edge cases.
5. **Manual insertion proved it works** — manually adding 3 counters to only user code in the original bundle passes all tests including store-selectors.

## What Would Fix store-selectors
**Option A**: Reliable vendor detection. Need a method that doesn't rely on comments or byte scanning.
- Could use esbuild's `--metafile` to get module→byte-range mapping
- Or: use the AST to track which `__esm`/`__commonJS` callbacks are vendor

**Option B**: Don't instrument `__commonJS`/`__esm` callback bodies at all. Only instrument code that's NOT inside any module wrapper (test setup code, entry code). Lose vendor AND user module body coverage but keep test assertion coverage.

**Option C**: Per-file instrumentation that doesn't change esbuild's output. Would require an esbuild plugin (JS API only, not CLI). Or: instrument after bundling using source maps to map back to original files.
