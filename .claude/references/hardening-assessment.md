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

1. **Fix directory-arg silent zero-run** (small, do before next release): expand
   directory positional args to their contained test files (reuse find_test_files
   scoped to the dir), or error loudly when an arg is a directory. Never exit 0
   having run nothing the user pointed at.
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
4. **Prune dead code**: split mode, run_persistent_cycle, and the never-used
   esbuild.rs functions. Keep the history in git, not in the source tree.
5. **De-flake async-data-fetcher example** and fix the print_jest_summary suite
   line.
6. **Comment-aware mock scanning** (nice-to-have): strip comments before the
   ht.mock regex pass, or move scanning to OXC once (3) lands.

## Operational guardrails for prod consumers (Topdanmark et al.)

- Pin esbuild and hermes versions; treat esbuild bumps as risky changes needing a
  full-suite validation run.
- Consider a CI guard asserting total test count doesn't drop unexpectedly between
  runs — the cheap defense against any future silent-skip regression.
