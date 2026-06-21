---
title: Basic hook test
---

# Basic hook test

```ts
import {test, renderHook, act} from 'hermes-test';

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

test('increments state', ({expect}) => {
  const {result} = renderHook(() => useCounter(0));
  act(() => result.current.increment());
  expect(result.current.count).toBe(1);
});
```
