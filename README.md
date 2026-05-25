# hermes-test

**29x faster than Jest.** A test runner for React Native and Expo that executes your tests in Hermes — the same JavaScript engine your app ships with.

No Babel transforms. No `transformIgnorePatterns`. No `jest-expo` mock layer. Just your code, running in the real engine, at native speed.

```
1472 tests — 0.79s
```

---

### The problem

Every React Native and Expo project tests in Node. Your app runs in Hermes. These are different JavaScript engines with different behaviors. That gap is where bugs hide — and Jest can't find them because it's running in the wrong engine entirely.

On top of that, Jest in React Native is slow by design. Every test file spawns a worker, runs Babel transforms, resolves `transformIgnorePatterns` for every `node_modules` import, collects coverage, and coordinates results over IPC. For a mid-size Expo app, that's 2-4 minutes per run. Developers stop running tests. Tests rot. Coverage drops.

### The fix

hermes-test runs your tests directly in Hermes. esbuild bundles your test + source into a single file in <100ms. A Rust CLI feeds it to the Hermes VM. One process, no workers, no transforms. Results appear before your hand leaves `Cmd+S`.

Most Jest mocks become unnecessary — Hermes runs your real hooks, real Redux store, real business logic natively. Native modules are auto-detected and externalized — zero manual configuration needed.

### Benchmarks

Production Expo app (Topdanmark, Danish insurance — 259 files, 1472 tests):

| | Jest (`--no-coverage`) | Jest (with coverage) | hermes-test |
|---|---|---|---|
| Full suite | 23s | 116s | **0.79s** |
| **Speedup** | **29x** | 147x* | — |
| Watch rerun | ~3s | — | **~300ms** |

\* hermes-test does not yet support coverage collection. The fair apples-to-apples comparison is **29x** (both without coverage). The 147x number reflects real-world workflow where Jest has `collectCoverage: true` enabled by default.

Micro benchmarks (Apple Silicon, no coverage):

| Scenario | hermes-test | Jest + @swc/jest | Speedup |
|----------|-------------|------------------|---------|
| 10 pure function tests | **16ms** | 714ms | **45x** |
| 50 hook tests (renderHook + act) | **75ms** | 721ms | **10x** |
| Trivial cold start | **4.6ms** | 1,486ms | **364x** |

---

## Quick start

```bash
bun add -D hermes-test
```

```ts
// useCounter.test.ts
import { test, expect, renderHook, act } from 'hermes-test';

test('useCounter increments', () => {
  const { result } = renderHook(() => useCounter(0));
  act(() => result.current.increment());
  expect(result.current.count).toBe(1);
});
```

```bash
hermes-test              # run all tests
hermes-test --watch      # watch mode
```

## API

### Test structure

```ts
import { test, expect, group, beforeEach, afterEach } from 'hermes-test';

group('myFeature', () => {
  beforeEach(() => { /* reset */ });

  test('does the thing', () => {
    expect(result).toBe(42);
    expect(arr).toEqual([1, 2, 3]);
    expect(str).toContain('hello');
    expect(fn).toThrow('error message');
  });

  test.skip('not yet', () => {});
  test.only('focus this', () => {});
  test('slow test', () => { /* ... */ }, { timeout: 10000 });
});
```

### Assertions

```ts
expect(val).toBe(exact)            expect(val).toEqual(deep)
expect(val).toBeTruthy()           expect(val).toBeFalsy()
expect(val).toBeDefined()          expect(val).toBeUndefined()
expect(val).toBeNull()             expect(val).toBeGreaterThan(n)
expect(val).toContain(item)        expect(val).toContainEqual(item)
expect(val).toMatch(/regex/)       expect(val).toBeCloseTo(n, precision)
expect(fn).toThrow('msg')          expect(val).not.toBe(other)

// Asymmetric matchers
expect.anything()                  expect.any(String)
expect.objectContaining({ key })   expect.arrayContaining([1, 2])
expect.stringContaining('sub')     expect.stringMatching(/pattern/)

// Async
await expect(promise).resolves.toBe(value)
await expect(promise).rejects.toThrow('msg')
```

