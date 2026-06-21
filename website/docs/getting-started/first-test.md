---
title: First test
---

# First test

```ts
import {test, expect, renderHook, act} from 'hermes-test';

function useCounter(initial = 0) {
  let count = initial;
  return {
    get count() {
      return count;
    },
    increment() {
      count += 1;
    },
  };
}

test('increments counter', () => {
  const {result} = renderHook(() => useCounter(0));

  act(() => result.current.increment());
  expect(result.current.count).toBe(1);
});
```
