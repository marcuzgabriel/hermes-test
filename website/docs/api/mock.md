---
title: mock
---

# mock API

## Runtime patching (`useMock`)

```ts
import {useMock, spy} from 'hermes-test';
import * as analytics from './analytics';

useMock(analytics, {
  track: spy(),
});
```

Use `useMock` when you already imported a module object and want to patch selected exports for the current test.

## Module mocks (`ht.mock`)

```ts
import {spy} from 'hermes-test';

const track = spy();

ht.mock('./analytics', () => ({
  track,
}));
```

## Unmock directive

```ts
ht.unmock('moment');
```

Use this when you need the real module bundled directly instead of shim/proxy interception.

## Shallow directive

```ts
ht.shallow('./HeavyChild');
```

`ht.shallow(...)` is a bundler directive used to force a module into shallow behavior.

## Fetch mocks (MSW-like)

```ts
import {http, HttpResponse} from 'hermes-test';

ht.mock.fetch(
  http.get('/api/count', () => HttpResponse.json({count: 42})),
);
```

Supports string or RegExp URL matching and multiple handlers in one call.

## Overwrite handlers

```ts
ht.mock.fetch.overwrite(
  http.get('/api/count', () => HttpResponse.json({count: 43})),
);
```

`overwrite(...)` is available for compatibility with existing test code.

## Reset handlers

```ts
ht.mock.fetch.reset();
ht.mock.fetch.clear();
```

- `clear()` clears all registered handlers.
- `reset()` is a legacy compatibility API; prefer `clear()` when you want a full reset.

## URL matching

- Exact string
- Prefix + query (`/api/user` matches `/api/user?id=1`)
- RegExp (for dynamic paths)

## Unhandled requests

Unhandled `ht.mock.fetch` requests return a non-OK mock response (status 500), which helps surface missing handlers in tests.
