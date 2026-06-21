---
title: hooks
---

# hooks API

## `renderHook`

```ts
import {renderHook} from 'hermes-test';

const {result, history, renderCount, rerender, unmount} = renderHook(() => useCounter(0));
```

Supports options:

- `initialProps`
- `wrapper`

## `act`

```ts
import {act} from 'hermes-test';

await act(async () => {
  await result.current.incrementAsync();
});
```

## `waitFor`

```ts
import {waitFor} from 'hermes-test';

waitFor(() => result.current.ready, {timeout: 3000, interval: 25});
```

## `flushAsync`

```ts
import {flushAsync} from 'hermes-test';

const value = flushAsync(promiseLikeValue);
```
