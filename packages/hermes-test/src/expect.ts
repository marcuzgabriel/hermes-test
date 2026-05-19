import type { Spy } from './spy';

function deepEqual(a: any, b: any): boolean {
  if (a === b) return true;
  if (a == null || b == null) return a === b;
  if (typeof a !== typeof b) return false;

  if (Array.isArray(a)) {
    if (!Array.isArray(b) || a.length !== b.length) return false;
    return a.every((v, i) => deepEqual(v, b[i]));
  }

  if (typeof a === 'object') {
    const keysA = Object.keys(a);
    const keysB = Object.keys(b);
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
  };

  if (!negated) {
    assertion.not = createAssertion(actual, true);
  }

  return assertion;
}

export function expect(actual: any): any {
  return createAssertion(actual, false);
}
