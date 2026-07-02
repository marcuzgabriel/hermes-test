# hermes-test

A fast, deterministic test runner for React Native hooks and pure functions. Rust CLI host, Hermes engine, esbuild bundling, typed TS API — designed for the AI-authoring era.

**Not Jest-compatible by design.** This is a deliberate choice (see challenges.md / hardening-assessment.md for the reasoning trail).

## Working in this repo — read first

When Claude Code opens this repo, before doing anything else:

1. Read `.claude/skills/hermes-test/SKILL.md` for operating principles
2. Read `.claude/references/hardening-assessment.md` for the solidity review, known risk areas, phased plugin-resolver plan, and prioritized enhancements
3. Read `website/docs/architecture/mock-resolution.md` for the CURRENT mock delivery model (onResolve receptionist + wrapper brain)
4. Read `.claude/references/mock-strategies.md` for runtime mock patterns and what's been tried
5. Read `.claude/references/mock-strategy.md` for detailed strategy history with all 11+ approaches
6. Read `.claude/references/shadow-wrappers.md` for the LEGACY delivery (shadow wrappers/package shims — HT_RESOLVER=legacy, pending phase-4 removal)
7. Read `.claude/references/shallow-rendering-fixes.md` for shallow rendering bugs, fixes, and remaining work
8. Read `.claude/references/challenges.md` for the full journey of challenges and solutions

(scope.md, decisions.md and roadmap.md were listed here historically but never
existed in this repo — removed from the list July 2026.)

The references are the source of truth. If something in this README disagrees with them, trust the references and ask the user.

## The pitch

Today's RN testing stack is structurally wrong:
- Tests run in Node, app runs in Hermes — engine-fidelity bugs leak through
- Jest transforms duplicate bundler work — `transformIgnorePatterns` is a maintenance hellscape
- `jest-expo` mocks are JS pretending to be native modules — type-incoherent and silently drift
- Watch mode is multi-second — Vitest/Bun run sub-100ms but neither supports RN

hermes-test fixes all four by running tests in Hermes (your app's engine), bundled once with esbuild, with a typed API that's explicit and AI-friendly.

## Quickstart

```bash
npm install -D hermes-test
```

```ts
// useCounter.test.ts
import { test, expect, renderHook, act, spy, http, HttpResponse } from 'hermes-test';

// Mock a module (relative paths resolve from THIS file; ht is a global)
ht.mock('../analytics', () => ({ track: spy() }));

// Mock fetch (MSW-like API)
ht.mock.fetch(
  http.get('/api/count', () => HttpResponse.json({ count: 42 })),
);

test('useCounter tracks state', () => {
  const { result } = renderHook(() => useCounter(0));

  act(() => result.current.increment());
  act(() => result.current.increment());

  expect(result.current.count).toBe(2);
});
```

```bash
npx hermes-test watch
# sub-200ms reruns on file save
```

### Mock API

```ts
ht.mock(path, factory)       // mock a module (relative = from the test file)
ht.unmock(path)              // bundle the real module, no interception
ht.shallow(componentPath)    // auto-mock JSX children of a component
ht.mock.fetch(handler...)    // register fetch handlers (auto-overwrites matching)
ht.mock.fetch.reset()        // clear all handlers
```

## Status

v1.0. Sole test runner for Topdanmark (Jest fully removed). 284 suites, 1766 tests, 7 snapshots, 0 failures.

Measured performance (Topdanmark, 284 suites, 1766 tests):

| Scenario | hermes-test | Jest | Speedup |
|----------|-------------|------|---------|
| Full suite | 5s | 116s | **23x** |
| Cached run | 0.84s | 54s | **64x** |
| With coverage | 5s | 128s | **26x** |
| Watch rerun | ~350ms | — | — |

## Structure

```
hermes-test/
├── .claude/                   # Claude Code context — read these first
│   ├── skills/hermes-test/SKILL.md
│   └── references/            # challenges, hardening-assessment, mock strategy history, …
├── crates/
│   ├── hermes-test-cli/       # Rust binary (bundler orchestration, Hermes host)
│   └── hermes-bridge/         # C++ shim linking the Hermes static library
├── packages/
│   ├── hermes-test/           # User-facing TS package (harness, wrappers, shims, bin)
│   └── hermes-test-{darwin-arm64,darwin-x64,linux-x64}/  # platform binaries
├── bench/                     # Benchmark fixtures
├── examples/
│   └── expo-app/              # Reference Expo project + regression fixtures
├── website/                   # Docusaurus docs site (deployed via GitHub Pages)
├── docs/                      # Architecture diagrams (mermaid)
├── BENCHMARKS.md              # Measured results
└── README.md                  # User-facing docs
```

## License

MIT (when published).
