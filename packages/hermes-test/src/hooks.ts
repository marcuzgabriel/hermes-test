// renderHook, act, waitFor — React hook testing primitives
// Uses react-reconciler to run hooks in a minimal React tree.
// No dependency on react-test-renderer (deprecated in React 19).
//
// react-reconciler is NOT bundled with the harness — it's loaded at runtime
// from the user's node_modules via globalThis.__HT_Reconciler. This ensures
// the reconciler always matches the user's React version.

function getReact(): typeof import('react') {
  const R = (globalThis as any).__HT_React;
  if (!R) throw new Error('React not available. Make sure react is installed in your project.');
  return R;
}

function getReconcilerModule(): any {
  const R = (globalThis as any).__HT_Reconciler;
  if (!R) throw new Error('react-reconciler not available. Make sure it is installed (it ships with hermes-test).');
  return R;
}

function getReconcilerConstants(): any {
  return (globalThis as any).__HT_ReconcilerConstants || {};
}

// Based on mdjastrzebski/test-renderer — the universal-test-renderer for React 19
// https://github.com/mdjastrzebski/test-renderer
let currentUpdatePriority: number = 0;

const hostConfig = {
  supportsMutation: true,
  supportsPersistence: false,
  supportsHydration: false,
  supportsMicrotasks: true,
  isPrimaryRenderer: true,
  warnsIfNotActing: true,
  createInstance(type: string, props: any) { const { children: _c, ...rest } = props; return { type, props: rest, children: [] }; },
  createTextInstance(text: string) { return { type: '__TEXT__', props: {}, text, children: [] }; },
  appendInitialChild(p: any, c: any) { p.children.push(c); c._parent = p; },
  appendChild(p: any, c: any) { p.children.push(c); c._parent = p; },
  appendChildToContainer(p: any, c: any) { p.children.push(c); c._parent = p; },
  removeChild(p: any, c: any) { const i = p.children.indexOf(c); if (i !== -1) p.children.splice(i, 1); },
  removeChildFromContainer(p: any, c: any) { const i = p.children.indexOf(c); if (i !== -1) p.children.splice(i, 1); },
  insertBefore(p: any, c: any, b: any) { const i = p.children.indexOf(b); p.children.splice(i, 0, c); c._parent = p; },
  insertInContainerBefore(p: any, c: any, b: any) { const i = p.children.indexOf(b); p.children.splice(i, 0, c); c._parent = p; },
  commitUpdate(inst: any, _type: any, _oldProps: any, newProps: any) { const { children: _c, ...rest } = newProps; inst.props = rest; },
  commitTextUpdate(inst: any, _oldText: string, newText: string) { inst.text = newText; },
  commitMount() {},
  prepareForCommit() { return null; },
  resetAfterCommit() {},
  resetTextContent() {},
  finalizeInitialChildren() { return false; },
  shouldSetTextContent() { return false; },
  getRootHostContext() { return null; },
  getChildHostContext(ctx: any) { return ctx; },
  getPublicInstance(inst: any) { return inst; },
  prepareUpdate() { return {}; },
  clearContainer(c: any) { c.children = []; },
  scheduleTimeout: (globalThis as any).setTimeout || ((fn: any) => fn()),
  cancelTimeout: (globalThis as any).clearTimeout || (() => {}),
  noTimeout: -1,
  scheduleMicrotask: typeof queueMicrotask === 'function' ? queueMicrotask : (fn: any) => Promise.resolve().then(fn),
  getCurrentEventPriority() { return getReconcilerConstants().DefaultEventPriority ?? 0; },
  setCurrentUpdatePriority(priority: number) { currentUpdatePriority = priority; },
  getCurrentUpdatePriority() { return currentUpdatePriority; },
  resolveUpdatePriority() { return currentUpdatePriority || (getReconcilerConstants().DefaultEventPriority ?? 0); },
  shouldAttemptEagerTransition() { return false; },
  trackSchedulerEvent() {},
  resolveEventType() { return ''; },
  resolveEventTimeStamp() { return -1.1; },
  requestPostPaintCallback() {},
  maySuspendCommit() { return false; },
  preloadInstance() { return true; },
  startSuspendingCommit() {},
  suspendInstance() {},
  waitForCommitToBeReady() { return null; },
  NotPendingTransition: null,
  resetFormInstance() {},
  hideInstance() {},
  unhideInstance() {},
  hideTextInstance() {},
  unhideTextInstance() {},
  getInstanceFromNode() { return null; },
  prepareScopeUpdate() {},
  getInstanceFromScope() { return null; },
  detachDeletedInstance() {},
  beforeActiveInstanceBlur() {},
  afterActiveInstanceBlur() {},
  preparePortalMount() {},
};

