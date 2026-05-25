import type { Spy } from './spy';

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

function createAssertion(actual: any, negated: boolean): any {
  function assert(condition: boolean, message: string) {
    const pass = negated ? !condition : condition;
    if (!pass) {
      throw new Error(message);
    }
  }

  const assertion: any = {
    toBe(expected: any) {
      assert(
        actual === expected,
        negated
          ? `Expected ${formatValue(actual)} not to be ${formatValue(expected)}`
          : `Expected ${formatValue(expected)}, got ${formatValue(actual)}`
      );
    },

    toEqual(expected: any) {
      assert(
        deepEqual(actual, expected),
        negated
          ? `Expected ${formatValue(actual)} not to deeply equal ${formatValue(expected)}`
          : `Expected deep equal to ${formatValue(expected)}, got ${formatValue(actual)}`
      );
    },

    toBeDefined() {
      assert(
        actual !== undefined,
        negated
          ? `Expected value to be undefined, got ${formatValue(actual)}`
          : `Expected value to be defined, got undefined`
      );
    },

    toBeUndefined() {
      assert(
        actual === undefined,
        negated
          ? `Expected value not to be undefined`
          : `Expected undefined, got ${formatValue(actual)}`
      );
    },

    toBeNull() {
      assert(
        actual === null,
        negated
          ? `Expected value not to be null`
          : `Expected null, got ${formatValue(actual)}`
      );
    },

    toHaveLength(expected: number) {
      const len = actual?.length;
      assert(
        len === expected,
        negated
          ? `Expected length not to be ${expected}, but it was`
          : `Expected length ${expected}, got ${len}`
      );
    },

    toBeInstanceOf(expected: any) {
      assert(
        actual instanceof expected,
        negated
          ? `Expected ${formatValue(actual)} not to be instance of ${expected?.name ?? expected}`
          : `Expected instance of ${expected?.name ?? expected}, got ${formatValue(actual)}`
      );
    },

    toBeTruthy() {
      assert(
        !!actual,
        negated
          ? `Expected ${formatValue(actual)} to be falsy`
          : `Expected truthy value, got ${formatValue(actual)}`
      );
    },

    toBeFalsy() {
      assert(
        !actual,
        negated
          ? `Expected ${formatValue(actual)} to be truthy`
          : `Expected falsy value, got ${formatValue(actual)}`
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
