import type { Spy } from './spy';

// --- Snapshot support ---

const _readFile = (globalThis as any).__HT_readFile || (() => null);
const _writeFile = (globalThis as any).__HT_writeFile || (() => false);

// Snapshot state — set by the harness before each test runs
let _snapshotFile = '';      // path to .snap file
let _snapshotTestName = '';  // current test name (used as snapshot key)
let _snapshotCounter = 0;    // counter for multiple snapshots in one test
let _updateSnapshots = false;

// Cache of loaded snapshot files: path → { key → serialized value }
const _snapshotCache: Record<string, Record<string, string>> = {};

function _setSnapshotContext(file: string, testName: string, update: boolean) {
  _snapshotFile = file;
  _snapshotTestName = testName;
  _snapshotCounter = 0;
  _updateSnapshots = update;
}

function _serializeSnapshot(value: any): string {
  return JSON.stringify(value, (_key, val) => {
    if (typeof val === 'function') return '[Function]';
    return val;
  }, 2);
}

function _loadSnapshots(path: string): Record<string, string> {
  if (_snapshotCache[path]) return _snapshotCache[path];
  const content = _readFile(path);
  if (content) {
    try {
      _snapshotCache[path] = JSON.parse(content);
    } catch {
      _snapshotCache[path] = {};
    }
  } else {
    _snapshotCache[path] = {};
  }
  return _snapshotCache[path];
}

function _saveSnapshots(path: string, data: Record<string, string>) {
  _snapshotCache[path] = data;
  _writeFile(path, JSON.stringify(data, null, 2) + '\n');
}

function _matchSnapshot(actual: any): void {
  _snapshotCounter++;
  const key = _snapshotTestName + (_snapshotCounter > 1 ? ` ${_snapshotCounter}` : '');
  const serialized = _serializeSnapshot(actual);

  if (!_snapshotFile) {
    throw new Error('toMatchSnapshot: no snapshot file configured. Is __currentTestFile set?');
  }

  const snapshots = _loadSnapshots(_snapshotFile);

  if (_updateSnapshots || !(key in snapshots)) {
    // Write new or updated snapshot
    snapshots[key] = serialized;
    _saveSnapshots(_snapshotFile, snapshots);
    return;
  }

  // Compare
  const expected = snapshots[key];
  if (serialized !== expected) {
    throw new Error(
      `Snapshot mismatch for "${key}":\n` +
      `Expected:\n${expected}\n\nReceived:\n${serialized}\n\n` +
      `Run with --update-snapshots to update.`
    );
  }
}

function deepEqual(a: any, b: any): boolean {
  // Support asymmetric matchers (expect.anything(), expect.any(), expect.objectContaining())
  if (b != null && typeof b === 'object' && b.__htMatcher && typeof b.matches === 'function') return b.matches(a);
  if (a != null && typeof a === 'object' && a.__htMatcher && typeof a.matches === 'function') return a.matches(b);

  if (a === b) return true;
  if (a == null || b == null) return a === b;
  if (typeof a !== typeof b) return false;

  if (Array.isArray(a)) {
    if (!Array.isArray(b) || a.length !== b.length) return false;
    return a.every((v, i) => deepEqual(v, b[i]));
  }

  if (a instanceof Date && b instanceof Date) {
    return a.getTime() === b.getTime();
  }

  if (typeof a === 'object') {
    const keysA = Object.keys(a).filter((k) => a[k] !== undefined);
    const keysB = Object.keys(b).filter((k) => b[k] !== undefined);
    if (keysA.length !== keysB.length) return false;
    return keysA.every((k) => deepEqual(a[k], b[k]));
  }

  return false;
}

function formatValue(v: any): string {
  if (v === undefined) return 'undefined';
  if (v === null) return 'null';
  if (typeof v === 'string') return JSON.stringify(v);
  if (typeof v === 'function') return '[Function]';
  try {
    return JSON.stringify(v);
  } catch {
    return String(v);
  }
}

