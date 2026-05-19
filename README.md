# hermes-test

**75x faster than Jest.** A test runner for React Native and Expo that executes your tests in Hermes — the same JavaScript engine your app ships with.

No Babel transforms. No `transformIgnorePatterns`. No `jest-expo` mock layer. No 20-second test runs. Just your code, running in the real engine, at native speed.

Built for **Expo** and **React Native** projects that use hooks, Redux, RTK Query, and TypeScript.

```
245 tests — 300ms
```

---

### The problem

Every React Native and Expo project tests in Node. Your app runs in Hermes. These are different JavaScript engines with different behaviors. That gap is where bugs hide — and Jest can't find them because it's running in the wrong engine entirely.

On top of that, Jest in React Native is slow by design. Every test file spawns a worker, runs Babel transforms, resolves `transformIgnorePatterns` for every `node_modules` import, and coordinates results over IPC. For a mid-size Expo app, that's 20-60 seconds per run. Developers stop running tests. Tests rot. Coverage drops.

`jest-expo` tries to help but adds its own problems — auto-mocking that silently drifts from real module behavior, complex preset configuration, and mocks that are JS pretending to be native modules.

### The fix

hermes-test runs your tests directly in Hermes. esbuild bundles your test + source into a single file in <100ms. A Rust CLI feeds it to the Hermes VM. One process, no workers, no transforms. Results appear before your hand leaves `Cmd+S`.

Most Jest mocks become unnecessary — Hermes runs your real hooks, real Redux store, real business logic natively. In a production Expo app (Topdanmark, Danish insurance), **84% of jest.mock calls were eliminated** because the real code just works in Hermes.

### Benchmarks

All measurements on Apple Silicon. `hyperfine` for micro, `time` for macro. Jest uses `@swc/jest` (fastest Jest transform).

**Micro — cold start, same test file:**

| Scenario | hermes-test | Jest + @swc/jest | Speedup |
|----------|-------------|------------------|---------|
| 10 pure function tests | **16ms** | 714ms | **45x** |
| 50 pure function tests | **16ms** | 737ms | **47x** |
| 50 hook tests (renderHook + act) | **75ms** | 721ms | **10x** |
| 1000 hook tests | **200ms** | 883ms | **4.4x** |
| Watch rerun (25 tests) | **69ms** | ~2,000ms | **29x** |
| Trivial cold start | **4.6ms** | 1,486ms | **364x** |

**Macro — real production test suites (Topdanmark insurance app):**

| | Jest | hermes-test | Speedup |
|---|------|-------------|---------|
| 20 test files, 245 tests | **22.5s** | **0.3s** | **75x** |
| CPU time | 86.3s | 0.28s | **308x** |

hermes-test scales linearly (~0.13ms per hook test). Jest's overhead is fixed startup (~700ms per worker) so the gap widens with more files and narrows with more tests per file.

Full benchmark methodology and reproduction commands in [BENCHMARKS.md](./BENCHMARKS.md).

---

### Quick start

```bash
bun add -D hermes-test
```

```ts
import { test, renderHook, act } from 'hermes-test';

test('useCounter increments', ({ expect }) => {
  const { current } = renderHook(() => useCounter(0));
  act(() => current.increment());
  expect(current.count).toBe(1);
});
```

```bash
hermes-test --watch
```

## Benchmarks

Real-world insurance app (Topdanmark), same test logic, same assertions:

| | Jest | hermes-test | Speedup |
|---|------|-------------|---------|
| 10 test files (197 tests) | 22.5s | 0.3s | **75x** |
| CPU time | 86.3s | 0.28s | **308x** |
| Watch rerun (1 file) | ~3s | <200ms | **15x** |

## API

### Test structure

```ts
import { test, group, beforeEach, afterEach, expect } from 'hermes-test';

group('myFeature', () => {
  beforeEach(() => { /* reset */ });

  test('does the thing', ({ expect }) => {
    expect(result).toBe(42);
    expect(arr).toEqual([1, 2, 3]);
    expect(str).toContain('hello');
    expect(fn).toThrow('error message');
  });

  test.skip('not yet', () => {});
  test.only('focus this', () => {});
});
```

### Assertions

```ts
expect(val).toBe(exact)            expect(val).toEqual(deep)
expect(val).toBeTruthy()           expect(val).toBeFalsy()
expect(val).toBeDefined()          expect(val).toBeUndefined()
expect(val).toBeNull()             expect(val).toBeGreaterThan(n)
expect(val).toContain(item)        expect(val).toMatch(/regex/)
expect(fn).toThrow('msg')          expect(val).not.toBe(other)
```

### Spies

```ts
import { spy, spyOn } from 'hermes-test';

const fn = spy(() => 'default');
fn.mockReturnValue('mocked');
fn.mockReturnValueOnce('first').mockReturnValueOnce('second');
fn.mockImplementation((x) => x * 2);
fn.mockResolvedValue('async');
fn.mockClear();    // clear calls, keep impl
fn.mockReset();    // clear everything
fn.mockRestore();  // revert spyOn

expect(fn).wasCalled();
expect(fn).wasCalledWith('arg1', 'arg2');
expect(fn).wasCalledOnce();
expect(fn).wasCalledTimes(3);
expect(fn).wasNeverCalled();

// spyOn — intercept real object methods
const s = spyOn(storage, 'get');
s.mockReturnValue('cached');
storage.get('key');        // returns 'cached'
expect(s).wasCalledWith('key');
s.mockRestore();           // reverts to original
```

