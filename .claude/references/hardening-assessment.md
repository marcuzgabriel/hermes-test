# Hardening Assessment — Solidity Review & Enhancement Priorities

External code-level review (July 2026, after the Day 23 `new.target` bug and the
Day 24 relative-mock fix — two deep debugging sessions across the whole pipeline).
Question asked: "is this lib solid, or too many patches and workarounds?" This file
records the answer and the prioritized enhancement list derived from it, so future
work starts here instead of re-deriving it.

## Verdict

Solid at the core. The speed is architectural, not a hack: one esbuild pass, one
process, the app's real engine — structurally sounder than jest's per-file Babel +
worker IPC. The mock system is NOT an accretion of workarounds: 11+ documented
failed attempts converged onto one repeated idea (Proxy wrapper checks per-file
mocks at access time, falls back to real) applied at three resolution layers:

| Layer | Mechanism |
|---|---|
| tsconfig-alias paths | shadow wrappers (shadow dir tree + Proxy wrapper files) |
| npm packages | package shims (esbuild alias → CJS Proxy → `@__ht_real_pkg/`) |
| test-file-relative paths | isolated bundle + absolute-path external + `__HT_mock_aliases` (Day 24) |

Patch count has gone DOWN over time (for-let-of and other Hermes compat patches
removed as Hermes matured). The reference-docs discipline (challenges.md,
decisions.md) is rare and measurably valuable — it prevented re-trying documented
dead ends during both debugging sessions.

Endorsing it as sole runner for a large prod app (Topdanmark: 288 suites, 1793
tests) is a justified bet PROVIDED the items below get addressed — the risk surface
is narrow and known, addressable with bounded work, not rewrites.

## Where the risk concentrates

### 1. patches.rs is a mini-transpiler written in regex over esbuild text output
- `fix_all_class_extends()` does class downleveling with regex + manual brace
  matching against UNMINIFIED esbuild output patterns.
- Both real bugs found in the July 2026 review lived here (Day 23 `new.target`
  dropped through chained `super()`; the no-constructor and has-constructor
  branches had silently diverged on the same invariant).
- An esbuild version bump can silently change output formatting and break pattern
  matching — the failure mode would be silent, not loud.
- Mitigation until fixed: keep esbuild pinned.

### 2. Silent-pass failure modes (worst class of bug for a test runner)
Both issues found in the review were silent-green:
- Relative ht.mock() silently not applying → test ran against REAL code and still
  passed (fixed Day 24).
- **STILL OPEN**: directory args — `hermes-test src/some-dir` (documented in the
  README!) treats the dir path as a test file, runs ZERO tests, exits 0. In CI this
  is green-while-running-nothing. Reproduced on published v1.1.5.

### 3. The tool itself has no unit tests
- Validation strategy is "run examples/expo-app + Topdanmark" — decent integration
  coverage (1200+ assertions, incl. the Day 24 relative-mock regression pair), but
  zero isolated tests over the bundler transforms.
- A fixture test compiling class-extends inputs through `fix_all_class_extends`
  and asserting output would have caught the `new.target` bug at authoring time.

### 4. Dead code accumulation
- Confirmed dead (cargo never-used warnings + zero callers): split mode
  (`bundle_split_with_shallow`, `run_tests_split` path, `generate_group_entry_internal`,
  `generate_setup_code`, `generate_vendor_entry`, `extract_required_packages`,
  `bundle_esbuild_with_config_pub`, `pkg_matches_external`) and
  `run_persistent_cycle` (zero callers).
- Cost is real: it misleads maintenance (review time was wasted reading split-mode
  code before noticing it was unreachable).

### 5. Minor known issues
- examples async-data-fetcher.test.ts "refetch reloads data" is timing-flaky
  (fails intermittently on published v1.1.5 too).
- `print_jest_summary` prints "Test Suites: N passed, N total" using file count for
  both numbers even when suites failed (cosmetic).
- Regex scanning of test files for ht.mock()/ht.shallow() is comment-blind — a
  commented-out ht.mock still registers as a mock path.

## Prioritized enhancement plan

1. **Fix directory-arg silent zero-run**: DONE (feat/plugin-resolver, pre-1.2.0).
   expand_file_args() expands directory args via find_test_files_with_pattern
   (respecting testMatch) and HARD-ERRORS on nonexistent paths and on directories
   containing no test files. Wired into both run and watch entry points.
2. **Fixture tests for patches.rs transforms** (small): golden-file tests running
   representative class-extends shapes (with/without constructor, 2- and 3-level
   chains, Array subclass, `this.constructor.name` pattern) through
   `fix_all_class_extends` + the other patches, asserting on output AND on executed
   behavior in Hermes. Same for `inject_mock_require_shim` and `hoist_mock_modules`.
3. **Port class downleveling from regex to OXC** (the one real project): oxc is
   already a dependency (0.132, semantic feature). An AST visit + codegen for
   class-extends removes the single riskiest component. Note the SWC attempt was
   rejected for real reasons (see esbuild.rs header comment: scoped thread-locals,
   helper injection incompatible with Hermes, full re-emit breaks other patches) —
   the OXC port must re-emit ONLY the class expressions, not the whole bundle, or
   must come after the other regex patches are also AST-based.
