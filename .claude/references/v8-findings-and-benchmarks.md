# V8 Findings & Benchmarks (Topdanmark)

## Scope
- Project: `~/Documents/mobile-insurance-app-expo/apps/topdanmark`
- Runner: `hermes-test-cli` on branch `feat/v8-engine`
- Goal: verify V8 viability (single runtime), performance, and maintenance tradeoffs vs Hermes.

## What we confirmed
1. **Single-runtime V8 no longer hard-hangs** after fixing V8 stderr/result capture deadlock.
2. **Dual-runtime determinism works** via `globalThis.__HT_ENGINE` boundaries (no cross-engine mock contamination).
3. Full suite has passed on both engines in non-watch mode (`1787/1787`).
4. V8 `--coverage` previously returned no data; this is now fixed in CLI coverage collection.

## Current benchmark snapshots

| Scenario | Engine | Result | Time |
|---|---|---|---|
| Full suite (non-coverage) | V8 | Pass (historically `1787/1787`) | ~fast (single-digit seconds in prior runs) |
| Full suite (non-coverage) | Hermes | Pass (`1787/1787`) | ~3–5s baseline (team-observed) |
| Full suite with `--coverage` | V8 | Pass (`288/288`, `1787/1787`) + LCOV/HTML | **~20–26s** observed |
| Full suite with `--coverage` | Hermes | Team baseline | **~7–8s** observed |

Notes:
- One captured V8 coverage run: `20.58s`.
- Latest local V8 coverage verification run after fix: `26.34s` with artifacts generated.
- V8 watch startup no longer crashes from split/shadow alias resolution, but initial watch cycle is not fully green yet.

## Why V8 coverage is currently much slower
1. Coverage path forces fresh source-map bundling and skips non-coverage fast paths.
2. Post-bundle instrumentation inflates bundle size significantly.
3. V8 evaluates a very large instrumented JS text bundle.
4. End-of-run coverage extraction + LCOV/HTML generation adds heavy post-processing cost.

This points to a **coverage architecture cost** (for V8 path), not runtime-per-file spawning.

## Architecture conclusion
- For current `hermes-test` implementation, **Hermes is the speed winner**, especially for coverage.
- V8 remains valuable for cross-platform/runtime reasons (notably `Intl` and broader V8 ecosystem), but coverage performance is currently a major gap.
- Net assessment: V8 brings strategic interop benefits, but today carries meaningful runtime/host overhead and tuning effort versus Hermes.

## Recommended V8 path forward
1. Implement a **V8-native coverage path** (precise coverage ranges) instead of injected Istanbul counters for V8.
2. Reduce giant end-of-run JSON serialization.
3. Keep report compatibility (LCOV/HTML) as a translation layer from native coverage data.

Expected effort: **~1–3 weeks** for production-quality parity and stability.

## Non-contamination rule (important)
- Keep shared architecture and behavior consistent.
- Add engine-specific logic only at explicit compatibility boundaries.
- Do not let V8 fixes regress Hermes baseline performance/reliability.

## Packaging strategy (recommended)
Goal: keep `hermes-test` default install lean and Hermes-first, with V8 as opt-in.

1. Publish Hermes binaries as the default platform packages used by `hermes-test`.
2. Publish V8 binaries as separate optional packages (for example `@hermes-test-v8/<platform>`).
3. In `bin/hermes-test.js`, resolve package by `(engine, platform, arch)`:
   - default/no flag -> Hermes package
   - `--engine v8` -> V8 package
4. If `--engine v8` is requested but V8 package is missing, print install hint and exit.
5. Build binaries per engine feature set so default installs do not pull V8 runtime payload unless explicitly requested.
