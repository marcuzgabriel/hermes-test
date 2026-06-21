---
title: Redux hook test
---

# Redux hook test

This pattern is based on real hermes-test usage in a production React Native app:

- `test/testStore.ts`
- `src/hooks/loading/__tests__/useIsLoading.test.ts`

The app exposes helper methods from `hermes-test/store` and builds a project-specific helper:

```ts
import { setupApiStore } from 'hermes-test/store';
import api from '@acme/app/reducers/slices/api';
import cms from '@acme/app/reducers/slices/cms';
import dawa from '@acme/app/reducers/slices/dawa';
import assistant from '@acme/app/reducers/slices/assistant';
import rootReducer from '@acme/app/reducers';

export const withReduxStore = (options?: {
  preloadedState?: Record<string, any>;
  middleware?: { prepend?: any[]; concat?: any[] };
}) => setupApiStore([api, cms, dawa, assistant], { app: rootReducer }, options);
```

Then tests use `renderHookWithReduxStore` with real state:

```ts
import { test, describe, expect } from 'hermes-test';
import { withReduxStore } from '../../../../test/testStore';
import { useIsLoading } from '@acme/app/hooks/loading/useIsLoading';

describe('hooks/useIsLoading', () => {
  test('should be loading when adding a loading tag', () => {
    const ctx = withReduxStore({
      preloadedState: {
        app: {
          loading: { tags: ['testLoginUser'] },
          errorHandling: { currentErrors: [] },
        },
      },
    });

    const { result } = ctx.renderHookWithReduxStore(() => useIsLoading('testLoginUser'));
    expect(result.current.isLoading).toBeTruthy();
  });
});
```

This is the preferred strategy over mocking `react-redux` directly.
