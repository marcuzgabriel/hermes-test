# hermes-test — Test Writing Guide

## STOP. Read this ENTIRELY before writing or fixing ANY test.

There are 1400+ passing tests. Study them. Follow the same patterns. Do NOT invent new approaches.

## hermes-test API (import from 'hermes-test')

### Core
- `test(name, ({ expect }) => { ... })` — expect comes from callback, NOT global
- `test.skip(name, fn)` — register a test but skip it (shows as "skip" in output)
- `test.only(name, fn)` — run only this test (and other `.only` tests), skip everything else
- `test(name, fn, { timeout: 5000 })` — set per-test timeout in ms (via options object)
- `group(name, fn)` — replaces Jest `describe` (no `.skip` or `.only` on group)
- `beforeEach`, `afterEach`, `beforeAll`, `afterAll`

### Spies
- `spy()` — returns a spy with: `.mockReturnValue()`, `.mockReturnValueOnce()`, `.mockImplementation()`, `.mockImplementationOnce()`, `.mockResolvedValue()`, `.mockResolvedValueOnce()`, `.mockRejectedValue()`, `.mockRejectedValueOnce()`, `.mockClear()`, `.mockReset()`, `.mockRestore()`
- `.calls` — array of call args (preferred over Jest's `.mock.calls`)
- `.callCount` — number of times called
- `.returnValues` — array of return values
- `spyOn(obj, method)` — spy on existing object method

### Expect Matchers
- `toBe`, `toEqual`, `toBeDefined`, `toBeUndefined`, `toBeNull`
- `toBeTruthy`, `toBeFalsy`
- `toBeGreaterThan`, `toBeLessThan`
- `toHaveLength`, `toBeInstanceOf`
- `toContain` (arrays and strings), `toContainEqual` (deep equality in arrays)
- `toBeCloseTo(expected, precision?)` — floating-point comparison
- `toMatch(regex | string)` — string matching
- `toThrow(message?)` — function throw assertion
- `.not.` prefix — negate any matcher (e.g. `expect(x).not.toBe(y)`)

#### Spy-specific matchers
- `wasCalled()`, `wasCalledOnce()`, `wasCalledTimes(n)`, `wasCalledWith(...args)`, `wasLastCalledWith(...args)`, `wasNeverCalled()`
- Jest aliases: `toHaveBeenCalled()`, `toHaveBeenCalledTimes(n)`, `toHaveBeenCalledWith(...)`, `toHaveBeenLastCalledWith(...)`

#### Asymmetric matchers
- `expect.anything()` — matches any value except `null` and `undefined`
- `expect.any(Type)` — matches values of given type (String, Number, Boolean, Function, or `instanceof`)
- `expect.objectContaining(subset)` — matches objects containing at least the given keys/values
- `expect.arrayContaining(arr)` — matches arrays containing at least the given elements
- `expect.stringContaining(substr)` — matches strings containing the substring
- `expect.stringMatching(pattern)` — matches strings against a regex or string pattern

#### Async matchers (promise chains)
- `await expect(promise).resolves.toBe(value)`
- `await expect(promise).resolves.toEqual(value)`
- `await expect(promise).resolves.toBeDefined()`
- `await expect(promise).resolves.toBeUndefined()`
- `await expect(promise).resolves.toBeNull()`
- `await expect(promise).resolves.toBeTruthy()`
- `await expect(promise).resolves.toBeFalsy()`
- `await expect(promise).rejects.toThrow(msg?)`

### Hooks & Async
- `renderHook(fn, { wrapper?, initialProps? })` — returns `{ result, rerender }`
- `act(fn)` — wrap state updates
- `waitFor(pred, { timeout?, interval? })` — poll until predicate passes
- `flushAsync(promise)` — synchronously resolve a promise

### Mock System
- `mockModule(path, factory)` — registers mock. Works for barrel paths and node_modules. Does NOT work for relative imports (see below).

### Fetch Mocking (MSW-like)
- `mockFetch(handler...)` — register base handlers
- `mockFetchUse(handler...)` — per-test overrides (like MSW server.use)
- `mockFetchReset()` — clear per-test overrides
- `http.get(url|RegExp, handler)`, `http.post(...)` etc.
- `HttpResponse.json(data, { status? })` — build response
- URL matching: exact string, prefix match, or RegExp

### Timers
- `useFakeTimers(timestamp?)`, `useRealTimers()`
- `advanceTimersByTime(ms)`, `runAllTimers()`, `getTimerCount()`

## NOT supported in hermes-test expect
- `toStrictEqual` — use `toEqual`

## Migrating from Jest

### `.calls` vs `.mock.calls`
hermes-test spies use `.calls` directly — there is no `.mock` wrapper object. Jest's `.mock.calls` is NOT available.
```ts
// Jest
expect(mySpy.mock.calls[0][0]).toBe('arg');
// hermes-test
expect(mySpy.calls[0][0]).toBe('arg');
```

### `expect` — import vs callback
Both work. The callback style is idiomatic hermes-test:
```ts
// Preferred: expect from callback (scoped to test)
test('name', ({ expect }) => { expect(1).toBe(1); });
// Also works: top-level import
import { expect } from 'hermes-test';
test('name', () => { expect(1).toBe(1); });
```

### `mockModule` vs `jest.mock`
In Jest, `jest.mock` is hoisted to the top of the file. In hermes-test, `mockModule` ordering does NOT matter — it registers a factory that shadow wrappers check at call time. You can place `mockModule` calls anywhere before the test runs.
```ts
// Both of these work identically in hermes-test:
mockModule('some-package', () => ({ foo: spy() }));
import { foo } from 'some-package';
// or
import { foo } from 'some-package';
mockModule('some-package', () => ({ foo: spy() }));
```

## The Two Test Patterns

### Pattern 1: Barrel-path hooks (useGetClaimDetails, useServicePills, etc.)
These hooks import dependencies via barrel paths (`@topdanmark/mobile-insurance-app/hooks/...`). Shadow wrappers intercept these. Use `mockModule` on the barrel path.

```ts
import { test, renderHook, mockModule, spy } from 'hermes-test';

const mockUseAppSelector = spy();
mockModule('@topdanmark/mobile-insurance-app/hooks/redux/useRedux', () => ({
  useAppSelector: mockUseAppSelector,
}));
mockModule('@topdanmark/mobile-insurance-app/hooks/i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));
mockModule('@topdanmark/mobile-insurance-app/hooks/errorHandling/useErrorHandling', () => ({
  useErrorHandling: () => ({ error: null, dispatchWithErrorHandler: spy() }),
}));

import { useMyHook } from '@topdanmark/mobile-insurance-app/hooks/myHook';

test('it works', ({ expect }) => {
  mockUseAppSelector.mockReturnValue({ data: 'test' });
  const { result } = renderHook(() => useMyHook());
  expect(result.current.data).toBe('test');
});
```

This is how the 835 original passing tests work. **Follow this pattern first.**

### Pattern 2: Relative-import hooks (useErrorHandling, useIsLoading, useActionMessages, etc.)
These hooks import dependencies via relative paths (`../redux/useRedux`, `../i18n`). Shadow wrappers do NOT intercept relative imports. mockModule on barrel path won't work.

For these, use `withStore` from `test/testStore.ts`:

```ts
import { test, group } from 'hermes-test';
import { withStore } from '../../../../test/testStore';

// react-i18next is shimmed globally in hermes-test.config.json
// so useI18n works automatically (identity t function)

const ctx = withStore({
  app: {
    errorHandling: { currentErrors: [{ tag: 'getX', error: 'NETWORK_ERROR' }] },
    loading: { tags: [] },
  },
});

test('shows error', ({ expect }) => {
  const { result } = ctx.renderHookWithReduxStore(() => useErrorHandling('getX'));
  expect(result.current.error).toBe('errorNoInternetConnection');
});
```

### Pattern 3: RTK Query API slice tests
Use `withApiStore` + `mockFetch`:

```ts
import { test, mockFetch, mockFetchReset, http, HttpResponse, afterEach } from 'hermes-test';
import { withApiStore } from '../../../../../../test/testStore';
import claimsApi from '@topdanmark/mobile-insurance-app/reducers/slices/api/claims';

afterEach(() => { mockFetchReset(); });

test('fetch claims', async ({ expect }) => {
  mockFetch(http.get(/\/claims/, () => HttpResponse.json(mockData)));
  const ctx = withApiStore({});
  const res = await ctx.store.dispatch(claimsApi.endpoints.getClaims.initiate(payload));
  expect(res.status).toBe('fulfilled');
});
```

Use RegExp for URLs with path params (`:claimId` etc.).

### Pattern 4: Integration tests (prod-tests style)
See `examples/expo-app/src/prod-tests/`. Copy hook locally, use `useSelector` from react-redux directly, inject deps via store state, use local stubs with `.reset()`.

## testStore.ts API (test/testStore.ts)
- `withStore(initialState)` — identity reducer, for hooks needing Redux Provider
- `withApiStore(initialState)` — real RTK Query middleware, for API slice tests
- `ctx.renderHookWithReduxStore(hookFn)` — renderHook with Provider wrapper
- `ctx.patchState(partial)` — update store state mid-test
- `ctx.setState(full)` — replace store state
- `ctx.store.dispatch(action)` — dispatch real actions
- `ctx.getState()` — read current state

## How to determine which pattern to use
1. Check the hook's imports: `grep "from '\.\." src/hooks/myHook/myHook.ts`
2. If all imports use barrel paths (`@topdanmark/...`) → Pattern 1 (mockModule)
3. If any imports use relative paths (`../redux`, `../i18n`) → Pattern 2 (withStore)
4. If it's an API slice test with store.dispatch → Pattern 3 (withApiStore + mockFetch)

## Mock contamination between files
- In single-bundle mode, all test files share one JS context
- mockModule writes to per-file scope (`__HT_file_mocks[filename]`)
- Shadow wrappers check per-file mocks at runtime
- Do NOT mock `react-redux` or `react-i18next` via mockModule — these contaminate globally
- `react-i18next` is shimmed globally in hermes-test.config.json
- For Redux, use `withStore` instead of mocking

## Hermes Engine — what works and what doesn't
### Works (polyfilled)
- `URL`, `URLSearchParams` — fully polyfilled including iterator, constructor from object
- `crypto.getRandomValues` — polyfilled
- `process.nextTick` — polyfilled
- `setImmediate` — polyfilled
- `async/await` — native Hermes support
- `console.log/warn/error` — installed via JSI

### Hermes quirks (not bugs, just different from Node)
- `toLocaleString('da-DK')` — no thousands separator: `1000000` not `1.000.000`
- Date locale — abbreviated months: `jan.` not `januar`
- `NaN` comparison — `toBe(NaN)` fails, use `Number.isNaN()`
- No DOM — UI component imports crash (AnimatedPressable, rive, etc.)
- No `Response` class — use `{ ok: true, status: 200, json: () => Promise.resolve({}) }`

### Config (hermes-test.config.json)
- `externals` — modules externalized from bundle (native modules, react-navigation, etc.)
- `shims` — replacement modules for externalized packages
- `testMatch` — test file suffix (e.g. `.hermes.test.ts`)
- `root` — monorepo workspace root
- `split` — enable vendor/group bundle splitting

## DO NOT
- Use agents to bulk-rewrite test files
- Create new test infrastructure (testStore etc.) — it already exists
- Mock react-redux or react-i18next via mockModule
- Use `.mock.calls` (Jest API) — use `.calls`
- Use `new Response()` — Hermes doesn't have it
- Assert exact call counts on `useAppDispatch` — React re-renders cause extra calls
- Guess at patterns — look at the 1400+ passing tests first
