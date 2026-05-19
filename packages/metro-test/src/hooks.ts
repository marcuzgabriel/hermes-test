// renderHook, act, waitFor — React hook testing primitives
// Uses react-test-renderer to run hooks in a lightweight React tree
//
// React and ReactTestRenderer are NOT bundled with the harness — they come from
// the user's project via esbuild. The harness expects them on globalThis.

function getReact(): typeof import('react') {
  const R = (globalThis as any).__React;
  if (!R) throw new Error('React not available. Make sure react is installed in your project.');
  return R;
}

function getTestRenderer(): typeof import('react-test-renderer') {
  const TR = (globalThis as any).__ReactTestRenderer;
  if (!TR) throw new Error('react-test-renderer not available. Install it: bun add -d react-test-renderer');
  return TR;
}

type HookResult<T> = {
  readonly current: T;
  readonly history: ReadonlyArray<T>;
  readonly renderCount: number;
  rerender(props?: any): void;
  unmount(): void;
};

const drain = (globalThis as any).__drainMicrotasks || (() => {});

// Flush all pending async work until the queue is stable.
// Each drain() resolves all current microtasks, but resolved promises may
// trigger new work (setState → effect → fetch → more promises). We keep
// draining until no new work is produced.
function flush() {
  drain();
}

export function act(fn: () => void | Promise<void>): void {
  const TR = getTestRenderer();
  TR.act(() => {
    const result = fn();
    if (result && typeof (result as any).then === 'function') {
      let settled = false;
      let error: any;
      (result as Promise<void>).then(
        () => { settled = true; },
        (e: any) => { settled = true; error = e; }
      );
      // Drain until the promise settles (each drain resolves all current microtasks,
      // but resolved work may queue more — keep going until settled)
      for (let i = 0; i < 50 && !settled; i++) {
        drain();
      }
      if (error) throw error;
    }
  });
  flush();
}

export function renderHook<T>(
  hookFn: (props?: any) => T,
  options?: { initialProps?: any }
): HookResult<T> {
  const history: T[] = [];
  let currentValue: T;
  let renderer: TestRenderer.ReactTestRenderer;

  const React = getReact();
  const TR = getTestRenderer();

  function TestComponent({ hookProps }: { hookProps: any }) {
    const value = hookFn(hookProps);
    currentValue = value;
    history.push(value);
    return null;
  }

  act(() => {
    renderer = TR.create(
      React.createElement(TestComponent, { hookProps: options?.initialProps })
    );
  });

  return {
    get current() {
      return currentValue!;
    },
    get history() {
      return history;
    },
    get renderCount() {
      return history.length;
    },
    rerender(props?: any) {
      act(() => {
        renderer!.update(
          React.createElement(TestComponent, { hookProps: props })
        );
      });
    },
    unmount() {
      act(() => {
        renderer!.unmount();
      });
    },
  };
}

export function waitFor<T>(
  predicate: () => T | false | null | undefined,
  options?: { timeout?: number; interval?: number }
): T {
  const timeout = options?.timeout ?? 1000;
  const start = Date.now();
  const TR = getTestRenderer();

  for (let attempt = 0; attempt < 100; attempt++) {
    TR.act(() => { drain(); });
    drain();

    const result = predicate();
    if (result !== false && result !== null && result !== undefined) {
      return result;
    }

    if (Date.now() - start >= timeout) {
      throw new Error(`waitFor timed out after ${timeout}ms`);
    }
  }

  throw new Error(`waitFor exceeded max attempts`);
}
