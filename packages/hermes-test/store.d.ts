// Type declarations for hermes-test/store

import type React from 'react';

export interface StoreContext {
  readonly store: any;
  readonly wrapper: React.ComponentType<{ children: React.ReactNode }>;
  dispatch(action: unknown): unknown;
  getState(): unknown;
  setState(state: Record<string, unknown>): void;
  patchState(partial: Record<string, unknown>): void;
  renderHookWithReduxStore<T>(
    hookFn: (props?: unknown) => T,
    options?: { initialProps?: unknown },
  ): import('./index').HookResult<T>;
}

export interface SetupApiStoreOptions {
  middleware?: {
    prepend?: unknown[];
    concat?: unknown[];
  };
  preloadedState?: Record<string, unknown>;
}

export interface RtkQueryApi {
  reducer: unknown;
  middleware: unknown;
  reducerPath: string;
}

export function setupApiStore(
  apis: RtkQueryApi[],
  extraReducers?: Record<string, unknown>,
  options?: SetupApiStoreOptions,
): StoreContext & { apis: RtkQueryApi[] };

export function withStore(initialState?: Record<string, unknown>): StoreContext;

export function withAppReducer(
  reducer: (state: unknown, action: unknown) => unknown,
  preloadedState?: Record<string, unknown>,
): StoreContext;
