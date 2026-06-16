# hermes-test

A fast, deterministic test runner for React Native hooks and pure functions. Rust CLI host, Hermes engine, esbuild bundling, typed TS API — designed for the AI-authoring era.

**Not Jest-compatible by design.** This is a deliberate choice. See `.claude/references/decisions.md`.

## Working in this repo — read first

When Claude Code opens this repo, before doing anything else:

1. Read `.claude/skills/hermes-test/SKILL.md` for operating principles
2. Read `.claude/references/scope.md` to know what's in v0 vs deferred
3. Read `.claude/references/decisions.md` to avoid relitigating settled choices
4. Read `.claude/references/roadmap.md` to know which week of work we're on
5. Read `.claude/references/mock-strategies.md` for current mock strategy and what's been tried
6. Read `.claude/references/mock-strategy.md` for detailed strategy history with all 11+ approaches
7. Read `.claude/references/shadow-wrappers.md` for how shadow wrappers and package shims work
9. Read `.claude/references/shallow-rendering-fixes.md` for shallow rendering bugs, fixes, and remaining work
8. Read `.claude/references/challenges.md` for the full journey of challenges and solutions

The references are the source of truth. If something in this README disagrees with them, trust the references and ask the user.

## The pitch

Today's RN testing stack is structurally wrong:
- Tests run in Node, app runs in Hermes — engine-fidelity bugs leak through
- Jest transforms duplicate bundler work — `transformIgnorePatterns` is a maintenance hellscape
- `jest-expo` mocks are JS pretending to be native modules — type-incoherent and silently drift
- Watch mode is multi-second — Vitest/Bun run sub-100ms but neither supports RN

hermes-test fixes all four by running tests in Hermes (your app's engine), via Metro (your app's bundler), with a typed API that's explicit and AI-friendly.

## Quickstart

```bash
npm install -D hermes-test
```

```ts
// useCounter.test.ts
import { test, mock, renderHook, act, spy, http, HttpResponse } from 'hermes-test';

// Mock a module (barrel paths + node_modules)
mock('./analytics', () => ({ track: spy() }));

// Mock fetch (MSW-like API)
mock.fetch(
  http.get('/api/count', () => HttpResponse.json({ count: 42 })),
);

test('useCounter tracks state', async ({ expect }) => {
  const { result } = renderHook(() => useCounter(0));

  await act(() => result.current.increment());
  await act(() => result.current.increment());

  expect(result.current.count).toBe(2);
});
```

```bash
npx hermes-test watch
# sub-200ms reruns on file save
```

### Mock API

```ts
mock(path, factory)          // mock a module
mock.fetch(handler...)       // register fetch handlers
mock.fetch.overwrite(handler...) // per-test overrides (like MSW server.use)
mock.fetch.reset()           // clear per-test overrides
mock.fetch.clear()           // clear all handlers
```

## Status

v0 complete. Published on npm as `hermes-test@0.2.1`. Topdanmark: 259 files, 1472 tests, 100% passing, 100% unit test coverage.

| Week | Deliverable | Status |
|------|-------------|--------|
| 1 | Hermes embed + Rust CLI skeleton | Done |
| 2 | esbuild integration + harness | Done |
| 3 | Hooks, mocks, state history | Done |
| 4 | Watch mode, reporter, codemod, ship 0.1 | Done |

Measured performance (Topdanmark, 1472 tests, 259 files):

| Scenario | hermes-test | Jest | Speedup |
|----------|-------------|------|---------|
| Cached run | 0.84s | 54s | **64x** |
| Cold run | 2.5s | 54s | **22x** |
| With coverage | 5s | 128s | **26x** |
| Watch rerun | ~350ms | — | — |

## Structure

```
hermes-test/
├── .claude/                   # Claude Code context — read these first
│   ├── skills/hermes-test/SKILL.md
│   └── references/
│       ├── vision.md          # Why this exists
│       ├── architecture.md    # The stack
│       ├── api-design.md      # The user-facing TS API
│       ├── scope.md           # v0 / v1 / out forever
│       ├── roadmap.md         # Week-by-week with gates
│       ├── decisions.md       # Settled choices
│       └── benchmarks.md      # Measurement methodology
├── crates/
│   ├── hermes-test-cli/        # Rust binary (week 1)
│   └── hermes-bridge/         # C++ shim (week 1)
├── packages/
│   ├── hermes-test/            # User-facing TS package (week 2+)
│   └── hermes-test-ai/         # AI companion (post-v0)
├── bench/
│   ├── fixtures/              # Standard benchmark fixtures
│   ├── fixtures-jest/         # Equivalent Jest versions for comparison
│   └── watch-bench.sh
├── examples/
│   └── expo-app/              # Reference Expo project using hermes-test
├── docs/
├── BENCHMARKS.md              # Measured results, updated weekly
└── README.md                  # User-facing, comes later
```

## License

MIT (when published).
