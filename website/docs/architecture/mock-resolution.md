---
title: Mock resolution
---

# Mock resolution: the receptionist and the brain

Every `ht.mock()` in hermes-test is served by the same two-part machine. Picture the
bundle as an **office building**: every module is an office, and every `import`
statement is a visitor walking up to the front desk asking for one.

```
BUNDLE TIME (once)                      RUN TIME (per test, per access)
─────────────────                       ───────────────────────────────
onResolve — the receptionist            get() — the brain
gives DIRECTIONS, never answers:        gives ANSWERS, one question at a time:
"which door do you walk through?"       "mock for the current test, or the
→ real office, or the front office        real thing through the connecting
  standing in front of it                 door?"
```

## The receptionist (`onResolve`, bundle time)

esbuild is driven through its JS API with a plugin. The plugin's `onResolve` hook is
the front desk every visitor must pass: it sees who is asking (the importing file)
and what they asked for, as written (`./flows/foo`, `@scope/hooks/useX`,
`expo-secure-store`) — and its only job is **directions**:

- import of an unmocked module → *"real office, straight ahead"* (normal esbuild
  resolution, full speed, no interception)
- import of a mocked module → *"take that door instead"* — a small **front office**
  (a generated wrapper file) that stands in front of the real one

The receptionist never answers the visitor's actual question — it cannot, because
bundling happens once, before any test runs, and it has no idea which test will be
asking. And crucially, redirect ≠ removal: the front office has a **connecting
door** to the real office (a `?ht-real` import the receptionist waves through), so
**the real code stays in the bundle**. One bundle, one Hermes VM, always.

## The brain (`get()`, run time)

The brain sits inside the front office and answers each question individually. The
wrapper is a CommonJS Proxy; its property getter runs on every access:

```js
get(target, prop) {
  var mocks = __HT_file_mocks[__currentTestFile];  // per-file isolation
  if (mocks && prop in mocks[key]) return mock;    // mock, at ACCESS time
  return getReal()[prop];                          // else: the real module
}
```

The mock-or-real decision is made **per running test file, per property access** —
not at import time. That's why `ht.mock()` can appear before or after imports, why
two test files can mock the same module differently inside one bundle, and why a
test file that mocks nothing falls straight through to the real implementation.

## Why two parts?

- The receptionist can't decide mock-or-real: bundling happens **once**, before any
  test runs, and the same compiled import line serves every test file. "Should this
  be mocked?" has no answer at bundle time.
- The brain can't intercept anything on its own: a Proxy only works if it **is**
  what the import resolves to. Something must place it in the doorway — and that
  placement is all the receptionist does.

Directions without answers, answers without directions: the receptionist routes
every visitor but can't tell mock from real; the brain tells mock from real but
only for visitors who were routed to its office.

## Matching rules

How the receptionist decides an import "corresponds to a mocked module" differs by
mock kind — deliberately:

| Mock kind | Matching | Why |
|---|---|---|
| Relative (`ht.mock('../flows/foo')`) | **File identity** — resolved from the test file's directory; intercepts every import route to that file, however spelled | The whole point of a relative mock is to catch the consumer's differently-spelled import |
| Alias (`ht.mock('@scope/hooks/useX')`) | **Specifier text** — exact match on the import string | Production-internal relative imports of the same file keep the real module, so module-level init-time reads can never capture another test file's mocks |
| Package (`ht.mock('expo-secure-store')`) | **Specifier text** — exact match | Same reasoning as alias mocks |
| Barrel ancestors of mocked alias paths | Text match + **prefix delegation** — the barrel wrapper serves any of the current test's mocks registered under a deeper path | `import { useX } from '@scope/hooks'` must see a mock registered as `@scope/hooks/redux/useX` |

Mocks with no bundleable real module — native packages, config externals, shimmed
packages, `react-native` — are externalized instead and resolved by the runtime
require shim, exactly as before.

## Escape hatch

`HT_RESOLVER=legacy` restores the previous delivery pipeline (shadow directory
trees, package shims, isolated bundles for relative mocks). It is kept for one
release cycle as a safety valve and will be removed after the plugin resolver has
soaked in production.
