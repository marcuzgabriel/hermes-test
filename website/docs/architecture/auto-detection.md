---
title: Auto-detection
---

# How auto-detection works

hermes-test inspects imported modules during bundling and test execution setup to reduce manual config.

## What it auto-detects

- Test files from configured patterns/CLI arguments
- Imported dependencies that should be bundled vs externalized
- Mocked module paths registered via `ht.mock(...)`

## Why this matters

- Less setup for React Native projects
- Fewer brittle manual mapping rules
- Faster startup because the runner builds exactly what is needed

## Example

```ts
import {test, renderHook} from 'hermes-test';
import {useFeature} from '../hooks/useFeature';

ht.mock('../services/api', () => ({
  fetchFeature: async () => ({enabled: true}),
}));

test('loads feature flag', async ({expect}) => {
  const {result} = renderHook(() => useFeature());
  expect(result.current.enabled).toBe(true);
});
```

In this flow, the runner detects the test file, detects that `../services/api` is mocked, and wires the mock path into the runtime mock layer.
