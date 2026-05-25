// hermes-test/store — Redux test store factories
//
// setupApiStore: thin wrapper matching Jest's setupApiStore pattern
// withStore: quick identity-reducer store for any state shape
// withAppReducer: real app reducer with patchState + real actions

import React from 'react';
import { Provider } from 'react-redux';
import { configureStore } from '@reduxjs/toolkit';

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

interface RtkQueryApi {
  reducer: any;
  middleware: any;
  reducerPath: string;
}

export interface SetupApiStoreOptions {
  middleware?: {
    prepend?: any[];
    concat?: any[];
  };
  preloadedState?: Record<string, any>;
}

/**
 * Thin store factory matching Jest's setupApiStore pattern.
 *
 * setupApiStore([api, cms], { app: rootReducer })
 * setupApiStore([api], { app: rootReducer }, { middleware: { prepend: [myMw] } })
 * setupApiStore([api], { app: rootReducer }, { preloadedState: { app: { auth: { ... } } } })
 */
export function setupApiStore(
  apis: RtkQueryApi[],
  extraReducers?: Record<string, any>,
  options?: SetupApiStoreOptions,
) {
  const reducerMap: Record<string, any> = {};
  for (const api of apis) {
    reducerMap[api.reducerPath] = api.reducer;
  }
  if (extraReducers) Object.assign(reducerMap, extraReducers);

  const store = configureStore({
    reducer: reducerMap,
    preloadedState: options?.preloadedState,
    middleware: (gdm) => {
      let chain = gdm({ serializableCheck: false, immutableCheck: false });
      for (const a of apis) chain = chain.concat(a.middleware);
      for (const mw of (options?.middleware?.concat ?? [])) chain = chain.concat(mw);
      for (const mw of (options?.middleware?.prepend ?? [])) chain = chain.prepend(mw);
      return chain;
    },
  });

  return { ...makeCtx(store), apis };
}
