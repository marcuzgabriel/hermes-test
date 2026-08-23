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

## What a configured shim does

```json
{
  "shims": {
    "react-native-launch-arguments": "./test/shims/launch-arguments.js",
    "@react-native-masked-view/masked-view": "./test/shims/masked-view.js"
  }
}
```

Two things happen for each entry:

1. **The real package is externalized** — esbuild never bundles it. That is what makes shims
   work for packages the bundler cannot even parse (Flow syntax such as
   `@react-native-masked-view/masked-view`, packages that `require()` fonts or media).
2. **Your file is served to every importer at runtime**, via the mock registry.

Only the exact module name is externalized: a shim for `pkg` does not affect `pkg/sub`.

Assets don't need shims at all: any relative import with a non-code extension
(`.ttf`, `.png`, `.mp3`, …) loads as an empty module.

## When to use a shim vs `mock(...)`

- Use **shim** when many tests need a safe default implementation
- Use **`mock(...)`** when a single test or file needs scenario-specific behavior
