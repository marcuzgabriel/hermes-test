# V8 Evaluation Summary (Topdanmark)

## Goal
Evaluate whether V8 should replace Hermes as the primary `hermes-test` runtime for the Topdanmark app, with focus on speed, setup complexity, and maintenance.

## Scope
- App: `~/Documents/mobile-insurance-app-expo/apps/topdanmark`
- Size: 288 suites, 1787 tests, 7 snapshots
- Modes evaluated: full run, coverage, watch

## Key results

| Scenario | Hermes | V8 | Outcome |
|---|---:|---:|---|
| Full run (no coverage) | ~3–5s | ~3s in best observed local run | Comparable in best case |
| Coverage run | ~7–8s | ~20–26s | Hermes clearly faster |
| Watch mode | Stable baseline | Improved, but still experimental/more fragile | Hermes better DX |

## Why Hermes won overall
1. **Bytecode-first execution path** is highly optimized in current architecture.
2. **Caching model** is tuned for Hermes (`.hbc` path + patched bundle path).
3. **Coverage architecture** is expensive on V8 today (large instrumented JS + heavier eval/collection cost), while Hermes path remains much faster for this workload.
4. **Operational maturity**: Hermes path is simpler for team usage and has lower ongoing maintenance burden.

## V8 findings worth keeping
1. V8 single-runtime hangs were addressed via output capture fix.
2. V8 full-suite execution can be fast in non-coverage mode.
3. V8 remains strategically useful for interop/engine experimentation.

## Decision guidance
- **Default recommendation:** keep Hermes as primary engine.
- **V8 status:** experimental/optional path for research or specific compatibility needs, not the default team workflow.
