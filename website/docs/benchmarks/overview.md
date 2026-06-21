---
title: Benchmark overview
---

# Benchmark overview

These numbers are measured on a production-scale React Native Expo app:

- **284 suites**
- **1766 tests**
- **7 snapshots**

## Headline results

| Scenario | hermes-test | Jest | Speedup |
|---|---:|---:|---:|
| Full suite | 5s | 116s | **23x** |
| Cached run | 0.84s | 54s | **64x** |
| With coverage | 5s | 128s | **26x** |
| Watch rerun | ~350ms | — | — |

## Micro benchmarks

| Scenario | hermes-test | Jest + @swc/jest | Speedup |
|---|---:|---:|---:|
| Trivial cold start | **4.6ms** | 1486ms | **364x** |
| 10 pure tests (cold) | **16ms** | 714ms | **45x** |
| 50 hook tests (cold) | **75ms** | 721ms | **9.6x** |
| 1000 hook tests (cold) | **200ms** | 883ms | **4.4x** |

## Why this is faster

1. Single-process runtime (no Jest worker farm overhead)
2. esbuild bundling with very low startup cost
3. Hermes execution path aligned with RN runtime
4. Cache strategy optimized for repeated local runs

## Source of truth

For detailed raw runs, command lines, and week-by-week tracking, see:

- [`BENCHMARKS.md`](https://github.com/marcuzgabriel/hermes-test/blob/main/BENCHMARKS.md)