4. **Prune dead code**: DONE (feat/plugin-resolver) — deleted 771 lines: split
   mode (bundle_split*, SplitBundle, group/vendor entry generators, split
   caching), run_tests_split, run_persistent_cycle, orphaned print helpers,
   compile_to_bytecode_cached, coverage map builders. Zero rustc warnings from
   crate code after prune; full gauntlet green in both resolver modes.
   NOT deleted (deliberate): the legacy resolver path (shadow trees, package
   shims, iso runner) behind HT_RESOLVER=legacy — phase 4, after release soak.
5. **De-flake async-data-fetcher example** and fix the print_jest_summary suite
   line.
6. **Comment-aware mock scanning** (nice-to-have): strip comments before the
   ht.mock regex pass, or move scanning to OXC once (3) lands.

## The endgame: esbuild JS API + onResolve plugin (unify all mock delivery)

Every mock-delivery mechanism in the codebase — shadow trees, package shims,
absolute-path externals + iso bundles (Day 24) — is a workaround for one missing
primitive: a resolver hook in esbuild's CLI. The JS API has it. `onResolve` sees
every import WITH its importer's path and may answer "use this file instead".
With it, all three delivery mechanisms collapse into one rule:

    resolves to a mocked file? → serve its wrapper file
    otherwise                  → resolve normally

Key separation to preserve — the shadow-wrapper invention is TWO things:
1. **The wrapper file (the revolution, keep byte-for-byte)**: Proxy that checks
   `__HT_file_mocks[__currentTestFile]` at ACCESS time and falls back to
   `_getReal()`. Per-file isolation, mock-or-real decided at runtime, barrel
   sub-path delegation. This is validated by 1793 prod tests. Do not touch.
2. **The delivery trick (replace)**: shadow directory forests, alias re-pointing,
   symlink/copy heuristics, package-shim aliases, absolute externals + iso
   bundles. All of it exists only to smuggle (1) past a hookless resolver.

Division of labor after the port: `onResolve` = installer (bundle time, which
FILE sits at this import site — always the wrapper); `get()` = brain (run time,
which VALUE comes out — mock for the current test file, else real). Neither can
do the other's job: mock-or-real is per-running-test (unknowable at bundle time);
interception requires the wrapper to BE what the import resolves to (impossible
without a resolution-time hook).

### Implementation sketch
- Rust CLI (cache miss): write `.hermes-test-build.mjs` + a JSON config (entry,
  resolved mocked-file map, wrapper paths, native externals, aliases), spawn the
  JS runtime, read the bundle back. Pipeline downstream (patches → .hbc → Hermes)
  unchanged — tests still execute in Hermes; the JS runtime only drives bundling.
- Runtime selection: `bin/hermes-test.js` passes `process.execPath` down (env
  var) — reuse whatever Node/Bun is already executing the launcher. Zero new
  install requirement (a JS runtime is guaranteed present by construction; a JS
  runtime already boots on every run for the bin shim). Bun ≈ 10–20ms boot,
  Node ≈ 50–100ms, cold bundles only — warm runs read .hbc and never bundle.
- Wrapper's own import of the real module: mark it (query suffix or importer
  check) so onResolve passes it through — same role `@__ht_real*` plays today.
- Native-module externalization is NOT mock delivery — it stays (can't bundle
  Swift/Kotlin-backed packages regardless of resolver).

### Risks (respect the graveyard — Attempts 2 and 3 also looked obviously right)
- **Round-trip cost**: `filter: /.*/` pings Go→JS for every import (tens of
  thousands in Topdanmark). Build targeted filter regexes (mocked basenames +
  alias prefixes) so the Go side pre-screens and only candidates cross the pipe.
- **The Day 19 tail**: barrels, index resolution, destructuring-at-init patching
  must be re-proven under the new delivery, not assumed.
- **Patches still regex-match esbuild output**: bundle shape changes slightly
  (wrappers in-bundle instead of external stumps) — re-validate patterns.

### Migration plan (parity-gated, deletion last)
1. Implement behind `HT_RESOLVER=plugin`; legacy path untouched.
2. Parity gauntlet on BOTH modes: examples suite, if-session 31/31 under both
   mock spellings, Topdanmark 288/1793, coverage mode, watch mode.
3. Run dual-mode in CI for a period; compare counts and timing.
4. Only then delete: shadow trees, package shims, iso partition +
   run_isolated_relative_mock_files, multi-VM path. Deletion is the trophy at
   the end, not the move itself.

Payoff: one bundle, one VM always; three delivery subsystems deleted; relative /
alias / package mocks become one code path; Day 24's iso machinery retires after
serving as the correctness bridge.

### Phase 3 results (feat/plugin-resolver, measured)
- Plugin resolver is now the DEFAULT; `HT_RESOLVER=legacy` restores the old
  pipeline (escape hatch for one release cycle — do not delete legacy until a
  release has soaked in prod).
