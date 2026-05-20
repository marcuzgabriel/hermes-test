// renderHook, act, waitFor — React hook testing primitives
// Uses react-reconciler to run hooks in a minimal React tree.
// No dependency on react-test-renderer (deprecated in React 19).
//
// React and ReactReconciler are NOT bundled with the harness — they come from
// the user's project via esbuild. The harness expects them on globalThis.

function getReact(): typeof import('react') {
  const R = (globalThis as any).__React;
  if (!R) throw new Error('React not available. Make sure react is installed in your project.');
  return R;
}

import Reconciler from 'react-reconciler';
import { DefaultEventPriority, NoEventPriority } from 'react-reconciler/constants';

// Based on mdjastrzebski/test-renderer — the universal-test-renderer for React 19
// https://github.com/mdjastrzebski/test-renderer
let currentUpdatePriority: number = NoEventPriority;

const hostConfig = {
  supportsMutation: true,
  supportsPersistence: false,
  supportsHydration: false,
  supportsMicrotasks: true,
  isPrimaryRenderer: true,
  warnsIfNotActing: true,
  createInstance() { return { children: [] }; },
  createTextInstance() { return {}; },
  appendInitialChild(p: any, c: any) { p.children.push(c); },
  appendChild(p: any, c: any) { p.children.push(c); },
  appendChildToContainer(p: any, c: any) { p.children.push(c); },
  removeChild(p: any, c: any) { const i = p.children.indexOf(c); if (i !== -1) p.children.splice(i, 1); },
  removeChildFromContainer(p: any, c: any) { const i = p.children.indexOf(c); if (i !== -1) p.children.splice(i, 1); },
  insertBefore(p: any, c: any, b: any) { const i = p.children.indexOf(b); p.children.splice(i, 0, c); },
  insertInContainerBefore(p: any, c: any, b: any) { const i = p.children.indexOf(b); p.children.splice(i, 0, c); },
  commitUpdate() {},
  commitTextUpdate() {},
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
  getCurrentEventPriority() { return DefaultEventPriority; },
  setCurrentUpdatePriority(priority: number) { currentUpdatePriority = priority; },
  getCurrentUpdatePriority() { return currentUpdatePriority; },
  resolveUpdatePriority() { return currentUpdatePriority || DefaultEventPriority; },
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

function createReconciler() {
  const create = typeof Reconciler === 'function' ? Reconciler : (Reconciler as any).default;
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

const drain = (globalThis as any).__drainMicrotasks || (() => {});

function flush() {
  drain();
}

// Enable React.act() support
(globalThis as any).IS_REACT_ACT_ENVIRONMENT = true;

export function act(fn: () => void | Promise<void>): void {
  const React = getReact();
  const reactAct = (React as any).act || (React as any).unstable_act;
  if (!reactAct) {
    fn();
    flush();
    return;
  }

  reactAct(() => {
    const result = fn();
    if (result && typeof (result as any).then === 'function') {
      let settled = false;
      let error: any;
      (result as Promise<void>).then(
        () => { settled = true; },
        (e: any) => { settled = true; error = e; }
      );
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
