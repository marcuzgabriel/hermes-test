# Decisions

Settled. Don't relitigate without explicit user approval. If a future task seems to require revisiting one of these, surface it and ask first.

## D-001: Hermes, not JSC or V8 or Node

**Decided**: 2026-05
**Rationale**: Engine fidelity is the unique value proposition. Tests must run in the engine the user's app runs in. JSC is close but not identical; V8 is the wrong engine; Node is the problem we're solving.
**Implication**: All host-environment differences from Node (no `process`, no `fs`, no `Buffer`) are accepted as features, not bugs.

## D-002: Rust for the host

**Decided**: 2026-05
**Rationale**: Cross-platform CLI, first-class C++ interop, single static binary distribution, mature async ecosystem.
**Rejected alternatives**:
- Swift: weak Linux C++ interop, macOS-centric ecosystem
- Node: recursive feel, slower than Rust for the orchestration layer
- C++: build-system pain, slower iteration
- Go: cgo overhead per FFI call would dominate
- Zig: smaller ecosystem, more from scratch

## D-003: Metro, not a custom bundler

**Decided**: 2026-05
**Rationale**: Users' apps are already configured for Metro. Reusing it means zero config drift, automatic support for `.ios.ts`/`.android.ts`, and inheriting Metro's RN-specific module resolution.
**Implication**: We are coupled to Metro's programmatic API stability. Pin Metro version; expect occasional adaptation on RN upgrades.

## D-004: Not Jest-compatible

**Decided**: 2026-05
**Rationale**: Jest's API has compounding tech debt (hoisting, globals, `expect.extend` typing, loose types). AI-era migration is cheap (codemods). Building fresh API is a one-time cost; preserving Jest compat is forever.
**Implication**: Some adoption friction. Mitigation: airtight codemod that makes migration a single command.

## D-005: No globals, all imports

**Decided**: 2026-05
**Rationale**: Globals are why AI generates broken Jest tests. Explicit imports are visible to type-checkers, refactorable, and unambiguous.
**Implication**: Test files have an import line. This is fine. Modern tooling expects it.

## D-006: `useMock` is a value, not a hoisted side effect

**Decided**: 2026-05
**Rationale**: `jest.mock` hoisting is a Babel plugin that does invisible AST surgery. It confuses humans and breaks AI generation. Returning the mock as a value from `useMock()` in the test body is explicit, typed, and makes mock state local to the test.
**Implication**: No `babel-plugin-jest-hoist` port needed. No `__mocks__/` folder. Mocks don't leak across tests.

## D-007: AI is write-time only, never runtime

**Decided**: 2026-05
**Rationale**: LLM in the runtime loop breaks determinism, adds 100-5000ms per test, costs money per CI run. The benefits (intent-based testing) can be captured at authoring time via codegen that produces regular `.test.ts` files.
**Implication**: `hermes-test-ai` is a separate npm package with separate binaries. It writes test files. It does not execute them. The runner has no network access during test execution.

## D-008: State history as a first-class concept

**Decided**: 2026-05
**Rationale**: Jest's hook tests require manual state capture via arrays + `useEffect` hacks. Making history a first-class return value of `renderHook` enables assertion patterns Jest can't express. Differentiated feature, low implementation cost.
**Implication**: `renderHook` returns `{ result, history, renderCount, rerender, unmount }`. The history captures every render's `result.current` value.

## D-009: Components deferred to v1

**Decided**: 2026-05
**Rationale**: Component testing in a mock environment is a weak middle ground — neither real UX (E2E does that) nor pure logic (hooks do that). Cutting components from v0 reduces scope by ~70% and avoids RNTL compatibility quagmire.
**Implication**: v0 doesn't ship `render`, `screen`, `byText`, `fire.press`, etc. Users testing components stay on Jest until v1.

## D-010: One Hermes runtime per thread, not per process

**Decided**: 2026-05
**Rationale**: Jest's worker-per-process model has 200ms+ fork overhead per worker. JSI runtimes are not thread-safe but are cheap to create (~5ms). Spawning N runtimes in one process scales better.
**Implication**: Tests within a file share a runtime (faster). Test isolation between files is automatic (separate runtimes). Test isolation within a file is opt-in via `// @isolate` directive.

## D-011: JSON-batched bridge protocol

**Decided**: 2026-05
**Rationale**: Each Rust↔Hermes FFI crossing has fixed overhead. One stringify per test file is much faster than walking JSI values per assertion. Counterintuitive but well-established.
**Implication**: Results are stringified at the end of each test file. No streaming per-assertion. Live progress comes from Rust's reporter watching stdout, not from per-test FFI calls.

## D-012: Speed targets are gates, not aspirations

