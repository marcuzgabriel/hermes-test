# Challenges & Solutions — Full Journey

## Day 1-3: Hermes Embed + esbuild Integration
### Challenge: Getting Hermes to run JS from Rust
- Built C++ bridge linking Hermes static library via cc crate
- JSI (JavaScript Interface) for eval, bytecode compilation
- Hermes stderr suppression for clean test output

### Challenge: esbuild output incompatible with Hermes
- Hermes previously had bugs with for-let-of closures, class-extends, etc.
- **Update**: All esbuild patches removed — modern Hermes (RN 0.85+) handles these correctly
- `patch_esbuild_for_hermes()` is now a no-op; mock hoisting and require shim remain

## Day 4-6: Hooks, Mocks, React Integration
### Challenge: renderHook without React Testing Library
- Built custom renderHook using React's internal act() mechanism
- Synchronous hook rendering with state history tracking
- waitFor/flushAsync for async hook patterns

### Challenge: mockModule timing
- esbuild hoists imports above mockModule calls
- `__require` Proxy with lazy property access solves this for externalized modules
- Named import destructuring captured at init time → Proxy intercepts at access time

### Challenge: __HT_noop for unknown externals
- Externalized modules return undefined → crashes on property access
- Proxy-based noop: infinite chaining (`noop.foo().bar.baz()`)
- Special handling: Symbol.toPrimitive, toJSON, ownKeys, constructor, length

## Day 7-8: Real-World Monorepo (Topdanmark)
### Challenge: tsconfig path aliases in monorepos
- `@topdanmark/mobile-insurance-app` → `./src` via tsconfig paths
- Followed "extends" chain to resolve parent tsconfig paths
- Alias-vs-external conflict: mocked paths must stay external

### Challenge: Zod v4 class-extends crash (RESOLVED)
- Hermes TDZ bug: `class X extends Variable` crashed when Variable was local
- Was worked around with `fix_all_class_extends()` regex transform
- **Update**: Modern Hermes (RN 0.85+) has fixed these bugs natively — all patches removed

### Challenge: Scoped lifecycle hooks
- beforeEach from nested group contaminating sibling groups
- **Solution**: hooks store their group scope, only ancestor hooks execute

## Day 9: Mock Conflicts Between Test Files
### Challenge: Multiple files mocking same module differently
- useChooseProfile and useServicePills both mock `@topdanmark/.../hooks`
- Single bundle: last mockModule wins, other files get wrong mock
- **Interim solution**: per-file isolation (separate esbuild + Hermes per file)
- Cost: ~120ms overhead per isolated file

### Challenge: Identifying which files need isolation
- `has_alias_mock_conflict()`: checks if file's mockModule paths match any alias
- Partition into batch (fast, shared bundle) and isolated (separate bundles)

## Day 10: The Single-Bundle Quest
### Challenge: Parallel esbuild for isolated files
- Sequential: 5 files x 80ms = 400ms bundling
- **Solution**: `std::thread::spawn` all esbuild processes, unique entry paths
- Saved ~120ms (0.73s → 0.60s for 5 isolated files)

### Challenge: Single-bundle for ALL files (the big goal)
#### Attempt 1: Post-bundle mock hoisting
- Move mockModule() before init_*() in bundled output
- Rust regex doesn't support lookahead
- Unnecessary anyway: __require Proxy already handles late-binding
- **Result**: dead end for externalized mocks

#### Attempt 2: OXC AST pre-transform (Vitest-style)
- Added OXC 0.132 as Rust dependency
- Parser + semantic analysis to find all import references
- Rewrote test file imports to Proxy wrappers
- **Problem**: only transforms test file, not transitive dependencies
- Hook implementations still get real module → "react-redux context" error
- **Result**: correct for test's own imports, useless for transitive deps

#### Attempt 3: Externalize alias-resolved paths
- Keep alias active, externalize the resolved path (e.g., /abs/src/hooks)
- esbuild: alias runs before external check → resolved path not caught
- Alternative: externalize everything under alias → breaks non-mock consumers
- **Result**: fundamental esbuild limitation

