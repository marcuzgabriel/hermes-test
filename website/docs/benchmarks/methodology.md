---
title: Methodology
---

# Benchmark methodology

This page summarizes how benchmark results are produced so numbers stay reproducible.

## Environment

- Apple Silicon macOS machine
- Node 22.x, Bun 1.x
- Hermes from pinned source revision
- `hyperfine` used for timing runs

## Measurement principles

1. Compare the same test workload between runners
2. Include cold-start scenarios (`--no-cache` style comparisons where relevant)
3. Keep command-line options explicit and versioned
4. Report both absolute time and relative speedup

## Example commands

```bash
# hermes-test cold run
hyperfine --warmup 3 --runs 10 \
  '../../target/release/hermes-test run src/pure-50.test.ts --root .'

# jest baseline
hyperfine --warmup 1 --runs 10 \
  'bunx jest --config jest.config.swc.js --no-cache pure-50.test.ts'
```

## Interpreting results

- Small tests emphasize startup/runtime overhead differences.
- Large suites emphasize scalability and cache behavior.
- Watch-mode numbers reflect local development feedback loop speed.

## Full benchmark log

Detailed benchmark history and commands:

- [`BENCHMARKS.md`](https://github.com/marcuzgabriel/hermes-test/blob/main/BENCHMARKS.md)
- [Performance reference](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/performance.md)
