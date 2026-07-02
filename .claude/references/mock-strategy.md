# Mock Strategy — Lessons Learned

## Current State: 1472/1472 passed (100%), split mode, one strategy: shadow wrappers everywhere

## What Works (and why)

### Shadow Wrappers — handles barrel imports (90% of tests)
- Proxy wrappers replace mocked files in a shadow directory tree
- esbuild alias points to shadow dir instead of real source
- Wrappers are deliberately CJS (`module.exports = new Proxy(...)`) because:
  - **ESM** imports are hoisted and statically resolved — esbuild turns `import { X } from 'pkg'` into a direct variable binding at build time. By runtime the import is gone, no interception possible.
  - **CJS** stays as a runtime operation — esbuild wraps it with `__toESM` + `__copyProps`, creating live getters. Every property access goes through `Proxy.get()` at runtime, which is the interception point.
- Proxy checks __HT_file_mocks[__currentTestFile] dynamically at access time
- Per-file mock isolation is automatic for barrel-path imports
- Relative imports (`../redux/useRedux`) can't be mocked this way — esbuild resolves them as ESM top-level vars

### Per-file mock scoping (__HT_file_mocks)
- mock() writes to __HT_file_mocks[filename][path], not global
- __require Proxy checks per-file mocks first, then global
- Prevents cross-file mock pollution for externalized modules

### Mock hoisting (Rust post-processing)
- Moves init_*() calls after mock() calls in test file bodies
- Ensures mocks are registered before modules capture values

### Mini-vendor for non-aliased mocked packages (Day 19)
- Bundles real implementations of non-aliased packages (jwt-decode, uuid, etc.)
- Registers in __HT_mocks BEFORE harness eval so __require Proxy has real fallback
- Per-package separate esbuild invocations to avoid cross-contamination
- Only `fix_all_class_extends` applied — full `patch_esbuild_for_hermes` breaks small bundles
- PROVEN: jwtDecoding passes in full suite now

### FileContext — per-file harness state isolation (Day 19)
- Each file gets its own test entries, lifecycle hooks (beforeEach/afterEach/beforeAll/afterAll)
- File boundary cleanup: restores real timers, resets mock descriptors
- Architecturally correct but had zero impact on the failures at the time
- Combined with later fixes (function Proxy apply traps, withApiStore rewrites, RTK contamination fix) to reach 100%

### Split mode fix — shadow wrappers + vendor alias fix (Day 19)
- Split mode was broken (233/1115) because:
  1. Group bundles skipped aliases when mock sub-paths existed (line 326-328 in bundler.rs)
     → all aliased imports became `__require` calls hitting empty `{}` placeholders
  2. Vendor bundle ALSO skipped aliases (same logic) → `require('@topdanmark/...')` in
     vendor hit local `__require` → circular call → stack overflow (87 crashes)
  3. No shadow wrappers were created in split mode (only `run_tests_single` used them)
- Fix:
  1. Vendor bundle passes empty `mock_modules` → aliases NOT skipped → real source bundled
  2. Group bundles use shadow wrappers (same as single-bundle mode) with non-aliased mocks only
  3. Shadow dirs cleaned up after split bundling
- Result: 233 → 1313 passed in split mode. Stack overflows eliminated.
- Mock vendor uses direct esbuild invocation (NOT `bundle_esbuild_with_config` — that
  applies post-processing that breaks small bundles). Config externals applied directly.
- Only `fix_all_class_extends` applied to combined vendor output.

### Package shims — shadow wrappers for npm packages (Day 19, THE FIX)
- Auto-generated CJS Proxy wrappers for non-aliased mocked packages
- Same pattern as shadow wrappers: lazy _getReal(), per-file mock check, real fallback
- Replaces externalization entirely — real module stays in bundle via @__ht_real_pkg/ alias
- esbuild alias: `jwt-decode` → shim file, `@__ht_real_pkg/jwt-decode` → real package
- __toESM patch makes CJS Proxy work with ESM named imports
- **Result: +52 tests fixed (1315 → 1367), failures 143 → 91**

### deepEqual Date fix (Day 19)
- Dates now compared by `.getTime()` instead of `Object.keys()` (which was always `[]`)
- All Dates previously compared equal regardless of value — now correctly compared

## Strategies Tried and Why They Failed

### 1. Deep Shadow Tree Copy (Day ~10)
- Idea: Copy ALL files (never symlink) so relative imports stay in shadow tree
- Result: FAILED — stack overflow. Duplicates entire module graph (shadow + real via @__ht_real)
- Lesson: Can't duplicate the module graph

### 2. Sibling Shadow Recursion (Day ~10)
- Idea: Only copy sibling directories of mocked dirs
- Result: FAILED — cascades. At src/ level, nearly everything is a sibling of something mocked
- Lesson: Dispersed mocks make any tree expansion equivalent to full copy