### Module mocking

```ts
import { mockModule } from 'hermes-test';

// Register before imports — like jest.mock() but explicit
mockModule('./useRedux', () => ({
  useAppSelector: (selector) => mockState,
}));

import { useMyHook } from './useMyHook';
```

### Hook testing

```ts
import { renderHook, act, waitFor } from 'hermes-test';

const { current, history, renderCount } = renderHook(() => useCounter(0));
act(() => current.increment());
expect(current.count).toBe(1);
expect(renderCount).toBe(2);

// With Redux Provider
const { current } = renderHook(() => useMyHook(), {
  wrapper: ({ children }) => <Provider store={store}>{children}</Provider>,
});
```

### Fetch mocking (MSW-style)

```ts
import { mockFetch, mockFetchUse, mockFetchReset, http, HttpResponse } from 'hermes-test';

// Base handlers
mockFetch(
  http.get('https://api.example.com/data', () => HttpResponse.json({ ok: true })),
  http.post('https://api.example.com/login', () => HttpResponse.json({ token: '...' })),
);

// Per-test override
mockFetchUse(
  http.get('https://api.example.com/data', () => HttpResponse.json({ error: 'fail' }, { status: 500 })),
);

// Cleanup
mockFetchReset();
```

### Async

```ts
import { flushAsync } from 'hermes-test';

// Synchronously resolve promises (Hermes microtask queue)
const result = flushAsync(store.dispatch(api.endpoints.login.initiate(payload)));
expect(result.data.token).toBe('...');
```

### Fake timers

```ts
import { useFakeTimers, advanceTimersByTime, useRealTimers, runAllTimers } from 'hermes-test';

useFakeTimers();
setTimeout(() => { fired = true }, 1000);
advanceTimersByTime(1000);
expect(fired).toBe(true);
expect(Date.now()).toBe(1000);  // Date.now() tracks fake time
useRealTimers();
```

### Redux test store

```ts
import { withStore, withAppReducer } from 'hermes-test/store';

// Quick seedable store
const ctx = withStore({ user: { name: 'Test' }, insurances: [] });
ctx.patchState({ insurances: [mockProduct] });
const { current } = ctx.renderHookWithReduxStore(() => useMyHook());

// Real app reducer — patchState + real actions both work
const ctx = withAppReducer(rootReducer, initialState);
ctx.dispatch(authActions.login({ token: '...' }));
ctx.patchState({ insurances: [mockProduct] });
const { current } = ctx.renderHookWithReduxStore(() => useMyHook());
```

## How it works

```
┌──────────────┐     ┌─────────┐     ┌────────────┐
│  .test.ts    │────▶│ esbuild │────▶│   Hermes   │
│  files       │     │ bundle  │     │   VM eval  │
└──────────────┘     └─────────┘     └────────────┘
       │                  │                 │
  mockModule()      <100ms bundle     native execution
  spy/expect         path aliases      drainMicrotasks
  renderHook        Hermes patches     real React tree
```

1. **esbuild** bundles your test + source into a single IIFE (~100ms)
2. Rust CLI applies **Hermes patches** (class-extends, for-let-of, configurable getters)
3. **Hermes VM** evaluates the bundle via `Runtime::run()` — same engine as your app
4. Results printed to terminal — single process, no workers, no IPC

## Stack

- **Hermes** — the JS engine that ships with React Native and Expo
- **esbuild** — bundler, 100x faster than Babel/Metro transforms
- **Rust** — CLI host, native Hermes FFI, zero-overhead process management
- **TypeScript** — test harness (spy, expect, renderHook, mockFetch, timers)

## Works with

- **Expo** (SDK 50+)
- **React Native** (0.73+ with Hermes)
- **Redux / RTK Query**
- **React hooks**
- **TypeScript**
- **Monorepos** (path aliases via tsconfig.json)

## Install

```bash
bun add -D hermes-test
```

Add to `package.json`:
```json
{
  "scripts": {
    "test": "hermes-test",
    "test:watch": "hermes-test --watch"
  }
}
```

Run:
```bash
bun run test                          # all .test.ts files
bun run test src/hooks/useLogin.test.ts  # specific file
bun run test:watch                    # watch mode
```

## Configuration

### tsconfig.json paths

hermes-test reads `tsconfig.json` paths automatically and passes them as esbuild aliases. Monorepo path aliases just work:

```json
{
  "compilerOptions": {
    "paths": {
      "@app/*": ["./src/*"],
      "@myorg/shared/*": ["../../packages/shared/src/*"]
    }
  }
}
```

### Native module stubs

Expo and React Native modules that call native code need to be stubbed since Hermes runs without a native runtime. Add a `hermes-test.config.json`:

```json
{
  "nativeModuleStubs": [
    "expo-web-browser",
    "expo-local-authentication",
    "expo-application",
    "expo-device",
    "react-native-launch-arguments"
  ]
}
```

Stubbed modules return empty objects. Your test code uses `mockModule` or `mockFetch` for the behavior you need.

## Why not Jest?

| | Jest + jest-expo | hermes-test |
|---|-----------------|-------------|
| Engine | Node (not your app) | Hermes (your app) |
| Startup | ~700ms per worker | ~5ms total |
| Transforms | Babel on every import | esbuild, one bundle |
| Mocks needed | 394 in a real app | ~60 (native only) |
| `transformIgnorePatterns` | Required, fragile | Not needed |
| Watch rerun | ~2-3s | ~70ms |

## License

MIT

