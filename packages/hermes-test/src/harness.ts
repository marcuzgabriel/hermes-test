// Polyfills (process, setImmediate) are injected via esbuild banner in bundle.mjs
// to ensure they run before any bundled dependency (React checks process.env.NODE_ENV at load time)

import { expect, _setSnapshotContext } from './expect';
import { spy, spyOn, clearAllMocks } from './spy';
import { renderHook, act, waitFor } from './hooks';
import { render, fireEvent } from './render';
import { useMock, mockModule, resetMocks, resetMockModulePatches } from './mock';
import { mockFetch, mockFetchUse, mockFetchReset, mockFetchClear, http, HttpResponse } from './fetch';
import { useFakeTimers, useRealTimers, advanceTimersByTime, runAllTimers, getTimerCount, advanceTimersToNextTimer } from './timers';

type TestFn = ((ctx: TestContext) => void | Promise<void>) | (() => void | Promise<void>);
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
  file?: string;
};

type TestResult = {
  name: string;
  status: 'pass' | 'fail' | 'skip';
  error?: string;
  duration: number;
  file?: string;
};

type LifecycleHook = () => void | Promise<void>;
type ScopedHook = { fn: LifecycleHook; group: string | undefined };

// Global state for test registration
const tests: TestEntry[] = [];
const beforeEachHooks: ScopedHook[] = [];
const afterEachHooks: ScopedHook[] = [];
const beforeAllHooks: ScopedHook[] = [];
const afterAllHooks: ScopedHook[] = [];
let currentGroup: string | undefined;