### Spies

```ts
import { spy, spyOn, clearAllMocks } from 'hermes-test';

const fn = spy(() => 'default');
fn.mockReturnValue('mocked');
fn.mockReturnValueOnce('first');
fn.mockImplementation((x) => x * 2);
fn.mockResolvedValue('async');

expect(fn).toHaveBeenCalled();
expect(fn).toHaveBeenCalledWith('arg1', 'arg2');
expect(fn).toHaveBeenCalledTimes(3);
expect(fn.calls[0][0]).toBe('arg1');   // direct access

// spyOn — intercept real object methods
const s = spyOn(storage, 'get');
s.mockReturnValue('cached');
s.mockRestore();   // revert to original

// Clear all spies at once
clearAllMocks();
```

### Module mocking

```ts
import { mockModule } from 'hermes-test';
import { useMyHook } from './useMyHook';  // import order doesn't matter

mockModule('./useRedux', () => ({
  useAppSelector: (selector) => mockState,
}));
```

Shadow wrappers check mocks at call time — `mockModule` can appear before or after imports.

### Hook testing

```ts
import { renderHook, act, waitFor } from 'hermes-test';

const { result, history, renderCount } = renderHook(() => useCounter(0));
act(() => result.current.increment());
expect(result.current.count).toBe(1);
expect(renderCount).toBe(2);
```

### Fetch mocking (MSW-style)

```ts
import { mockFetch, mockFetchUse, mockFetchReset, http, HttpResponse } from 'hermes-test';

mockFetch(
  http.get('https://api.example.com/data', () => HttpResponse.json({ ok: true })),
  http.post('https://api.example.com/login', () => HttpResponse.json({ token: '...' })),
);

// Per-test override
mockFetchUse(http.get('https://api.example.com/data', () => HttpResponse.error()));

// Cleanup
mockFetchReset();
```

### Redux store

```ts
import { setupApiStore } from 'hermes-test/store';

const ctx = setupApiStore([api, cms], { app: rootReducer }, {
  preloadedState: { app: { auth: { session: mockSession } } },
});
const { result } = ctx.renderHookWithReduxStore(() => useMyHook());
ctx.store.dispatch(authActions.logout());
```

### Fake timers

```ts
import { useFakeTimers, advanceTimersByTime, useRealTimers } from 'hermes-test';

useFakeTimers();
setTimeout(() => { fired = true }, 1000);
advanceTimersByTime(1000);
expect(fired).toBe(true);
useRealTimers();
```

## How it works

```
┌──────────────┐     ┌─────────┐     ┌────────────┐
│  .test.ts    │────▶│ esbuild │────▶│   Hermes   │
│  files       │     │ bundle  │     │   VM eval  │
└──────────────┘     └─────────┘     └────────────┘
       │                  │                 │
  mockModule()      <100ms bundle     native execution
  spy/expect        path aliases      drainMicrotasks
  renderHook        Hermes patches     real React tree
```

1. **esbuild** bundles your test + source into a single IIFE (~100ms)
2. Rust CLI applies **Hermes patches** (class-extends, for-let-of)
3. **Bytecode compilation** — cached .hbc for instant loading on subsequent runs
4. **Hermes VM** evaluates the bytecode — same engine as your app
5. Results printed to terminal — single process, no workers, no IPC

### Three-tier cache

| Tier | What | Speed |
|---|---|---|
| **Bytecode (.hbc)** | Pre-compiled Hermes bytecode | Fastest — skip JS parsing |
| **Patched JS** | Post-patched esbuild output | Fast — skip bundling + patching |
| **Fresh bundle** | Full esbuild + patch pipeline | Cold start only |

