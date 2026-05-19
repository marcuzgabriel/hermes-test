# hermes-test

A fast, deterministic test runner for React Native hooks and pure functions. Rust CLI host, Hermes engine, Metro bundling, typed TS API — designed for the AI-authoring era.

**Not Jest-compatible by design.** This is a deliberate choice. See `.claude/references/decisions.md`.

## Working in this repo — read first

When Claude Code opens this repo, before doing anything else:

1. Read `.claude/skills/hermes-test/SKILL.md` for operating principles
2. Read `.claude/references/scope.md` to know what's in v0 vs deferred
3. Read `.claude/references/decisions.md` to avoid relitigating settled choices
4. Read `.claude/references/roadmap.md` to know which week of work we're on

The references are the source of truth. If something in this README disagrees with them, trust the references and ask the user.

## The pitch

Today's RN testing stack is structurally wrong:
- Tests run in Node, app runs in Hermes — engine-fidelity bugs leak through
- Jest transforms duplicate Metro's work — `transformIgnorePatterns` is a maintenance hellscape
- `jest-expo` mocks are JS pretending to be native modules — type-incoherent and silently drift
- Watch mode is multi-second — Vitest/Bun run sub-100ms but neither supports RN

hermes-test fixes all four by running tests in Hermes (your app's engine), via Metro (your app's bundler), with a typed API that's explicit and AI-friendly.

## Quickstart (target API, not yet implemented)

```bash
npm install -D hermes-test
```

```ts
// useCounter.test.ts
import { test, renderHook, act } from 'hermes-test';

test('useCounter tracks state history', async ({ expect }) => {
  const { result, history, renderCount } = renderHook(() => useCounter(0));
  
  await act(() => result.current.increment());
  await act(() => result.current.increment());
  
  expect(result.current.count).toBe(2);
  expect(history.map(h => h.count)).toEqual([0, 1, 2]);
  expect(renderCount).toBe(3);
});
```

```bash
npx hermes-test watch
# sub-200ms reruns on file save
```

## Status

v0 in progress. Roadmap in `.claude/references/roadmap.md`.

| Week | Deliverable | Status |
|------|-------------|--------|
| 1 | Hermes embed + Rust CLI skeleton | Not started |
| 2 | Metro integration + harness | Not started |
| 3 | Hooks, mocks, state history | Not started |
| 4 | Watch mode, reporter, codemod, ship 0.1 | Not started |

Speed targets (predictions until measured — see `BENCHMARKS.md` for real data):

| Scenario | Target |
|----------|--------|
| 50 hook tests cold | < 350ms |
| Watch rerun (1 file) | < 200ms |
| 1000 mixed tests cold | < 4s |
| vs Jest+@swc/jest+jest-expo | 5-10x faster |

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
