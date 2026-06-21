---
title: test
---

# test API

## Test and grouping

```ts
import {test, describe, beforeEach, afterEach} from 'hermes-test';

describe('feature', () => {
  beforeEach(() => {});
  afterEach(() => {});

  test('works', () => {});
  test.skip('later', () => {});
  test.only('focus', () => {});
});
```

## Callback context

`test` callbacks receive a context object with:

- `expect`
- `spy`
- `useMock`
- `renderHook`
- `act`
- `waitFor`

```ts
test('example', ({expect, spy}) => {
  const fn = spy();
  fn();
  expect(fn).toHaveBeenCalled();
});
```

## Timeout per test

```ts
test(
  'slow operation',
  async () => {
    // ...
  },
  {timeout: 5000},
);
```
