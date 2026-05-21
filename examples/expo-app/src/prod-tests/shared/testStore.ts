// Test store utilities.
//
// Two modes:
//
//   // 1. Quick store — any shape, for fast test porting:
//   const ctx = withStore({ insurances: { products: [] } });
//   ctx.patchState({ insurances: { products: [mockCar] } });
//
//   // 2. Real app reducer — real logic + test control:
//   import rootReducer from '@app/reducers';
//   const ctx = withAppReducer(rootReducer, { insurances: { products: [] } });
//   ctx.patchState({ insurances: { products: [mockCar] } });
//   ctx.dispatch(authActions.login({ token: '...' }));
//
// Both: ctx.renderHook(() => useMyHook());

import React from 'react';
import { Provider } from 'react-redux';
import { configureStore } from '@reduxjs/toolkit';

function withTestActions(reducer: (state: any, action: any) => any) {
  return (state: any, action: any) => {
    if (action.type === '__SET_STATE__') return action.payload;
    if (action.type === '__PATCH__')     return { ...state, ...action.payload };
    return reducer(state, action);
  };
}

function makeCtx(store: any) {
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

// Quick store from plain state object
export function withStore(initialState: Record<string, any> = {}) {
  return makeCtx(configureStore({
    reducer: withTestActions((s: any = initialState) => s),
    preloadedState: initialState,
    middleware: (gdm) => gdm({ serializableCheck: false, immutableCheck: false }),
  }));
}

// Real app reducer — patchState + real actions both work
export function withAppReducer(
  reducer: (state: any, action: any) => any,
  preloadedState?: Record<string, any>,
) {
  return makeCtx(configureStore({
    reducer: withTestActions(reducer),
    preloadedState,
    middleware: (gdm) => gdm({ serializableCheck: false, immutableCheck: false }),
  }));
}