#### Attempt 4: Shadow wrappers (SUCCESS)
- User idea: "create a mock copy per file, use filename as identifier"
- Implementation: shadow directory tree with symlinks + Proxy wrapper files
- First try: `require(absolute_path)` → duplicate modules, circular crash
- Second try: relative paths → still duplicated by esbuild
- Third try: `@__ht_real/` alias for wrappers → same bundled instance
- Fourth try: lazy `_getReal()` → avoids circular dependency crashes
- **Result**: 91/94 at 0.49s (was 0.89s). 45% faster. Single bundle.

### Challenge: __DEV__ global missing
- createLogger accesses `__DEV__` which doesn't exist in Hermes test env
- **Solution**: `globalThis.__DEV__ = false` in entry generator

### Challenge: Index file matching
- Mock path `@scope/hooks` must match file `hooks/index.ts`
- Code generates `@scope/hooks/index` which doesn't match
- **Solution**: check both direct path AND parent-directory-for-index-files

## Day 19: Split Mode + Remaining 143 Failures

### Challenge: Split mode broken (233/1115)
- `bundle_split` creates vendor + group bundles
- Group bundles skip aliases when mock sub-paths exist (line 326-328 in bundler.rs)
  → all aliased imports become `__require` calls hitting empty `{}` placeholders
- Vendor bundle ALSO skipped aliases (same logic) → `require('@topdanmark/...')` in
  vendor hit local `__require` → circular call → stack overflow (87 crashes)
- **Solution**: vendor passes empty `mock_modules` so aliases resolve and real source is
  bundled. Group bundles use shadow wrappers (same as single-bundle) with only non-aliased
  mocks externalized. Shadow dirs cleaned up after split bundling.
- **Result**: 233 → 1315 passed. Stack overflows eliminated.

### Challenge: deepEqual treats all Dates as equal
- `Object.keys(new Date())` returns `[]` — zero own properties
- `deepEqual` compared objects by iterating `Object.keys`, so any two Dates were "equal"
- **Solution**: `if (a instanceof Date && b instanceof Date) return a.getTime() === b.getTime()`

### Challenge: Package shims — eliminating externalization (THE FIX)
- Root cause of ~104 "pass alone, fail in suite": `mockModule('X')` in ANY file causes `X`
  to be externalized globally → files that don't mock `X` get `__HT_noop` instead of real module
- Realized: shadow wrappers already solve this for aliased paths — same CJS Proxy pattern
  works for npm packages too
- **Solution**: `create_package_shims()` — auto-generate CJS Proxy wrappers for non-aliased
  mocked packages. Alias `pkg` → shim file, `@__ht_real_pkg/pkg` → real package.
  `__toESM` patch makes CJS Proxy work with ESM named imports.
  No externalization needed — real module stays in bundle.
- **Result**: +52 tests (1315 → 1367). One strategy for everything.

### Challenge: 91 remaining failures
**Relative imports (~40 fails, ~8 files):**
- `../redux/useRedux` bypasses shadow wrappers — esbuild resolves to shared top-level vars
- No Proxy interception possible at this level
- Fix: convert test files to `withStore` pattern (proven for 21 tests)

**Standalone bugs (~30 fails, ~9 files):**
- Tests fail even when run alone — not contamination
- apiBaseQuery (6), resourceBundle (2), useFormCoordinator (1), etc.

**Remaining contamination (~20 fails):**
- Some "pass alone, fail in suite" tests remain after package shims

### Strategies tried and ruled out (Day 19):
1. **FileContext** (per-file harness state) — zero impact, contamination isn't from hooks
2. **Auto-split mode** — worse (368/1458), split has own issues
3. **Package ESM wrappers** — esbuild requires static named exports
4. **--preserve-symlinks** — breaks monorepo node_modules resolution
5. **Shadow tree full copy** — zero impact, symlinks weren't the cause
6. **Mock vendor** (pre-bundle non-aliased packages) — worked but redundant once package shims landed

