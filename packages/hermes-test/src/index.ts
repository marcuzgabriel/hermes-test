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

export type ApiStoreConfig = {
  slices: { reducerPath: string; reducer: any; middleware: any }[];
  defaultAppState?: Record<string, any>;
};

export function createApiStoreFactory(config: ApiStoreConfig) {
  return function withApiStore(initialState: Record<string, any> = {}) {
    const { configureStore } = require('@reduxjs/toolkit');
    const callerApp = initialState.app ?? {};
    const defaults = config.defaultAppState ?? {};
    const mergedApp = { ...defaults, ...callerApp };
    for (const key of Object.keys(defaults)) {
      if (typeof defaults[key] === 'object' && defaults[key] !== null) {
        mergedApp[key] = { ...defaults[key], ...(callerApp[key] ?? {}) };
      }
    }

    const reducers: Record<string, any> = {};
    for (const key of Object.keys(initialState)) {
      if (key === 'app') continue;
      reducers[key] = _withTestActions((s: any = initialState[key]) => s);
    }
    reducers['app'] = _withTestActions((s: any = mergedApp) => s);
    for (const slice of config.slices) {
      reducers[slice.reducerPath] = slice.reducer;
    }

    let mw = (gdm: any) => {
      let chain = gdm({ serializableCheck: false, immutableCheck: false });
      for (const slice of config.slices) {
        chain = chain.concat(slice.middleware);
      }
      return chain;
    };

    return _makeCtx(configureStore({
      reducer: reducers,
      preloadedState: { ...initialState, app: mergedApp },
      middleware: mw,
    }));
  };
}

export const test = ht.test;
export const group = ht.group;
export const expect = ht.expect;
export const spy = ht.spy;
export const spyOn = ht.spyOn;
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
export const flushAsync = ht.flushAsync;
export const useFakeTimers = ht.useFakeTimers;
export const useRealTimers = ht.useRealTimers;
export const advanceTimersByTime = ht.advanceTimersByTime;
export const runAllTimers = ht.runAllTimers;
export const getTimerCount = ht.getTimerCount;
export const advanceTimersToNextTimer = ht.advanceTimersToNextTimer;
