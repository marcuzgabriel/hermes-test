# Performance Profile & Optimization Opportunities

## Current Numbers (Day 21 — Topdanmark: 1472 tests, 259 files)

| Metric | Before optimizations | After optimizations |
|---|---|---|
| Cold run (wall clock) | 4.8s | **4.0s** |
| Cached run (wall clock) | 3.0s | **1.9s** |
| Cached run (internal Time) | 1.93s | **0.79s** |
| Jest same project (full config, coverage) | 116s | 116s |
| **Speedup vs Jest** | 60x | **147x** |

### Three-tier cache system
1. **Bytecode (.hbc)** — fastest, Hermes loads pre-compiled bytecode directly
2. **Patched JS (.js)** — cached AFTER class-extends/Hermes patches applied
3. **Fresh bundle** — esbuild + patch + cache on first run

## Optimizations Applied

### 1. Bytecode caching (saved ~1.1s internal)
- Hermes compiles JS to bytecode (.hbc) on first cached run
- Subsequent runs load bytecode directly — skips JS parsing entirely
- 8.7MB JS → 7.7MB .hbc, loads 2x+ faster

### 2. Cache patched bundle (saved ~200ms)
- Previously: cached raw esbuild output, re-applied regex patches every run
- Now: caches the FINAL patched output, no re-patching on cache hits

### 3. Skip shadow wrapper setup on cache hit (saved ~100ms)
- Previously: created shadow dirs + shim dirs on every run, even cached
- Now: skips all filesystem setup when bytecode or JS cache exists

## Time Breakdown: Cached Run (bytecode hit, ~1.9s wall clock)

| Phase | Time | % | Notes |
|---|---|---|---|
| Process startup | ~200ms | 11% | Rust binary load, CLI parsing |
| Config + auto-detect scan | ~50ms | 3% | Read config, scan node_modules |
| Cache hit check | ~10ms | <1% | Hash comparison, bytecode exists check |
| Hermes eval (harness) | ~100ms | 5% | 390KB harness |
| **Hermes eval (bytecode)** | **~300ms** | **16%** | 7.7MB .hbc loaded directly |
| **Test execution** | **~600ms** | **32%** | 1472 tests running |
| Result collection + output | ~100ms | 5% | JSON parsing, terminal output |
| **Process/OS overhead** | **~500ms** | **26%** | Shell, OS, stdout buffering |

## Time Breakdown: Cold Run (~4.0s wall clock)

| Phase | Time | Notes |
|---|---|---|
| Config + auto-detect | ~50ms | Read config, scan node_modules |
| Shadow wrappers + shims | ~100ms | Create filesystem dirs |
| **esbuild bundling** | **~1.5s** | 259 files → single 8.7MB bundle |
| Hermes compat patching | ~300ms | Regex transforms on 8.7MB |
| Cache write (JS + bytecode) | ~100ms | Write both .js and .hbc |
| Hermes eval (bytecode) | ~300ms | First eval uses freshly compiled bytecode |
| Test execution | ~600ms | 1472 tests |
| Cleanup + output | ~200ms | Remove temp dirs, print results |

## Bundle Stats

| Metric | Value |
|---|---|
| JS bundle | 8.7MB (185K lines) |
| Bytecode (.hbc) | 7.7MB |
| Harness | 390KB |
| Test files | 259 |
| Tests | 1472 |

## Remaining Opportunities

### Medium Impact

#### 1. esbuild `--minify=syntax` (estimated: -50ms eval, -200ms cold)
- Smaller bundle → faster bytecode compilation and loading
- Must apply AFTER Hermes compat patches (patches match unminified patterns)
- Risk: medium — verify patches still work

#### 2. Parallel test execution (estimated: -300ms)
- Split tests across 2-4 Hermes runtimes in parallel threads
- Test execution (600ms) could halve
- Risk: high — shared global state, mock contamination

#### 3. Incremental bundling in watch mode (estimated: -1s rerun)
- Use esbuild incremental API for watch re-bundles
- Risk: low

#### 4. Harness bytecode (estimated: -50ms)
- Pre-compile harness to bytecode at build time
- Small return (harness is only 390KB)

### Low Impact

#### 5. Tree-shake unused test dependencies
- esbuild already does this, marginal gains

#### 6. Lazy React reconciler pooling
- Reuse reconcilers across renderHook calls
- Small impact, depends on test patterns

## Watch Mode Performance

| Phase | One-shot | Watch (rerun) |
|---|---|---|
| Process startup | 200ms | 0ms (persistent) |
| Config/scan | 50ms | 0ms (cached) |
| Bundle | 1500ms (cold) / 0ms (cached) | ~200ms (incremental) |
| Hermes eval | 300ms | ~50ms (only changed tests) |
| Test execution | 600ms | ~50ms (only affected tests) |
| **Total** | **1.9s** | **~300ms** |

## Historical Progress

| Date | Tests | Internal Time | Wall Clock | vs Jest |
|---|---|---|---|---|
| Day 20 | 1409/1456 | ~1.69s | ~3.0s | 13.8x (wrong: `--no-coverage`) |
| Day 21 (pre-opt) | 1472/1472 | 1.93s | 3.0s | 60x |
| Day 21 (post-opt) | 1472/1472 | **0.79s** | **1.9s** | **147x** |
