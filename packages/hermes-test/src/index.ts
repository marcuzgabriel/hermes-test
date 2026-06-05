// Thin re-export from the hermes-test harness runtime.
// The harness is eval'd before the user bundle, so globalThis.__HT is always available.
// Users import from 'hermes-test' instead of accessing globalThis directly.

const ht = (globalThis as any).__HT;

// --- Redux test store factories ---
// withStore: quick identity-reducer store for any state shape
// withApiStore: configurable RTK Query store factory

function _withTestActions(reducer: (state: any, action: any) => any) {
  return (state: any, action: any) => {
    if (action.type === '__SET_STATE__') return action.payload;
    if (action.type === '__PATCH__') return { ...state, ...action.payload };
    return reducer(state, action);
  };
}

function _makeCtx(store: any) {
  const React = require('react');
  const { Provider } = require('react-redux');

  const wrapper = ({ children }: { children: any }) =>
    React.createElement(Provider, { store } as any, children);

  return {
    store,
    wrapper,
    dispatch: store.dispatch.bind(store),
    getState: store.getState.bind(store),
    setState(state: Record<string, any>) { store.dispatch({ type: '__SET_STATE__', payload: state }); },
    patchState(partial: Record<string, any>) { store.dispatch({ type: '__PATCH__', payload: partial }); },
    renderHookWithReduxStore<T>(hookFn: (props?: any) => T, options?: { initialProps?: any }) {
      return ht.renderHook(hookFn, { ...options, wrapper });
    },
  };
}

export function withStore(initialState: Record<string, any> = {}) {
  const { configureStore } = require('@reduxjs/toolkit');
  return _makeCtx(configureStore({
    reducer: _withTestActions((s: any = initialState) => s),
    preloadedState: initialState,
    middleware: (gdm: any) => gdm({ serializableCheck: false, immutableCheck: false }),
  }));
}

export function withAppReducer(
  reducer: (state: any, action: any) => any,
  preloadedState?: Record<string, any>,
) {
  const { configureStore } = require('@reduxjs/toolkit');
  return _makeCtx(configureStore({
    reducer: _withTestActions(reducer),
    preloadedState,
    middleware: (gdm: any) => gdm({ serializableCheck: false, immutableCheck: false }),
  }));
}

interface RtkQueryApi {
  reducer: any;
  middleware: any;
  reducerPath: string;
}

interface SetupApiStoreOptions {
  middleware?: {
    prepend?: any[];
    concat?: any[];
  };
  preloadedState?: Record<string, any>;
}

export function setupApiStore(
  apis: RtkQueryApi[],
  extraReducers?: Record<string, any>,
  options?: SetupApiStoreOptions,
) {
  const { configureStore } = require('@reduxjs/toolkit');

  const reducerMap: Record<string, any> = {};
  for (const api of apis) {
    reducerMap[api.reducerPath] = api.reducer;
  }
  if (extraReducers) Object.assign(reducerMap, extraReducers);

  const store = configureStore({
    reducer: reducerMap,
    preloadedState: options?.preloadedState,
    middleware: (gdm: any) => {
      let chain = gdm({ serializableCheck: false, immutableCheck: false });
      for (const a of apis) chain = chain.concat(a.middleware);
      for (const mw of (options?.middleware?.concat ?? [])) chain = chain.concat(mw);
      for (const mw of (options?.middleware?.prepend ?? [])) chain = chain.prepend(mw);
      return chain;
    },
  });

  return { ..._makeCtx(store), apis };
}

export const test = ht.test;
export const group = ht.group;
export const expect = ht.expect;
export const spy = ht.spy;
export const spyOn = ht.spyOn;
export const clearAllMocks = ht.clearAllMocks;
export const beforeEach = ht.beforeEach;
export const afterEach = ht.afterEach;
export const beforeAll = ht.beforeAll;
export const afterAll = ht.afterAll;
export const renderHook = ht.renderHook;
export const act = ht.act;
export const waitFor = ht.waitFor;
export const mockModule = ht.mockModule;
export const useMock = ht.useMock;
export const mockFetch = ht.mockFetch;
export const mockFetchUse = ht.mockFetchUse;
export const mockFetchReset = ht.mockFetchReset;
export const mockFetchClear = ht.mockFetchClear;
export const http = ht.http;
export const HttpResponse = ht.HttpResponse;
export const render = ht.render;
export const fireEvent = ht.fireEvent;
export const flushAsync = ht.flushAsync;
export const useFakeTimers = ht.useFakeTimers;
export const useRealTimers = ht.useRealTimers;
export const advanceTimersByTime = ht.advanceTimersByTime;
export const runAllTimers = ht.runAllTimers;
export const getTimerCount = ht.getTimerCount;
export const advanceTimersToNextTimer = ht.advanceTimersToNextTimer;
