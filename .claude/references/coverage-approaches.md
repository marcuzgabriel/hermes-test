# Coverage Approaches — What We Tried

## The Goal
`hermes-test --coverage` adds Istanbul-style counters to the bundler's output.
The bundler already produces a perfect bundle (100% test pass rate).

## Final Solution — 46/46 PASS
- Post-bundle instrumentation: bundler produces perfect bundle, we add counters
- Istanbul hoisted function pattern: `function __cov()` with `__c` cache
- Preamble inside IIFE, split maps to disk (statementMap/fnMap/branchMap stored externally)
- AST-informed bare-body wrapping, concise arrow skipping
- OXC parser for AST analysis (read-only), string insertion for output
- **No vendor filtering needed** — counters everywhere work fine
- **Bytecode compilation before eval** (the only fix that mattered — see Approach 12)

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

## Approach 11: Module boundary detection + user-code filtering
**What**: Various attempts to only instrument user code (not vendor). Comment-based, AST-based, brace-counting, 4-space-indent module keys, init boilerplate skipping.
**Result**: All approaches had edge cases. Some worked for single files but broke on multi-file bundles.
**Lesson**: This was a waste of time. See Approach 12.

## Approach 12: Bytecode compilation (THE ACTUAL FIX)
**What**: One line change — compile instrumented code to bytecode before eval, same as non-coverage path.
**Result**: **46/46 pass**. All tests. No filtering needed.
**The critical insight**: The `--coverage` path used `rt.eval(js)` (raw JS text) while the normal path used `rt.eval_bytes(bytecode)`. Hermes handles instrumented code differently in raw JS eval vs bytecode. Compiling to bytecode first fixed everything. All the vendor detection work (approaches 8-11) was chasing a symptom, not the cause.
**How we found it**: Replaced the cached bundle with the instrumented version and ran through the normal (non-coverage) path → passed. That proved the instrumentation was correct and the eval method was wrong.

## Approach 13: Remove all filtering (FINAL)
**What**: After discovering bytecode was the fix, removed all vendor detection, init boilerplate skipping, and module boundary scanning.
**Result**: **46/46 pass** with 100 fewer lines of code. Counters everywhere work fine with bytecode.
**Lesson**: When the root cause is found, delete the workarounds.

---

## Key Insights

1. **Post-bundle is the only viable path** — pre-bundle (overlay, transforms) changes how esbuild bundles.
2. **Istanbul's function pattern is essential** — `function __cov()` is hoisted alongside esbuild's hoisted function declarations. `var __cov` gets split.
3. **Preamble must be inside the IIFE** — before the IIFE causes ASI issues.
4. **Bytecode compilation is required** — Hermes's raw JS eval handles code differently from bytecode execution. Always compile to bytecode.
5. **The bundler is always right** — instrument the perfect bundle post-esbuild. Don't fight it.
6. **Find the root cause before adding complexity** — 10 approaches tried to work around a symptom. The fix was one line.
7. **Test the eval path, not just the code** — swapping the instrumented bundle into the non-coverage path immediately revealed the real issue.
