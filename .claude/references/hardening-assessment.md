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
- **RESOLVED (July 2026, feat/hermes-v1-engine / PR #4): the mini-transpiler is
  DELETED.** Vendored engine moved to the Hermes V1/static_h line
  (hermes-v250829098.0.14, RN 0.84+), probe-verified to handle classes natively
  (TDZ, new.target, super.method, native-builtin subclassing). ~400 lines gone;
  what remains of patches.rs is small mocking-feature rewrites (configurable
  getters, __toESM passthrough, require shim, hoist) — engine-independent by
  design and each covered by fixtures. The esbuild-pin guardrail still applies
  to the remaining text matches; the fixture corpus is the tripwire.

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
2. **Fixture tests for patches.rs transforms**: DONE (test/fixture-corpus, July
   2026). crates/hermes-test-cli/tests/fixtures/ — 11 fixtures across 4 groups
   (class-extends, esbuild-helpers, mock-require-shim, hoist-mocks), each
   input.js/expected.js golden pair PLUS behavior.js executed in real Hermes
   via the embedded Runtime (text-match alone missed a blessed-broken-output
   case in mutation testing; the Hermes layer caught it). Bless mode:
   HT_UPDATE_FIXTURES=1. See tests/fixtures/README.md for the per-fixture map.
   Writing the corpus found and fixed THREE production bugs in one day:
   (a) pre-bundled CJS deps (react-redux) inline their own `2`-suffixed esbuild
   helpers — patches 1-3 were first-occurrence-only, so nested copies shipped
   unpatched (non-configurable getters, Proxy-destroying __toESM2);
   (b) super.method() in methods rebound `this` to the prototype;
   (c) super.method() in constructors wasn't rewritten at all → Hermes compile
   error. Engine facts documented in fixtures: Hermes ignores Symbol.species
   in Array methods; the raw for-let-of closure bug no longer reproduces in
   current vendored Hermes (nested-helper patches matter for mockability).
   Also: src/cli_tests.rs unit-tests the never-run-zero-tests invariant from
   item 1 (try_expand_file_args) + result parsing + summary formatting, and
   CI runs `cargo test -p hermes-test-cli`.
   MERGE GATE: Topdanmark parity run before test/fixture-corpus → main
   (patches now modify react-redux internals and class output).
3. **Class downleveling: do NOT re-attempt an AST port.** MOOT since the V1
   engine (July 2026, PR #4): the downleveler itself was deleted — classes are
   engine-native, so there is nothing left to port. The paragraph below is
   preserved as the decision record for the remaining regex patches: full-AST
   re-emit is still inadmissible for THEM, for the same whitespace reasons.
   Originally SETTLED (July 2026, maintainer decision). OXC/AST approaches were tried multiple times and failed;
   the documented reasons (SWC: scoped thread-locals, helper injection incompatible
   with Hermes; and for ANY full-AST tool: re-emitting the bundle changes
   whitespace and breaks every other regex patch) apply to OXC as well — this is
   a property of full re-emit, not of a specific library. The regex transform in
   patches.rs stays. Hardening therefore comes from item (2): golden fixture tests
   over the class-extends shapes, executed in Hermes, plus a pinned esbuild
   version. If an AST approach is ever revisited, the ONLY admissible shape is
   span-surgery (parse, replace exactly the class-extends byte ranges in the
   original text, no re-emit) — and it needs a champion with time to burn, not a
   drive-by refactor.
4. **Prune dead code**: DONE (feat/plugin-resolver) — deleted 771 lines: split
   mode (bundle_split*, SplitBundle, group/vendor entry generators, split
   caching), run_tests_split, run_persistent_cycle, orphaned print helpers,
   compile_to_bytecode_cached, coverage map builders. Zero rustc warnings from
   crate code after prune; full gauntlet green in both resolver modes.
   NOT deleted (deliberate): the legacy resolver path (shadow trees, package
   shims, iso runner) behind HT_RESOLVER=legacy — phase 4, after release soak.
5. **De-flake async-data-fetcher example** (still open — fails near-every run
   now, noise in exactly the validation signal). print_jest_summary suite line:
   FIXED (test/fixture-corpus) — count_failed_suites() from results JSON, iso
   runner returns failed-suite count, honest "N passed, M failed, T total".
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
   **PHASE 4 DONE (July 2026, feat/delete-legacy-resolver, stacked on the V1
   engine PR): −956 lines net.** Deleted: shadow trees + package shims
   (shadow.rs 534→134 lines, renamed shims.rs — only config-shim helpers
   remain), run_isolated_relative_mock_files, run_tests_per_file, the iso
   partition in run/watch, the HT_RESOLVER and HT_PER_FILE flags, and the
   now-unreachable CLI bundling entry points (bundle_auto*,
   bundle_esbuild_with_sourcemap, compile_to_bytecode). The onResolve plugin
   is the only resolver. main.rs: 1478→1129 lines. Gate held: plugin default
   soaked in prod (1.2.0/1.2.1 in Topdanmark CI for weeks). Gauntlet: 16 Rust
   tests, examples 24 suites, watch smoke, Topdanmark 291/1812 cold 7.61s /
   warm 0.86s / coverage 74.8%.

Payoff: one bundle, one VM always; three delivery subsystems deleted; relative /
alias / package mocks become one code path; Day 24's iso machinery retires after
serving as the correctness bridge.

### Phase 3 results (feat/plugin-resolver, measured)
- Plugin resolver is now the DEFAULT; `HT_RESOLVER=legacy` restores the old
  pipeline (escape hatch for one release cycle — do not delete legacy until a
  release has soaked in prod).
- Coverage: bundle_via_plugin_with_sourcemap mirrors the CLI sourcemap path;
  if-session and Topdanmark coverage runs green in default mode.
- COVERAGE BUG (found in prod by Topdanmark's 65% threshold, fixed in 1.2.1):
  the plugin outfile lived in the temp dir, so esbuild emitted sourcemap
  `sources` relative to /tmp — every project file became "../..."-relative
  (excluded by coverage's ..-filter → 321 files silently dropped) while the
  wrapper files became bare relative paths (wrongly INCLUDED: dozens of
  identical 45.0%/42.9% template entries). Total read 56.9% vs threshold 65.
  Fix: write the plugin outfile in the PROJECT ROOT (sources become
  project-relative like the CLI path) + explicit coverage exclusion of
  plugin-wrappers/ and hermes-test-work- paths. After fix: 573 files measured
  (85 MORE than legacy's 488 — legacy silently hid mocked modules' real code
  behind /tmp shadow-tree paths), line total 74.4%. Function/branch totals read
  lower than legacy (32% vs 52%) for the same reason: the newly visible mocked
  modules are lightly exercised. Plugin numbers are the honest ones.
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
