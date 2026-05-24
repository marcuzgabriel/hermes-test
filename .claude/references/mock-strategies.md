# Mock Strategies — Reference

## Current State
- 1388 passing, 70 failing (Topdanmark, 259 files, ~3s)
- 1411 passing (expo-app, 30 files, ~1.5s)
- Run from: `apps/topdanmark/` (NOT monorepo root)
- Core strategy: **shadow wrappers + function Proxy apply traps**

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

### Function Proxy Apply Traps (Day 20)
How: When a shadow wrapper or package shim Proxy returns a function, wrap it in `new Proxy(fn, { apply })`. The `apply` trap re-checks per-file mocks at CALL time.
Why it works: `const fn = proxy.fn` captures a Proxy-wrapped function. `fn()` triggers apply → re-checks `__currentTestFile` mock → returns mock result if registered. The captured ref is valid for GC. One Proxy per function per module (`_fnCache`), zero perf cost.
Works for: Function-typed exports captured at module scope (useDispatch, useSelector, initiate).
Doesn't work for: Non-function exports (selectors, objects, constants) captured at init.

### Ecosystem Wrapper Shims (Day 20)
How: `"shims": { "pkg": "hermes-test/shims/name" }` in config. Bundler resolves from `packages/hermes-test/src/shims/` on disk. Uses esbuild alias: `pkg → shim`, `@__ht_real_pkg/pkg → real`. Shim wraps real package, adds test instrumentation (e.g. createApi singleton cache).
Files: rtk-query.js, reduxjs-toolkit.js, react-redux.js, tanstack-query.js

### Mock Hoisting
How: Rust post-processing moves `init_*()` calls to AFTER `mockModule()` calls in test file bodies.

### withStore / withApiStore (Pattern 2)
How: Real Redux store with configurable initial state. Hooks use real `useSelector` — no mocking needed.
Works for: Relative-import hooks that use Redux. Proven for 21 tests.

## The 70 Remaining Failures

### Category 1: Module-scope non-function capture (~30 fails)
Problem: `const { selector } = api.endpoints` captures at init time. Selector is an object, not a function — can't use Proxy apply trap. Mock registered later can't intercept.
Files: useActionMessages (23), useMarketingConsent (7), useTopGPTConsent (4)
Fix: Convert to withStore (Pattern 2) or wrap selectors in functions.

### Category 2: Standalone bugs (~20 fails)
Problem: Tests fail even when run alone. Not contamination.
Files: apiBaseQuery (6), useSsoLogin (5), useFetchOverviewDetails (3), useFileUpload (3), resourceBundle (2), useFormCoordinator (1), usePrimoPurchase (1)

### Category 3: Remaining contamination (~10 fails)
Problem: Data-shape mismatches (guidewire Zod unwrapData), AsyncStorage mock caching, console externalization.
Files: guidewireDetails (2), guidewirePolicy (3), keyValueStorage (4), useContactInfo (1), useUserPanelParticipation (1), FirebaseAnalyticsTracker (3)

## Strategies Tried (16 total, see challenges.md for details)

Working strategies:
1. Shadow wrappers (barrel-path Proxy interception)
2. Package shims (npm package Proxy interception)
3. Function Proxy apply traps (call-time mock checking)
4. Ecosystem wrapper shims (single-instance + test instrumentation)
5. Live Proxy mock placeholders (defense-in-depth for externals)
6. mockModule patching + resetMockModulePatches (save/restore between files)
7. Shared mock mutation detection (deep-clone before delete)

Failed strategies:
8. __esm re-initialization — singleton corruption, 2x slower
9. Full source module reset — same 28 files fail, wrong fix
10. OXC AST transform — only transforms test file, not transitive deps
11. Deep shadow tree copy — stack overflow from duplicate module graph
12. Mock dispatchers — wrapper !== spy breaks === checks
13. Factory reset / active-inactive bins — marginal impact

Key lesson: **function Proxy apply traps** are the breakthrough for module-scope capture.
They preserve references (no GC issues), check mocks at call time (not capture time),
and have zero perf cost with caching. The remaining gap is non-function values — those
need architectural changes (withStore) or getter-based wrapping.

## Operational Notes
- Always run Topdanmark from `apps/topdanmark/`, NOT monorepo root
- Always rebuild harness before binary: `cd packages/hermes-test && node bundle.mjs`
- Cache can cause stale results: `rm -rf .hermes-test-cache` when in doubt
- Config has `"split": true` — split mode is the default for Topdanmark