export function createReconciler() {
  const Reconciler = getReconcilerModule();
  const create = typeof Reconciler === 'function' ? Reconciler : Reconciler.default;
  return create(hostConfig);
}

type HookResult<T> = {
  readonly result: { readonly current: T };
  readonly current: T;
  readonly history: ReadonlyArray<T>;
  readonly renderCount: number;
  rerender(props?: any): void;
  unmount(): void;
};

const drain = (globalThis as any).__HT_drain || (() => {});

function flush() {
  drain();
}

// React act() environment — same pattern as React Testing Library.
// true inside act() → React processes updates and can warn about missing act.
// false outside act() → React doesn't warn about async state updates.
(globalThis as any).IS_REACT_ACT_ENVIRONMENT = false;

export function act(fn: () => void | Promise<void>): void {
  const React = getReact();
  const reactAct = (React as any).act || (React as any).unstable_act;
  if (!reactAct) {
    fn();
    flush();
    return;
  }

  const prev = (globalThis as any).IS_REACT_ACT_ENVIRONMENT;
  (globalThis as any).IS_REACT_ACT_ENVIRONMENT = true;

  try {
    reactAct(() => {
      const result = fn();
      if (result && typeof (result as any).then === 'function') {
        let settled = false;
        let error: any;
        (result as Promise<void>).then(
          () => { settled = true; },
          (e: any) => { settled = true; error = e; }
        );
        drain();
        if (error) throw error;
      }
    });
    // Restore env BEFORE flush so async effects resolved by flush
    // don't trigger "not wrapped in act" warnings
    (globalThis as any).IS_REACT_ACT_ENVIRONMENT = prev;
    flush();
  } catch (error) {
    (globalThis as any).IS_REACT_ACT_ENVIRONMENT = prev;
    throw error;
  }
}

export function renderHook<T>(
  hookFn: (props?: any) => T,
  options?: { initialProps?: any; wrapper?: any }
): HookResult<T> {
  const history: T[] = [];
  let currentValue: T;

  const React = getReact();
  const reconciler = createReconciler();

  const container = { children: [] };
  const root = reconciler.createContainer(
    container,
    0, // LegacyRoot — effects fire synchronously in act()
    null,  // hydrationCallbacks
    false, // isStrictMode
    false, // concurrentUpdatesByDefaultOverride
    '',    // identifierPrefix
    (err: any) => { throw err; }, // onUncaughtError
    (err: any) => { throw err; }, // onCaughtError
    null,  // onRecoverableError
    () => {}, // onDefaultTransitionIndicator
  );

  function TestComponent({ hookProps }: { hookProps: any }) {
    const value = hookFn(hookProps);
    currentValue = value;
    history.push(value);
    return null;
  }

  function createTree(props?: any) {
    const testEl = React.createElement(TestComponent, { hookProps: props });
    if (options?.wrapper) {
      return React.createElement(options.wrapper, null, testEl);
    }
    return testEl;
  }

  act(() => {
    reconciler.updateContainer(createTree(options?.initialProps), root, null, null);
  });

  return {
    result: {
      get current() {
        return currentValue!;
      },
    },
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
        reconciler.updateContainer(createTree(props), root, null, null);
      });
    },
    unmount() {
      act(() => {
        reconciler.updateContainer(null, root, null, null);
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

  for (let attempt = 0; attempt < 100; attempt++) {
    act(() => { drain(); });
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