### Auto-detect native externals

Native modules are detected automatically by scanning `node_modules` for `ios/`, `android/`, `*.podspec`, and `app.plugin.js`. No manual `externals` config needed for standard React Native packages.

### Mock isolation (Shadow Wrappers)

When multiple test files mock the same module differently, hermes-test uses **shadow wrappers** — filesystem-based Proxy wrappers that check which test file is running at call time. One bundle, one runtime, per-file mock isolation.

## Configuration

### Built-in shims

hermes-test ships with ready-to-use shims for common React Native ecosystem packages. Use `hermes-test/shims/<name>` in your config — no local shim files needed.

| Shim | What it provides |
|------|-----------------|
| `hermes-test/shims/react-native` | Platform, StyleSheet, Dimensions, Alert, Linking stubs |
| `hermes-test/shims/react-i18next` | Identity translation (`t('key')` returns `'key'`) |
| `hermes-test/shims/async-storage` | In-memory AsyncStorage (getItem, setItem, clear, etc.) |
| `hermes-test/shims/rtk-query` | RTK Query createApi singleton cache |
| `hermes-test/shims/react-redux` | Pass-through for react-redux |
| `hermes-test/shims/reduxjs-toolkit` | Pass-through for @reduxjs/toolkit |

### hermes-test.config.json

```json
{
  "root": "../..",
  "testMatch": ".hermes.test.ts",
  "shims": {
    "react-i18next": "hermes-test/shims/react-i18next",
    "@reduxjs/toolkit/query/react": "hermes-test/shims/rtk-query",
    "@react-native-async-storage/async-storage": "hermes-test/shims/async-storage"
  }
}
```

| Key | Description |
|-----|-------------|
| `root` | Monorepo workspace root (for resolving node_modules) |
| `testMatch` | Test file suffix (default: `.test.ts`) |
| `externals` | Additional modules to externalize (most are auto-detected) |
| `shims` | Built-in (`hermes-test/shims/...`) or custom (`./path/to/shim.js`) replacements |
| `split` | Enable vendor/group bundle splitting for large suites |

Most projects need zero `externals` — auto-detection handles `react-native-*`, `expo-*`, `@react-navigation/*`, `@sentry/*`, and any package with native code.

## Stack

- **Hermes** — the JS engine that ships with React Native and Expo
- **esbuild** — bundler, 100x faster than Babel/Metro transforms
- **Rust** — CLI host, native Hermes FFI, bytecode caching
- **TypeScript** — test harness (spy, expect, renderHook, mockFetch, timers)

## Why not Jest?

| | Jest + jest-expo | hermes-test |
|---|-----------------|-------------|
| Engine | Node (not your app) | Hermes (your app) |
| Startup | ~700ms per worker | ~5ms total |
| Transforms | Babel on every import | esbuild, one bundle |
| Native externals | Manual `transformIgnorePatterns` | Auto-detected |
| Config needed | `externals`, `transformIgnorePatterns`, `moduleNameMapper` | Zero for most projects |
| Watch rerun | ~2-3s | ~300ms |
| 1472 tests (no coverage) | 23s | **0.79s** |
| Coverage | Built-in (v8/Istanbul) | Planned (see roadmap) |

## Roadmap

- [ ] **Coverage reporting** — Istanbul-compatible via `swc-coverage-instrument` (pure Rust, no Node)
- [ ] **Component rendering** — `render(<Component />)` with query API (`getByText`, `getByTestId`, `fireEvent`)
- [ ] **Jest compatibility shim** — `jest.fn()` → `spy()`, `jest.mock()` → `mockModule()`, enables reuse of library `__mocks__/` files
- [ ] **Library mock support** — auto-load mocks from expo-router, react-native-reanimated, zustand, etc.
- [ ] **`setupFiles` config** — load setup files before tests (like Jest's `setupFilesAfterFramework`)

## License

MIT