- Coverage: bundle_via_plugin_with_sourcemap mirrors the CLI sourcemap path;
  if-session and Topdanmark coverage runs green in default mode. NOTE: coverage
  totals differ from legacy because mocked modules' real code now stays in the
  instrumented bundle (legacy externalized/isolated them out of the
  denominator) — more files measured, arguably more honest.
- Watch: both initial run and reruns bundle via the plugin; verified with a
  mixed relative+alias watch session (initial + source-change rerun).
- Default-mode gauntlet: examples 24 suites parity, if-session 31/31
  (plugin-* cache), Topdanmark 1793/1793 ×3 stable; legacy flag verified
  (single-* cache, 1793/1793).
- Phase 4 (deletion of shadow trees, package shims, iso runner, and the
  HT_RESOLVER flag itself) is deliberately deferred until the flipped default
  has shipped in a release and soaked in Topdanmark CI.

## Operational guardrails for prod consumers (Topdanmark et al.)

- Pin esbuild and hermes versions; treat esbuild bumps as risky changes needing a
  full-suite validation run.
- Consider a CI guard asserting total test count doesn't drop unexpectedly between
  runs — the cheap defense against any future silent-skip regression.

### Phase 1 results (feat/plugin-resolver, measured)
- HT_RESOLVER=plugin ships behind a flag: relative mocks run in the SINGLE bundle
  / single VM via onResolve wrappers with real-module fallback. Parity gauntlet
  green: examples 20 suites (plugin == legacy), if-session 31/31 both spellings
  (warm 0.01s, one bundle), Topdanmark 288/1793 both modes.
- Zero-wrapper builds delegate to the CLI path (byte-identical) — suites without
  relative mocks pay nothing.
- Measured JS-API service overhead (HT_PLUGIN_FORCE=1, zero round trips,
  min-of-3 on Topdanmark): CLI 5.27s vs JS API 5.74s cold → **≈ +0.5s (~9%)**
  per wrapper-carrying cold bundle of that size. Warm runs: 0 (bytecode cache).
- Phase 2 decision input: unifying alias/package mocks onto the plugin would
  make big suites wrapper-carrying, i.e. buy "delete shadow trees + shims + iso"
  at ~+0.5s cold / 0 warm. Shave candidates before defaulting: stdout instead of
  outfile+read, persistent build service (watch), profile bun vs node service.

### Phase 2 results (feat/plugin-resolver, measured)
- Plugin mode now delivers ALL mock kinds through onResolve: relative + alias
  mocks as identity-keyed file wrappers, package mocks as pkg wrappers
  (?ht-real resolved via build.resolve with a pluginData re-entry guard);
  natives/shimmed/unresolvable keep legacy externalization. Shadow trees and
  package shims are fully bypassed in plugin mode. Barrel sub-path delegation
  falls out of identity matching for free (no special barrel Proxy needed).
- Fixtures: alias-mock, pkg-mock, relative-mock pairs (each with an unmocked
  sibling needing the real module) — green in BOTH modes, one bundle, one VM.
- Fixture-writing found + fixed a PRE-EXISTING legacy bug (repro on published
  1.1.6): mocking a CJS default-export package (moment) broke `default` access
  to the REAL module for non-mocking files. Fixed in the package-shim template
  and inherited by plugin wrappers (default = module itself when !__esModule).
- Topdanmark (288 suites / 1793 tests, ~70 alias mocks now wrapper-delivered):
  min-of-3 interleaved — legacy cold 5.04s, plugin cold 5.69s (**+0.65s ≈ 13%**,
  of which ~0.5s is JS-API service overhead and only ~0.15s is filtered
  round-trips); warm 1.04s == legacy. if-session 31/31; examples 24 suites
  parity (one pre-existing flake).
- ~~OPEN: 87-test "nondeterminism"~~ **RESOLVED — was never nondeterministic.**
  Post-mortem: (a) the "passing" verification runs used `2>/dev/null | grep -c
  FAIL`, discarding the stream carrying suite lines → false passes; (b) the
  bisect harness appended the victim file LAST, which genuinely changes the
  outcome. Reality: a DETERMINISTIC, order-dependent failure.
  Root cause: phase 2's identity matching intercepted EVERY import route to an
  alias-mocked module — including production-internal relative imports that
  legacy shadow trees never intercept (symlink bypass, Day 19). Modules doing
  module-level init-time reads (e.g. allPayments reading constants/environment)
  initialize once, under whichever test file triggered the first import — an
  alphabetically-earlier Claims test that MOCKS constants/environment — freezing
  mocked values into module state for every later test (gwUtil et al).
  Found by ordered bisect: 3 Claims tests + gwUtil = minimal repro.
  Fix: alias and package mocks now match by IMPORT-SPECIFIER TEXT (exact), with
  barrel-ancestor wrappers doing prefix delegation — the legacy boundary,
  faithfully ported (esbuild runs plugin onResolve BEFORE alias substitution,
  verified empirically, so the pre-substitution text is visible). Only phase-1
  relative-mock targets keep identity matching (that is their purpose).
  After fix: topdanmark 1793/1793 plugin mode, 3× stable, warm 1.08s.
  Lesson recorded: never count failures through a discarded stream; never bisect
  with a harness that changes test order.
