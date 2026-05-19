// Test store setup — mirrors setupApiStore from the real app's testing package
import { configureStore, combineReducers } from '@reduxjs/toolkit';
import appApi from './api';

export function createTestStore() {
  return configureStore({
    reducer: combineReducers({
      [appApi.reducerPath]: appApi.reducer,
    }),
    middleware: (gdm) =>
      gdm({ serializableCheck: false, immutableCheck: false })
        .concat(appApi.middleware),
  });
}
