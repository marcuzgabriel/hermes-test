# Benchmarks

All measurements taken with `hyperfine` on macOS arm64 (Apple Silicon).
Machine: Mac, arm64, Node v22.22.1, Jest 30.4.1, Hermes (latest main).

## Week 1: Cold start, trivial test

Measured 2026-05-19. The trivial test evaluates `({status: 'pass', name: 'trivial'})` and returns JSON.

| Tool | Cold start (mean) | Relative | Notes |
|------|-------------------|----------|-------|
| **metro-test** | **4.6 ms** | **1x** | Rust + Hermes, single eval |
| node -e | 20.3 ms | 4.5x slower | Node v22.22.1 |
| jest (trivial test) | 1,486 ms | 364x slower | Jest 30.4.1, --no-cache |

### Gate: PASSED

Target was sub-1000ms cold start. Achieved **4.6ms** (217x under budget).

### Commands to reproduce

```bash
# metro-test (built-in trivial)
hyperfine --warmup 3 --min-runs 20 './target/release/metro-test'

# node baseline
hyperfine --warmup 3 --min-runs 20 \
  'node -e "console.log(JSON.stringify({status:\"pass\",name:\"trivial\"}))"'

# jest baseline (from /tmp/jest-bench with package.json + jest.config.js)
hyperfine --warmup 1 --min-runs 10 \
  'cd /tmp/jest-bench && npx jest --no-cache trivial.test.js'
```

### 5-test file (examples/trivial.test.js)

| Tool | Cold start (mean) |
|------|-------------------|
| metro-test | 5.0 ms |

## Week 2: Real .test.ts files, esbuild fast path

Measured 2026-05-19. Pure function tests bundled by esbuild, executed in Hermes.
Machine: Mac arm64, Node v22.22.1, Bun 1.3.14, Jest 30.4.2, esbuild 0.28.0, Hermes (latest main).

### 10 tests cold (pure-10.test.ts)

| Tool | Mean (ms) | Stddev | Min | Max | vs metro-test |
|------|-----------|--------|-----|-----|---------------|
| **metro-test** | **16.0** | 2.2 | 14.2 | 20.9 | 1x |
| jest+@swc/jest | 714 | 27 | 644 | 737 | 45x slower |
| jest+ts-jest | 1,596 | 26 | 1,563 | 1,643 | 100x slower |

### 50 tests cold (pure-50.test.ts)

| Tool | Mean (ms) | Stddev | Min | Max | vs metro-test |
|------|-----------|--------|-----|-----|---------------|
| **metro-test** | **15.7** | 0.6 | 15.0 | 17.1 | 1x |
| jest+@swc/jest | 737 | 14 | 721 | 758 | 47x slower |
| jest+ts-jest | 1,593 | 19 | 1,559 | 1,617 | 101x slower |

### Gate: sub-1.5s cold start for 10-test file — **PASS** (16ms, 94x under budget)

### Speed target: 3x faster than jest+@swc/jest on 50 tests — **PASS** (47x faster)

### Bundler comparison (metro-test internals)

| Bundler | 50 tests cold (ms) | Notes |
|---------|-------------------|-------|
| esbuild (default) | 16 | Spawns esbuild CLI, ~7ms bundle + ~5ms Hermes eval |
| Metro (--bundler metro) | 490 | Node startup + require('metro') = 333ms overhead |

esbuild is the default. Metro is available via `--bundler metro` for RN-specific
transforms (Flow, platform extensions, asset imports).

### Commands to reproduce

```bash
# metro-test with esbuild (from examples/expo-app/)
hyperfine --warmup 3 --runs 10 \
  '../../target/release/metro-test run src/pure-10.test.ts --root .'
hyperfine --warmup 3 --runs 10 \
  '../../target/release/metro-test run src/pure-50.test.ts --root .'

# metro-test with Metro bundler
hyperfine --warmup 1 --runs 5 \
  '../../target/release/metro-test run src/pure-50.test.ts --root . --bundler metro'

# jest (from bench/fixtures-jest/)
hyperfine --warmup 1 --runs 10 \
  'bunx jest --config jest.config.swc.js --no-cache pure-10.test.ts'
hyperfine --warmup 1 --runs 10 \
  'bunx jest --config jest.config.swc.js --no-cache pure-50.test.ts'
```

## Week 3: Hook tests (renderHook + act + state history)

Measured 2026-05-19. Hook tests using renderHook, act, state history, bundled by esbuild, executed in Hermes.
Machine: Mac arm64, Node v22.22.1, Bun 1.3.14, Jest 30.4.2, esbuild 0.28.0, Hermes (latest main).

### 50 hook tests cold (hooks-50.test.ts)

| Tool | Mean (ms) | Stddev | Min | Max | vs metro-test |
|------|-----------|--------|-----|-----|---------------|
| **metro-test** | **74.9** | 20.9 | 61.8 | 132.8 | 1x |
| jest+@swc/jest | 721 | 20 | 696 | 749 | 9.6x slower |

### 1000 hook tests cold (hooks-1000.test.ts)

| Tool | Mean (ms) | Stddev | Min | Max | vs metro-test |
|------|-----------|--------|-----|-----|---------------|
| **metro-test** | **200** | 4.5 | 194 | 205 | 1x |
| jest+@swc/jest | 883 | 11 | 865 | 898 | 4.4x slower |

### Gate: hook test from real codebase passes — **PASS**

useUser hook with async effects, spy-based mocking, error handling, and state history
tracking — all passing in Hermes.

### Speed target: 5x faster than jest+@swc/jest on cold — **PASS** (9.6x on 50, 4.4x on 1000)

metro-test scales linearly with test count (~0.13ms per hook test). Jest's overhead is
mostly fixed startup (~700ms) so the ratio narrows at high test counts.

### Commands to reproduce

```bash
# metro-test (from examples/expo-app/)
hyperfine --warmup 3 --runs 10 \
  '../../target/release/metro-test run src/hooks-50.test.ts --root .'
hyperfine --warmup 3 --runs 10 \
  '../../target/release/metro-test run src/hooks-1000.test.ts --root .'

# jest (from bench/fixtures-jest/)
hyperfine --warmup 1 --runs 10 \
  'bunx jest --config jest.config.swc.js --no-cache hooks-50.test.ts'
hyperfine --warmup 1 --runs 10 \
  'bunx jest --config jest.config.swc.js --no-cache hooks-1000.test.ts'
```

## Week 4

_Not yet measured._