// Extract text content from an HTNode tree (for element matchers)
function _getTextContent(node: any): string {
  if (!node || typeof node !== 'object') return '';
  if (node.type === '__TEXT__') return node.text || '';
  if (!node.children) return '';
  return node.children.map(_getTextContent).join('');
}

function createAssertion(actual: any, negated: boolean): any {
  function assert(condition: boolean, message: string) {
    const pass = negated ? !condition : condition;
    if (!pass) {
      let hint = '';
      if (actual === undefined && !negated) {
        hint = '\n    Hint: Received is undefined. This usually means the module needs to be mocked with ht.mock().';
      }
      throw new Error(message + hint);
    }
  }

  const assertion: any = {
    toBe(expected: any) {
      assert(
        actual === expected,
        negated
          ? `expect(received).not.toBe(expected)\n\n    Expected: not ${formatValue(expected)}\n    Received: ${formatValue(actual)}`
          : `expect(received).toBe(expected)\n\n    Expected: ${formatValue(expected)}\n    Received: ${formatValue(actual)}`
      );
    },

    toEqual(expected: any) {
      assert(
        deepEqual(actual, expected),
        negated
          ? `expect(received).not.toEqual(expected)\n\n    Expected: not ${formatValue(expected)}\n    Received: ${formatValue(actual)}`
          : `expect(received).toEqual(expected)\n\n    Expected: ${formatValue(expected)}\n    Received: ${formatValue(actual)}`
      );
    },

    toBeDefined() {
      assert(
        actual !== undefined,
        negated
          ? `expect(received).not.toBeDefined()\n\n    Received: ${formatValue(actual)}`
          : `expect(received).toBeDefined()\n\n    Received: undefined`
      );
    },

    toBeUndefined() {
      assert(
        actual === undefined,
        negated
          ? `expect(received).not.toBeUndefined()\n\n    Received: ${formatValue(actual)}`
          : `expect(received).toBeUndefined()\n\n    Received: ${formatValue(actual)}`
      );
    },

    toBeNull() {
      assert(
        actual === null,
        negated
          ? `expect(received).not.toBeNull()\n\n    Received: ${formatValue(actual)}`
          : `expect(received).toBeNull()\n\n    Received: ${formatValue(actual)}`
      );
    },

    toHaveLength(expected: number) {
      const len = actual?.length;
      assert(
        len === expected,
        negated
          ? `expect(received).not.toHaveLength(expected)\n\n    Expected: not ${expected}\n    Received length: ${len}`
          : `expect(received).toHaveLength(expected)\n\n    Expected: ${expected}\n    Received length: ${len}`
      );
    },

    toBeInstanceOf(expected: any) {
      assert(
        actual instanceof expected,
        negated
          ? `expect(received).not.toBeInstanceOf(expected)\n\n    Expected: not ${expected?.name ?? expected}`
          : `expect(received).toBeInstanceOf(expected)\n\n    Expected: ${expected?.name ?? expected}\n    Received: ${formatValue(actual)}`
      );
    },

    toBeTruthy() {
      assert(
        !!actual,
        negated
          ? `expect(received).not.toBeTruthy()\n\n    Received: ${formatValue(actual)}`
          : `expect(received).toBeTruthy()\n\n    Received: ${formatValue(actual)}`
      );
    },

    toBeFalsy() {
      assert(
        !actual,
        negated
          ? `expect(received).not.toBeFalsy()\n\n    Received: ${formatValue(actual)}`
          : `expect(received).toBeFalsy()\n\n    Received: ${formatValue(actual)}`
      );
    },

    toBeGreaterThan(n: number) {
      assert(
        actual > n,
        negated
          ? `Expected ${actual} not to be greater than ${n}`
          : `Expected ${actual} to be greater than ${n}`
      );
    },

    toBeLessThan(n: number) {
      assert(
        actual < n,
        negated
          ? `Expected ${actual} not to be less than ${n}`
          : `Expected ${actual} to be less than ${n}`
      );
    },

    toContain(item: any) {
      const contains = Array.isArray(actual)
        ? actual.some((v: any) => deepEqual(v, item))
        : typeof actual === 'string'
          ? actual.includes(item)
          : false;
      assert(
        contains,
        negated
          ? `Expected ${formatValue(actual)} not to contain ${formatValue(item)}`
          : `Expected ${formatValue(actual)} to contain ${formatValue(item)}`
      );
    },

    toContainEqual(item: any) {
      const contains = Array.isArray(actual) && actual.some((v: any) => deepEqual(v, item));
      assert(
        contains,
        negated
          ? `Expected array not to contain equal ${formatValue(item)}`
          : `Expected array to contain equal ${formatValue(item)}, got ${formatValue(actual)}`
      );
    },

    toBeCloseTo(expected: number, precision: number = 2) {
      const pass = Math.abs(actual - expected) < Math.pow(10, -precision) / 2;
      assert(
        pass,
        negated
          ? `Expected ${actual} not to be close to ${expected}`
          : `Expected ${actual} to be close to ${expected} (precision ${precision})`
      );
    },

    toMatch(pattern: RegExp | string) {
      const regex = typeof pattern === 'string' ? new RegExp(pattern) : pattern;
      assert(
        regex.test(String(actual)),
        negated
          ? `Expected ${formatValue(actual)} not to match ${pattern}`
          : `Expected ${formatValue(actual)} to match ${pattern}`
      );
    },

    toThrow(message?: string | RegExp) {
      let threw = false;
      let error: any;
      try {
        actual();
      } catch (e) {
        threw = true;
        error = e;
      }

      if (message === undefined) {
        assert(
          threw,
          negated
            ? `Expected function not to throw, but it threw ${formatValue(error)}`
            : `Expected function to throw, but it did not`
        );
      } else {
        const errMsg = error?.message ?? String(error ?? '');
        const matches =
          typeof message === 'string'
            ? errMsg.includes(message)
            : message.test(errMsg);
        assert(
          threw && matches,
          negated
            ? `Expected function not to throw matching ${message}`
            : threw
              ? `Expected thrown error to match ${message}, got "${errMsg}"`
              : `Expected function to throw, but it did not`
        );
      }
    },

    // Spy assertions
    wasCalled() {
      assert(
        (actual as Spy).callCount > 0,
        negated
          ? `Expected spy not to have been called, but it was called ${(actual as Spy).callCount} times`
          : `Expected spy to have been called, but it was never called`
      );
    },

    wasCalledOnce() {
      assert(
        (actual as Spy).callCount === 1,
        negated
          ? `Expected spy not to have been called once, but it was`
          : `Expected spy to have been called once, but it was called ${(actual as Spy).callCount} times`
      );
    },

    wasCalledTimes(n: number) {
      assert(
        (actual as Spy).callCount === n,
        negated
          ? `Expected spy not to have been called ${n} times`
          : `Expected spy to have been called ${n} times, but it was called ${(actual as Spy).callCount} times`
      );
    },

    wasCalledWith(...args: any[]) {
      const s = actual as Spy;
      const match = s.calls.some((call: any[]) => deepEqual(call, args));
      assert(
        match,
        negated
          ? `Expected spy not to have been called with ${formatValue(args)}`
          : `Expected spy to have been called with ${formatValue(args)}, calls: ${formatValue(s.calls)}`
      );
    },

    wasLastCalledWith(...args: any[]) {
      const s = actual as Spy;
      const lastCall = s.calls[s.calls.length - 1];
      assert(
        deepEqual(lastCall, args),
        negated
          ? `Expected last call not to be ${formatValue(args)}`
          : `Expected last call to be ${formatValue(args)}, got ${formatValue(lastCall)}`
      );
    },

    wasNeverCalled() {
      assert(
        (actual as Spy).callCount === 0,
        negated
          ? `Expected spy to have been called, but it was never called`
          : `Expected spy to never have been called, but it was called ${(actual as Spy).callCount} times`
      );
    },

    // Jest-compatible aliases
    toHaveBeenCalled() { return this.wasCalled(); },
    toHaveBeenCalledTimes(n: number) { return this.wasCalledTimes(n); },
    toHaveBeenCalledWith(...args: any[]) { return this.wasCalledWith(...args); },
    toHaveBeenLastCalledWith(...args: any[]) { return this.wasLastCalledWith(...args); },

    // --- Element matchers (for render() HTNode results) ---

    toBeRendered() {
      const el = actual;
      const isNode = el && typeof el === 'object' && 'type' in el && 'children' in el;
      assert(
        isNode && el.type !== '__ROOT__',
        negated
          ? `Expected element not to be rendered`
          : `Expected element to be rendered, got ${formatValue(el)}`
      );
    },

    toHaveTextContent(expected: string | RegExp) {
      const text = _getTextContent(actual);
      const matches = typeof expected === 'string'
        ? text === expected || text.includes(expected)
        : expected.test(text);
      assert(
        matches,
        negated
          ? `Expected element not to have text content "${expected}", but it does`
          : `Expected text content "${expected}", got "${text}"`
      );
    },

    toContainElement(child: any) {
      function _contains(node: any, target: any): boolean {
        if (node === target) return true;
        if (!node?.children) return false;
        return node.children.some((c: any) => _contains(c, target));
      }
      assert(
        _contains(actual, child),
        negated
          ? `Expected element not to contain the given child`
          : `Expected element to contain the given child`
      );
    },

    toBeEmpty() {
      const empty = !actual?.children || actual.children.length === 0;
      assert(
        empty,
        negated
          ? `Expected element not to be empty, but it has no children`
          : `Expected element to be empty, but it has ${actual?.children?.length} children`
      );
    },

    toHaveDisplayValue(expected: string | RegExp) {
      const value = actual?.props?.value ?? '';
      const matches = typeof expected === 'string' ? value === expected : expected.test(value);
      assert(
        matches,
        negated
          ? `Expected display value not to be "${expected}"`
          : `Expected display value "${expected}", got "${value}"`
      );
    },

    toHaveProp(name: string, value?: any) {
      const hasProp = actual?.props && name in actual.props;
      if (value === undefined) {
        assert(
          hasProp,
          negated
            ? `Expected element not to have prop "${name}"`
            : `Expected element to have prop "${name}"`
        );
      } else {
        const propVal = actual?.props?.[name];
        assert(
          hasProp && deepEqual(propVal, value),
          negated
            ? `Expected prop "${name}" not to be ${formatValue(value)}`
            : `Expected prop "${name}" to be ${formatValue(value)}, got ${formatValue(propVal)}`
        );
      }
    },

    toHaveStyle(expected: Record<string, any>) {
      const style = actual?.props?.style || {};
      // RN styles can be arrays — flatten
      const flat: Record<string, any> = {};
      const styles = Array.isArray(style) ? style : [style];
      for (const s of styles) {
        if (s && typeof s === 'object') Object.assign(flat, s);
      }
      const allMatch = Object.keys(expected).every((k) => deepEqual(flat[k], expected[k]));
      const mismatches = Object.keys(expected)
        .filter((k) => !deepEqual(flat[k], expected[k]))
        .map((k) => `${k}: expected ${formatValue(expected[k])}, got ${formatValue(flat[k])}`);
      assert(
        allMatch,
        negated
          ? `Expected element not to have styles ${formatValue(expected)}`
          : `Style mismatch: ${mismatches.join('; ')}`
      );
    },

    toBeEnabled() {
      const disabled = actual?.props?.disabled === true
        || actual?.props?.editable === false
        || actual?.props?.accessibilityState?.disabled === true
        || actual?.props?.['aria-disabled'] === true;
      assert(
        !disabled,
        negated
          ? `Expected element to be disabled, but it is enabled`
          : `Expected element to be enabled, but it is disabled`
      );
    },

    toBeDisabled() {
      const disabled = actual?.props?.disabled === true
        || actual?.props?.editable === false
        || actual?.props?.accessibilityState?.disabled === true
        || actual?.props?.['aria-disabled'] === true;
      assert(
        disabled,
        negated
          ? `Expected element not to be disabled, but it is`
          : `Expected element to be disabled, but it is enabled`
      );
    },

    toBeVisible() {
      const style = actual?.props?.style || {};
      const styles = Array.isArray(style) ? style : [style];
      const flat: Record<string, any> = {};
      for (const s of styles) {
        if (s && typeof s === 'object') Object.assign(flat, s);
      }
      const hidden = flat.display === 'none' || flat.opacity === 0
        || actual?.props?.accessibilityElementsHidden === true
        || actual?.props?.importantForAccessibility === 'no-hide-descendants';
      assert(
        !hidden,
        negated
          ? `Expected element not to be visible`
          : `Expected element to be visible, but it is hidden`
      );
    },

    // --- Snapshot matcher ---

    toMatchSnapshot() {
      _matchSnapshot(actual);
    },
  };

  if (!negated) {
    assertion.not = createAssertion(actual, true);
  }

  return assertion;
}

