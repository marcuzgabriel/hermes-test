// hermes-test/store — Redux test store factories
//
// withStore: quick identity-reducer store for any state shape
// withAppReducer: real app reducer with patchState + real actions
//
// Usage:
//   import { withStore } from 'hermes-test/store';
//   const ctx = withStore({ user: { name: 'Test' } });
//   const { current } = ctx.renderHookWithReduxStore(() => useMyHook());
//   ctx.patchState({ user: { name: 'Updated' } });

import React from 'react';
import { Provider } from 'react-redux';
import { configureStore, type Middleware } from '@reduxjs/toolkit';

type StoreContext = {
  store: any;
  wrapper: any;
  dispatch: (action: any) => any;
  getState: () => any;
  setState: (state: Record<string, any>) => void;
  patchState: (partial: Record<string, any>) => void;
  renderHookWithReduxStore: <T>(hookFn: (props?: any) => T, options?: { initialProps?: any }) => any;
};

function withTestActions(reducer: (state: any, action: any) => any) {
  return (state: any, action: any) => {
    if (action.type === '__SET_STATE__') return action.payload;
    if (action.type === '__PATCH__') return { ...state, ...action.payload };
    return reducer(state, action);
  };
}

function makeCtx(store: any): StoreContext {
  const { renderHook } = require('hermes-test');

  const wrapper = ({ children }: { children: React.ReactNode }) =>
    React.createElement(Provider, { store } as any, children);

  return {
    store,
    wrapper,
    dispatch: store.dispatch.bind(store),
    getState: store.getState.bind(store),
    setState(state: Record<string, any>) { store.dispatch({ type: '__SET_STATE__', payload: state }); },
    patchState(partial: Record<string, any>) { store.dispatch({ type: '__PATCH__', payload: partial }); },
    renderHookWithReduxStore<T>(hookFn: (props?: any) => T, options?: { initialProps?: any }) {
      return renderHook(hookFn, { ...options, wrapper });
    },
  };
}

/** Quick store from plain state object — identity reducer, any shape */
export function withStore(initialState: Record<string, any> = {}): StoreContext {
  return makeCtx(configureStore({
    reducer: withTestActions((s: any = initialState) => s),
    preloadedState: initialState,
    middleware: (gdm) => gdm({ serializableCheck: false, immutableCheck: false }),
  }));
}

/** Real app reducer — patchState + real actions both work */
export function withAppReducer(
  reducer: (state: any, action: any) => any,
  preloadedState?: Record<string, any>,
): StoreContext {
  return makeCtx(configureStore({
    reducer: withTestActions(reducer),
    preloadedState,
    middleware: (gdm) => gdm({ serializableCheck: false, immutableCheck: false }),
  }));
}

/** Configurable RTK Query store — pass your API slices and middleware */
export function withApiStore(options: {
  slices: { reducerPath: string; reducer: any; middleware: any }[];
  defaultAppState?: Record<string, any>;
  initialState?: Record<string, any>;
  middleware?: Middleware[];
}): StoreContext {
  const { slices, defaultAppState = {}, initialState = {}, middleware: extraMiddleware = [] } = options;

  const callerApp = initialState.app ?? {};
  const mergedApp = { ...defaultAppState, ...callerApp };
  for (const key of Object.keys(defaultAppState)) {
    if (typeof defaultAppState[key] === 'object' && defaultAppState[key] !== null) {
      mergedApp[key] = { ...defaultAppState[key], ...(callerApp[key] ?? {}) };
    }
  }

  const reducers: Record<string, any> = {};
  for (const key of Object.keys(initialState)) {
    if (key === 'app') continue;
    reducers[key] = withTestActions((s: any = initialState[key]) => s);
  }
  reducers['app'] = withTestActions((s: any = mergedApp) => s);
  for (const slice of slices) {
    reducers[slice.reducerPath] = slice.reducer;
  }

  return makeCtx(configureStore({
    reducer: reducers,
    preloadedState: { ...initialState, app: mergedApp },
    middleware: (gdm) => {
      let chain = gdm({ serializableCheck: false, immutableCheck: false });
      for (const slice of slices) chain = chain.concat(slice.middleware);
      for (const mw of extraMiddleware) chain = chain.concat(mw);
      return chain;
    },
  }));
}
