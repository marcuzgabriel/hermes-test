---
title: mock
---

# mock API

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

## Fetch mocks (MSW-like)

```ts
import {http, HttpResponse} from 'hermes-test';

ht.mock.fetch(
  http.get('/api/count', () => HttpResponse.json({count: 42})),
);
```

Supports string or RegExp URL matching and multiple handlers in one call.

## Reset handlers

```ts
ht.mock.fetch.reset();
ht.mock.fetch.clear();
```

## URL matching

- Exact string
- Prefix + query (`/api/user` matches `/api/user?id=1`)
- RegExp (for dynamic paths)

## Unhandled requests

Unhandled `ht.mock.fetch` requests return a non-OK mock response (status 500), which helps surface missing handlers in tests.
