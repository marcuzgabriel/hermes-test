# Coverage Approaches — What We Tried

## The Goal
`hermes-test --coverage` adds Istanbul-style counters to the bundler's output.
The bundler already produces a perfect bundle (100% test pass rate).

## Final Solution — 42/42 PASS
- Post-bundle instrumentation with Istanbul hoisted function pattern
- `function __cov()` with `__c` cache (hoisted by JS spec, safe in esbuild's `__esm` blocks)
- Preamble inside IIFE, split maps to disk (statementMap/fnMap/branchMap stored externally)
- AST-informed bare-body wrapping, concise arrow skipping
- User-code-only filtering via `    "path"(` module boundary detection
- Skips esbuild init boilerplate (`init_*`, `import_*`, `require_*` statements)
- **Bytecode compilation before eval** (the critical fix — see Approach 11)

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

## Approach 3: AST-informed string insertion (kept)
**What**: OXC parser (read-only) for analysis + string insertion for output. Source preserved byte-for-byte.
**Result**: 38/38 pass (without store-selectors). This IS the correct instrumentation approach.
**Lesson**: Parse AST for analysis, insert text for output. Never re-emit with codegen.

## Approach 4: Per-file overlay directory
**What**: Instrument each source file individually, write to `.hermes-test-cache/cov-src/`, redirect esbuild.
**Result**: 38/38 for non-redux tests. store-selectors fails.
**Why it failed**: esbuild resolves different package entry points from the overlay directory (ESM vs CJS react-redux). Fought the bundler's resolution logic.

## Approach 5: Per-file transforms (shadow wrapper pattern)
**What**: Instrument test files, write temp copies next to originals, use bundler's `transforms` mechanism.
**Result**: Same as overlay. store-selectors fails.
**Why it failed**: esbuild produces different bundle structure from a temp file — even in the same directory with the same extension. Different module name in `__esm`, function hoisting changes. Pre-bundle instrumentation changes how esbuild processes the file.

## Approach 6: `var __cov` preamble before IIFE
**What**: `var __cov = (function(){...})()` prepended to the bundle.
**Result**: `Object is not a function` — ASI merges preamble with `(() => {` on next line.
**Why it failed**: `var` declaration gets split by esbuild (declaration hoisted, assignment stays inside `__esm`). Hoisted function declarations that reference `__cov` run before the assignment.

## Approach 7: Istanbul function pattern (kept)
**What**: `function __cov(){...}` declaration (hoisted by JS spec). Counters use `__cov().s[N]++`.
**Result**: Correct pattern. Hoisted functions can call `__cov()` safely.
**Lesson**: Istanbul uses `cov_HASH()` function calls for exactly this reason — function declarations are hoisted alongside user code.

## Approach 8: Comment-based user range detection
**What**: Parse esbuild's `// path/to/module` comments to find vendor vs user sections.
**Result**: Broken. Regular code comments (`// If the importer...`) inside vendor code get misdetected as module boundaries, closing vendor ranges prematurely.
**Lesson**: Comments are unreliable — any `//` in vendor code can fool the detector.

## Approach 9: `__esm`/`__commonJS` path detection (AST-based)
**What**: In AST walker, detect `__commonJS({...})` / `__esm({...})` calls, check path key for `node_modules`.
**Result**: Worked for single-file bundles but failed for multi-file. The walker couldn't reliably propagate vendor/user flag through the AST without false positives.

## Approach 10: Quoted path byte scanning with brace counting
**What**: Scan source bytes for `"src/..."(` patterns (module method keys), brace-count to find body ranges.
**Result**: Brace counting breaks on string literals containing `{` and `}` inside vendor code. Vendor ranges overrun into user code.

## Approach 11: Module boundary detection + bytecode compilation (THE FIX)
**What**: Scan for `    "path"(` at 4-space indent (esbuild's module method keys). Each module boundary defines a range. User ranges capped at `  });` (end of __esm block). User-code-only counters. **Compile instrumented code to bytecode before eval.**
**Result**: **42/42 pass** including store-selectors.
**The critical insight**: The store-selectors failure was NEVER about counters, vendor detection, or the preamble. It was about **Hermes's raw JS eval mode** handling the instrumented code differently from **bytecode mode**. The `--coverage` path used `rt.eval(js)` (raw JS) while the normal path used `rt.eval_bytes(bytecode)`. Compiling to bytecode first — the same one-line change that the non-coverage path already does — fixed everything.

---

## Key Insights

1. **Post-bundle is the only viable path** — pre-bundle (overlay, transforms) changes how esbuild bundles, breaking vendor code resolution.
2. **Istanbul's function pattern is essential** — `function __cov()` is hoisted alongside esbuild's hoisted function declarations. `var __cov` gets split.
3. **Preamble must be inside the IIFE** — before the IIFE causes ASI issues or scope problems.
4. **User-code detection via 4-space indent module keys** — `    "path"(` is esbuild's exact format for `__esm`/`__commonJS` method keys. Reliable, no false positives from comments or string content.
5. **Skip init boilerplate** — `init_*()`, `import_* = __toESM(require_*())` are esbuild module init statements. Counters before them change execution context.
6. **Bytecode compilation is required** — Hermes's raw JS eval handles instrumented code differently from bytecode execution. Always compile to bytecode first, even for coverage.
7. **The bundler is always right** — every attempt to bypass, overlay, or transform around the bundler failed. The bundler produces a perfect bundle. Instrument it post-bundle. Don't fight it.
8. **Manual testing reveals the real issue** — replacing the cached bundle with instrumented code and running through the normal (non-coverage) path proved the instrumentation was correct. The bug was in the eval path, not the counters.
