---
title: Installation
---

# Installation

If you are new to hermes-test, read [Introduction](./introduction) first.

```bash
npm install -D hermes-test
```

Or with Bun:

```bash
bun add -D hermes-test
```

## Run tests

```bash
hermes-test
hermes-test --watch
```

## Add `hermes-test.config.json`

Create this file at your project root:

```json
{
  // if monorepo
  "root": "../..",
  "testMatch": ".test.ts"
}
```

### Common fields

- `root`: workspace/app root (important for monorepos)
- `tsconfig`: optional tsconfig path for alias resolution
- `testMatch`: file suffix used for discovery (for example `.test.ts` or `.hermes.test.ts`)
- `externals`: modules to externalize from bundle
- `shims`: module replacements (built-in `hermes-test/shims/*` or local file path)
- `coverageThreshold`: minimum coverage percentage for `--coverage`

Start minimal, then add externals/shims only when your project needs them.

## Supported config keys

| Key | Type | Purpose |
|---|---|---|
| `root` | `string` | Workspace root (mainly monorepos) |
| `tsconfig` | `string` | Explicit tsconfig path for alias resolution |
| `testMatch` | `string` | Test file suffix used by discovery |
| `externals` | `string[]` | Modules externalized from bundle |
| `external` | `string[]` | Alias of `externals` |
| `shims` | `Record<string, string>` | Module replacements (`hermes-test/shims/*` or local files) |
| `coverageThreshold` | `number` | Minimum total coverage percent for `--coverage` |

## Typical migration script updates

```json
{
  "scripts": {
    "test": "hermes-test",
    "test:watch": "hermes-test --watch"
  }
}
```

## Next steps

1. Write your first hook test: [First test](./first-test)
2. If your code uses Redux: [Redux example](../test-examples/redux)
3. See speed expectations: [Benchmark overview](../benchmarks/overview)
