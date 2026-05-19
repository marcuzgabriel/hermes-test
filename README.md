# hermes-test

**75x faster than Jest.** A test runner for React Native that executes your tests in Hermes — the same JavaScript engine your app ships with.

No Babel transforms. No `transformIgnorePatterns`. No `jest-expo` mock layer. No 20-second test runs. Just your code, running in the real engine, at native speed.

```
245 tests — 300ms
```

---

### The problem

Every React Native project tests in Node. Your app runs in Hermes. These are different JavaScript engines with different behaviors. That gap is where bugs hide — and Jest can't find them because it's running in the wrong engine entirely.

On top of that, Jest in React Native is slow by design. Every test file spawns a worker, runs Babel transforms, resolves `transformIgnorePatterns` for every `node_modules` import, and coordinates results over IPC. For a mid-size app, that's 20-60 seconds per run. Developers stop running tests. Tests rot. Coverage drops.

### The fix

hermes-test runs your tests directly in Hermes. esbuild bundles your test + source into a single file in <100ms. A Rust CLI feeds it to the Hermes VM. One process, no workers, no transforms. Results appear before your hand leaves `Cmd+S`.

Real-world benchmark — 20 production test suites from a Danish insurance app (Topdanmark):

| | Jest | hermes-test | Speedup |
|---|------|-------------|---------|
| Wall clock | **22.5s** | **0.3s** | **75x** |
| CPU time | 86.3s | 0.28s | **308x** |

Same tests, same assertions, same business logic. The difference is everything around the tests.

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

- **Hermes** — V8-class JS engine that ships with React Native
- **esbuild** — bundler, 100x faster than Babel/Metro transforms
- **Rust** — CLI host, native Hermes FFI, zero-overhead process management
- **TypeScript** — test harness (spy, expect, renderHook, mockFetch, timers)

## Install

```bash
# In your RN project
bun add -D hermes-test

# Add test script
# package.json: "test": "hermes-test", "test:watch": "hermes-test --watch"
```

## Configuration

### tsconfig.json paths

hermes-test reads `tsconfig.json` paths automatically and passes them as esbuild aliases:

```json
{
  "compilerOptions": {
    "paths": {
      "@app/*": ["./src/*"]
    }
  }
}
```

### hermes-test.config.json

```json
{
  "nativeModuleStubs": [
    "expo-web-browser",
    "expo-local-authentication",
    "react-native-launch-arguments"
  ]
}
```

## License

MIT
