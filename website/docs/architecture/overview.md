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

## Bundler file map

Where each pipeline step lives in `crates/hermes-test-cli/src/bundler/`:

- **`mod.rs`** — Module wiring only: declares submodules and re-exports.
  `main.rs` just calls `bundler::bundle_tests(...)`.
- **`config.rs`** — Parses `hermes-test.config.json` + tsconfig into `BundleConfig` (root, testMatch, shims, aliases, externals, coverageThreshold).
  The single source for "what the user configured."
- **`entry.rs`** — Generates the synthetic entry module: discovers test files, scans for `ht.mock`/`ht.unmock`/`ht.shallow` directives.
  Writes the mock **wrapper** files — the "brain": Proxies consulting the per-file mock registry at access time.
- **`esbuild.rs`** — Drives bundling: spawns Node with `plugin_build.cjs`, assembles esbuild options (es2016, `async-await=false`, aliases, externals, loaders, sourcemaps).
  Returns the bundle for eval + caching.
- **`plugin_build.cjs`** — Node-side esbuild JS-API build script; its `onResolve` hook is the "receptionist."
  Routes imports of mocked modules to wrapper files, everything else to the real file.
- **`patches.rs`** — Post-processes esbuild output for Hermes: `__toESM` interop fix, `hoist_mock_modules`, require-shim injection.
  Every patch is covered by a golden fixture.
- **`shims.rs`** — Native-module shim registration: maps RN native packages to externalized stand-ins.
  Real native code never enters the bundle.
- **`fixture_tests.rs`** — Golden-fixture corpus for `patches.rs`: expected-output text match plus behavior assertions run in real Hermes.
  Text can look right and still behave wrong — always assert behavior.

See [Mock resolution](./mock-resolution) for the receptionist/brain design these files implement.

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
