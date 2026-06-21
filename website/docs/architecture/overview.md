---
title: Overview
---

# Architecture overview

hermes-test architecture follows one goal: **fast, deterministic RN tests with engine parity**.

## Problem the architecture solves

In typical RN testing pipelines, Node-based execution and heavy transform stacks create:

- slower runs
- config drift
- behavior differences vs Hermes runtime

## The fix (README model)

hermes-test replaces the traditional worker + Babel-heavy path with:

- **one esbuild pass**
- **one process**
- **zero Babel transforms in the runtime path**

The core stack is:

- **Rust CLI** orchestration
- **esbuild** bundling
- **Hermes** execution
- **typed TypeScript API** for tests

## How it works (bytecode-first)

1. Collect test files from CLI/config (`testMatch` + args).
2. Bundle test graph with esbuild.
3. Apply mock/shim wiring and runtime patches.
4. Compile bundle to Hermes bytecode (`.hbc`).
5. Execute bytecode in Hermes.
6. Reuse cached JS/bytecode artifacts for reruns.

This keeps startup and reruns fast while preserving RN runtime behavior.

## Design outcomes (from this architecture)

- Sub-second or low-second local feedback loops
- Fewer moving parts than traditional Jest+Babel stacks
- Better confidence for Hermes-specific behavior

See also:

- [Auto-detection](./auto-detection)
- [Shims](./shims)
- [Intl observations](../issues/intl-observations)
- [Linux support](../issues/linux-support)
- [References index](../references)

For deeper architecture notes, see the repository root docs folder:

- `docs/architecture.md`
- `docs/detail-bundling.mmd`
- `docs/detail-hermes.mmd`
- `docs/detail-mocking.mmd`