// --- Asymmetric matchers ---
// Asymmetric matchers — plain objects with __htMatcher flag
function makeMatcher(matchFn: (v: any) => boolean) {
  return { __htMatcher: true, matches: matchFn };
}

export function expect(actual: any): any {
  const base = createAssertion(actual, false);

  // resolves / rejects for promise assertions
  base.resolves = {
    toBeUndefined: async () => { const r = await actual; if (r !== undefined) throw new Error(`Expected undefined, got ${formatValue(r)}`); },
    toBe: async (expected: any) => { const r = await actual; if (r !== expected) throw new Error(`Expected ${formatValue(expected)}, got ${formatValue(r)}`); },
    toEqual: async (expected: any) => { const r = await actual; if (!deepEqual(r, expected)) throw new Error(`Expected deep equal to ${formatValue(expected)}, got ${formatValue(r)}`); },
    toBeDefined: async () => { const r = await actual; if (r === undefined) throw new Error(`Expected value to be defined`); },
    toBeTruthy: async () => { const r = await actual; if (!r) throw new Error(`Expected truthy, got ${formatValue(r)}`); },
    toBeFalsy: async () => { const r = await actual; if (r) throw new Error(`Expected falsy, got ${formatValue(r)}`); },
    toBeNull: async () => { const r = await actual; if (r !== null) throw new Error(`Expected null, got ${formatValue(r)}`); },
  };

  base.rejects = {
    toThrow: async (msg?: string | RegExp) => {
      try { await actual; throw new Error('Expected promise to reject'); }
      catch (e: any) { if (msg) { const m = e?.message ?? String(e); const ok = typeof msg === 'string' ? m.includes(msg) : msg.test(m); if (!ok) throw new Error(`Expected rejection matching ${msg}, got "${m}"`); } }
    },
  };

  return base;
}

// Static matchers on expect
expect.anything = () => makeMatcher((v) => v !== null && v !== undefined);
expect.any = (ctor: any) => makeMatcher((v) => {
  if (ctor === String) return typeof v === 'string';
  if (ctor === Number) return typeof v === 'number';
  if (ctor === Boolean) return typeof v === 'boolean';
  if (ctor === Function) return typeof v === 'function';
  return v instanceof ctor;
});
expect.objectContaining = (subset: Record<string, any>) => makeMatcher((v) => {
  if (typeof v !== 'object' || v === null) return false;
  return Object.keys(subset).every((k) => deepEqual(v[k], subset[k]));
});
expect.arrayContaining = (expected: any[]) => makeMatcher((v) => {
  if (!Array.isArray(v)) return false;
  return expected.every((e) => v.some((item: any) => deepEqual(item, e)));
});
expect.stringContaining = (substr: string) => makeMatcher((v) => typeof v === 'string' && v.includes(substr));
expect.stringMatching = (pattern: RegExp | string) => makeMatcher((v) => {
  const re = typeof pattern === 'string' ? new RegExp(pattern) : pattern;
  return typeof v === 'string' && re.test(v);
});

export { _setSnapshotContext };
