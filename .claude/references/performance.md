# Performance Profile & Optimization Opportunities

## Current Numbers (Day 21 — Topdanmark: 1472 tests, 259 files)

| Metric | Time |
|---|---|
| Cold run (wall clock) | **4.8s** |
| Cached run (wall clock) | **3.0s** |
| Cached run (internal Time) | **1.9s** |
| Jest same project (full config, coverage) | **116s** |
| Speedup vs Jest | **60x** |

## Time Breakdown: Cached Run (~3.0s wall clock)

| Phase | Time | % | Notes |
|---|---|---|---|
| Process startup | ~200ms | 7% | Rust binary load, CLI parsing |
| Config + auto-detect scan | ~50ms | 2% | Read config, scan node_modules for native dirs |
| Shadow wrappers + shims setup | ~100ms | 3% | Create filesystem dirs, generate proxy wrappers |
| Cache hit check | ~10ms | <1% | Hash comparison |
| Hermes eval (harness) | ~100ms | 3% | 390KB harness bundle |
| **Hermes eval (test bundle)** | **~800ms** | **27%** | **8.7MB JS bundle parsed + executed** |
| **Test execution** | **~600ms** | **20%** | 1472 tests (hooks, renderHook, act, etc.) |
| Hermes class-extends patches | ~200ms | 7% | Regex post-processing on 8.7MB |
| Result collection + output | ~100ms | 3% | JSON parsing, terminal output |
| **Process/OS overhead** | **~800ms** | **27%** | Gap between internal Time and wall clock |

## Time Breakdown: Cold Run Adds (~1.8s extra)

| Phase | Time | Notes |
|---|---|---|
| **esbuild bundling** | **~1.5s** | 259 files → single 8.7MB bundle |
| Hermes compat patching | ~300ms | `fix_all_class_extends`, `patch_esbuild_for_hermes` |
| Cache write | ~50ms | Write 8.7MB to disk |

## Bundle Stats

| Metric | Value |
|---|---|
| Bundle size | 8.7MB (185K lines) |
| Harness size | 390KB |
| Test files | 259 |
| Tests | 1472 |

## Top 3 Bottlenecks

### 1. Hermes parsing 8.7MB JS (~800ms, 27%)
The single biggest cost. Hermes parses the entire JS bundle from text on every cached run.
**Why**: bytecode compilation is only used when the bundle contains `= class ` syntax, and even then only for split-mode vendor bundles. The single-bundle path doesn't cache bytecode.

### 2. Process/OS overhead (~800ms, 27%)
Gap between internal `Time:` (1.9s) and wall clock (3.0s). Includes shell startup, Rust process init, filesystem operations (shadow wrapper creation/cleanup), and output buffering.
**Why**: every `hermes-test` invocation is a fresh process. Watch mode eliminates this.

### 3. Test execution (~600ms, 20%)
Actual test logic: React renderHook, act(), async resolution, spy tracking, expect assertions.
**Why**: React reconciliation is inherently not free. 1472 tests × ~0.4ms average.

## Optimization Opportunities

### High Impact

#### 1. Bytecode caching for single-bundle (estimated: -500ms)
- **Current**: single-bundle path caches raw JS text, re-parses on every run
- **Fix**: compile to Hermes bytecode (.hbc) after patching, cache the bytecode
- **Impact**: Hermes bytecode loads 2-3x faster than JS text parsing
- **Effort**: Medium — `compile_to_bytecode_cached` already exists for split-mode, reuse for single-bundle
- **Risk**: Low — bytecode is deterministic from same JS source

#### 2. Cache patched bundle, not raw esbuild output (estimated: -200ms)
- **Current**: cache stores raw esbuild output, re-applies `fix_all_class_extends` + `patch_esbuild_for_hermes` on every cached run
- **Fix**: cache the FINAL patched output (post class-extends fix, post Hermes patches)
- **Impact**: eliminates ~200ms regex processing on 8.7MB
- **Effort**: Low — just move the cache write after patching instead of before
- **Risk**: None

#### 3. Skip shadow wrapper filesystem ops when cached (estimated: -100ms)
- **Current**: creates shadow dirs + shim dirs on every run, even when bundle is cached
- **Fix**: skip filesystem setup when cache hit, shadow wrappers aren't needed for eval
- **Impact**: ~100ms saved from dir creation/cleanup
- **Effort**: Low — conditional around create_shadow_wrappers/create_package_shims
- **Risk**: Low — cached bundle already has the aliases baked in

### Medium Impact

#### 4. esbuild `--minify=syntax` (estimated: -100ms eval, -300ms cold)
- **Current**: no minification (needed for Hermes compat patches to match patterns)
- **Fix**: apply minify AFTER patches, or use syntax-only minification
- **Impact**: smaller bundle → faster parse, faster esbuild
- **Effort**: Medium — need to verify patches still work after syntax minification
- **Risk**: Medium — some patches rely on specific code patterns

#### 5. Parallel test execution (estimated: -300ms)
- **Current**: all 1472 tests run sequentially in one Hermes runtime
- **Fix**: split tests across 2-4 Hermes runtimes running in parallel threads
- **Impact**: test execution phase (600ms) could halve
- **Effort**: High — requires thread-safe test isolation, result merging
- **Risk**: High — shared global state, mock contamination between threads

#### 6. Incremental bundling in watch mode (estimated: -1s on rerun)
- **Current**: watch mode re-bundles affected files
- **Fix**: use esbuild's incremental API to avoid re-resolving unchanged deps
- **Impact**: watch reruns could drop from ~1s to ~200ms
- **Effort**: Medium — esbuild supports `--watch` and incremental builds
- **Risk**: Low

### Low Impact / Nice-to-have

#### 7. Tree-shake unused test dependencies
- Remove dead code from bundle (unused RTK endpoints, unimported helpers)
- Impact: smaller bundle, faster parse
- esbuild already tree-shakes, but test files may pull in large transitive deps

#### 8. Harness bytecode
- Pre-compile harness to bytecode at build time
- Impact: ~50ms saved (harness is only 390KB)
- Low effort but small return

#### 9. Lazy React reconciler creation
- Currently creates a new React reconciler per `renderHook` call
- Could pool/reuse reconcilers
- Impact: small, depends on test patterns

## Watch Mode Performance

Watch mode eliminates process startup and full re-bundling:

| Phase | One-shot | Watch (rerun) |
|---|---|---|
| Process startup | 200ms | 0ms (persistent) |
| Config/scan | 50ms | 0ms (cached) |
| Bundle | 1500ms (cold) / 0ms (cached) | ~200ms (incremental) |
| Hermes eval | 800ms | ~100ms (only changed tests) |
| Test execution | 600ms | ~50ms (only affected tests) |
| **Total** | **3.0s** | **~350ms** |

Watch mode is already the biggest performance win for developer workflow.

## Theoretical Minimum

With all optimizations applied:

| Scenario | Estimated time |
|---|---|
| Current cached | 1.9s (internal) / 3.0s (wall) |
| + bytecode caching | 1.4s / 2.5s |
| + cache patched bundle | 1.2s / 2.3s |
| + skip shadow setup on cache hit | 1.1s / 2.2s |
| + syntax minification | 1.0s / 2.1s |
| Watch mode rerun | ~350ms |