## Day 20: Ecosystem Shims + RTK Contamination Fix (1367 → 1385)

### Challenge: RTK slice tests fail in full suite (27 tests, 6 files)
- primo, topBiz, topBizMachine, guidewire* — "Actions must be plain objects, type was 'undefined'"
- Pass alone, fail in full suite
- Initially suspected dual-instance problem (shadow wrapper `_getReal()` re-entrancy)

### Investigation: What didn't cause it
- **NOT dual-instance**: debug proved `insuranceDetailsPrimoApi === primoFromRequire` (same object)
- **NOT shadow wrapper re-entrancy**: api module has no circular deps, `_loading` never fires
- **NOT missing middleware**: `withApiStore` receives valid `{ reducerPath, reducer, middleware }`
- **NOT createApi singleton**: RTK shim's `_apiCache` works but doesn't fix it
- **Key clue**: `initiate()` returns `function` when alone, `undefined` in full suite —
  same api object, different return value → something mutates the endpoint at runtime

### Root cause: `useGetInsuranceDetailsFetcher.hermes.test.ts`
This test directly **overwrites** `endpoint.initiate` on the shared RTK API singleton in
`beforeEach`, without restoring in `afterEach`:
```ts
// beforeEach — replaces real initiate with spy
insuranceDetailsPrimoApi.endpoints.getInsuranceDetailsPrimo.initiate = spy();
insuranceDetailsTopBizApi.endpoints.getInsuranceDetailsTopBiz.initiate = spy();
insuranceDetailsTopBizApi.endpoints.getInsuranceDetailsTopBizMachine.initiate = spy();
insuranceDetailsGuidewireApi.endpoints.getInsuranceDetailsGuidewire.initiate = spy();
// NO afterEach to restore originals!
```
Since all modules share ONE api singleton (createApi is called once), the spy persists
across all subsequent test files. Any test that calls `endpoint.initiate(...)` gets
`undefined` (spy's default return value) instead of the real thunk.

### How I found it
1. Confirmed primo passes alone (7/7), fails in suite (2/7)
2. Confirmed passing when listed first (esbuild entry order changes module init)
3. Added debug test: api object is correct (`hasInitiate: "function"`)
4. Added debug test: `initiate()` returns `undefined` even though function exists
5. Searched bundle for `.initiate =` assignments → found line 186656 in cached bundle
6. Traced back to `useGetInsuranceDetailsFetcher.hermes.test.ts` line 91

### Fix
Save originals before mutating, restore in `afterEach`:
```ts
const _orig = insuranceDetailsPrimoApi.endpoints.getInsuranceDetailsPrimo.initiate;
afterEach(() => {
  insuranceDetailsPrimoApi.endpoints.getInsuranceDetailsPrimo.initiate = _orig;
});
```
**Result**: 1367 → 1385 (+18 tests). primo 7/7, topBiz 3/3, topBizMachine 5/5.

### Challenge: Ecosystem shim infrastructure
- **Goal**: let users add `"hermes-test/shims/rtk-query"` to config for package-level wrappers
- **Problem**: shims that need the real package can't use `__HT_mocks` (externalization)
- **Solution**: esbuild alias mechanism — `pkg → shim file`, `@__ht_real_pkg/pkg → real`
- Agnostic resolution: shim files discovered from `packages/hermes-test/src/shims/` on disk
- No hardcoded registry in Rust — any `.js` file in the shims directory works
- `create_wrapper_shims()` in bundler.rs, called before shadow wrappers + package shims
- BundleConfig gains `wrapper_shims: Vec<(String, String)>` field

### Shims created
- `rtk-query.js` — Proxy wrapper with singleton `createApi` cache by reducerPath
- `reduxjs-toolkit.js`, `react-redux.js` — transparent pass-through (single-instance)
- `tanstack-query.js` — test defaults (retry:false) + `withQueryClient` helper

### Research: how other test runners handle state management
- **No test runner ships built-in state management utilities** (Jest, Vitest, Bun)
- Patterns come from library docs: fresh store per test + Provider wrapper + afterEach cleanup
- RTK dual-instance is a module identity problem — Vitest hit same bug (issue #4180)
- Zustand: library ships own `__mocks__/zustand.ts` with auto-reset
- hermes-test's `withStore`/`withApiStore` already follows standard patterns

### TanStack Query: class-extends edge case (RESOLVED)
- Was blocked by `fix_all_class_extends` breaking comma-expression patterns
- **Update**: `fix_all_class_extends` removed entirely — modern Hermes handles class-extends natively

## Patterns & Principles Learned

1. **esbuild aliases run before externals** — can't selectively externalize aliased paths
2. **ESM named exports are static bindings** — can't intercept with Proxy at import time
3. **CJS Proxy + __toESM patch = universal mock interception** — works for aliased AND npm packages
4. **One strategy beats layered strategies** — shadow wrappers everywhere > externalization + vendor + shims
5. **Proxy + lazy loading = mock isolation** — one module appears different to different consumers
6. **Filesystem-level interception > AST transforms** — simpler, no codegen, no edge cases
7. **Circular deps need lazy wrappers** — eager require in wrapper deadlocks module init
8. **Hermes compat** — modern Hermes (RN 0.85+) handles ES6 classes and let-bindings natively; legacy patches removed
9. **Per-file isolation is the fallback** — parallel esbuild makes it acceptable (~60ms/file)
10. **Shared singletons + mutation = silent contamination** — RTK's `endpoint.initiate` is a
    mutable property on a singleton. Any test that overwrites it without restoring poisons all
    subsequent tests. In Jest this is masked by per-file VM isolation; in single-bundle runners
    (hermes-test, Vitest inline mode) it's fatal. Always `afterEach` restore.
11. **Search the bundle, not the source** — when debugging "pass alone, fail in suite", grep the
    cached bundle for property assignments (`.initiate =`, `.dispatch =`) to find mutations
    on shared objects. The contaminator may be a completely unrelated test file.

## Day 20 continued: Function Proxy Wrappers (1385 → 1388)

### Challenge: Module-level destructuring bypasses Proxy mock check
- `const { useDispatch } = require('react-redux')` captures at init time
- The package shim Proxy checks `__currentTestFile` on every `get`, but destructuring
  stores the result in a LOCAL variable — no more Proxy interception after init
- Affects: `useRedux` (captures `useDispatch/useSelector`), `useMarketingConsent`
  (captures `getMarketingConsents.initiate`), `useActionMessages` (captures selectors)
- Root cause: esbuild's `__esm` inits run once and cache results in local vars

### Strategy 12: __esm re-initialization (FAILED — reverted)
- Idea: Patch `__esm` to support a "dirty" flag. Between test files, mark all source
  modules dirty. Dirty modules re-run init on next access, re-reading from Proxies.
- Result: **WORSE** (1385 → 1380, -5 regressions) AND **2x slower** (3s → 6s)
- Why: re-initializing ALL source modules causes singleton state corruption (Redux
  stores re-created, React context lost). Too broad — needed surgical targeting, but
  knowing which modules to reset requires the dep graph at runtime.
- Lesson: module re-init is the nuclear option — breaks more than it fixes.

### Strategy 13: Function Proxy apply trap (SUCCESS — +3 tests)
- Idea: When a shadow wrapper or package shim Proxy returns a function, wrap it in
  ANOTHER Proxy with an `apply` trap. The `apply` trap re-checks per-file mocks at
  CALL time, not at capture time.
- How: `_fnCache[p] = new Proxy(realFn, { apply: (target, thisArg, args) => { ... } })`
- Why it works: destructuring `const fn = proxy.fn` captures a Proxy-wrapped function.
  Later `fn()` triggers the `apply` trap → re-checks `__HT_file_mocks[__currentTestFile]`
  → returns mock result if registered. The captured reference is still valid for GC.
- Result: **+3 tests** (1385 → 1388). `redux.hermes.test.ts` and `marketingConsent.hermes.test.ts` fixed.
- No performance cost — `_fnCache` ensures only one Proxy wrapper per function per module.
- Limitation: only works for FUNCTION-typed exports. Non-function values (selectors,
  objects, constants) captured at init time can't be intercepted this way.

### Strategy 14: Live Proxy mock placeholders (infrastructure, no count change)
- Idea: Replace static `{}` mock placeholders for externalized modules with live Proxies
  that check `__HT_file_mocks` on every property access. Same pattern as shadow wrappers.
- Why: shared modules (e.g. `keyValueStorage`) cache `__require("pkg")` at init time.
  If the placeholder is a plain `{}`, per-file mocks can't intercept later.
- Result: infrastructure correct but insufficient alone — the `__require` shim already
  returns per-file-checking Proxies, so the placeholder Proxy is redundant for most cases.
- Kept in codebase as defense-in-depth.

### Strategy 15: mockModule patching + resetMockModulePatches (infrastructure)
- Idea: When `mockModule()` is called, also directly mutate the real module's properties
  so that already-destructured refs see the mock. Save originals, restore between files.
- Result: mechanism works but target object (`__HT_mocks[path]`) is a Proxy wrapping `{}` —
  `key in proxy` returns false, so no properties get patched. Would need access to the
  real module's exports object inside esbuild's closure.
- Kept as infrastructure for future use.

### Strategy 16: Shared mock mutation detection (manual fixes, +1 test)
- Deep-clone shared mock data before `delete`. `paymentUtil`, `useGetInsuranceMetaInfoGuidewire`,
  `basicInvoice` tests were deleting properties on shared mock imports without cloning.
- `JSON.parse(JSON.stringify(...))` for deep clones before mutation.
- Result: guidewireBilling 2/3 → 3/3 (+1 test).

### Remaining 70 failures breakdown
- **~23 useActionMessages**: destructures non-function values (selectors) at module scope
- **~7 useMarketingConsent**: same — destructured selectors not interceptable
- **~6 apiBaseQuery**: standalone bugs (fail even alone)
- **~5 guidewire data-shape**: Zod `unwrapData` returns `orgResult` with extra fields
- **~5 useSsoLogin**: standalone bugs (iterator method errors)
- **~4 keyValueStorage/useTopGPTConsent**: externalized module caching
- **~4 useFileUpload/useFetchOverviewDetails**: standalone bugs
- **~3 FirebaseAnalyticsTracker**: console.warn spy not called (console externalized)
- **~3 misc**: useFormCoordinator crash, usePrimoPurchase, useContactInfo

### Strategy 17: withApiStore+mockFetch rewrite (SUCCESS — +21 tests, 1388→1409)
- Rewrote `useActionMessages.hermes.test.ts` from mockModule pattern to withApiStore+mockFetch
- Instead of mocking `useIsLoading`, `useErrorHandling`, `actionMessagesSelector` (all relative
  imports that can't be intercepted), provide real Redux store with seeded state
- Seed `app.loading.tags` for loading state, `app.errorHandling.currentErrors` for errors
- Use `mockFetch(http.get(url, () => HttpResponse.json(data)))` for API responses
- Wrap fetch calls in `act(async () => { await ... })` for RTK Query cache updates
- Added `loading` and `errorHandling` to testStore's `defaultAppState`
- Key: use `apiMockDomain` from environment constants for correct URL matching
- **Result**: useActionMessages 3/26 → 24/24. Total 1388 → 1409.

### Remaining 47 failures (16 files)
- **~12 relative imports + complex mocking**: useMarketingConsent(7), useTopGPTConsent(4), usePrimoPurchase(1)
  → need same withApiStore+mockFetch rewrite as useActionMessages
- **~18 standalone bugs**: apiBaseQuery(6), useSsoLogin(5), useFetchOverviewDetails(3), useFileUpload(3),
  useFormCoordinator(1), useGetInsuranceMetaInfoGuidewire(1) → fail even alone
- **~12 data-shape/contamination**: guidewire(5), keyValueStorage(4), FirebaseAnalyticsTracker(3)
- **~5 misc**: resourceBundle(2), useContactInfo(1), useUserPanelParticipation(1)

## Day 21: setupApiStore + 100% pass rate (1409 → 1472/1472)

### Store API refactor
- Replaced `withApiStore`/`createApiStoreFactory` with `setupApiStore(apis, extraReducers, options)`
- Matches Jest's `setupApiStore` pattern — thin, composable, real reducers
- Topdanmark: `withReduxStore(options?)` wraps `setupApiStore` with real `rootReducer`
- Supports `preloadedState` and `middleware: { prepend, concat }` options
- **Fixed RTK Tuple breakage**: `.concat()` on RTK's middleware Tuple returns plain array,
  losing `.prepend()`. Use for-loop `chain = chain.concat(mw)` instead of spread.

### URLSearchParams polyfill extended
- Array-of-pairs constructor: `new URLSearchParams([["k","v"]])`
- Added `entries()`, `keys()`, `values()`, `forEach()`, `delete()`, `Symbol.iterator`
- Added `Object.fromEntries` polyfill for Hermes
- Fixed definition order: URLSearchParams before URL to avoid stale reference capture

### AsyncStorage shim
- In-memory implementation: `test/shims/async-storage.js`
- `globalThis.__HT_asyncStorageData` backing store shared across all proxy-wrapped references
- Reset via `__HT_resetAsyncStorage()`
- Configured as replacement shim in hermes-test.config.json
- Note: keyValueStorage test still uses module-level mock (proxy layer doesn't share
  backing store between different `__currentTestFile` contexts for the same shim)

### Auto-detect native externals (implemented but disabled)
- `detect_native_modules()` scans node_modules for `ios/`, `android/`, `*.podspec`, `app.plugin.js`
- ~5ms scan time for 2400 packages (stat-only, no file reads)
- **Problem**: over-externalizes packages that tests need bundled (e.g. `form-data` has `android/`
  dir but must be bundled for test runtime). Also misses JS-only wrappers that transitively
  need native (e.g. `@gorhom/bottom-sheet`, `expo-status-bar`)
- **Status**: code in bundler.rs, disabled pending opt-in flag or allowlist approach

### Test fixes that reached 100% (+63 tests)

**Contamination fixes:**
- guidewire (+5): deep-clone shared mocks before `delete` (paymentUtil, lobUtil)
- FirebaseAnalyticsTracker (+3): singleton logger captured at init → assign spy in beforeEach
- keyValueStorage (+4): mock at module level to bypass AsyncStorage proxy chain
- useFormCoordinator (+4): rename Claims version to avoid `__currentTestFile` basename collision

**Missing mock/setup fixes:**
- apiBaseQuery (+6): add fetch handlers, crypto mock, mutex cleanup, fix spy API
- useMarketingConsent (+7): full rewrite to `withReduxStore` + `mockFetch`
- useTopGPTConsent (+4+1): `api.util.invalidateTags` mock + warm-up renderHook for React dispatcher
- useFetchOverviewDetails (+3): sync `act()` + `removeError` mock
- useContactInfo (+1): add Redux Provider wrapper

**Polyfill/shim fixes:**
- useSsoLogin (+5): URLSearchParams polyfill improvements
- useFileUpload (+3): FormData mock needs `{ default: FormDataMock }` for default imports
- useGetInsuranceMetaInfoGuidewire (+1): destructure instead of JSON.parse/stringify (preserves Dates)

**Other:**
- resourceBundle (+2): regex URL matching for CMS endpoints
- usePrimoPurchaseTravelCoverageSubmission (+1): replace missing `toHaveBeenNthCalledWith`
- useUserPanelParticipation (+1): patch `keyValueStorage.get/set` directly

## Updated Patterns & Principles

12. **Function Proxy wrappers = mock interception through destructuring** — wrapping
    returned functions in `new Proxy(fn, { apply })` keeps mock checking alive even after
    `const fn = proxy.fn` captures the reference. Zero perf cost with caching.
13. **Module re-init is the nuclear option** — re-initializing source `__esm` modules between
    files causes singleton corruption and is 2x slower. Only viable with surgical targeting.
14. **Copy strategy > reset strategy** — save/restore (mockModulePatches) is safer than
    re-initialization. No singleton corruption, no performance cost, easy to reason about.
15. **Shallow spread doesn't deep-clone** — `{ ...obj }` copies top-level refs. Nested objects
    are still shared. Always `JSON.parse(JSON.stringify(...))` when deleting nested properties.
16. **Basename collision = mock collision** — two test files with the same filename in different
    directories share `__currentTestFile` ID → their `mockModule` registrations overwrite each
    other. Fix: rename one file to have a unique basename.
17. **React dispatcher contamination** — heavy `renderHook` usage with real Redux Providers can
    corrupt React's internal hooks dispatcher for subsequent files. Fix: warm-up
    `renderHook(() => null)` in `beforeEach` to reset the dispatcher.
18. **Default export mocks need `{ default: X }`** — when a test mocks a module that's imported
    via `import X from 'pkg'`, the shadow wrapper proxy looks up `m["default"]`. A plain
    function/class won't have a `default` property. Return `{ default: MockClass }`.
19. **Auto-detect externals is fragile** — scanning for `ios/`/`android/` dirs catches native
    modules but also hits packages like `form-data` that have native code but need bundling.
    JS-only wrappers around native deps are missed entirely. Manual externals list is safer
    until a smarter heuristic (import-graph analysis or runtime error detection) is available.
20. **`JSON.parse(JSON.stringify(x))` destroys Date objects** — converts them to ISO strings.
    Use destructuring with rest operator when you need to omit fields from mock objects.

## Day 22: Linux Docker validation + Intl fallback hardening

### Challenge: Running Topdanmark from Linux against local hermes-test source
- Added Docker flow in this repo:
  - `docker/topdanmark-linux/Dockerfile`
  - `docker/topdanmark-linux/run-topdanmark.sh`
- Script mirrors CI/release steps: clone+patch Hermes, build Hermes, bundle harness, build CLI, run Topdanmark.
- Topdanmark install defaults to `bun install --ignore-scripts` to avoid monorepo `postinstall` (`bob build`/codegen) failures in container-only test runs.

### Challenge: Linux static linking failed with many undefined Hermes symbols
- Symptom: `error: linking with cc failed` on Linux ARM Docker despite successful Hermes build.
- Root cause: static archive resolution on GNU ld dropped cross-archive symbols in the previous link strategy.
- Fix: link Hermes static libs with `whole-archive` on Linux in `crates/hermes-test-cli/build.rs` (macOS keeps normal static linking).

### Challenge: One Linux-only failing test from locale casing behavior
- Failing test expected `"Migration to Guidewire".toLocaleLowerCase()` to return lowercased text.
- On Linux Hermes ICU stub, `toLocaleLowerCase()` returned placeholder output (`"lowered"`).
- Fix in `packages/hermes-test/src/polyfills.js`:
  - Add runtime-gated fallback for `String.prototype.toLocaleLowerCase/toLocaleUpperCase`.
  - Activation check is generic (`'AbC' -> 'abc'`, `'aBc' -> 'ABC'`), no project-specific strings.
  - Preserve native behavior on platforms where locale casing already works.

### Guardrails for future Intl work
- Keep polyfills **runtime-detected**, not platform-forced.
- Keep scope narrow (patch only broken surfaces, avoid full CLDR reimplementation).
- Add/update regression tests in `examples/expo-app/src/examples/intl-locale.test.ts`.
