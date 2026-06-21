---
title: Difficult mock scenarios
---

# Difficult mock scenarios

These patterns are based on real project tests using hermes-test's MSW-like fetch API:

- `src/reducers/slices/geocode/reverse/__tests__/reverse.test.ts`
- `src/hooks/actionMessages/__tests__/useActionMessages.test.ts`

## 1. RTK Query endpoint test with `ht.mock.fetch`

```ts
import { test, describe, afterEach, http, HttpResponse, expect } from 'hermes-test';
import reverseLookupApi from '@acme/app/reducers/slices/geocode/reverse';
import { withReduxStore } from '../../../../../../test/testStore';

describe('reducers/slices/geocode/reverse', () => {
  afterEach(() => {
    ht.mock.fetch.reset();
  });

  test('handle successful request', async () => {
    ht.mock.fetch(
      http.get(/reverse/, () =>
        HttpResponse.json({
          id: '1',
          vejnavn: 'Landgreven',
          husnr: '10',
        }),
      ),
    );

    const ctx = withReduxStore();
    const res = await ctx.store.dispatch(
      reverseLookupApi.endpoints.getReverseGeocode.initiate({ x: '12', y: '55' }),
    );

    expect(res.status).toBe('fulfilled');
    expect(res.data?.streetName).toBe('Landgreven');
  });
});
```

## 2. Hook flow test with GET/POST handlers

```ts
import { test, act, http, HttpResponse } from 'hermes-test';
import { withReduxStore } from '../../../../test/testStore';
import { useActionMessages } from '@acme/app/hooks/actionMessages/useActionMessages';

test('fetch + dismiss action message', async ({ expect }) => {
  ht.mock.fetch(
    http.get(actionMessagesUrl, () => HttpResponse.json({ messages: [msg] })),
    http.post(actionMessagesUrl, () => HttpResponse.json({})),
  );

  const ctx = withReduxStore();
  const { result } = ctx.renderHookWithReduxStore(() => useActionMessages());

  await act(async () => {
    await result.current.fetchActionMessages();
  });

  expect(result.current.messageItems.length).toBe(1);
  await result.current.dismiss(msg);
});
```

`ht.mock.fetch(...)` is MSW-like: register handlers by method + URL matcher and return `HttpResponse`.
