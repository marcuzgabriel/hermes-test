# Rolldown Evaluation Summary (July 2026)

## Goal
Evaluate whether rolldown (in-process Rust crate) should replace esbuild as the hermes-test bundler, with focus on speed, execution parity, and overhead. Mandate from the user: "using rolldown should be a luxury with less overhead, not a headache."

## Scope
- Branch: `feat/rolldown-bundler` (parked at commit `c9a4399`, never merged)
- Gauntlet: TDD fixtures → examples app (24 suites / 1211 tests) → Topdanmark (291 suites / 1812 tests)
- Approach: pure-rolldown branch — esbuild, `plugin_build.cjs`, and all output patches deleted; receptionist ported to an in-process `resolve_id` hook

## Key results

| Metric | esbuild | rolldown | Outcome |
|---|---:|---:|---|
| Cold bundle (examples) | 177.6ms | 46.5ms | rolldown 3.8x faster |
| Bundle size | 1646 KB | 1383 KB | rolldown 16% smaller |
| Examples parity | 1211/1211 | 1211/1211 | equal |
| Topdanmark parity | 1812/1812 | 1800/1812 (99.3%), walls remaining | **esbuild wins** |
| Machinery required | 4 regex patches + Node plugin | liveness proxies, virtual ctx modules, mock-preamble hoisting, hand-written JS statement parsing in Rust | **esbuild wins decisively** |

## Why esbuild won
1. **The mock architecture depends on a lazy-init window.** `ht.mock()` calls are runtime code in the test body. esbuild's lazy CJS module init lets registration run before the mocked module evaluates (with `hoist_mock_modules` sequencing within the module). Rollup-dialect scope hoisting evaluates ALL imports eagerly, in topological order, before any body code — the window does not exist.
2. **The receptionist ported fine; the timing did not.** Routing (`resolve_id`) and access-time resolution (wrapper "brain") both worked in rolldown. Values *captured during module evaluation* (singletons like `export default new FirebaseAnalyticsTracker()`, init-time destructuring) were computed before mocks existed.
3. **Every fix escalated in kind, not degree.** Reaching 99.3% required: wrapper liveness proxies, CJS-default external-proxy semantics, per-path recursion guards, `__htIsLiveProxy` branding, virtual `ht-ctx:` first-import context modules, and dependency-driven mock-preamble hoisting built on a hand-rolled JS statement scanner. The remaining failures (let/var reassignment across the ctx boundary → `ASSIGN_TO_IMPORT`) demanded still more parsing.
4. **The prize did not cover the cost.** ~130ms cold-bundle savings on a tool whose cached run is 0.84s and watch rerun ~350ms.

## Findings worth keeping
1. Rolldown's in-process plugin API is excellent — no Node spawn, no JSON round-trips; the resolver hook is cleaner than the esbuild plugin process.
2. The Vite ecosystem agrees implicitly: rolldown drives Vite *production builds*; Vitest executes tests through a lazy module runner (SSR-transformed imports, per-module init) — never scope-hoisted bundles.
3. If ever resumed, the correct blueprint is: `oxc_parser` + `oxc_semantic` (already in the dep tree) for AST-based hoisting with real binding resolution, plus Vitest-style rewriting of reassignments into setter calls. That fixes the parser-shaped hacks but NOT the ctx-module machinery or the eager-evaluation model.
4. Six reusable TDD fixtures live in the branch (`wrapper_capture_tests.rs`): init-time capture, POJO liveness, mocked-external roundtrip, spyOn recursion, self-referential mocks, module-load singletons. They are mechanism specs for ANY future bundler/engine evaluation.
5. Process lesson (user feedback): under a "less overhead" mandate, stop and surface the verdict at the first qualitative escalation in fix complexity (e.g. needing a parser) — not at a failure-count plateau.

## Decision guidance
- **Default recommendation:** esbuild stays. Verified post-evaluation: Topdanmark 1812/1812 in 5.68s on main.
- **Rolldown status:** rejected for this architecture. Do not re-attempt without a fundamentally different mock delivery design (lazy module runner à la Vitest, or first-class mock hoisting in the bundler).
