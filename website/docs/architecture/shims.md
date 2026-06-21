---
title: Shims
---

# Shims

A **shim** is a replacement module used when a real native or environment-specific module is not available in the test runtime.

## Why shims exist

- Hermes test runtime has no full mobile native environment
- Some packages assume platform APIs that are not present in tests
- Shims provide predictable behavior for those modules

## Conceptual example

If production code imports a native package:

```ts
import {LaunchArguments} from 'react-native-launch-arguments';
```

A shim can expose a stable test-friendly implementation:

```ts
export const LaunchArguments = {
  value: () => ({env: 'test'}),
};
```

Your tests then run against this deterministic shim behavior instead of crashing on missing native bindings.

## When to use a shim vs `mock(...)`

- Use **shim** when many tests need a safe default implementation
- Use **`mock(...)`** when a single test or file needs scenario-specific behavior
