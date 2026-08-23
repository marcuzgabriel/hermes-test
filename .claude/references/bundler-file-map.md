# Bundler file map (`crates/hermes-test-cli/src/bundler/`)

Two lines per file. The design doc is `website/docs/architecture/mock-resolution.md`
(receptionist + brain); this is just the map.

esbuild is deliberate: its lazy CJS module init is the window that lets `ht.mock()`
in a test body run before the mocked module evaluates. A pure-rolldown port was
attempted and REJECTED July 2026 (eager scope-hoisted ESM has no such window) —
verdict in the final commit of branch `feat/rolldown-bundler`. Do not re-attempt.

```
config.rs ──► entry.rs ──► esbuild.rs ⇄ plugin_build.cjs ──► patches.rs ──► bundle
                  ▲
              shims.rs
```

**`mod.rs`** —
Module wiring only: declares submodules and re-exports, so `main.rs` just calls `bundler::bundle_tests(...)`.

**`config.rs`** —
Parses `hermes-test.config.json` + tsconfig into `BundleConfig`: root, testMatch, shims, aliases, externals, coverageThreshold.
The single source for "what the user configured."

**`entry.rs`** —
Generates the synthetic entry module: discovers test files, scans for `ht.mock`/`ht.unmock`/`ht.shallow` directives.
Writes the mock **wrapper** files — the "brain": Proxies consulting the per-file mock registry at access time.

**`esbuild.rs`** —
Drives bundling: spawns Node with `plugin_build.cjs`, assembles esbuild options (es2016, `async-await=false`, aliases, externals, loaders, sourcemaps).
Returns the bundle for eval + caching.

**`plugin_build.cjs`** —
Node-side esbuild JS-API build script; its `onResolve` hook is the "receptionist".
Routes imports of mocked modules to wrapper files, everything else to the real file — directions only, never answers.

**`patches.rs`** —
Post-processes esbuild output for Hermes: `__toESM` interop fix, `hoist_mock_modules` (mock registration above requires), require-shim injection.
The layer that encodes esbuild-output shape — every patch has a golden fixture.

**`shims.rs`** —
Native-module shim registration: maps RN native packages (AsyncStorage, Firebase, …) to externalized stand-ins.
Real native code never enters the bundle.

**`fixture_tests.rs`** —
`#[cfg(test)]` golden-fixture corpus for `patches.rs`: `input.js` → `expected.js` text match PLUS `behavior.js` run in real Hermes.
Text can look right and still behave wrong — always assert behavior.
