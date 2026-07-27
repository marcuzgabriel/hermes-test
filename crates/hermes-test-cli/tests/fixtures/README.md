# Fixture corpus for `patches.rs`

Golden-file tests for the transforms that rewrite esbuild's bundle output for
Hermes. Harness: `src/bundler/fixture_tests.rs` (hardening plan item 2 in
`.claude/references/hardening-assessment.md`).

Each fixture directory holds three files:

| File | Role |
|---|---|
| `input.js` | Source *before* the transform, imitating real esbuild output byte-for-byte. The patches are exact-text matches, so whitespace here is load-bearing — do not reformat. |
| `expected.js` | Golden file: the exact output the transform must produce. Never hand-edit; regenerate with `HT_UPDATE_FIXTURES=1 cargo test` and review the diff. |
| `behavior.js` | Assertions executed in **real Hermes** against the transformed code. Encodes the *patched* semantics, so it fails even if a broken golden gets blessed. Each file's comments explain the bug it guards. |

Run: `cargo test -p hermes-test-cli fixture` (~0.2s).

## Workflow

- **Fixing a bug in a transform:** add a fixture dir reproducing it, watch it
  fail, patch the Rust until green. The other fixtures guard against regression.
- **Changing a transform intentionally:** `HT_UPDATE_FIXTURES=1 cargo test`,
  review the `expected.js` diffs like any code change, commit.
- **Bumping esbuild:** regenerate the `input.js` files from a real new-version
  bundle (see below); the `expected.js` diffs show exactly what changed.

## class-extends/ — Patch 4: `fix_all_class_extends`

Hermes bugs: TDZ crash on `class X extends LocalVariable`, and `super()` in
native subclasses discarding the return value. The transform downlevels every
class-extends to `Reflect.construct`-based functions.

| Fixture | Guards |
|---|---|
| `01-no-constructor` | Synthesized default constructor forwards all args to the parent; prototype chain and `constructor` identity intact. |
| `02-constructor-and-internal-name` | `super(args)` reaches the parent; `this.x` after `super()` lands on the instance; esbuild's `class Name = class _Name` internal-name references rewritten in method/static bodies. |
| `03-super-chain-new-target` | **The Day 23 bug shape.** 4-level chain with no-constructor classes in the MIDDLE and at the leaf — both downleveling branches must forward `new.target`, or instances silently get the wrong prototype. (A no-ctor class at the *top* of a chain cannot catch this; gap found by mutation-testing the corpus.) |
| `04-array-subclass` | Hermes native-super bug: subclassing Array works (push/length/indexing). Also documents an engine fact: Hermes ignores `Symbol.species` in Array methods — `map()` returns plain `Array`, matching on-device behavior. |
| `05-class-declaration-static` | Bare `class Name extends Expr` declarations (pattern C) and `static` methods attached to the class, not the prototype. |
| `06-super-method-this` | `super.method()` keeps `this` bound to the instance. Guarded two real bugs (found by this fixture, July 2026): in methods the naive `Parent.prototype.method()` rewrite silently rebound `this` to the prototype; in constructors `super.` wasn't rewritten at all — a Hermes compile error. |

## esbuild-helpers/ — Patches 1–3 on esbuild's runtime helpers

Inputs reconstructed byte-exact from a real cached bundle
(`examples/expo-app/.hermes-test-cache/plugin-*.js`), reversed to pre-patch form.

| Fixture | Patch | Guards |
|---|---|---|
| `01-copy-props-for-let-of` | 1 | Hermes for-let-of closure bug: unpatched, every re-export getter returns the LAST key's value. Patch binds each key by value and adds `configurable:true`. |
| `02-export-configurable` | 2 | esbuild's `__export` getters are non-configurable — replacing an export (mocking) would throw. Patch makes them redefinable. |
| `03-to-esm-passthrough` | 3 | Unpatched `__toESM` copies properties into a fresh object, destroying mock Proxies. Patch adds an early return: `__esModule` modules pass through as the SAME object; plain CJS still gets `default` interop wrapping. |
| `04-duplicated-nested-helpers` | 1–3 | Dependencies shipped as pre-bundled CJS (react-redux et al.) inline their OWN copy of the helpers, `2`-suffixed and indented deeper. All three patches must fix every copy — first-occurrence-only patching left nested helpers unpatched in production bundles (fixed July 2026, found by this fixture). Engine note: the raw for-let-of closure bug no longer reproduces in current vendored Hermes; what the nested patches still buy is mocking capability (configurable getters, Proxy passthrough). |

## mock-require-shim/ — `inject_mock_require_shim`

Replaces esbuild's "Dynamic require of X is not supported" throw with a Proxy
over the `__HT_mocks` registry — how externalized/native modules become
mockable. Silent-green risk class: if the pattern stops matching, tests can run
against nothing.

| Fixture | Guards |
|---|---|
| `01-mock-registry-proxy` | Global-registry lookup, `__esModule` flag, per-file mocks winning at ACCESS time (the ESM hoisting problem), fallback to global, infinite-noop Proxy for unmocked modules (string/length/thenable behavior), `default` interop. |

## hoist-mocks/ — `hoist_mock_modules`

esbuild initializes modules (`init_*()`) where the imports were — before the
`ht.mock()` calls below them. The transform pushes `init_*()` below the last
mock so modules that capture values at init time see the mock.

| Fixture | Guards |
|---|---|
| `01-init-after-mocks` | `init_*()` calls execute after all `ht.mock()` registrations; `init_hermes*` stays put; statement order otherwise preserved (verified by recorded execution order in Hermes). |

## Known gaps (candidates for next fixtures)

- `hoist-mocks`: only the `ht.mock(` spelling is scanned; the
  `(0, import_hermes_test.mock)(` spelling mentioned in the code comment has
  no fixture (and possibly no code path — check before writing).
- `mock-require-shim`: `__HT_mock_aliases` (resolved-path → original-path
  mapping) is untested.
- Comment-blindness: a commented-out `ht.mock()` still registers (hardening
  assessment §5) — fixture would currently FAIL; write it when fixing.