function test(name: string, fn: TestFn, options?: TestOptions): void {
  tests.push({
    name: currentGroup ? `${currentGroup} > ${name}` : name,
    fn,
    options: options ?? {},
    group: currentGroup,
    file: (globalThis as any).__currentTestFile,
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
  beforeEachHooks.push({ fn, group: currentGroup });
}

function afterEach(fn: LifecycleHook): void {
  afterEachHooks.push({ fn, group: currentGroup });
}

function beforeAll(fn: LifecycleHook): void {
  beforeAllHooks.push({ fn, group: currentGroup });
}

function afterAll(fn: LifecycleHook): void {
  afterAllHooks.push({ fn, group: currentGroup });
}

/// Check if a hook's group scope applies to a test's group.
/// A hook applies if: (1) it's global (no group), or (2) the test's group
/// starts with the hook's group (ancestor or same group).
function hookApplies(hook: ScopedHook, testGroup: string | undefined): boolean {
  if (hook.group === undefined) return true; // global hook
  if (testGroup === undefined) return false; // global test, scoped hook
  return testGroup === hook.group || testGroup.startsWith(hook.group + ' > ');
}

const drain = (globalThis as any).__HT_drain || (() => {});

// Global timeout context for the currently running test.
// flushAsync checks this to abort long-running drain loops.
// Use _realNow to avoid interference from useFakeTimers.
// Imported from timers.ts where the real Date.now is saved before any faking.
// Timeout tracking uses drain-loop iterations, not wall clock time.
// Hermes's Date.now cannot be reliably saved — useFakeTimers replaces the
// native slot, so even saved references return faked time.
// At ~1ms per drain cycle, 5000 iterations ≈ 5 seconds.
let __testMaxDrains: number = 0; // 0 = no limit
let __testDrainCount: number = 0;
let __testTimeoutMs: number = 0;
const DEFAULT_TIMEOUT_MS = 0; // 0 = no default timeout; users opt in via test('name', fn, { timeout: 5000 })
const DRAINS_PER_MS = 1; // approximate: 1 drain ≈ 1ms

function checkDeadline(): void {
  if (__testMaxDrains > 0 && ++__testDrainCount >= __testMaxDrains) {
    throw new Error('Test timed out after ' + __testTimeoutMs + 'ms');
  }
}

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
    // Check timeout during drain loop to catch deadlocked async work
    checkDeadline();
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

// Live output — print to stderr immediately via native __HT_print (if available)
const _print = (globalThis as any).__HT_print || (() => {});

let _filesCompleted = 0;
let _testsCompleted = 0;
let _totalFiles = 0;

function _printFileResult(file: string, passed: number, failed: number, duration: number) {
  const total = passed + failed;
  const time = duration > 0 ? ` \x1b[2m(${duration}ms)\x1b[0m` : '';
  _filesCompleted++;
  _testsCompleted += total;
  if (failed > 0) {
    if ((globalThis as any).__HT_coverage) {
      // Clear progress line before printing failure
      _print(`\r\x1b[K`);
    }
    _print(` \x1b[31mFAIL\x1b[0m  ${file} \x1b[2m(${passed} passed, ${failed} failed)\x1b[0m${time}\n`);
  } else if ((globalThis as any).__HT_coverage) {
    // In-place progress counter
    _print(`\r\x1b[K \x1b[2mRunning...\x1b[0m ${_filesCompleted}/${_totalFiles} files (${_testsCompleted} tests)`);
  } else {
    _print(` \x1b[32mPASS\x1b[0m  ${file} \x1b[2m(${total} tests)\x1b[0m${time}\n`);
  }
}

function runTests(): TestResult[] {
  const results: TestResult[] = [];
  const hasOnly = tests.some((t) => t.options.only);

  // Count unique files for progress counter
  const uniqueFiles = new Set(tests.map(t => t.file));
  _totalFiles = uniqueFiles.size;
  _filesCompleted = 0;
  _testsCompleted = 0;

  // Track per-file results for live output
  let _currentFile: string | undefined;
  let _filePassed = 0;
  let _fileFailed = 0;
  let _fileStart = Date.now();
  let _fileFailures: { name: string; error: string }[] = [];

  function _flushFileResult() {
    if (_currentFile && (_filePassed + _fileFailed) > 0) {
      _printFileResult(_currentFile, _filePassed, _fileFailed, Date.now() - _fileStart);
      // Print failure details
      for (const f of _fileFailures) {
        _print(`       \x1b[31m✗ ${f.name}\x1b[0m\n`);
        if (f.error) _print(`         \x1b[2m${f.error}\x1b[0m\n`);
      }
    }
    _filePassed = 0;
    _fileFailed = 0;
    _fileFailures = [];
    _fileStart = Date.now();
  }

  // Run beforeAll hooks (scoped)
  const beforeAllRan = new Set<ScopedHook>();

  for (const entry of tests) {
    // Flush live output when switching to a new file
    if (entry.file !== _currentFile) {
      _flushFileResult();
      _currentFile = entry.file;
    }

    // Set current test file for per-file mock scoping
    (globalThis as any).__currentTestFile = entry.file;

    if (entry.options.skip || (hasOnly && !entry.options.only)) {
      results.push({ name: entry.name, status: 'skip', duration: 0, file: entry.file });
      continue;
    }

    // Run beforeAll hooks that apply to this test (once per scope)
    for (const hook of beforeAllHooks) {
      if (!beforeAllRan.has(hook) && hookApplies(hook, entry.group)) {
        beforeAllRan.add(hook);
        resolveSync(hook.fn());
      }
    }

    // Set up snapshot context for this test
    {
      // Use full file path (set by entry code) for correct snapshot directory
      const filePath = (globalThis as any).__currentTestFilePath || entry.file || 'unknown';
      // Strip leading ./ if present
      const clean = filePath.startsWith('./') ? filePath.substring(2) : filePath;
      const lastSlash = clean.lastIndexOf('/');
      const dir = lastSlash >= 0 ? clean.substring(0, lastSlash) : '.';
      const basename = lastSlash >= 0 ? clean.substring(lastSlash + 1) : clean;
      const snapFile = dir + '/__snapshots__/' + basename + '.snap';
      _setSnapshotContext(snapFile, entry.name, !!(globalThis as any).__HT_updateSnapshots);
    }

    // Set up timeout for this test
    const timeoutMs = entry.options.timeout ?? DEFAULT_TIMEOUT_MS;
    __testTimeoutMs = timeoutMs;
    __testDrainCount = 0;
    __testMaxDrains = timeoutMs > 0 ? timeoutMs * DRAINS_PER_MS : 0;
    const start = Date.now();

    try {
      // Run beforeEach hooks that apply to this test's group
      for (const hook of beforeEachHooks) {
        if (hookApplies(hook, entry.group)) {
          resolveSync(hook.fn());
          checkDeadline();
        }
      }

      const ctx: TestContext = { expect, spy, useMock, renderHook, act, waitFor };
      resolveSync(entry.fn(ctx));
      checkDeadline();

      // Run afterEach hooks that apply to this test's group
      for (const hook of afterEachHooks) {
        if (hookApplies(hook, entry.group)) {
          resolveSync(hook.fn());
        }
      }

      // Reset mocks between tests
      resetMocks();

      // Clear deadline
      __testMaxDrains = 0;

      _filePassed++;
      results.push({
        name: entry.name,
        status: 'pass',
        duration: Date.now() - start,
        file: entry.file,
      });
    } catch (e: any) {
      // Clear deadline before running afterEach on failure
      __testMaxDrains = 0;

      // Still run afterEach even on failure
      for (const hook of afterEachHooks) {
        if (hookApplies(hook, entry.group)) {
          try {
            resolveSync(hook.fn());
          } catch {}
        }
      }

      // Reset mocks between tests
      resetMocks();

      _fileFailed++;
      let errMsg = e?.stack ?? e?.message ?? String(e);
      // Parse stack trace for source file paths and suggest ht.mock() fixes.
      // esbuild names __esm init functions with the source path (e.g. "src/utils/string.ts").
      if (e?.stack) {
        const lines = (e.stack as string).split('\n');
        // Find first application source file in the stack (not harness, not native, not node_modules)
        for (const line of lines) {
          const match = line.match(/at\s+(?:([^\s(]+)\s+\()?(?:bundle\.js|[^)]+):(\d+)/);
          if (!match) continue;
          const fnName = match[1] || '';
          // Skip framework internals
          if (fnName.includes('harness') || fnName === 'anonymous' || fnName === '__init'
              || fnName === 'apply' || fnName === 'global' || fnName.startsWith('react')
              || fnName.startsWith('run') || fnName.startsWith('perform')
              || fnName.startsWith('work') || fnName.startsWith('flush')
              || fnName.startsWith('begin') || fnName.startsWith('update')
              || fnName.startsWith('reconcile') || fnName.startsWith('create')) continue;
          // If fnName looks like a file path (contains / or .tsx/.ts), suggest mocking it
          if (fnName.includes('/') && (fnName.includes('.ts') || fnName.includes('.js'))) {
            // Convert src path to alias path for the hint
            const srcPath = fnName.replace(/^(\.\.\/)*/, '').replace(/\/index\.(tsx?|jsx?)$/, '');
            errMsg += '\n\n  Hint: Module "' + srcPath + '" crashed during initialization.'
              + '\n  Consider adding: ht.mock(\'<alias>/' + srcPath + '\', () => ({ ... }))';
            break;
          }
          // If fnName is a function name (like formatLocalePrice), show it
          // and resolve module path from the shallow import map if available
          if (fnName && !fnName.includes('(') && fnName.length > 2) {
            const importMap = (globalThis as any).__HT_shallow_imports;
            const modPath = importMap && importMap[fnName];
            if (modPath) {
              // Collect all exports from the same module for a complete mock
              const siblings: string[] = [];
              for (const k in importMap) {
                if (importMap[k] === modPath && siblings.indexOf(k) === -1) siblings.push(k);
              }
              const mockBody = siblings.map(s => '  ' + s + ': () => {}').join(',\n');
              errMsg += '\n\n  Hint: "' + fnName + '" from "' + modPath + '" failed.'
                + '\n  Add this mock to your test file:\n\n'
                + "  ht.mock('" + modPath + "', () => ({\n" + mockBody + '\n  }));\n';
            } else {
              errMsg += '\n\n  Hint: Function "' + fnName + '" is undefined or calls undefined.'
                + '\n  Find which module exports it and add ht.mock() for that module.';
            }
            break;
          }
        }
      }
      _fileFailures.push({ name: entry.name, error: errMsg });
      results.push({
        name: entry.name,
        status: 'fail',
        error: errMsg,
        duration: Date.now() - start,
        file: entry.file,
      });
    }
  }

  // Flush last file
  _flushFileResult();

  // Clear progress line if in coverage mode
  if ((globalThis as any).__HT_coverage) {
    _print(`\r\x1b[K`);
  }

  // Run afterAll hooks
  for (const hook of afterAllHooks) {
    try {
      resolveSync(hook.fn());
    } catch {}
  }

  return results;
}

// Reset between watch cycles (persistent runtime)
function registerCrash(file: string, error: string): void {
  tests.push({
    name: `[CRASH] ${file}`,
    fn: () => { throw new Error(error); },
    options: {},
    file,
  });
}

function resetRegistry(): void {
  tests.length = 0;
  beforeEachHooks.length = 0;
  afterEachHooks.length = 0;
  beforeAllHooks.length = 0;
  afterAllHooks.length = 0;
  currentGroup = undefined;
  // Clear mock and spy state for clean watch reruns
  clearAllMocks();
  if ((globalThis as any).__HT_file_mocks) (globalThis as any).__HT_file_mocks = {};
  resetMockModulePatches();
}

// --- ht global: ht.mock(path, factory) + ht.mock.fetch / ht.mock.fetch.overwrite / etc. ---
// Available globally without import, like jest.mock().
const mock = mockModule as typeof mockModule & {
  fetch: typeof mockFetch & {
    overwrite: typeof mockFetchUse;
    reset: typeof mockFetchReset;
    clear: typeof mockFetchClear;
  };
};
mock.fetch = mockFetch as typeof mockFetch & {
  overwrite: typeof mockFetchUse;
  reset: typeof mockFetchReset;
  clear: typeof mockFetchClear;
};
mock.fetch.overwrite = mockFetchUse;
mock.fetch.reset = mockFetchReset;
mock.fetch.clear = mockFetchClear;

const shallow = (_componentPath: string) => {};
(globalThis as any).ht = { mock, shallow };

// Expose to the global scope for the harness entry
(globalThis as any).__HT = {
  test,
  expect,
  spy,
  spyOn,
  clearAllMocks,
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
  http,
  HttpResponse,
  render,
  fireEvent,
  flushAsync,
  registerCrash,
  resetRegistry,
  resetMockModulePatches,
  // Timer control
  useFakeTimers,
  useRealTimers,
  advanceTimersByTime,
  runAllTimers,
  getTimerCount,
  advanceTimersToNextTimer,
};

export { test, expect, spy, spyOn, clearAllMocks, group, beforeEach, afterEach, beforeAll, afterAll, renderHook, act, waitFor, render, fireEvent, useMock, http, HttpResponse, flushAsync, useFakeTimers, useRealTimers, advanceTimersByTime, runAllTimers, getTimerCount, advanceTimersToNextTimer };
