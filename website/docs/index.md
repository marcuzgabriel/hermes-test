---
sidebar_position: 1
title: Overview
---

# hermes-test docs

**Built with Hermes engine parity — hermes-test is fast, easy, and Jest-API friendly for React Native and Expo.**

What you get out of the box:

- Rust CLI + esbuild + bytecode-first Hermes execution
- Auto mock detection in the bundling/runtime pipeline
- `ht.shallow()` support for shallow component mock workflows
- Built-in shims, plus custom shim support when needed
- Typed API surface for hooks, render, store helpers, and timers

## Performance at a glance

- **23x faster** full-suite runs (5s vs 116s)
- **64x faster** cached runs (0.84s vs 54s)
- **~350ms** watch reruns on real projects

## Start here

1. [Introduction](./getting-started/introduction)
2. [Installation](./getting-started/installation)
3. [First test](./getting-started/first-test)
4. [Basic hook example](./test-examples/basic-hook)
5. [Redux example](./test-examples/redux)
6. [Difficult mocks](./test-examples/difficult-mock)
7. [Snapshot example](./test-examples/snapshot)
8. [expect API](./api/expect)
9. [Full API overview](./api)

## Benchmarks

- [Benchmark overview](./benchmarks/overview)
- [Methodology](./benchmarks/methodology)

## Architecture topics

- [How auto-detection works](./architecture/auto-detection)
- [What shims are](./architecture/shims)

## References

- [References index](./references)
- [Challenges reference](./references/challenges)
