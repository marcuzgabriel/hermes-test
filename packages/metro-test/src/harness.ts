// Polyfills (process, setImmediate) are injected via esbuild banner in bundle.mjs
// to ensure they run before any bundled dependency (React checks process.env.NODE_ENV at load time)

import { expect } from './expect';
import { spy } from './spy';
import { renderHook, act, waitFor } from './hooks';
import { useMock, mockModule, resetMocks } from './mock';
import { mockFetch, mockFetchUse, mockFetchReset, mockFetchClear, http, HttpResponse } from './fetch';

type TestFn = (ctx: TestContext) => void | Promise<void>;
type TestContext = {
  expect: typeof expect;
  spy: typeof spy;
  useMock: typeof useMock;
  renderHook: typeof renderHook;
  act: typeof act;
  waitFor: typeof waitFor;
};
type TestOptions = { timeout?: number; skip?: boolean; only?: boolean };

type TestEntry = {
  name: string;
  fn: TestFn;
  options: TestOptions;
  group?: string;
};

type TestResult = {
  name: string;
  status: 'pass' | 'fail' | 'skip';
  error?: string;
  duration: number;
};

type LifecycleHook = () => void | Promise<void>;

// Global state for test registration
const tests: TestEntry[] = [];
const beforeEachHooks: LifecycleHook[] = [];
const afterEachHooks: LifecycleHook[] = [];
const beforeAllHooks: LifecycleHook[] = [];
const afterAllHooks: LifecycleHook[] = [];
let currentGroup: string | undefined;

function test(name: string, fn: TestFn, options?: TestOptions): void {
  tests.push({
    name: currentGroup ? `${currentGroup} > ${name}` : name,
    fn,
    options: options ?? {},
    group: currentGroup,
  });
}

test.only = function(name: string, fn: TestFn): void {
  test(name, fn, { only: true });
};

test.skip = function(name: string, fn: TestFn): void {
  test(name, fn, { skip: true });
};

function group(name: string, fn: () => void): void {
  const prev = currentGroup;
  currentGroup = prev ? `${prev} > ${name}` : name;
  fn();
  currentGroup = prev;
}

function beforeEach(fn: LifecycleHook): void {
  beforeEachHooks.push(fn);
}

function afterEach(fn: LifecycleHook): void {
  afterEachHooks.push(fn);
}

function beforeAll(fn: LifecycleHook): void {
  beforeAllHooks.push(fn);
}

function afterAll(fn: LifecycleHook): void {
  afterAllHooks.push(fn);
}

const drain = (globalThis as any).__drainMicrotasks || (() => {});

// Synchronously resolve a promise by flushing the microtask queue.
// Usage: const result = flushAsync(store.dispatch(api.endpoints.login.initiate(payload)));
function flushAsync<T = any>(promise: Promise<T> | T): T {
  if (!promise || typeof (promise as any).then !== 'function') {
    return promise as T;
  }
  let result: T | undefined;
  let error: any;
  let settled = false;
  (promise as Promise<T>).then(
    (v) => { result = v; settled = true; },
    (e) => { error = e; settled = true; }
  );
  // Each drain() flushes all current microtasks. We loop because resolved work
  // may schedule new async work (promise chains, effects, timers). The loop
  // exits as soon as our promise settles. The cap prevents infinite loops.
  for (let i = 0; i < 100 && !settled; i++) {
    drain();
  }
  if (!settled) {
    throw new Error('flushAsync: promise did not resolve after 100 drain cycles');
  }
  if (error) throw error;
  return result as T;
}

// Synchronously resolve a value that may be a Promise by draining microtasks
function resolveSync(value: any): void {
  if (value && typeof value.then === 'function') {
    flushAsync(value);
  }
}

function runTests(): TestResult[] {
  const results: TestResult[] = [];
  const hasOnly = tests.some((t) => t.options.only);

  // Run beforeAll hooks
  for (const hook of beforeAllHooks) {
    resolveSync(hook());
  }

  for (const entry of tests) {
    if (entry.options.skip || (hasOnly && !entry.options.only)) {
      results.push({ name: entry.name, status: 'skip', duration: 0 });
      continue;
    }

    const start = Date.now();
    try {
      // Run beforeEach hooks
      for (const hook of beforeEachHooks) {
        resolveSync(hook());
      }

      const ctx: TestContext = { expect, spy, useMock, renderHook, act, waitFor };
      resolveSync(entry.fn(ctx));

      // Run afterEach hooks
      for (const hook of afterEachHooks) {
        resolveSync(hook());
      }

      // Reset mocks between tests
      resetMocks();

      results.push({
        name: entry.name,
        status: 'pass',
        duration: Date.now() - start,
      });
    } catch (e: any) {
      // Still run afterEach even on failure
      for (const hook of afterEachHooks) {
        try {
          resolveSync(hook());
        } catch {}
      }

      // Reset mocks between tests
      resetMocks();

      results.push({
        name: entry.name,
        status: 'fail',
        error: e?.message ?? String(e),
        duration: Date.now() - start,
      });
    }
  }

  // Run afterAll hooks
  for (const hook of afterAllHooks) {
    try {
      resolveSync(hook());
    } catch {}
  }

  return results;
}

// Expose to the global scope for the harness entry
(globalThis as any).__metroTest = {
  test,
  expect,
  spy,
  group,
  beforeEach,
  afterEach,
  beforeAll,
  afterAll,
  runTests,
  renderHook,
  act,
  waitFor,
  useMock,
  mockModule,
  mockFetch,
  mockFetchUse,
  mockFetchReset,
  mockFetchClear,
  http,
  HttpResponse,
  flushAsync,
};

export { test, expect, spy, group, beforeEach, afterEach, beforeAll, afterAll, renderHook, act, waitFor, useMock, mockModule, mockFetch, mockFetchUse, mockFetchReset, mockFetchClear, http, HttpResponse, flushAsync };