### 3. Mock Dispatchers (Day ~12)
- Idea: __require Proxy returns wrapper function that re-checks mocks on every CALL
- Result: Fixed 29 contamination tests, BROKE 29 identity tests (wrapper !== spy)
- Lesson: Can't wrap functions without breaking === checks

### 4. MockRuntime with Factory Reset (__esm cache reset, Day ~14)
- Idea: Patch __esm to be resettable. Between files, reset inits that call __require.
- Result: +3 tests (marginal). Transitive closure: WORSE (-8 tests).
- Lesson: Factory reset doesn't reach relative imports, breaks singleton state.

### 5a. Separate Hermes VM for mocks (Day ~14)
- NOT ATTEMPTED — can't share JS objects between VMs (different heaps)

### 5b. MockRuntime with Active/Inactive Bins (TS-side FIFO GC, Day ~15)
- Idea: Track module inits into active/inactive bins, reset inactive in FIFO order
- Result: Mechanism worked correctly but same marginal impact as #4
- Lesson: Contamination is NOT from module init caching

### 6a. Full Source Module Reset — Vitest/Bun style (Day ~16)
- Idea: Reset ALL source module inits between files. Keep vendor cached.
- Result: 1311 passed (WORSE than 1316 baseline by 5)
- Key finding: Same 28 FILES fail. Module reset is the WRONG fix.

### 6b. OXC Source Transform (Day ~17)
- Idea: OXC-transform files with relative imports to mocked modules
- Result: 0 test improvement. Transform runs but integration with shadow tree conflicted.

### 7. FileContext — Per-file harness state isolation (Day 19)
- Idea: Scope harness state (hooks, timers, fetch, mocks) per file via FileContext class
- Result: ZERO impact on "pass alone, fail in suite" tests
- Key finding: Contamination is NOT from lifecycle hooks leaking across files
- DIAG confirmed: 142 beforeEach hooks accumulated, 1 global — but isolating them changed nothing

### 8. Auto-split mode for mocked packages (Day 19)
- Idea: Auto-enable split mode when non-aliased mock modules exist
- Result: MUCH WORSE — 368/1458 passed. Split mode has own issues with Topdanmark.

### 9. Package shadow wrappers (ESM Proxy wrappers, Day 19)
- Idea: Alias mocked packages to ESM Proxy wrapper files instead of externalizing
- Result: FAILED — esbuild requires static named exports for ESM destructuring
- Lesson: Can't dynamically re-export named exports through runtime Proxy

### 10. --preserve-symlinks (Day 19)
- Idea: Tell esbuild to preserve symlink paths, not follow to real location
- Result: FAILED — breaks monorepo node_modules resolution globally
- Lesson: Global flag, can't selectively apply to shadow tree only

### 11. Shadow tree full copy — no symlinks at all (Day 19)
- Idea: Copy ALL files in shadow tree instead of symlinking (was selective before)
- Result: ZERO impact — same 1313/1458
- Key finding: esbuild symlink behavior was NOT the cause of contamination
- The shadow tree copies vs symlinks makes no difference to the test results

## How the Remaining Failures Were Solved (Days 20-21)

The 143 failures from Day 19 were resolved through a combination of strategies:

1. **Function Proxy apply traps (Day 20)** — wrap function exports in `new Proxy(fn, { apply })` so mock checks happen at call time, not import time. Fixed module-scope captures like useDispatch, useSelector.
2. **Ecosystem wrapper shims (Day 20)** — config-driven shims for RTK Query, react-redux, etc. with singleton cache management.
3. **RTK contamination fix** — afterEach restore for mutated shared singletons (+18 tests)
4. **withApiStore + mock.fetch rewrites (Day 20-21)** — real Redux store + mock network layer instead of mocking hooks (+21 tests)
5. **setupApiStore pattern (Day 21)** — final push to 1472/1472 (100%)

See `challenges.md` for the full day-by-day journey.

## Key Technical Insights

1. esbuild's __toESM + __copyProps creates live getters that go through Proxy.
2. esbuild ESM-to-ESM uses shared top-level vars — relative imports bypass mocking
   INSIDE a shared bundle. **Update (Day 24)**: test-file-relative ht.mock() paths
   that resolve to a real file now work — the test file gets its own bundle and the
   target is externalized by absolute path (challenges.md Day 24). The
   shared-top-level-vars limitation still holds for anything bundled together.
3. esbuild follows symlinks by default (resolves to real path) — but this turned out
   NOT to be the cause of our issue.
4. `patch_esbuild_for_hermes` has 4 patches including `fix_all_class_extends`. Only the
   class-extends fix should be applied to small vendor bundles (other patches break).
5. Mock vendor must be eval'd AFTER harness (harness sets up __HT_mocks, React, etc.)

## Operational Notes
- Run Topdanmark from: `cd ~/Documents/mobile-insurance-app-expo/apps/topdanmark`
- NOT from monorepo root (wrong config, most tests crash)
- Rebuild harness before binary: `cd packages/hermes-test && node bundle.mjs`
- Clear cache: `rm -rf .hermes-test-cache`