**Decided**: 2026-05
**Rationale**: A test runner that isn't faster than Jest+SWC has no reason to exist. Speed targets in the roadmap are go/no-go.
**Implication**: Week 1 must hit sub-1s cold start on trivial test, or architecture replans. Week 3 must hit sub-200ms watch rerun, or watch-mode design replans. Misses by 2x or more stop the project until resolved.

## D-013: TypeScript-first user API

**Decided**: 2026-05
**Rationale**: RN ecosystem is overwhelmingly TS in 2026. Sharp generic types are a feature, not a chore. Plain JS users still work but get less help.
**Implication**: Type definitions ship with the npm package. `spy<F>()` is generic over the function signature. `useMock<T>()` is generic over the module's type.

## D-014: Sharable Hermes bytecode cache

**Decided**: 2026-05
**Rationale**: The harness JS doesn't change between test runs. Compiling to bytecode once and caching to disk saves ~50-150ms per cold run.
**Implication**: `.hermes-test-cache/` directory at repo root, configurable. Cache key is hash of harness version + Metro bundle hash.

## D-015: Distribute as a single npm package + Rust binary

**Decided**: 2026-05
**Rationale**: Users install one thing: `npm install -D hermes-test`. The npm package downloads the appropriate prebuilt binary on `postinstall` (like esbuild does).
**Implication**: We ship prebuilt binaries for macOS arm64/x64, Linux x64/arm64, Windows x64 via GitHub Releases. CI builds them.

## D-016: Metro is a load-bearing dependency — accept the risk, design around it

**Decided**: 2026-05
**Rationale**: Metro is still 0.x and has a history of breaking changes at minor versions (e.g., 0.60, 0.66, ongoing experimental flag churn). But the alternatives are all worse: building our own bundler is project-killing scope; using esbuild/SWC alone loses RN-specific module resolution; running raw Hermes loses TS/JSX/assets. Metro is the only viable path, and the fidelity gain (exact production bundle pipeline) is the whole point of hermes-test's pitch.
**Risk profile**:
- Metro disappearing: ~0% (Facebook-critical, actively developed)
- Metro programmatic API breaking at minor version: real, has happened before
- Metro becoming much slower: low (active optimization direction)
- RN moving off Metro: years out at minimum; consolidation (Static Hermes on top of Metro) is the trend
**Mitigation strategy**:
- Pin Metro to a narrow peer-dependency range per hermes-test release (`^0.82.0 || ^0.83.0`, bump deliberately)
- Support the latest two RN minor versions only; older RN users stay on older hermes-test
- Use only documented public APIs — `runBuild`, the daemon WebSocket protocol Expo CLI uses
- Avoid anything prefixed `unstable_`, custom serializers, `enhanceMiddleware`
- Test against the latest Metro release within each supported RN version on CI
- Budget 1-2 engineering days per quarter for Metro API drift adaptation
- If a Metro breakage takes more than a week to adapt, that's a signal to revisit the dependency strategy
**Implication**: All Metro interactions go through one file (`crates/hermes-test-cli/src/metro.rs`) so API changes localize to a single adaptation point. Treat Metro like a third-party service: small surface, easy to swap if forced.

## D-017: Rust from day one, no Node prototype phase

**Decided**: 2026-05
**Rationale**: An earlier conversation considered building a Node prototype first to "validate the architecture" before committing to Rust. We chose against this. Reasoning:
- The honest gap between Node and Rust prototypes is ~one weekend, not multiple weeks (we corrected an earlier inflated estimate).
- The destination tool is Rust regardless of starting point. A Node prototype is throwaway work.
- The "validation question" the Node prototype was meant to answer — does Hermes startup fit our speed budget — gets answered just as definitively by the Rust prototype, only one weekend later.
- The JS layer (harness, expect, spy, useMock, renderHook) is portable between hosts. Only the bridge code differs. The bridge code is exactly what we need to learn.
- User's stated upskilling target includes Rust + Hermes + Metro all at once. A Node prototype skips two of three.
**Acknowledged risks accepted**:
- First-time `cxx` crate setup will burn 1-2 days
- CMake/Hermes build issues are likely on first attempt (0.5-1 day)
- Debugging spans Rust + C++ + JS, which is harder than debugging in one language
**Mitigation**:
- Lean on Claude Code aggressively for boilerplate (C++ shim, `cxx` bindings, CMake config)
- Time-box Week 1 to two weekends of focused work; if not running by then, surface for replan, not silently slip
- Treat the `cxx` bridge as the hardest part of the entire project — once it works, the rest is incremental
**Implication**: No `packages/hermes-test-node-prototype/` directory. The `Cargo.toml` workspace is committed to from day one. The Node-host option is closed; reopening it requires explicit user approval.
