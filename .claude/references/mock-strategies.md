# Mock Strategies — Reference

## Current State
- 1367 passing, 91 failing (Topdanmark, 259 files, split mode, 3.3s)
- 1411 passing (expo-app, 30 files, 0.6s)
- Run from: `apps/topdanmark/` (NOT monorepo root)
- One strategy: **shadow wrappers everywhere**

## What Works Today

### Shadow Wrappers (aliased paths)
How: For each aliased mock path, create a shadow directory tree. Mocked files are replaced with CJS Proxy wrappers that check `__HT_file_mocks[__currentTestFile]` on every property access. esbuild alias points to shadow dir. Non-mocked files are symlinked to the real source.

Why it works: esbuild's `__toESM` + `__copyProps` creates live getters. The `__toESM` patch returns Proxy directly when `__esModule` is true. Every property access goes through the Proxy → per-file mock check → real module fallback via `_getReal()`.

Works for: Barrel-path imports (`@scope/hooks/redux/useRedux`).
Doesn't work for: Relative imports (`../redux/useRedux`) — esbuild resolves these to shared top-level vars at bundle time. No Proxy interception point exists.

### Package Shims (non-aliased npm packages)
How: Auto-generated CJS Proxy wrappers for mocked npm packages (jwt-decode, react-redux, uuid, etc.). Same pattern as shadow wrappers. esbuild alias: `pkg` → shim file, `@__ht_real_pkg/pkg` → real package. Replaces externalization entirely — real module stays in bundle.

Why it works: Same Proxy + `__toESM` mechanism as shadow wrappers. Per-file mock isolation. Real module always available as fallback.

Result: +52 tests fixed (1315 → 1367) by eliminating `__HT_noop` fallback for unmocked files.

### Split Mode
How: Vendor bundle (npm packages + aliased source) + group bundles (10 test files each with --packages=external). Shadow wrappers and package shims applied to group bundles. Vendor passes empty mock_modules so aliases resolve and real source is bundled.

### mockModule + Per-file Scoping
How: `mockModule('path', factory)` registers in `__HT_file_mocks[currentFile][path]`. The `__require` Proxy and shadow wrapper Proxies check per-file mocks first, then fall through to real module.

### Mock Hoisting
How: Rust post-processing moves `init_*()` calls to AFTER `mockModule()` calls in test file bodies.

### withStore / withApiStore (Pattern 2)
How: Real Redux store with configurable initial state. Hooks use real `useSelector` — no mocking needed.
Works for: Relative-import hooks that use Redux. Proven for 21 tests.

## The 91 Remaining Failures

### Category 1: Relative-import hooks (~40 fails, ~8 files)
Problem: Hooks import `../redux/useRedux` via relative path. esbuild resolves to shared top-level vars — no Proxy interception possible.
Files: useActionMessages (26), useSsoLogin (5), useMarketingConsent (11), others
Fix: Convert to withStore (Pattern 2) — proven but manual per-file work.

### Category 2: Standalone bugs (~30 fails, ~9 files)
Problem: Tests fail even when run alone. Not contamination.
Files: apiBaseQuery (6), resourceBundle (2), useFormCoordinator (1), useFetchOverviewDetails (3), others

### Category 3: Remaining contamination (~20 fails)
Problem: Some "pass alone, fail in suite" tests remain after package shims.

## Strategies Tried and Ruled Out (11 total)
See `docs/mock-strategy.md` for the full list with results.

Key lesson: all module-graph-level approaches (factory reset, module reset, OXC transform, active/inactive bins) failed. The fix was extending the existing shadow wrapper pattern to npm packages via auto-generated shims.

## Operational Notes
- Always run Topdanmark from `apps/topdanmark/`, NOT monorepo root
- Always rebuild harness before binary: `cd packages/hermes-test && node bundle.mjs`
- Cache can cause stale results: `rm -rf .hermes-test-cache` when in doubt
- Config has `"split": true` — split mode is the default for Topdanmark
