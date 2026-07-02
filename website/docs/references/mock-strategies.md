---
title: Mock strategies reference
---

# Mock strategies reference

Source: [`/.claude/references/mock-strategies.md`](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/mock-strategies.md)

**Status: partially historical.** Mock delivery now runs through the esbuild
onResolve plugin — see [Mock resolution](../architecture/mock-resolution.md) for
the current model. The runtime patterns documented here (function proxy apply
traps, per-file mock scoping, Redux/API testing patterns) survive unchanged inside
the plugin wrappers; the delivery mechanisms (shadow wrappers, package shims) are
legacy, reachable via `HT_RESOLVER=legacy` for one release cycle.
