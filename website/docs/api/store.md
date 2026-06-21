---
title: store
---

# store API

Import from `hermes-test/store`:

```ts
import {withStore, withAppReducer, setupApiStore} from 'hermes-test/store';
```

## `withStore(initialState?)`

Quick identity-reducer store for plain state-driven tests.

## `withAppReducer(reducer, preloadedState?)`

Uses a real reducer while keeping test helpers.

## `setupApiStore(apis, extraReducers?, options?)`

RTK Query-style store setup with API middleware wiring.

## Store context helpers

All store builders return context with:

- `store`
- `dispatch(action)`
- `getState()`
- `setState(fullState)`
- `patchState(partialState)`
- `renderHookWithReduxStore(hookFn, options?)`
